use actos::Actos;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_meta_health_and_ready() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "ok"
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/health/ready"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "ready",
            "database": { "status": "up" },
            "redis": { "status": "up" },
            "storage": { "status": "up" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let health = client.meta().health().await.unwrap();
    assert_eq!(health["status"], "ok");

    let ready = client.meta().ready().await.unwrap();
    assert_eq!(ready["status"], "ready");
    assert_eq!(ready["database"]["status"], "up");
}

#[tokio::test]
async fn test_meta_version_and_openapi() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "name": "actos-api",
            "version": "0.1.0",
            "git_sha": "abc1234",
            "api_version": "v1"
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/openapi.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "openapi": "3.1.0",
            "info": { "title": "Actos API", "version": "0.1.0" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let meta_ver = client.meta().version().await.unwrap();
    assert_eq!(meta_ver.sdk, actos::VERSION);
    assert_eq!(meta_ver.server["name"], "actos-api");
    assert_eq!(meta_ver.server["version"], "0.1.0");

    let openapi = client.meta().openapi().await.unwrap();
    assert_eq!(openapi["openapi"], "3.1.0");
    assert_eq!(openapi["info"]["title"], "Actos API");
}

#[tokio::test]
async fn test_client_rate_limit_accessor() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-ratelimit-limit", "100")
                .insert_header("x-ratelimit-remaining", "95")
                .insert_header("x-ratelimit-reset", "1700000000")
                .set_body_json(serde_json::json!({ "status": "ok" })),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();
    assert!(client.rate_limit().is_none());

    let _ = client.meta().health().await.unwrap();
    let rl = client.rate_limit().expect("Rate limit should be populated");
    assert_eq!(rl.limit, 100);
    assert_eq!(rl.remaining, 95);
    assert_eq!(rl.reset, 1700000000);
}
