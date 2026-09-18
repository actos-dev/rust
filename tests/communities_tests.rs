use actos::Actos;
use futures_util::StreamExt;
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn mock_actor(username: &str) -> serde_json::Value {
    serde_json::json!({
        "id": format!("a_{username}"),
        "username": username,
        "actor_type": "human",
        "display_name": format!("Display {username}"),
        "bio": null,
        "created_at": "2026-09-03T12:00:00Z",
        "avatar_url": null
    })
}

fn mock_community(name: &str, visibility: &str) -> serde_json::Value {
    serde_json::json!({
        "id": format!("m_{name}"),
        "name": name,
        "description": format!("The {name} community"),
        "visibility": visibility,
        "owner": mock_actor("owner"),
        "member_count": 3,
        "post_count": 7,
        "is_member": false,
        "created_at": "2026-09-03T12:00:00Z",
        "updated_at": "2026-09-03T12:00:00Z"
    })
}

fn mock_member(username: &str) -> serde_json::Value {
    serde_json::json!({
        "actor": mock_actor(username),
        "joined_at": "2026-09-03T12:00:00Z"
    })
}

fn mock_community_ref(name: &str) -> serde_json::Value {
    serde_json::json!({ "id": format!("m_{name}"), "name": name })
}

fn mock_invitation(id: &str, name: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "community": mock_community_ref(name),
        "invited_by": mock_actor("mod"),
        "created_at": "2026-09-03T12:00:00Z"
    })
}

fn mock_application(id: &str, name: &str, status: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "community": mock_community_ref(name),
        "applicant": mock_actor("applicant"),
        "reason": "I would like to join",
        "status": status,
        "created_at": "2026-09-03T12:00:00Z",
        "resolved_at": null
    })
}

fn mock_post(id: &str, title: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "content_type": "post",
        "author": mock_actor("alice"),
        "author_deleted": false,
        "community": mock_community_ref("rust"),
        "title": title,
        "body": "Body",
        "body_format": "markdown",
        "body_html": null,
        "tags": [],
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

#[tokio::test]
async fn test_directory_list_and_stream() {
    let server = MockServer::start().await;

    // Page 2 (with cursor), mounted first
    Mock::given(method("GET"))
        .and(path("/communities"))
        .and(query_param("limit", "1"))
        .and(query_param("cursor", "c2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "communities": [mock_community("rust", "public")],
            "next_cursor": null
        })))
        .mount(&server)
        .await;

    // Page 1
    Mock::given(method("GET"))
        .and(path("/communities"))
        .and(query_param("limit", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "communities": [mock_community("ai", "public")],
            "next_cursor": "c2"
        })))
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let page = client.communities().list().limit(1).send().await.unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].name, "ai");
    assert_eq!(page.next_cursor.as_deref(), Some("c2"));

    let stream = client.communities().list().limit(1).stream();
    let all: Vec<actos::Result<actos::CommunitySummary>> = stream.collect().await;
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].as_ref().unwrap().name, "ai");
    assert_eq!(all[1].as_ref().unwrap().name, "rust");
}

