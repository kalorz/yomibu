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
        let request = story_request();
        let error = client
            .generate_story_candidates(&request)
            .await
            .unwrap_err();
        assert!(
            matches!(error, GenerationError::Provider(ProviderError::Timeout)) == timeout,
            "{error:?}"
        );
        if !timeout {
            assert!(
                matches!(error, GenerationError::Provider(ProviderError::Transport)),
                "{error:?}"
            );
        }
        assert_eq!(count.load(Ordering::SeqCst), 1);
        server.abort();
        let _ = server.await;
    }
}

#[tokio::test]
async fn body_bound_applies_to_chunked_and_close_delimited_responses() {
    let request = story_request();
    let envelope = json!({"id":"resp_synthetic","model":"reported","status":"completed","output":[
        {"type":"message","role":"assistant","status":"completed","content":[
            {"type":"output_text","text":"{\"candidates\":[\"犬です。\",\"猫です。\"]}"}]}]})
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
            let result = client.generate_story_candidates(&request).await;
            if size == 65536 {
                assert!(result.is_ok(), "{result:?}");
            } else {
                assert!(
                    matches!(
                        result,
                        Err(GenerationError::Provider(ProviderError::ResponseTooLarge))
                    ),
                    "{result:?}"
                );
            }
            assert_eq!(count.load(Ordering::SeqCst), 1);
            server.abort();
            let _ = server.await;
        }
    }
}

fn story_request() -> crate::story::AiModelRequest {
    use crate::{
        inventory::LearnerInventory,
        retrieval::{EmbeddingCache, EmbeddingModelIdentity, prepare_embedding_inputs},
        story::{StoryRequest, build_ai_model_request, select_vocabulary},
    };
    let inventory = LearnerInventory::from_manual(
        serde_json::from_str(include_str!("../../tests/fixtures/story/inventory.json")).unwrap(),
    )
    .unwrap();
    let request: StoryRequest =
        serde_json::from_str(include_str!("../../tests/fixtures/story/request.json")).unwrap();
    let inputs = prepare_embedding_inputs(&inventory, &request).unwrap();
    let model = EmbeddingModelIdentity {
        provider: "test".into(),
        model: "fixture-vectors".into(),
        revision: "1".into(),
        dimensions: 2,
        encoding_revision: "1".into(),
    };
    let cache =
        EmbeddingCache::from_vectors(model.clone(), &inputs, vec![vec![1., 0.]; inputs.len()])
            .unwrap();
    let plan = select_vocabulary(&inventory, &request, &cache, &model, 2).unwrap();
    build_ai_model_request(&inventory, &request, plan, Default::default())
        .unwrap()
        .1
}

#[test]
fn encoded_request_byte_limit_is_exact() {
    let short = prepare_candidate_body("prompt", "x", 2).unwrap();
    let data = "x".repeat(1 + 16384 - short.len());
    assert_eq!(
        prepare_candidate_body("prompt", &data, 2).unwrap().len(),
        16384
    );
    assert!(matches!(
        prepare_candidate_body("prompt", &(data + "x"), 2),
        Err(ProviderError::RequestTooLarge)
    ));
}
