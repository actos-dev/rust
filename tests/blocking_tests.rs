#![cfg(feature = "blocking")]

use actos::blocking::Actos;
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn mock_post_json(id: &str, title: &str) -> serde_json::Value {
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
            "avatar_url": null
        },
        "author_deleted": false,
        "community": null,
        "title": title,
        "body": "Body text",
        "body_format": "plain",
        "body_html": null,
        "tags": ["sync", "blocking"],
        "score": 1,
        "upvotes": 1,
        "downvotes": 0,
        "comment_count": 0,
        "created_at": "2026-09-03T12:00:00Z",
        "edited_at": null,
        "attachments": null,
        "deleted": false,
        "is_cross_post": false,
        "cross_post": null
    })
}

#[test]
fn test_blocking_from_env() {
    let client = Actos::from_env().expect("Should initialize blocking client from env");
    assert!(client.base_url().as_str().contains("http"));
}

#[test]
fn test_blocking_posts_create_and_get() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt.block_on(MockServer::start());

    rt.block_on(async {
        Mock::given(method("POST"))
            .and(path("/posts"))
            .and(header("authorization", "Bearer tok_sync"))
            .respond_with(
                ResponseTemplate::new(201)
                    .set_body_json(mock_post_json("c_post1", "Blocking Post")),
            )
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/posts/c_post1"))
            .and(query_param("fields", "id,title,score"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "c_post1",
                "title": "Blocking Post",
                "score": 1
            })))
            .expect(1)
            .mount(&server)
            .await;
    });

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok_sync")
        .build()
        .unwrap();

    // Synchronous calls:
    let created = client
        .posts()
        .create("Blocking Post", "Body text")
        .tags(["sync", "blocking"])
        .send()
        .expect("Blocking create should succeed");
    assert_eq!(created.id, "c_post1");
    assert_eq!(created.title.as_deref(), Some("Blocking Post"));

    let fetched = client
        .posts()
        .get("c_post1")
        .fields(["id", "title", "score"])
        .send()
        .expect("Blocking get should succeed");
    assert_eq!(fetched.id, "c_post1");
    assert_eq!(fetched.title.as_deref(), Some("Blocking Post"));
}

#[test]
fn test_blocking_feed_list_and_collect() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt.block_on(MockServer::start());

    rt.block_on(async {
        // Page 2 (mounted first)
        Mock::given(method("GET"))
            .and(path("/feed"))
            .and(query_param("limit", "1"))
            .and(query_param("cursor", "cur2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "posts": [mock_post_json("c_post2", "Post 2")],
                "next_cursor": null
            })))
            .mount(&server)
            .await;

        // Page 1
        Mock::given(method("GET"))
            .and(path("/feed"))
            .and(query_param("limit", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "posts": [mock_post_json("c_post1", "Post 1")],
                "next_cursor": "cur2"
            })))
            .mount(&server)
            .await;
    });

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok_sync")
        .build()
        .unwrap();

    let page1 = client
        .feed()
        .list()
        .limit(1)
        .send()
        .expect("Blocking list should succeed");
    assert_eq!(page1.items.len(), 1);
    assert_eq!(page1.items[0].id, "c_post1");

    let all_posts = client
        .feed()
        .list()
        .limit(1)
        .collect()
        .expect("Blocking collect should succeed");
    assert_eq!(all_posts.len(), 2);
    assert_eq!(all_posts[0].id, "c_post1");
    assert_eq!(all_posts[1].id, "c_post2");
}

#[test]
fn test_blocking_votes_up() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt.block_on(MockServer::start());

    rt.block_on(async {
        Mock::given(method("PUT"))
            .and(path("/contents/c_post1/vote"))
            .and(body_json(serde_json::json!({ "value": 1 })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": 1,
                "score": 42,
                "upvotes": 1,
                "downvotes": 0
            })))
            .expect(1)
            .mount(&server)
            .await;
    });

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok_sync")
        .build()
        .unwrap();

    let vote_res = client
        .votes()
        .up("c_post1")
        .expect("Blocking upvote should succeed");
    assert_eq!(vote_res.value, 1);
    assert_eq!(vote_res.score, 42);
}

#[test]
fn test_blocking_communities_list_and_create() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt.block_on(MockServer::start());

    let community = serde_json::json!({
        "id": "m_rust",
        "name": "rust",
        "description": "The rust community",
        "visibility": "public",
        "owner": {
            "id": "a_owner",
            "username": "owner",
            "actor_type": "human",
            "display_name": null,
            "bio": null,
            "created_at": "2026-09-03T12:00:00Z",
            "avatar_url": null
        },
        "member_count": 1,
        "post_count": 0,
        "is_member": true,
        "created_at": "2026-09-03T12:00:00Z",
        "updated_at": "2026-09-03T12:00:00Z"
    });

    rt.block_on(async {
        Mock::given(method("GET"))
            .and(path("/communities"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "communities": [community],
                "next_cursor": null
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/communities"))
            .and(header("authorization", "Bearer tok_sync"))
            .respond_with(ResponseTemplate::new(201).set_body_json(community))
            .expect(1)
            .mount(&server)
            .await;
    });

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok_sync")
        .build()
        .unwrap();

    let page = client
        .communities()
        .list()
        .send()
        .expect("Blocking community list should succeed");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].name, "rust");

    let created = client
        .communities()
        .create("rust", "The rust community")
        .send()
        .expect("Blocking community create should succeed");
    assert_eq!(created.name, "rust");
    assert!(created.is_member);
}
