use actos::Actos;
use actos_types::ErrorCode;
use futures_util::StreamExt;
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn mock_comment(id: &str, body: &str, deleted: bool) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "content_type": "comment",
        "author": {
            "id": "a_1",
            "username": "bob",
            "actor_type": "human",
            "display_name": "Bob",
            "bio": null,
            "created_at": "2026-09-03T12:00:00Z",
            "trust_level": 1,
            "avatar_url": null
        },
        "author_deleted": false,
        "title": null,
        "body": body,
        "body_format": "plain",
        "body_html": null,
        "metadata": {},
        "tags": [],
        "score": 5,
        "upvotes": 5,
        "downvotes": 0,
        "comment_count": 0,
        "created_at": "2026-09-03T12:00:00Z",
        "edited_at": null,
        "attachments": null,
        "deleted": deleted
    })
}

fn mock_comment_node(id: &str, body: &str, replies: Vec<serde_json::Value>) -> serde_json::Value {
    let mut val = mock_comment(id, body, false);
    val["replies"] = serde_json::Value::Array(replies);
    val
}

#[tokio::test]
async fn test_create_top_level_and_reply() {
    let server = MockServer::start().await;

    // 1. Top-level comment
    Mock::given(method("POST"))
        .and(path("/posts/c_post1/comments"))
        .and(body_json(serde_json::json!({
            "body": "Top level comment"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_comment(
            "c_com1",
            "Top level comment",
            false,
        )))
        .expect(1)
        .mount(&server)
        .await;

    // 2. Nested reply with parent_id
    Mock::given(method("POST"))
        .and(path("/posts/c_post1/comments"))
        .and(body_json(serde_json::json!({
            "body": "Nested reply",
            "parent_id": "c_com1"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_comment(
            "c_reply1",
            "Nested reply",
            false,
        )))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let top = client
        .comments()
        .create("c_post1", "Top level comment")
        .send()
        .await
        .unwrap();
    assert_eq!(top.id, "c_com1");
    assert_eq!(top.content_type, "comment");

    let reply = client
        .comments()
        .create("c_post1", "Nested reply")
        .parent_id("c_com1")
        .send()
        .await
        .unwrap();
    assert_eq!(reply.id, "c_reply1");
}

#[tokio::test]
async fn test_list_and_stream_comments() {
    let server = MockServer::start().await;

    // Page 2 (with cursor) - mounted first to match before generic mock without cursor
    Mock::given(method("GET"))
        .and(path("/posts/c_post1/comments"))
        .and(query_param("sort", "top"))
        .and(query_param("depth", "2"))
        .and(query_param("parent", "c_root"))
        .and(query_param("limit", "1"))
        .and(query_param("cursor", "cursor_p2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "comments": [mock_comment_node("c_node2", "Second root comment", vec![])],
            "next_cursor": null
        })))
        .mount(&server)
        .await;

    // Page 1 (without cursor)
    let child_node = mock_comment_node("c_child1", "Child reply", vec![]);
    Mock::given(method("GET"))
        .and(path("/posts/c_post1/comments"))
        .and(query_param("sort", "top"))
        .and(query_param("depth", "2"))
        .and(query_param("parent", "c_root"))
        .and(query_param("limit", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "comments": [mock_comment_node("c_node1", "First root comment", vec![child_node])],
            "next_cursor": "cursor_p2"
        })))
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    // 1. Test single page
    let page1 = client
        .comments()
        .list("c_post1")
        .sort("top")
        .depth(2)
        .parent("c_root")
        .limit(1)
        .send()
        .await
        .unwrap();

    assert_eq!(page1.items.len(), 1);
    assert_eq!(page1.items[0].content.id, "c_node1");
    assert_eq!(page1.items[0].replies.len(), 1);
    assert_eq!(page1.items[0].replies[0].content.id, "c_child1");
    assert_eq!(page1.next_cursor.as_deref(), Some("cursor_p2"));
    assert!(page1.has_next());

    // 2. Test stream across all pages
    let stream = client
        .comments()
        .list("c_post1")
        .sort("top")
        .depth(2)
        .parent("c_root")
        .limit(1)
        .stream();

    let all_nodes: Vec<actos::Result<actos::resources::CommentNodeResponse>> =
        stream.collect().await;
    assert_eq!(all_nodes.len(), 2);
    assert_eq!(all_nodes[0].as_ref().unwrap().content.id, "c_node1");
    assert_eq!(all_nodes[1].as_ref().unwrap().content.id, "c_node2");
}

