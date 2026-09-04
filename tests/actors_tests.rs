use actos::Actos;
use actos_types::ErrorCode;
use futures_util::StreamExt;
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn mock_actor(id: &str, username: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "username": username,
        "actor_type": "human",
        "display_name": format!("Display {username}"),
        "bio": null,
        "created_at": "2026-09-03T12:00:00Z",
        "trust_level": 1,
        "avatar_url": null
    })
}

fn mock_content(id: &str, content_type: &str, author_username: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "content_type": content_type,
        "author": mock_actor("a_author", author_username),
        "author_deleted": false,
        "title": if content_type == "post" { Some("Post Title".to_string()) } else { None },
        "body": "Content body",
        "body_format": "markdown",
        "body_html": null,
        "metadata": {},
        "tags": [],
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
async fn test_list_and_stream_actors() {
    let server = MockServer::start().await;

    // Page 2 (with cursor) - mounted first so it matches before the generic limit-only mock
    Mock::given(method("GET"))
        .and(path("/actors"))
        .and(query_param("limit", "2"))
        .and(query_param("cursor", "page_two"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "actors": [mock_actor("a_3", "user3")],
            "next_cursor": null
        })))
        .mount(&server)
        .await;

    // Page 1 (without cursor)
    Mock::given(method("GET"))
        .and(path("/actors"))
        .and(query_param("limit", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "actors": [mock_actor("a_1", "user1"), mock_actor("a_2", "user2")],
            "next_cursor": "page_two"
        })))
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    // 1. Test single page
    let page1 = client.actors().list().limit(2).send().await.unwrap();
    assert_eq!(page1.items.len(), 2);
    assert_eq!(page1.next_cursor.as_deref(), Some("page_two"));
    assert!(page1.has_next());

    // 2. Test stream across all pages
    let stream = client.actors().list().limit(2).stream();
    let actors: Vec<actos::Result<actos_types::auth::ActorSummary>> = stream.collect().await;

    assert_eq!(actors.len(), 3);
    let usernames: Vec<String> = actors
        .into_iter()
        .map(|r| r.expect("valid actor").username)
        .collect();
    assert_eq!(usernames, vec!["user1", "user2", "user3"]);
}

#[tokio::test]
async fn test_get_profile_success_not_found_gone() {
    let server = MockServer::start().await;

    // 1. Active actor
    Mock::given(method("GET"))
        .and(path("/actors/alice"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "actor": mock_actor("a_alice", "alice"),
            "stats": {
                "post_count": 5,
                "comment_count": 12,
                "total_score": 140
            }
        })))
        .mount(&server)
        .await;

    // 2. Non-existent actor -> 404 NOT_FOUND
    Mock::given(method("GET"))
        .and(path("/actors/nonexistent"))
        .respond_with(
            ResponseTemplate::new(404)
                .insert_header("content-type", "application/problem+json")
                .set_body_json(serde_json::json!({
                    "code": "NOT_FOUND",
                    "detail": "Actor not found"
                })),
        )
        .mount(&server)
        .await;

    // 3. Soft-deleted actor -> 410 GONE
    Mock::given(method("GET"))
        .and(path("/actors/ghost"))
        .respond_with(
            ResponseTemplate::new(410)
                .insert_header("content-type", "application/problem+json")
                .set_body_json(serde_json::json!({
                    "code": "GONE",
                    "detail": "Actor account has been deleted"
                })),
        )
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let profile = client.actors().get("alice").await.unwrap();
    assert_eq!(profile.actor.username, "alice");
    assert_eq!(profile.stats.post_count, 5);
    assert_eq!(profile.stats.comment_count, 12);
    assert_eq!(profile.stats.total_score, 140);

    let not_found_err = client.actors().get("nonexistent").await.unwrap_err();
    assert!(not_found_err.is_not_found());
    assert_eq!(not_found_err.code(), Some(ErrorCode::NotFound));

    let gone_err = client.actors().get("ghost").await.unwrap_err();
    assert!(gone_err.is_gone());
    assert_eq!(gone_err.code(), Some(ErrorCode::Gone));
}

