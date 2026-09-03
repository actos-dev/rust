use actos::Actos;
use futures_util::StreamExt;
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

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
        "tags": ["rust"],
        "score": 10,
        "upvotes": 10,
        "downvotes": 0,
        "comment_count": 0,
        "created_at": "2026-09-03T12:00:00Z",
        "edited_at": null,
        "attachments": null,
        "deleted": false
    })
}

#[tokio::test]
async fn test_votes_up_down_clear() {
    let server = MockServer::start().await;

    // Upvote (+1)
    Mock::given(method("PUT"))
        .and(path("/contents/c_1/vote"))
        .and(body_json(serde_json::json!({ "value": 1 })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "value": 1,
            "score": 11,
            "upvotes": 11,
            "downvotes": 0
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Downvote (-1)
    Mock::given(method("PUT"))
        .and(path("/contents/c_1/vote"))
        .and(body_json(serde_json::json!({ "value": -1 })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "value": -1,
            "score": 9,
            "upvotes": 10,
            "downvotes": 1
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Clear (0)
    Mock::given(method("PUT"))
        .and(path("/contents/c_1/vote"))
        .and(body_json(serde_json::json!({ "value": 0 })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "value": 0,
            "score": 10,
            "upvotes": 10,
            "downvotes": 0
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok_123")
        .build()
        .unwrap();

    let res_up = client.votes().up("c_1").await.unwrap();
    assert_eq!(res_up.value, 1);
    assert_eq!(res_up.score, 11);

    let res_down = client.votes().down("c_1").await.unwrap();
    assert_eq!(res_down.value, -1);
    assert_eq!(res_down.score, 9);

    let res_clear = client.votes().clear("c_1").await.unwrap();
    assert_eq!(res_clear.value, 0);
    assert_eq!(res_clear.score, 10);
}

#[tokio::test]
async fn test_votes_list_with_and_without_content_ids() {
    let server = MockServer::start().await;

    // With content_ids filter (mounted first to match before generic mock)
    Mock::given(method("GET"))
        .and(path("/me/votes"))
        .and(query_param("content_ids", "c_1,c_2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "votes": {
                "c_1": 1
            }
        })))
        .mount(&server)
        .await;

    // Without content_ids
    Mock::given(method("GET"))
        .and(path("/me/votes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "votes": {
                "c_1": 1,
                "c_2": -1
            }
        })))
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok_123")
        .build()
        .unwrap();

    let all_votes = client.votes().list(None).await.unwrap();
    assert_eq!(all_votes.len(), 2);
    assert_eq!(all_votes.get("c_1"), Some(&1));
    assert_eq!(all_votes.get("c_2"), Some(&-1));

    let filtered_votes = client.votes().list(Some(&["c_1", "c_2"])).await.unwrap();
    assert_eq!(filtered_votes.len(), 1);
    assert_eq!(filtered_votes.get("c_1"), Some(&1));
}

#[tokio::test]
async fn test_saves_add_remove() {
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/contents/c_save1/save"))
        .and(header("authorization", "Bearer tok_123"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/contents/c_save1/save"))
        .and(header("authorization", "Bearer tok_123"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok_123")
        .build()
        .unwrap();

    client.saves().add("c_save1").await.unwrap();
    client.saves().remove("c_save1").await.unwrap();
}

#[tokio::test]
async fn test_saves_list_and_stream_with_fields() {
    let server = MockServer::start().await;

    // Page 2 - mounted first to match before generic mock without cursor
    Mock::given(method("GET"))
        .and(path("/me/saves"))
        .and(query_param("fields", "id,title"))
        .and(query_param("limit", "1"))
        .and(query_param("cursor", "cur_s2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "saves": [mock_post("c_s2", "Saved Post 2", "body 2")],
            "next_cursor": null
        })))
        .mount(&server)
        .await;

    // Page 1
    Mock::given(method("GET"))
        .and(path("/me/saves"))
        .and(query_param("fields", "id,title"))
        .and(query_param("limit", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "saves": [mock_post("c_s1", "Saved Post 1", "body 1")],
            "next_cursor": "cur_s2"
        })))
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok_123")
        .build()
        .unwrap();

    let page1 = client
        .saves()
        .list()
        .fields(["id", "title"])
        .limit(1)
        .send()
        .await
        .unwrap();
    assert_eq!(page1.items.len(), 1);
    assert_eq!(page1.items[0].id, "c_s1");
    assert_eq!(page1.items[0].title.as_deref(), Some("Saved Post 1"));
    assert_eq!(page1.next_cursor.as_deref(), Some("cur_s2"));

    let stream = client
        .saves()
        .list()
        .fields(["id", "title"])
        .limit(1)
        .stream();

    let all_saves: Vec<actos::Result<actos::resources::Post>> = stream.collect().await;
    assert_eq!(all_saves.len(), 2);
    assert_eq!(all_saves[0].as_ref().unwrap().id, "c_s1");
    assert_eq!(all_saves[1].as_ref().unwrap().id, "c_s2");
}

#[tokio::test]
async fn test_idempotency_votes_and_saves() {
    let server = MockServer::start().await;

    // 1. votes.set repeated twice
    Mock::given(method("PUT"))
        .and(path("/contents/c_idem/vote"))
        .and(body_json(serde_json::json!({ "value": 1 })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "value": 1,
            "score": 5,
            "upvotes": 5,
            "downvotes": 0
        })))
        .expect(2)
        .mount(&server)
        .await;

    // 2. votes.up repeated twice
    Mock::given(method("PUT"))
        .and(path("/contents/c_idem_up/vote"))
        .and(body_json(serde_json::json!({ "value": 1 })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "value": 1,
            "score": 10,
            "upvotes": 10,
            "downvotes": 0
        })))
        .expect(2)
        .mount(&server)
        .await;

    // 3. saves.add repeated twice
    Mock::given(method("PUT"))
        .and(path("/contents/c_idem_save/save"))
        .respond_with(ResponseTemplate::new(204))
        .expect(2)
        .mount(&server)
        .await;

    // 4. saves.remove repeated twice
    Mock::given(method("DELETE"))
        .and(path("/contents/c_idem_save/save"))
        .respond_with(ResponseTemplate::new(204))
        .expect(2)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok_123")
        .build()
        .unwrap();

    // Repeated votes.set
    client.votes().set("c_idem", 1).await.unwrap();
    client.votes().set("c_idem", 1).await.unwrap();

    // Repeated votes.up
    client.votes().up("c_idem_up").await.unwrap();
    client.votes().up("c_idem_up").await.unwrap();

    // Repeated saves.add
    client.saves().add("c_idem_save").await.unwrap();
    client.saves().add("c_idem_save").await.unwrap();

    // Repeated saves.remove
    client.saves().remove("c_idem_save").await.unwrap();
    client.saves().remove("c_idem_save").await.unwrap();
}
