use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// Raw HTTP is needed for chunking, truncated bodies, and a server that never
// finishes headers/body. Each server and its connection tasks are scoped here.
struct RawServer {
    base: String,
    calls: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for RawServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl RawServer {
    fn client(&self) -> Client {
        Client::with_base_url("synthetic-http-credential", &self.base).unwrap()
    }

    async fn start(response: Vec<u8>, stall: bool) -> Self {
        Self::sequence(vec![response], stall).await
    }

    async fn sequence(responses: Vec<Vec<u8>>, stall: bool) -> Self {
        assert!(!responses.is_empty());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/v2/", listener.local_addr().unwrap());
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&calls);
        let responses = Arc::new(responses);
        let task = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let responses = Arc::clone(&responses);
                let observed = Arc::clone(&observed);
                connections.spawn(async move {
                    let mut request = Vec::new();
                    while !request.ends_with(b"\r\n\r\n") {
                        request.push(socket.read_u8().await.unwrap());
                        assert!(request.len() < 8192);
                    }
                    let attempt = observed.fetch_add(1, Ordering::SeqCst);
                    let response = &responses[attempt.min(responses.len() - 1)];
                    // Oversized-response tests deliberately close the connection early.
                    let _ = socket.write_all(response).await;
                    if stall {
                        std::future::pending::<()>().await;
                    }
                });
            }
        });
        Self { base, calls, task }
    }
}

