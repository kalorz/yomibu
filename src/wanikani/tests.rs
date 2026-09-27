use super::{resilience::fails_without_replacing_cache, *};
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
async fn malformed_json_never_retries_or_echoes_the_response_body() {
    let server = MockServer::start().await;
    Mock::given(path("/v2/user"))
        .respond_with(ResponseTemplate::new(200).set_body_string(format!("{{{TOKEN}")))
        .expect(1)
        .mount(&server)
        .await;
    let error = fails_without_replacing_cache(&mut client(&server)).await;
    assert!(matches!(error, Error::InvalidResponse { endpoint: "user" }));
    assert_secret_free(&error);
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
async fn invalid_source_data_preserves_the_cache_including_exclusions() {
    let mut cases = Vec::new();
    for (endpoint, pointer, value) in [
        ("subjects", "/data/4/id", json!(999)),
        (
            "assignments",
            "/data/0/data/subject_type",
            json!("vocabulary"),
        ),
        ("assignments", "/data/3/data/subject_type", json!("kanji")),
        ("subjects", "/data/4/data/characters", json!("")),
        ("subjects", "/data/4/data/level", json!(61)),
        ("subjects", "/data/0/data/readings/0/type", json!("")),
        ("subjects", "/data/0/object", json!("unknown")),
        ("assignments", "/data/0/object", json!("unknown")),
        ("review_statistics", "/data/0/object", json!("unknown")),
        (
            "review_statistics",
            "/data/0/data/percentage_correct",
            json!(101),
        ),
    ] {
        let mut body = fixture(endpoint);
        *body.pointer_mut(pointer).unwrap() = value;
        cases.push((endpoint, pointer, body));
    }
    let mut missing = fixture("subjects");
    missing["data"].as_array_mut().unwrap().pop();
    cases.push(("subjects", "missing requested subject", missing));
    for (pointer, field) in [("/data/0/data", "burned_at"), ("/pages", "next_url")] {
        let mut body = fixture("assignments");
        body.pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(field);
        cases.push(("assignments", field, body));
    }
    for endpoint in ["assignments", "review_statistics"] {
        let mut body = fixture(endpoint);
        let mut duplicate = body["data"][0].clone();
        duplicate["id"] = json!(999);
        body["data"].as_array_mut().unwrap().push(duplicate);
        cases.push((endpoint, "duplicate subject reference", body));
    }
    for (endpoint, name, body) in cases {
        let server = changed_server(endpoint, body).await;
        let error = fails_without_replacing_cache(&mut client(&server)).await;
        assert!(
            matches!(
                error,
                Error::InvalidResponse { .. } | Error::InvalidSnapshot(_)
            ),
            "{endpoint}/{name}: {error}"
        );
        assert_secret_free(&error);
    }
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
    for (value, seconds) in [
        ("malformed".to_string(), 60),
        ("-1".to_string(), 60),
        ("0".to_string(), 0),
        (fixed.timestamp().to_string(), 0),
        ((fixed.timestamp() + 120).to_string(), 120),
    ] {
        let mut headers = HeaderMap::new();
        headers.insert("ratelimit-reset", HeaderValue::from_str(&value).unwrap());
        assert_eq!(
            reset_delay(&headers, fixed).unwrap(),
            Duration::from_secs(seconds)
        );
    }
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

#[tokio::test]
async fn cancelling_a_rate_limit_wait_keeps_the_deadline_for_the_next_fetch() {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };

    let server = MockServer::start().await;
    let mut client = client(&server);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    client.next_request_at = Some(deadline);
    {
        let mut fetch = std::pin::pin!(client.fetch());
        assert!(matches!(
            fetch.as_mut().poll(&mut Context::from_waker(Waker::noop())),
            Poll::Pending
        ));
    }
    assert_eq!(client.next_request_at, Some(deadline));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn hostile_pagination_and_redirects_never_forward_authorization_or_echo_urls() {
    let foreign = MockServer::start().await;
    for (template, requests) in [
        ("{base}/v2/user", 2),
        ("{base}/v2/assignments", 2),
        ("{base}/v2/assignments?cycle=1", 3),
        ("http://{token}@{authority}/v2/assignments", 2),
        ("http://user:{token}@{authority}/v2/assignments", 2),
        ("{base}/v2/assignments#{token}", 2),
        ("{base}/v2/assignments%2f..%2fuser?secret={token}", 2),
        ("/v2/assignments?secret={token}", 2),
        ("{foreign}/v2/assignments?secret={token}", 2),
    ] {
        let server = MockServer::start().await;
        serve(&server, "user", user()).await;
        let next = template
            .replace("{base}", &server.uri())
            .replace("{authority}", server.address().to_string().as_str())
            .replace("{foreign}", &foreign.uri())
            .replace("{token}", TOKEN);
        let mut page = collection(vec![]);
        page["pages"]["next_url"] = json!(next);
        Mock::given(path("/v2/assignments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(page))
            .expect(requests - 1)
            .mount(&server)
            .await;
        let error = fails_without_replacing_cache(&mut client(&server)).await;
        assert!(
            matches!(
                error,
                Error::Pagination {
                    endpoint: "assignments"
                }
            ),
            "{template}: {error}"
        );
        assert_secret_free(&error);
        assert_eq!(
            server.received_requests().await.unwrap().len() as u64,
            requests
        );
    }
    for status in [301, 302, 303, 307, 308] {
        let server = MockServer::start().await;
        Mock::given(path("/v2/user"))
            .respond_with(ResponseTemplate::new(status).insert_header(
                "location",
                format!("{}/v2/user?secret={TOKEN}", foreign.uri()),
            ))
            .expect(1)
            .mount(&server)
            .await;
        let error = fails_without_replacing_cache(&mut client(&server)).await;
        assert!(matches!(error, Error::Http { status: actual, .. } if actual == status));
        assert_secret_free(&error);
    }
    assert!(foreign.received_requests().await.unwrap().is_empty());
}

fn assert_secret_free(error: &dyn std::error::Error) {
    let mut current = Some(error);
    while let Some(error) = current {
        assert!(!format!("{error} {error:?}").contains(TOKEN));
        current = error.source();
    }
}

#[tokio::test]
async fn permanent_errors_never_retry_and_transient_categories_share_one_budget() {
    for status in [400, 401, 403, 404, 422] {
        let server = MockServer::start().await;
        Mock::given(path("/v2/user"))
            .respond_with(ResponseTemplate::new(status).set_body_string(TOKEN))
            .expect(1)
            .mount(&server)
            .await;
        let error = fails_without_replacing_cache(&mut client(&server)).await;
        assert_secret_free(&error);
        match status {
            401 | 403 => assert!(matches!(error, Error::Authentication)),
            _ => assert!(matches!(error, Error::Http { status: actual, .. } if actual == status)),
        }
    }
    use std::sync::atomic::{AtomicUsize, Ordering};
    let server = MockServer::start().await;
    let attempt = AtomicUsize::new(0);
    Mock::given(path("/v2/user"))
        .respond_with(move |_: &wiremock::Request| {
            let status = if attempt.fetch_add(1, Ordering::SeqCst) == 1 {
                429
            } else {
                503
            };
            ResponseTemplate::new(status).insert_header("ratelimit-reset", "0")
        })
        .expect(3)
        .mount(&server)
        .await;
    assert!(matches!(
        client(&server).fetch().await,
        Err(Error::Http { status: 503, .. })
    ));
}