#[tokio::test]
async fn test_get_comment_detail_with_ancestors() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/comments/c_child"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "comment": mock_comment("c_child", "Child reply body", false),
            "ancestors": [
                {
                    "id": "c_post",
                    "content_type": "post",
                    "author": {
                        "id": "a_author",
                        "username": "author",
                        "actor_type": "human",
                        "display_name": "Author",
                        "bio": null,
                        "created_at": "2026-09-03T12:00:00Z",
                        "trust_level": 1,
                        "avatar_url": null
                    },
                    "author_deleted": false,
                    "title": Some("Root Post"),
                    "body": "Post body",
                    "body_format": "markdown",
                    "body_html": null,
                    "metadata": {},
                    "tags": [],
                    "score": 10,
                    "upvotes": 10,
                    "downvotes": 0,
                    "comment_count": 2,
                    "created_at": "2026-09-03T12:00:00Z",
                    "edited_at": null,
                    "attachments": null,
                    "deleted": false
                },
                mock_comment("c_parent", "Parent comment body", false)
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let detail = client.comments().get("c_child").await.unwrap();
    assert_eq!(detail.comment.id, "c_child");
    assert_eq!(detail.ancestors.len(), 2);
    assert_eq!(detail.ancestors[0].id, "c_post");
    assert_eq!(detail.ancestors[1].id, "c_parent");
}

#[tokio::test]
async fn test_get_soft_deleted_comment() {
    let server = MockServer::start().await;

    // Soft-deleted comment returns 200 OK (NOT 410) with deleted: true and body: "[silindi]"
    Mock::given(method("GET"))
        .and(path("/comments/c_deleted"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "comment": mock_comment("c_deleted", "[silindi]", true),
            "ancestors": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let detail = client.comments().get("c_deleted").await.unwrap();
    assert_eq!(detail.comment.id, "c_deleted");
    assert!(detail.comment.deleted);
    assert_eq!(detail.comment.body, "[silindi]");
}

#[tokio::test]
async fn test_get_comment_not_found() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/comments/c_missing"))
        .respond_with(
            ResponseTemplate::new(404)
                .insert_header("content-type", "application/problem+json")
                .set_body_json(serde_json::json!({
                    "code": "NOT_FOUND",
                    "detail": "Comment not found"
                })),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let err = client.comments().get("c_missing").await.unwrap_err();
    assert!(err.is_not_found());
    assert_eq!(err.code(), Some(ErrorCode::NotFound));
}

#[tokio::test]
async fn test_update_and_delete_comment() {
    let server = MockServer::start().await;

    // Update
    Mock::given(method("PATCH"))
        .and(path("/comments/c_mod"))
        .and(header("authorization", "Bearer tok_123"))
        .and(body_json(
            serde_json::json!({ "body": "Updated comment text" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_comment(
            "c_mod",
            "Updated comment text",
            false,
        )))
        .expect(1)
        .mount(&server)
        .await;

    // Delete
    Mock::given(method("DELETE"))
        .and(path("/comments/c_mod"))
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

    let updated = client
        .comments()
        .update("c_mod", "Updated comment text")
        .await
        .unwrap();
    assert_eq!(updated.body, "Updated comment text");

    client.comments().delete("c_mod").await.unwrap();
}
