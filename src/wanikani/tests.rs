use super::*;
use serde_json::{Value, json};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

const TOKEN: &str = "synthetic-test-credential";
fn user() -> Value {
    serde_json::from_str(include_str!("../../tests/fixtures/wanikani/user.json")).unwrap()
}
fn collection(data: Vec<Value>) -> Value {
    json!({"object":"collection", "pages":{"next_url":null}, "data":data})
}
async fn serve(server: &MockServer, endpoint: &str, body: Value) {
    Mock::given(method("GET"))
        .and(path(format!("/v2/{endpoint}")))
        .and(header("authorization", format!("Bearer {TOKEN}")))
        .and(header("wanikani-revision", "20170710"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(server)
        .await;
}
fn client(server: &MockServer) -> Client {
    Client::with_base_url(TOKEN, &format!("{}/v2/", server.uri())).unwrap()
}

#[tokio::test]
async fn retrieves_empty_account_with_headers_and_normalizes_profile() {
    let server = MockServer::start().await;
    serve(&server, "user", user()).await;
    serve(&server, "assignments", collection(vec![])).await;
    serve(&server, "review_statistics", collection(vec![])).await;
    let snapshot = client(&server).fetch().await.unwrap();
    snapshot.validate().unwrap();
    assert_eq!(snapshot.learner.id, "synthetic-learner");
    assert_eq!(snapshot.learner.username, "テスト");
    assert_eq!(snapshot.learner.subscription.kind, "free");
    assert_eq!(snapshot.learner.current_vacation_started_at, None);
    assert_eq!(snapshot.learner.subscription.period_ends_at, None);
    assert_eq!(
        snapshot.learner.updated_at.to_rfc3339(),
        "2026-09-27T09:00:00+00:00"
    );
    assert!(snapshot.subjects.is_empty());
    assert!(snapshot.sync_started_at <= snapshot.sync_completed_at);
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
}

#[tokio::test]
async fn http_boundary_rejects_auth_redirects_malformed_and_oversized_responses_without_echoing_secrets()
 {
    for (response, expected) in [
        (
            ResponseTemplate::new(401).set_body_string(TOKEN),
            "Authentication",
        ),
        (
            ResponseTemplate::new(302)
                .insert_header("location", format!("https://example.invalid/{TOKEN}")),
            "302",
        ),
        (
            ResponseTemplate::new(200).set_body_string(format!("{{{TOKEN}")),
            "Invalid response",
        ),
        (
            ResponseTemplate::new(200).set_body_bytes(vec![b' '; 16 * 1024 * 1024 + 1]),
            "16 MiB",
        ),
    ] {
        let server = MockServer::start().await;
        Mock::given(path("/v2/user"))
            .respond_with(response)
            .expect(1)
            .mount(&server)
            .await;
        let error = client(&server).fetch().await.unwrap_err();
        let text = format!("{error} {error:?}");
        assert!(text.contains(expected), "{text}");
        assert!(!text.contains(TOKEN), "{text}");
    }
}

fn fixture(endpoint: &str) -> Value {
    serde_json::from_str(match endpoint {
        "assignments" => include_str!("../../tests/fixtures/wanikani/assignments.json"),
        "review_statistics" => include_str!("../../tests/fixtures/wanikani/review_statistics.json"),
        "subjects" => include_str!("../../tests/fixtures/wanikani/subjects.json"),
        _ => panic!("unknown test fixture"),
    })
    .unwrap()
}

async fn mixed_server() -> MockServer {
    let server = MockServer::start().await;
    serve(&server, "user", user()).await;
    for endpoint in ["assignments", "review_statistics", "subjects"] {
        serve(&server, endpoint, fixture(endpoint)).await;
    }
    server
}

#[tokio::test]
async fn normalizes_all_subject_shapes_and_progress_without_inventing_absence() {
    let server = mixed_server().await;
    let snapshot = client(&server).fetch().await.unwrap();
    let mut actual = serde_json::to_value(&snapshot).unwrap();
    let expected: Value =
        serde_json::from_str(include_str!("../../tests/fixtures/mixed.json")).unwrap();
    actual["sync_started_at"] = expected["snapshot"]["sync_started_at"].clone();
    actual["sync_completed_at"] = expected["snapshot"]["sync_completed_at"].clone();
    assert_eq!(actual, expected["snapshot"]);
    let requests = server.received_requests().await.unwrap();
    let subjects = requests
        .iter()
        .find(|r| r.url.path() == "/v2/subjects")
        .unwrap();
    assert_eq!(
        subjects
            .url
            .query_pairs()
            .find(|(k, _)| k == "ids")
            .unwrap()
            .1,
        "1,2,3,4,5"
    );
    assert_eq!(requests.len(), 4);
}

#[tokio::test]
async fn follows_explicit_pagination_even_after_empty_pages_on_each_collection() {
    for paginated in ["assignments", "review_statistics", "subjects"] {
        let server = MockServer::start().await;
        serve(&server, "user", user()).await;
        for endpoint in ["assignments", "review_statistics", "subjects"] {
            if endpoint == paginated {
                let mut first = collection(vec![]);
                first["pages"]["next_url"] =
                    json!(format!("{}/v2/{endpoint}?page_after_id=1", server.uri()));
                Mock::given(path(format!("/v2/{endpoint}")))
                    .and(wiremock::matchers::query_param("page_after_id", "1"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(fixture(endpoint)))
                    .expect(1)
                    .with_priority(1)
                    .mount(&server)
                    .await;
                serve(&server, endpoint, first).await;
            } else {
                serve(&server, endpoint, fixture(endpoint)).await;
            }
        }
        let snapshot = client(&server).fetch().await.unwrap();
        assert_eq!(snapshot.subjects.len(), 4);
        assert_eq!(snapshot.assignments.len(), 4);
        assert_eq!(snapshot.review_statistics.len(), 4);
    }
}

#[tokio::test]
async fn rejects_untrusted_or_repeated_pagination_without_sending_credentials() {
    for suffix in [
        "/v2/user",
        "/v2/assignments#fragment",
        "/v2/assignments",
        "/v2/assignments?cycle=1",
        "foreign",
    ] {
        let server = MockServer::start().await;
        let foreign = MockServer::start().await;
        serve(&server, "user", user()).await;
        let next = if suffix == "foreign" {
            format!("{}/v2/assignments", foreign.uri())
        } else {
            format!("{}{suffix}", server.uri())
        };
        let mut page = collection(vec![]);
        page["pages"]["next_url"] = json!(next);
        Mock::given(path("/v2/assignments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(page))
            .mount(&server)
            .await;
        let result = client(&server).fetch().await;
        assert!(
            result.unwrap_err().to_string().contains("pagination"),
            "wrong failure for {suffix}"
        );
        assert!(foreign.received_requests().await.unwrap().is_empty());
        assert!(server.received_requests().await.unwrap().len() <= 3);
    }
}

#[tokio::test]
async fn requests_subject_ids_in_batches_of_at_most_100() {
    let server = MockServer::start().await;
    serve(&server, "user", user()).await;
    let mut assignments = Vec::new();
    let mut subjects = Vec::new();
    for id in 1..=101 {
        let mut a = fixture("assignments")["data"][0].clone();
        a["id"] = json!(1000 + id);
        a["data"]["subject_id"] = json!(id);
        assignments.push(a);
        let mut s = fixture("subjects")["data"][0].clone();
        s["id"] = json!(id);
        subjects.push(s);
    }
    serve(&server, "assignments", collection(assignments)).await;
    serve(&server, "review_statistics", collection(vec![])).await;
    for batch in subjects.chunks(100) {
        let ids = batch
            .iter()
            .map(|s| s["id"].to_string())
            .collect::<Vec<_>>()
            .join(",");
        Mock::given(path("/v2/subjects"))
            .and(wiremock::matchers::query_param("ids", ids))
            .respond_with(ResponseTemplate::new(200).set_body_json(collection(batch.to_vec())))
            .expect(1)
            .mount(&server)
            .await;
    }
    assert_eq!(client(&server).fetch().await.unwrap().subjects.len(), 101);
}

async fn changed_server(endpoint: &str, body: Value) -> MockServer {
    let server = MockServer::start().await;
    for name in ["user", "assignments", "review_statistics", "subjects"] {
        let value = if name == endpoint {
            body.clone()
        } else if name == "user" {
            user()
        } else {
            fixture(name)
        };
        Mock::given(path(format!("/v2/{name}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(value))
            .mount(&server)
            .await;
    }
    server
}

#[tokio::test]
async fn identical_source_duplicates_collapse_but_conflicts_are_rejected() {
    for endpoint in ["assignments", "review_statistics", "subjects"] {
        let mut body = fixture(endpoint);
        let record = body["data"][0].clone();
        body["data"].as_array_mut().unwrap().push(record);
        let server = changed_server(endpoint, body.clone()).await;
        let snapshot = client(&server).fetch().await.unwrap();
        assert_eq!(
            (
                snapshot.subjects.len(),
                snapshot.assignments.len(),
                snapshot.review_statistics.len()
            ),
            (4, 4, 4)
        );
        let last = body["data"].as_array_mut().unwrap().last_mut().unwrap();
        last["data_updated_at"] = json!("2026-09-27T10:00:00Z");
        let server = changed_server(endpoint, body).await;
        assert!(client(&server).fetch().await.is_err());
    }
}

#[tokio::test]
async fn rejects_missing_mismatched_and_invalid_source_data_including_exclusions() {
    let mut cases = Vec::new();
    let mut missing = fixture("subjects");
    missing["data"].as_array_mut().unwrap().pop();
    cases.push(("subjects", missing, "missing requested subject"));
    let mut mismatch = fixture("assignments");
    mismatch["data"][0]["data"]["subject_type"] = json!("vocabulary");
    cases.push(("assignments", mismatch, "mismatched kind"));
    for (pointer, value, name) in [
        (
            "/data/4/data/level",
            json!(61),
            "out-of-range excluded level",
        ),
        (
            "/data/4/data/characters",
            json!(""),
            "invalid excluded content",
        ),
        (
            "/data/0/data/readings/0/type",
            json!(""),
            "blank reading type",
        ),
    ] {
        let mut body = fixture("subjects");
        *body.pointer_mut(pointer).unwrap() = value;
        cases.push(("subjects", body, name));
    }
    let mut body = fixture("assignments");
    body["data"][0]["data"]
        .as_object_mut()
        .unwrap()
        .remove("burned_at");
    cases.push(("assignments", body, "missing nullable required field"));
    let mut body = fixture("assignments");
    body["pages"].as_object_mut().unwrap().remove("next_url");
    cases.push(("assignments", body, "missing pagination terminator"));
    let mut accepted = Vec::new();
    for (endpoint, body, name) in cases {
        let server = changed_server(endpoint, body).await;
        if client(&server).fetch().await.is_ok() {
            accepted.push(name);
        }
    }
    assert!(accepted.is_empty(), "accepted invalid source: {accepted:?}");
}

#[tokio::test]
async fn transient_failures_retry_at_most_twice_and_rate_resets_are_bounded() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let server = MockServer::start().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    Mock::given(path("/v2/user"))
        .respond_with(move |_: &wiremock::Request| {
            if observed.fetch_add(1, Ordering::SeqCst) < 2 {
                ResponseTemplate::new(503)
            } else {
                ResponseTemplate::new(200).set_body_json(user())
            }
        })
        .expect(3)
        .mount(&server)
        .await;
    serve(&server, "assignments", collection(vec![])).await;
    serve(&server, "review_statistics", collection(vec![])).await;
    assert!(client(&server).fetch().await.is_ok());
    for status in [503, 429] {
        let server = MockServer::start().await;
        Mock::given(path("/v2/user"))
            .respond_with(ResponseTemplate::new(status).insert_header("ratelimit-reset", "0"))
            .expect(3)
            .mount(&server)
            .await;
        assert!(client(&server).fetch().await.is_err());
    }
    let server = MockServer::start().await;
    Mock::given(path("/v2/user"))
        .respond_with(ResponseTemplate::new(429).insert_header("ratelimit-reset", "9999999999"))
        .expect(1)
        .mount(&server)
        .await;
    assert!(
        client(&server)
            .fetch()
            .await
            .unwrap_err()
            .to_string()
            .contains("120 seconds")
    );
}

#[test]
fn retry_waits_use_source_reset_or_bounded_defaults() {
    use reqwest::header::HeaderMap;
    let fixed = DateTime::parse_from_rfc3339("2026-09-27T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    assert_eq!(transient_delay(0), Duration::from_secs(1));
    assert_eq!(transient_delay(1), Duration::from_secs(2));
    let mut headers = HeaderMap::new();
    assert_eq!(
        reset_delay(&headers, fixed).unwrap(),
        Duration::from_secs(60)
    );
    headers.insert(
        "ratelimit-reset",
        HeaderValue::from_str(&(fixed.timestamp() + 90).to_string()).unwrap(),
    );
    assert_eq!(
        reset_delay(&headers, fixed).unwrap(),
        Duration::from_secs(90)
    );
    headers.insert(
        "ratelimit-reset",
        HeaderValue::from_str(&(fixed.timestamp() + 121).to_string()).unwrap(),
    );
    assert!(reset_delay(&headers, fixed).is_err());
}

#[test]
fn configured_origin_rejects_insecure_remote_urls_and_url_credentials() {
    for base in [
        "http://example.com/v2/",
        "https://user:secret@example.com/v2/",
        "https://example.com/v2/?secret=bad",
        "https://example.com/v2/#fragment",
        "https://example.com/v2",
    ] {
        assert!(
            Client::with_base_url(TOKEN, base).is_err(),
            "accepted {base}"
        );
    }
}
