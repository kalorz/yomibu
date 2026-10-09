use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

async fn raw_server(
    bytes: Vec<u8>,
    stall: bool,
) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/v1/", listener.local_addr().unwrap());
    let count = Arc::new(AtomicUsize::new(0));
    let calls = Arc::clone(&count);
    let task = tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            calls.fetch_add(1, Ordering::SeqCst);
            let mut request = Vec::new();
            loop {
                let mut buffer = [0; 2048];
                let n = socket.read(&mut buffer).await.unwrap();
                if n == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..n]);
                if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..end]);
                    let size: usize = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|n| n.parse().ok())
                        })
                        .unwrap();
                    if request.len() >= end + 4 + size {
                        break;
                    }
                }
            }
            let _ = socket.write_all(&bytes).await;
            if stall {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    });
    (base, count, task)
}

#[tokio::test]
async fn real_socket_deadlines_disconnects_and_truncated_bodies_make_one_attempt() {
    for (bytes, stall, timeout) in [
        (Vec::new(), true, true),
        (
            b"HTTP/1.1 200 OK\r\nContent-Length: 40\r\n\r\n{".to_vec(),
            true,
            true,
        ),
        (Vec::new(), false, false),
        (
            b"HTTP/1.1 200 OK\r\nContent-Length: 40\r\n\r\n{".to_vec(),
            false,
            false,
        ),
    ] {
        let (base, count, server) = raw_server(bytes, stall).await;
        let client = Client::build(
            "synthetic-secret",
            &base,
            Duration::from_millis(100),
            Duration::from_millis(150),
        )
        .unwrap();
        let request = prepared_request();
        let error = client.generate_candidates(&request).await.unwrap_err();
        assert!(
            matches!(error, ProviderError::Timeout) == timeout,
            "{error:?}"
        );
        if !timeout {
            assert!(matches!(error, ProviderError::Transport), "{error:?}");
        }
        assert_eq!(count.load(Ordering::SeqCst), 1);
        server.abort();
        let _ = server.await;
    }
}

#[tokio::test]
async fn body_bound_applies_to_chunked_and_close_delimited_responses() {
    let request = prepared_request();
    let envelope = json!({"id":"resp_synthetic","model":"reported","status":"completed","output":[
        {"type":"message","role":"assistant","status":"completed","content":[
            {"type":"output_text","text":json!({"candidates":[{"sentences":["犬です。"]},{"sentences":["猫です。"]}]}).to_string()}]}]})
    .to_string();
    for chunked in [false, true] {
        for size in [65536, 65537] {
            let mut body = envelope.as_bytes().to_vec();
            body.resize(size, b' ');
            let bytes = if chunked {
                let mut wire = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec();
                for chunk in body.chunks(997) {
                    wire.extend_from_slice(format!("{:x}\r\n", chunk.len()).as_bytes());
                    wire.extend_from_slice(chunk);
                    wire.extend_from_slice(b"\r\n");
                }
                wire.extend_from_slice(b"0\r\n\r\n");
                wire
            } else {
                let mut wire = b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".to_vec();
                wire.extend_from_slice(&body);
                wire
            };
            let (base, count, server) = raw_server(bytes, false).await;
            let client = Client::with_base_url("synthetic", &base).unwrap();
            let result = client.generate_candidates(&request).await;
            if size == 65536 {
                let result = result.unwrap();
                assert_eq!(result.provenance().prompt_revision, "adapter-test-v1");
                assert_eq!(result.provenance().request_sha256, request.sha256());
            } else {
                assert!(
                    matches!(result, Err(ProviderError::ResponseTooLarge)),
                    "{result:?}"
                );
            }
            assert_eq!(count.load(Ordering::SeqCst), 1);
            server.abort();
            let _ = server.await;
        }
    }
}

fn prepared_request() -> PreparedRequest {
    PreparedRequest::new(
        "prompt",
        "data",
        "adapter-test-v1",
        &sentence_options(2),
        16384,
    )
    .unwrap()
}

#[test]
fn preparation_uses_caller_budget_for_escaped_utf8_and_keeps_request_identity() {
    use sha2::{Digest, Sha256};

    let data = "\u{001b}\"\\\n猫".repeat(2000);
    let request = PreparedRequest::new(
        "prompt",
        &data,
        "test-prompt-v1",
        &sentence_options(3),
        usize::MAX,
    )
    .unwrap();
    let bytes = request.body_utf8().len();
    assert!(bytes > 16384);
    assert_eq!(request.candidate_count(), 3);
    assert_eq!(request.prompt_revision(), "test-prompt-v1");
    assert_eq!(
        request.sha256(),
        format!("{:x}", Sha256::digest(request.body_utf8()))
    );
    let body: serde_json::Value = serde_json::from_str(request.body_utf8()).unwrap();
    assert_eq!(body["input"][1]["content"], data);
    assert_eq!(body["max_output_tokens"], 1536);

    let exact = PreparedRequest::new(
        "prompt",
        &data,
        "test-prompt-v1",
        &sentence_options(3),
        bytes,
    )
    .unwrap();
    assert_eq!(exact.body_utf8(), request.body_utf8());
    assert_eq!(exact.sha256(), request.sha256());
    assert!(matches!(
        PreparedRequest::new("prompt", &data, "test-prompt-v1", &sentence_options(3), bytes - 1),
        Err(PreparationError::RequestTooLarge { bytes: actual, limit })
            if actual == bytes && limit == bytes - 1
    ));
}

#[test]
fn encoded_request_byte_limit_is_exact() {
    let short =
        PreparedRequest::new("prompt", "x", "test-v1", &sentence_options(2), 16384).unwrap();
    let data = "x".repeat(1 + 16384 - short.body_utf8().len());
    assert_eq!(
        PreparedRequest::new("prompt", &data, "test-v1", &sentence_options(2), 16384)
            .unwrap()
            .body_utf8()
            .len(),
        16384
    );
    assert!(matches!(
        PreparedRequest::new(
            "prompt",
            &(data + "x"),
            "test-v1",
            &sentence_options(2),
            16384
        ),
        Err(PreparationError::RequestTooLarge {
            bytes: 16385,
            limit: 16384
        })
    ));
}

fn sentence_options(candidate_count: usize) -> StoryGenerationOptions {
    StoryGenerationOptions {
        candidate_count,
        format: yomibu_core::domain::story::StoryFormat::Sentence,
        ..Default::default()
    }
}
