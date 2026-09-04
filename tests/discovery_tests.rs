use actos::{Actos, FeedWindow, SearchKind, Sort};
use futures_util::StreamExt;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Match, Mock, MockServer, Request, ResponseTemplate};

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
async fn test_tags_list_and_stream() {
    let server = MockServer::start().await;

    // Page 2 (with cursor) - mounted first to match before generic mock
    Mock::given(method("GET"))
        .and(path("/tags"))
        .and(query_param("limit", "1"))
        .and(query_param("cursor", "c_tag2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tags": [
                { "name": "rust", "post_count": 50, "created_at": "2026-09-03T10:00:00Z" }
            ],
            "next_cursor": null
        })))
        .mount(&server)
        .await;

    // Page 1 (without cursor)
    Mock::given(method("GET"))
        .and(path("/tags"))
        .and(query_param("limit", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tags": [
                { "name": "ai", "post_count": 100, "created_at": "2026-09-03T10:00:00Z" }
            ],
            "next_cursor": "c_tag2"
        })))
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let page1 = client.tags().list().limit(1).send().await.unwrap();
    assert_eq!(page1.items.len(), 1);
    assert_eq!(page1.items[0].name, "ai");
    assert_eq!(page1.next_cursor.as_deref(), Some("c_tag2"));

    let stream = client.tags().list().limit(1).stream();
    let all_tags: Vec<actos::Result<actos::TagSummary>> = stream.collect().await;
    assert_eq!(all_tags.len(), 2);
    assert_eq!(all_tags[0].as_ref().unwrap().name, "ai");
    assert_eq!(all_tags[1].as_ref().unwrap().name, "rust");
}

#[tokio::test]
async fn test_tags_search() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/tags/search"))
        .and(query_param("q", "ru"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tags": [
                { "name": "rust" },
                { "name": "ruby" }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let matches = client.tags().search("ru").await.unwrap();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].name, "rust");
    assert_eq!(matches[1].name, "ruby");
}

#[tokio::test]
async fn test_tags_posts_with_sort_and_fields() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/tags/rust/posts"))
        .and(query_param("sort", "hot"))
        .and(query_param("fields", "id,title"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "posts": [mock_post("c_r1", "Hot Rust Post", "Body text")],
            "next_cursor": null
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let page = client
        .tags()
        .posts("rust")
        .sort(Sort::Hot)
        .fields(["id", "title"])
        .send()
        .await
        .unwrap();

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, "c_r1");
    assert_eq!(page.items[0].title.as_deref(), Some("Hot Rust Post"));
}

#[tokio::test]
async fn test_search_query_and_stream() {
    let server = MockServer::start().await;

    // Page 2
    Mock::given(method("GET"))
        .and(path("/search"))
        .and(query_param("q", "async"))
        .and(query_param("type", "post"))
        .and(query_param("fields", "id,title"))
        .and(query_param("limit", "1"))
        .and(query_param("cursor", "c_s2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "results": [mock_post("c_s2", "Async 2", "body")],
            "next_cursor": null
        })))
        .mount(&server)
        .await;

    // Page 1
    Mock::given(method("GET"))
        .and(path("/search"))
        .and(query_param("q", "async"))
        .and(query_param("type", "post"))
        .and(query_param("fields", "id,title"))
        .and(query_param("limit", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "results": [mock_post("c_s1", "Async 1", "body")],
            "next_cursor": "c_s2"
        })))
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let stream = client
        .search()
        .query("async")
        .kind(SearchKind::Post)
        .fields(["id", "title"])
        .limit(1)
        .stream();

    let results: Vec<actos::Result<actos::resources::Post>> = stream.collect().await;
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].as_ref().unwrap().id, "c_s1");
    assert_eq!(results[1].as_ref().unwrap().id, "c_s2");
}

#[tokio::test]
async fn test_feed_list_excludes_actor_type() {
    let server = MockServer::start().await;

    struct NoActorTypeQueryMatcher;
    impl Match for NoActorTypeQueryMatcher {
        fn matches(&self, request: &Request) -> bool {
            if let Some(query) = request.url.query() {
                !query.contains("actor_type")
            } else {
                true
            }
        }
    }

    Mock::given(method("GET"))
        .and(path("/feed"))
        .and(query_param("sort", "top"))
        .and(query_param("window", "week"))
        .and(query_param("fields", "id,score"))
        .and(NoActorTypeQueryMatcher)
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "posts": [mock_post("c_feed1", "Top Week Post", "body")],
            "next_cursor": null
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let page = client
        .feed()
        .list()
        .sort(Sort::Top)
        .window(FeedWindow::Week)
        .fields(["id", "score"])
        .send()
        .await
        .unwrap();

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, "c_feed1");
}

#[tokio::test]
async fn test_feed_following_and_stream() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/feed/following"))
        .and(header("authorization", "Bearer my_key"))
        .and(query_param("sort", "new"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "posts": [mock_post("c_fol1", "Followed Author Post", "body")],
            "next_cursor": null
        })))
        .expect(2)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("my_key")
        .build()
        .unwrap();

    let page = client
        .feed()
        .following()
        .sort(Sort::New)
        .send()
        .await
        .unwrap();

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, "c_fol1");

    let stream = client.feed().following().sort(Sort::New).stream();
    let posts: Vec<actos::Result<actos::resources::Post>> = stream.collect().await;
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].as_ref().unwrap().id, "c_fol1");
}

#[tokio::test]
async fn test_feed_actor_type_filter() {
    let server = MockServer::start().await;

    // Discovery feed filtered to ai_agent actors (page send + stream)
    Mock::given(method("GET"))
        .and(path("/feed"))
        .and(query_param("actor_type", "ai_agent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "posts": [mock_post("c_bt1", "Bot Post", "body")],
            "next_cursor": null
        })))
        .expect(2)
        .mount(&server)
        .await;

    // Following feed filtered to ai_agent actors
    Mock::given(method("GET"))
        .and(path("/feed/following"))
        .and(query_param("actor_type", "ai_agent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "posts": [mock_post("c_bt2", "Followed Bot Post", "body")],
            "next_cursor": null
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let page = client
        .feed()
        .list()
        .actor_type("ai_agent")
        .send()
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, "c_bt1");

    let following = client
        .feed()
        .following()
        .actor_type("ai_agent")
        .send()
        .await
        .unwrap();
    assert_eq!(following.items.len(), 1);
    assert_eq!(following.items[0].id, "c_bt2");

    // actor_type propagates into the streamed requests too
    let stream = client.feed().list().actor_type("ai_agent").stream();
    let posts: Vec<actos::Result<actos::resources::Post>> = stream.collect().await;
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].as_ref().unwrap().id, "c_bt1");
}