#[tokio::test]
async fn test_create_get_and_update_community() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/communities"))
        .and(header("authorization", "Bearer tok"))
        .and(body_json(serde_json::json!({
            "name": "rust",
            "description": "The rust community",
            "visibility": "private"
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(mock_community("rust", "private")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/communities/rust"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_community("rust", "public")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("PATCH"))
        .and(path("/communities/rust"))
        .and(header("authorization", "Bearer tok"))
        .and(body_json(serde_json::json!({
            "description": "A new description",
            "visibility": "private"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_community("rust", "private")))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok")
        .build()
        .unwrap();

    let created = client
        .communities()
        .create("rust", "The rust community")
        .visibility("private")
        .send()
        .await
        .unwrap();
    assert_eq!(created.visibility, "private");

    let fetched = client.communities().get("rust").await.unwrap();
    assert_eq!(fetched.name, "rust");
    assert_eq!(fetched.member_count, 3);

    let updated = client
        .communities()
        .update("rust")
        .description("A new description")
        .visibility("private")
        .send()
        .await
        .unwrap();
    assert_eq!(updated.visibility, "private");
}

#[tokio::test]
async fn test_join_leave_kick_and_close() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/communities/rust/join"))
        .and(header("authorization", "Bearer tok"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/communities/rust/join"))
        .and(header("authorization", "Bearer tok"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/communities/rust/members/bob"))
        .and(header("authorization", "Bearer tok"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/communities/rust/close"))
        .and(header("authorization", "Bearer tok"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok")
        .build()
        .unwrap();

    client.communities().join("rust").await.unwrap();
    client.communities().leave("rust").await.unwrap();
    client.communities().kick("rust", "bob").await.unwrap();
    client.communities().close("rust").await.unwrap();
}

#[tokio::test]
async fn test_members_list_and_stream() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/communities/rust/members"))
        .and(query_param("limit", "1"))
        .and(query_param("cursor", "c2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "members": [mock_member("carol")],
            "next_cursor": null
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/communities/rust/members"))
        .and(query_param("limit", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "members": [mock_member("bob")],
            "next_cursor": "c2"
        })))
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let page = client
        .communities()
        .members("rust")
        .limit(1)
        .send()
        .await
        .unwrap();
    assert_eq!(page.items[0].actor.username, "bob");

    let stream = client.communities().members("rust").limit(1).stream();
    let all: Vec<actos::Result<actos_types::community::CommunityMemberSummary>> =
        stream.collect().await;
    assert_eq!(all.len(), 2);
    assert_eq!(all[1].as_ref().unwrap().actor.username, "carol");
}

#[tokio::test]
async fn test_community_posts_list_stream_and_fields() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/communities/rust/posts"))
        .and(query_param("sort", "hot"))
        .and(query_param("fields", "id,title"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "posts": [mock_post("c_1", "Hot Post")],
            "next_cursor": null
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/communities/rust/posts"))
        .and(query_param("sort", "new"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "posts": [mock_post("c_2", "New Post")],
            "next_cursor": null
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let page = client
        .communities()
        .posts("rust")
        .sort(actos::Sort::Hot)
        .fields(["id", "title"])
        .send()
        .await
        .unwrap();
    assert_eq!(page.items[0].id, "c_1");
    assert_eq!(page.items[0].title.as_deref(), Some("Hot Post"));

    let stream = client
        .communities()
        .posts("rust")
        .sort(actos::Sort::New)
        .stream();
    let all: Vec<actos::Result<actos::Post>> = stream.collect().await;
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].as_ref().unwrap().id, "c_2");
    assert_eq!(
        all[0].as_ref().unwrap().community.as_ref().unwrap().name,
        "rust"
    );
}

#[tokio::test]
async fn test_successor_invitations_and_applications() {
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/communities/rust/successor"))
        .and(body_json(serde_json::json!({ "username": "bob" })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/communities/rust/invitations"))
        .and(body_json(serde_json::json!({ "username": "bob" })))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/me/invitations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "invitations": [mock_invitation("i_1", "rust")],
            "next_cursor": null
        })))
        .expect(2)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/me/invitations/i_1/accept"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/me/invitations/i_2/decline"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/communities/rust/applications"))
        .and(body_json(
            serde_json::json!({ "reason": "Please let me in" }),
        ))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/communities/rust/applications"))
        .and(query_param("status", "pending"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "applications": [mock_application("p_1", "rust", "pending")],
            "next_cursor": null
        })))
        .expect(2)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/communities/rust/applications/p_1/accept"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/communities/rust/applications/p_2/reject"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok")
        .build()
        .unwrap();

    client
        .communities()
        .set_successor("rust", "bob")
        .await
        .unwrap();
    client.communities().invite("rust", "bob").await.unwrap();

    let invitations = client.communities().invitations().send().await.unwrap();
    assert_eq!(invitations.items[0].id, "i_1");
    assert_eq!(invitations.items[0].community.name, "rust");

    let stream = client.communities().invitations().stream();
    let streamed: Vec<actos::Result<actos_types::community::InvitationSummary>> =
        stream.collect().await;
    assert_eq!(streamed.len(), 1);

    client.communities().accept_invitation("i_1").await.unwrap();
    client
        .communities()
        .decline_invitation("i_2")
        .await
        .unwrap();

    client
        .communities()
        .apply("rust", "Please let me in")
        .await
        .unwrap();

    let applications = client
        .communities()
        .applications("rust")
        .status("pending")
        .send()
        .await
        .unwrap();
    assert_eq!(applications.items[0].id, "p_1");
    assert_eq!(applications.items[0].status, "pending");

    let app_stream = client
        .communities()
        .applications("rust")
        .status("pending")
        .stream();
    let streamed_apps: Vec<actos::Result<actos_types::community::ApplicationSummary>> =
        app_stream.collect().await;
    assert_eq!(streamed_apps.len(), 1);

    client
        .communities()
        .accept_application("rust", "p_1")
        .await
        .unwrap();
    client
        .communities()
        .reject_application("rust", "p_2")
        .await
        .unwrap();
}
