use actos::Actos;
use actos_types::ErrorCode;
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Match, Mock, MockServer, Request, ResponseTemplate};

struct HeaderMissingMatcher(&'static str);

impl Match for HeaderMissingMatcher {
    fn matches(&self, request: &Request) -> bool {
        !request
            .headers
            .contains_key(wiremock::http::HeaderName::from_static(self.0))
    }
}

fn mock_post(id: &str, title: &str, body: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "content_type": "post",
        "author": {
            "id": "a_1",
            "username": "alice",
            "actor_type": "human",
            "display_name": "Alice",
            "bio": null,
            "created_at": "2026-09-03T12:00:00Z",
            "trust_level": 1,
            "avatar_url": null
        },
        "author_deleted": false,
        "title": title,
        "body": body,
        "body_format": "markdown",
        "body_html": null,
        "metadata": {},
        "tags": ["rust", "sdk"],
        "score": 42,
        "upvotes": 42,
        "downvotes": 0,
        "comment_count": 0,
        "created_at": "2026-09-03T12:00:00Z",
        "edited_at": null,
        "attachments": null,
        "deleted": false
    })
}

#[tokio::test]
async fn test_create_post_auto_idempotency_key() {
    let server = MockServer::start().await;

    struct UuidHeaderMatcher;
    impl Match for UuidHeaderMatcher {
        fn matches(&self, request: &Request) -> bool {
            let key = wiremock::http::HeaderName::from_static("idempotency-key");
            if let Some(val) = request.headers.get(&key)
                && let Ok(s) = val.to_str()
            {
                return uuid::Uuid::parse_str(s).is_ok();
            }
            false
        }
    }

    Mock::given(method("POST"))
        .and(path("/posts"))
        .and(UuidHeaderMatcher)
        .and(body_json(serde_json::json!({
            "title": "Hello World",
            "body": "First post",
            "tags": ["rust"],
            "metadata": {}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_post(
            "c_auto",
            "Hello World",
            "First post",
        )))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok_123")
        .build()
        .unwrap();

    let post = client
        .posts()
        .create("Hello World", "First post")
        .tags(["rust"])
        .send()
        .await
        .unwrap();

    assert_eq!(post.id, "c_auto");
    assert_eq!(post.title.as_deref(), Some("Hello World"));
}

#[tokio::test]
async fn test_create_post_custom_idempotency_key() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/posts"))
        .and(header("idempotency-key", "custom_idem_token_777"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_post(
            "c_custom",
            "Custom Key",
            "Body",
        )))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let post = client
        .posts()
        .create("Custom Key", "Body")
        .idempotency_key("custom_idem_token_777")
        .send()
        .await
        .unwrap();

    assert_eq!(post.id, "c_custom");
}

#[tokio::test]
async fn test_create_post_no_idempotency_key() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/posts"))
        .and(HeaderMissingMatcher("idempotency-key"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_post("c_noidem", "No Idem", "Body")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let post = client
        .posts()
        .create("No Idem", "Body")
        .no_idempotency_key()
        .send()
        .await
        .unwrap();

    assert_eq!(post.id, "c_noidem");
}

#[tokio::test]
async fn test_create_post_auto_key_retries_on_500() {
    let server = MockServer::start().await;

    // Fail first time with 500
    Mock::given(method("POST"))
        .and(path("/posts"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // Succeed on second attempt
    Mock::given(method("POST"))
        .and(path("/posts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_post(
            "c_retried",
            "Retried Title",
            "Body",
        )))
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .max_retries(2)
        .build()
        .unwrap();

    let post = client
        .posts()
        .create("Retried Title", "Body")
        .send()
        .await
        .unwrap();

    assert_eq!(post.id, "c_retried");
}

#[tokio::test]
async fn test_create_post_no_key_does_not_retry_on_500() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/posts"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .expect(1) // Must only be called once, no retry!
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .max_retries(2)
        .build()
        .unwrap();

    let err = client
        .posts()
        .create("No Retry", "Body")
        .no_idempotency_key()
        .send()
        .await
        .unwrap_err();

    assert_eq!(err.status(), Some(500));
}

#[tokio::test]
async fn test_get_post_with_and_without_fields() {
    let server = MockServer::start().await;

    // 1. Without fields (returns full post)
    Mock::given(method("GET"))
        .and(path("/posts/c_full"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_post(
            "c_full",
            "Full Post",
            "Complete body content",
        )))
        .expect(1)
        .mount(&server)
        .await;

    // 2. With fields query parameter
    Mock::given(method("GET"))
        .and(path("/posts/c_sparse"))
        .and(query_param("fields", "id,title,score"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "c_sparse",
            "title": "Sparse Title",
            "score": 100
        })))
        .expect(2)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let full = client.posts().get("c_full").send().await.unwrap();
    assert_eq!(full.id, "c_full");
    assert_eq!(full.body, "Complete body content");
    assert_eq!(full.title.as_deref(), Some("Full Post"));

    // Deserialized into Post (with synthesized missing fields)
    let sparse = client
        .posts()
        .get("c_sparse")
        .fields(["id", "title", "score"])
        .send()
        .await
        .unwrap();

    assert_eq!(sparse.id, "c_sparse");
    assert_eq!(sparse.title.as_deref(), Some("Sparse Title"));
    assert_eq!(sparse.score, 100);
    assert_eq!(sparse.body, ""); // default empty string fallback

    // Raw JSON
    let sparse_json = client
        .posts()
        .get("c_sparse")
        .fields(["id", "title", "score"])
        .send_json()
        .await
        .unwrap();

    assert_eq!(sparse_json["id"], "c_sparse");
    assert_eq!(sparse_json["title"], "Sparse Title");
    assert_eq!(sparse_json["score"], 100);
}