#[tokio::test]
async fn test_update_me() {
    let server = MockServer::start().await;

    Mock::given(method("PATCH"))
        .and(path("/actors/me"))
        .and(header("authorization", "Bearer token_alice"))
        .and(body_json(serde_json::json!({
            "display_name": "Alice in Wonderland",
            "bio": "Curiouser and curiouser!"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "actor": {
                "id": "a_alice",
                "username": "alice",
                "actor_type": "human",
                "display_name": "Alice in Wonderland",
                "bio": "Curiouser and curiouser!",
                "created_at": "2026-09-03T12:00:00Z",
                "trust_level": 1,
                "avatar_url": null
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("token_alice")
        .build()
        .unwrap();

    let updated = client
        .actors()
        .update_me()
        .display_name("Alice in Wonderland")
        .bio("Curiouser and curiouser!")
        .send()
        .await
        .unwrap();

    assert_eq!(updated.display_name.as_deref(), Some("Alice in Wonderland"));
    assert_eq!(updated.bio.as_deref(), Some("Curiouser and curiouser!"));
}

#[tokio::test]
async fn test_update_me_avatar_set() {
    let server = MockServer::start().await;

    // `.avatar(..)` must be sent as a string value in the PATCH body
    Mock::given(method("PATCH"))
        .and(path("/actors/me"))
        .and(header("authorization", "Bearer token_alice"))
        .and(body_json(serde_json::json!({
            "avatar": "up_avatar_123"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "actor": {
                "id": "a_alice",
                "username": "alice",
                "actor_type": "human",
                "display_name": null,
                "bio": null,
                "created_at": "2026-09-03T12:00:00Z",
                "trust_level": 1,
                "avatar_url": "https://cdn.actos.dev/up_avatar_123"
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("token_alice")
        .build()
        .unwrap();

    let updated = client
        .actors()
        .update_me()
        .avatar("up_avatar_123")
        .send()
        .await
        .unwrap();

    assert_eq!(
        updated.avatar_url.as_deref(),
        Some("https://cdn.actos.dev/up_avatar_123")
    );
}

#[tokio::test]
async fn test_update_me_tristate_clear() {
    let server = MockServer::start().await;

    // The clear_* variants must send explicit `null` for each field
    Mock::given(method("PATCH"))
        .and(path("/actors/me"))
        .and(header("authorization", "Bearer token_alice"))
        .and(body_json(serde_json::json!({
            "display_name": null,
            "bio": null,
            "avatar": null
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "actor": {
                "id": "a_alice",
                "username": "alice",
                "actor_type": "human",
                "display_name": null,
                "bio": null,
                "created_at": "2026-09-03T12:00:00Z",
                "trust_level": 1,
                "avatar_url": null
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("token_alice")
        .build()
        .unwrap();

    let updated = client
        .actors()
        .update_me()
        .clear_display_name()
        .clear_bio()
        .clear_avatar()
        .send()
        .await
        .unwrap();

    assert_eq!(updated.display_name, None);
    assert_eq!(updated.bio, None);
    assert_eq!(updated.avatar_url, None);
}

#[tokio::test]
async fn test_delete_me() {
    let server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/actors/me"))
        .and(header("authorization", "Bearer token_alice"))
        .and(body_json(serde_json::json!({
            "recovery_code": "rec_final_goodbye"
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("token_alice")
        .build()
        .unwrap();

    client
        .actors()
        .delete_me()
        .recovery_code("rec_final_goodbye")
        .send()
        .await
        .unwrap();
}

#[tokio::test]
async fn test_followers_and_following_with_stream() {
    let server = MockServer::start().await;

    // Followers page 2 (with cursor) - mounted first to match before generic mock
    Mock::given(method("GET"))
        .and(path("/actors/alice/followers"))
        .and(query_param("limit", "1"))
        .and(query_param("cursor", "fol_p2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "actors": [mock_actor("a_charlie", "charlie")],
            "next_cursor": null
        })))
        .mount(&server)
        .await;

    // Followers page 1 (without cursor)
    Mock::given(method("GET"))
        .and(path("/actors/alice/followers"))
        .and(query_param("limit", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "actors": [mock_actor("a_bob", "bob")],
            "next_cursor": "fol_p2"
        })))
        .mount(&server)
        .await;

    // Following
    Mock::given(method("GET"))
        .and(path("/actors/alice/following"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "actors": [mock_actor("a_dave", "dave")],
            "next_cursor": null
        })))
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let followers: Vec<actos::Result<actos_types::auth::ActorSummary>> = client
        .actors()
        .followers("alice")
        .limit(1)
        .stream()
        .collect()
        .await;

    assert_eq!(followers.len(), 2);
    assert_eq!(followers[0].as_ref().unwrap().username, "bob");
    assert_eq!(followers[1].as_ref().unwrap().username, "charlie");

    let following_page = client.actors().following("alice").send().await.unwrap();
    assert_eq!(following_page.items.len(), 1);
    assert_eq!(following_page.items[0].username, "dave");
}

#[tokio::test]
async fn test_posts_and_comments_with_fields() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/actors/alice/posts"))
        .and(query_param("fields", "id,title,score"))
        .and(query_param("sort", "top"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "posts": [mock_content("c_post1", "post", "alice")],
            "next_cursor": null
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/actors/alice/comments"))
        .and(query_param("fields", "id,body"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "comments": [mock_content("c_com1", "comment", "alice")],
            "next_cursor": null
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let posts = client
        .actors()
        .posts("alice")
        .fields(["id", "title", "score"])
        .sort("top")
        .send()
        .await
        .unwrap();

    assert_eq!(posts.items.len(), 1);
    assert_eq!(posts.items[0].id, "c_post1");
    assert_eq!(posts.items[0].content_type, "post");

    let comments = client
        .actors()
        .comments("alice")
        .field("id")
        .field("body")
        .send()
        .await
        .unwrap();

    assert_eq!(comments.items.len(), 1);
    assert_eq!(comments.items[0].id, "c_com1");
    assert_eq!(comments.items[0].content_type, "comment");
}

#[tokio::test]
async fn test_follow_and_unfollow_idempotency() {
    let server = MockServer::start().await;

    // Follow endpoint mounted, expecting exactly 2 calls returning 204
    Mock::given(method("PUT"))
        .and(path("/actors/bob/follow"))
        .and(header("authorization", "Bearer token_alice"))
        .respond_with(ResponseTemplate::new(204))
        .expect(2)
        .mount(&server)
        .await;

    // Unfollow endpoint mounted, expecting exactly 2 calls returning 204
    Mock::given(method("DELETE"))
        .and(path("/actors/bob/follow"))
        .and(header("authorization", "Bearer token_alice"))
        .respond_with(ResponseTemplate::new(204))
        .expect(2)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("token_alice")
        .build()
        .unwrap();

    // Repeated follow succeeds (idempotent)
    client.actors().follow("bob").await.unwrap();
    client.actors().follow("bob").await.unwrap();

    // Repeated unfollow succeeds (idempotent)
    client.actors().unfollow("bob").await.unwrap();
    client.actors().unfollow("bob").await.unwrap();
}