#[tokio::test]
async fn request_deadline_covers_stalled_headers_and_stalled_body() {
    let check_deadline = async |bytes| {
        let server = RawServer::start(bytes, true).await;
        let mut client = Client::with_timeout(
            "synthetic-deadline-credential",
            &server.base,
            Duration::from_millis(250),
        )
        .unwrap();
        let error = tokio::time::timeout(
            Duration::from_secs(10),
            fails_without_replacing_cache(&mut client),
        )
        .await
        .expect("request deadline did not terminate retrieval");
        assert!(matches!(error, Error::Transport { endpoint: "user" }));
        assert_eq!(server.calls.load(Ordering::SeqCst), 3);
    };
    tokio::join!(
        check_deadline(Vec::new()),
        check_deadline(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n{".to_vec()),
    );
}

pub(super) async fn fails_without_replacing_cache(client: &mut Client) -> Error {
    use crate::cache::{self, SyncGuard};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wanikani.json");
    let bytes = include_bytes!("../../../../tests/fixtures/mixed.json");
    std::fs::write(&path, bytes).unwrap();
    let original = cache::load(dir.path()).unwrap();
    let guard = SyncGuard::acquire(dir.path()).unwrap();
    let result = client.fetch().await;
    if let Ok(sync_data) = &result {
        guard.replace(sync_data).unwrap();
    }
    let error = result.unwrap_err();
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    assert_eq!(cache::load(dir.path()).unwrap(), original);
    drop(guard);
    SyncGuard::acquire(dir.path()).unwrap();
    error
}

fn chunked(body: &[u8]) -> Vec<u8> {
    let mut response =
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n".to_vec();
    for chunk in body.chunks(8192) {
        response.extend_from_slice(format!("{:x}\r\n", chunk.len()).as_bytes());
        response.extend_from_slice(chunk);
        response.extend_from_slice(b"\r\n");
    }
    response.extend_from_slice(b"0\r\n\r\n");
    response
}

#[tokio::test]
async fn streamed_response_limit_accepts_the_boundary_and_rejects_excess_without_content_length() {
    let mut body = vec![b' '; MAX_PAGE_BYTES];
    body[..2].copy_from_slice(b"{}");
    let server = RawServer::start(chunked(&body), false).await;
    let mut client = server.client();
    assert_eq!(
        client.get::<serde_json::Value>("user").await.unwrap(),
        serde_json::json!({})
    );
    assert_eq!(server.calls.load(Ordering::SeqCst), 1);

    body.push(b' ');
    let mut close_delimited = b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".to_vec();
    close_delimited.extend_from_slice(&body);
    for response in [chunked(&body), close_delimited] {
        let server = RawServer::start(response, false).await;
        let mut client = server.client();
        let error = fails_without_replacing_cache(&mut client).await;
        assert!(matches!(
            error,
            Error::ResponseTooLarge { endpoint: "user" }
        ));
        assert_eq!(server.calls.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn declared_oversize_is_rejected_without_waiting_for_a_body() {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
        MAX_PAGE_BYTES + 1
    )
    .into_bytes();
    let server = RawServer::start(response, true).await;
    let mut client = Client::with_timeout(
        "synthetic-stream-credential",
        &server.base,
        Duration::from_millis(250),
    )
    .unwrap();
    assert!(matches!(
        fails_without_replacing_cache(&mut client).await,
        Error::ResponseTooLarge { .. }
    ));
    assert_eq!(server.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn truncated_bodies_exhaust_transport_retries_and_preserve_the_cache() {
    let check_truncation = async |response| {
        let server = RawServer::start(response, false).await;
        let mut client = server.client();
        assert!(matches!(
            fails_without_replacing_cache(&mut client).await,
            Error::Transport { .. }
        ));
        assert_eq!(server.calls.load(Ordering::SeqCst), 3);
    };
    tokio::join!(
        check_truncation(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n{}".to_vec()),
        check_truncation(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\na\r\n{}".to_vec()),
    );
}

#[tokio::test]
async fn rate_headers_delay_reused_clients_until_reset_without_wall_clock_sleep() {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers::path};
    for status in [200, 429] {
        let server = MockServer::start().await;
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&calls);
        let reset = (now().timestamp() + 90).to_string();
        Mock::given(path("/v2/user"))
            .respond_with(move |_: &wiremock::Request| {
                if observed.fetch_add(1, Ordering::SeqCst) == 0 {
                    ResponseTemplate::new(status)
                        .insert_header("ratelimit-remaining", "0")
                        .insert_header("ratelimit-reset", reset.clone())
                        .set_body_json(serde_json::json!({}))
                } else {
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({}))
                }
            })
            .expect(2)
            .mount(&server)
            .await;
        let mut client = Client::with_base_url(
            "synthetic-rate-credential",
            &format!("{}/v2/", server.uri()),
        )
        .unwrap();
        let first = client
            .request("user", client.base_url.join("user").unwrap())
            .await;
        assert_eq!(first.is_ok(), status == 200);
        let deadline = client.next_request_at.expect("reset must be retained");
        tokio::time::pause();
        let remaining = deadline - tokio::time::Instant::now();
        {
            let mut next = std::pin::pin!(client.get::<serde_json::Value>("user"));
            assert!(matches!(
                next.as_mut().poll(&mut Context::from_waker(Waker::noop())),
                Poll::Pending
            ));
            tokio::time::advance(remaining - Duration::from_millis(1)).await;
            assert!(matches!(
                next.as_mut().poll(&mut Context::from_waker(Waker::noop())),
                Poll::Pending
            ));
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            tokio::time::advance(Duration::from_millis(1)).await;
            // Resume before socket I/O: automatic paused-clock advancement can
            // otherwise race reqwest's deadline against the actual network.
            tokio::time::resume();
            assert_eq!(next.await.unwrap(), serde_json::json!({}));
        }
        assert!(client.next_request_at.is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}

#[tokio::test]
async fn successful_transport_retry_discards_partial_body_bytes() {
    let partial = b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n{\"discard\":".to_vec();
    let server = RawServer::sequence(
        vec![partial.clone(), partial, chunked(b"{\"complete\":true}")],
        false,
    )
    .await;
    let mut client = server.client();
    assert_eq!(
        client.get::<serde_json::Value>("user").await.unwrap(),
        serde_json::json!({"complete":true})
    );
    assert_eq!(server.calls.load(Ordering::SeqCst), 3);
}
