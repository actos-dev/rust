use actos::Actos;
use futures_util::StreamExt;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn mock_notification(id: &str, kind: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "kind": kind,
        "actor": {
            "id": "a_1",
            "username": "alice",
            "actor_type": "human",
            "display_name": "Alice",
            "bio": null,
            "created_at": "2026-09-03T12:00:00Z",
            "trust_level": 1,
            "avatar_url": null
        },
        "target_type": "content",
        "target_id": "c_post1",
        "payload": {},
        "created_at": "2026-09-03T12:00:00Z",
        "read_at": null
    })
}

fn inbox_response(
    notifications: Vec<serde_json::Value>,
    next_cursor: Option<&str>,
    unread_count: i64,
) -> serde_json::Value {
    serde_json::json!({
        "notifications": notifications,
        "next_cursor": next_cursor,
        "unread_count": unread_count
    })
}

#[tokio::test]
async fn test_inbox_list_with_params() {
    let server = MockServer::start().await;

    // `.unread(..)` and `.limit(..)` must be sent as query parameters
    Mock::given(method("GET"))
        .and(path("/me/inbox"))
        .and(query_param("unread", "true"))
        .and(query_param("limit", "5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(inbox_response(
            vec![mock_notification("n_1", "comment_on_post")],
            Some("inbox_p2"),
            3,
        )))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let page = client
        .inbox()
        .list()
        .unread(true)
        .limit(5)
        .send()
        .await
        .unwrap();

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, "n_1");
    assert_eq!(page.items[0].kind, "comment_on_post");
    assert_eq!(page.next_cursor.as_deref(), Some("inbox_p2"));
    assert!(page.has_next());
}

#[tokio::test]
async fn test_inbox_stream_paginates() {
    let server = MockServer::start().await;

    // Page 2 (with cursor) - mounted first so it matches before the generic no-cursor mock
    Mock::given(method("GET"))
        .and(path("/me/inbox"))
        .and(query_param("cursor", "inbox_p2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(inbox_response(
            vec![mock_notification("n_2", "new_follower")],
            None,
            0,
        )))
        .mount(&server)
        .await;

    // Page 1 (no cursor)
    Mock::given(method("GET"))
        .and(path("/me/inbox"))
        .respond_with(ResponseTemplate::new(200).set_body_json(inbox_response(
            vec![mock_notification("n_1", "comment_on_post")],
            Some("inbox_p2"),
            1,
        )))
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let stream = client.inbox().stream();
    let all: Vec<actos::Result<actos_types::notification::NotificationSummary>> =
        stream.collect().await;
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].as_ref().unwrap().id, "n_1");
    assert_eq!(all[1].as_ref().unwrap().id, "n_2");
}

#[tokio::test]
async fn test_inbox_read() {
    let server = MockServer::start().await;

    Mock::given(method("PATCH"))
        .and(path("/me/inbox/n_99/read"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    client.inbox().read("n_99").await.unwrap();
}

#[tokio::test]
async fn test_inbox_read_all() {
    let server = MockServer::start().await;

    // More specific mock (cursor present) must be mounted first so wiremock
    // picks it before the generic no-cursor mock.
    Mock::given(method("POST"))
        .and(path("/me/inbox/read"))
        .and(query_param("cursor", "inbox_p1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "marked": 1
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Bulk read without cursor marks everything
    Mock::given(method("POST"))
        .and(path("/me/inbox/read"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "marked": 3
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let all = client.inbox().read_all(None::<String>).await.unwrap();
    assert_eq!(
        all.marked, 3,
        "read_all() without a cursor must mark every notification"
    );

    let up_to = client.inbox().read_all(Some("inbox_p1")).await.unwrap();
    assert_eq!(
        up_to.marked, 1,
        "read_all(cursor) must mark only notifications up to cursor"
    );
}

#[tokio::test]
async fn test_inbox_unread_count() {
    let server = MockServer::start().await;

    // unread_count() probes GET /me/inbox with limit=1&unread=true
    Mock::given(method("GET"))
        .and(path("/me/inbox"))
        .and(query_param("limit", "1"))
        .and(query_param("unread", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(inbox_response(vec![], None, 7)))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let count = client.inbox().unread_count().await.unwrap();
    assert_eq!(count, 7);
}