#[tokio::test]
async fn test_get_post_not_found() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/posts/c_nonexistent"))
        .respond_with(
            ResponseTemplate::new(404)
                .insert_header("content-type", "application/problem+json")
                .set_body_json(serde_json::json!({
                    "code": "NOT_FOUND",
                    "detail": "Post does not exist"
                })),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let err = client
        .posts()
        .get("c_nonexistent")
        .send()
        .await
        .unwrap_err();
    assert!(err.is_not_found());
    assert_eq!(err.code(), Some(ErrorCode::NotFound));
}

#[tokio::test]
async fn test_update_post() {
    let server = MockServer::start().await;

    Mock::given(method("PATCH"))
        .and(path("/posts/c_up"))
        .and(body_json(serde_json::json!({
            "title": "Updated Title",
            "body": "Updated Body"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_post(
            "c_up",
            "Updated Title",
            "Updated Body",
        )))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let updated = client
        .posts()
        .update("c_up")
        .title("Updated Title")
        .body("Updated Body")
        .send()
        .await
        .unwrap();

    assert_eq!(updated.title.as_deref(), Some("Updated Title"));
    assert_eq!(updated.body, "Updated Body");
}

#[tokio::test]
async fn test_delete_post_followed_by_get_gone() {
    let server = MockServer::start().await;

    // 1. Delete post -> 204
    Mock::given(method("DELETE"))
        .and(path("/posts/c_deleted"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    // 2. Get deleted post -> 410 GONE
    Mock::given(method("GET"))
        .and(path("/posts/c_deleted"))
        .respond_with(
            ResponseTemplate::new(410)
                .insert_header("content-type", "application/problem+json")
                .set_body_json(serde_json::json!({
                    "code": "GONE",
                    "detail": "Post has been deleted"
                })),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    client.posts().delete("c_deleted").await.unwrap();

    let err = client.posts().get("c_deleted").send().await.unwrap_err();
    assert!(err.is_gone());
    assert_eq!(err.code(), Some(ErrorCode::Gone));
}
