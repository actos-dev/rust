use actos::Actos;
use actos_types::ErrorCode;
use futures_util::StreamExt;
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn mock_report(id: &str, target_id: &str, status: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "target_type": "post",
        "target_id": target_id,
        "reason": "Spam content",
        "status": status,
        "notes": null,
        "created_at": "2026-09-03T12:00:00Z",
        "resolved_at": null
    })
}

fn mock_action(id: &str, admin_username: &str, action_type: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "admin_username": admin_username,
        "action_type": action_type,
        "target_type": "post",
        "target_id": 12345,
        "reason": "Rule violation",
        "created_at": "2026-09-03T12:00:00Z"
    })
}

#[tokio::test]
async fn test_reports_create() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/reports"))
        .and(body_json(serde_json::json!({
            "target_type": "post",
            "target_id": "c_post1",
            "reason": "Inappropriate content"
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_report("rep_1", "c_post1", "pending")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok_user")
        .build()
        .unwrap();

    let report = client
        .reports()
        .create("post", "c_post1", "Inappropriate content")
        .await
        .unwrap();

    assert_eq!(report.id, "rep_1");
    assert_eq!(report.target_id, "c_post1");
    assert_eq!(report.status, "pending");
}

#[tokio::test]
async fn test_admin_reports_list_and_stream() {
    let server = MockServer::start().await;

    // Page 2 (mounted first to match before generic mock)
    Mock::given(method("GET"))
        .and(path("/admin/reports"))
        .and(query_param("status", "pending"))
        .and(query_param("limit", "1"))
        .and(query_param("cursor", "cur_rep2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "reports": [mock_report("rep_2", "c_post2", "pending")],
            "next_cursor": null
        })))
        .mount(&server)
        .await;

    // Page 1
    Mock::given(method("GET"))
        .and(path("/admin/reports"))
        .and(query_param("status", "pending"))
        .and(query_param("limit", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "reports": [mock_report("rep_1", "c_post1", "pending")],
            "next_cursor": "cur_rep2"
        })))
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok_mod")
        .build()
        .unwrap();

    let page1 = client
        .admin()
        .reports()
        .list()
        .status("pending")
        .limit(1)
        .send()
        .await
        .unwrap();
    assert_eq!(page1.items.len(), 1);
    assert_eq!(page1.items[0].id, "rep_1");
    assert_eq!(page1.next_cursor.as_deref(), Some("cur_rep2"));

    let stream = client
        .admin()
        .reports()
        .list()
        .status("pending")
        .limit(1)
        .stream();
    let all_reports: Vec<actos::Result<actos::ReportSummary>> = stream.collect().await;
    assert_eq!(all_reports.len(), 2);
    assert_eq!(all_reports[0].as_ref().unwrap().id, "rep_1");
    assert_eq!(all_reports[1].as_ref().unwrap().id, "rep_2");
}

#[tokio::test]
async fn test_admin_reports_update() {
    let server = MockServer::start().await;

    let mut resolved_report = mock_report("rep_1", "c_post1", "resolved");
    resolved_report["notes"] = serde_json::Value::String("Resolved by moderator".to_string());

    Mock::given(method("PATCH"))
        .and(path("/admin/reports/rep_1"))
        .and(body_json(serde_json::json!({
            "status": "resolved",
            "notes": "Resolved by moderator"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(resolved_report))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok_mod")
        .build()
        .unwrap();

    let updated = client
        .admin()
        .reports()
        .update("rep_1", "resolved")
        .notes("Resolved by moderator")
        .send()
        .await
        .unwrap();

    assert_eq!(updated.id, "rep_1");
    assert_eq!(updated.status, "resolved");
    assert_eq!(updated.notes.as_deref(), Some("Resolved by moderator"));
}

#[tokio::test]
async fn test_admin_contents_delete() {
    let server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/admin/contents/c_spam"))
        .and(body_json(serde_json::json!({
            "reason": "Violated terms of service"
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok_mod")
        .build()
        .unwrap();

    client
        .admin()
        .contents()
        .delete("c_spam", "Violated terms of service")
        .await
        .unwrap();
}

#[tokio::test]
async fn test_admin_bans_create_and_remove() {
    let server = MockServer::start().await;

    // Create ban
    Mock::given(method("POST"))
        .and(path("/admin/bans"))
        .and(body_json(serde_json::json!({
            "username": "spammer",
            "reason": "Repeated spamming",
            "expires_at": "2026-10-01T00:00:00Z"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "username": "spammer",
            "reason": "Repeated spamming",
            "banned_at": "2026-09-03T12:00:00Z",
            "expires_at": "2026-10-01T00:00:00Z"
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Remove ban
    Mock::given(method("DELETE"))
        .and(path("/admin/bans/spammer"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok_mod")
        .build()
        .unwrap();

    let ban = client
        .admin()
        .bans()
        .create("spammer", "Repeated spamming")
        .expires_at("2026-10-01T00:00:00Z")
        .send()
        .await
        .unwrap();

    assert_eq!(ban.username, "spammer");
    assert_eq!(ban.expires_at.as_deref(), Some("2026-10-01T00:00:00Z"));

    client.admin().bans().remove("spammer").await.unwrap();
}

#[tokio::test]
async fn test_admin_roles_set() {
    let server = MockServer::start().await;

    // Set moderator
    Mock::given(method("POST"))
        .and(path("/admin/roles"))
        .and(body_json(serde_json::json!({
            "username": "trusted_user",
            "role": "moderator"
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    // Revoke role
    Mock::given(method("POST"))
        .and(path("/admin/roles"))
        .and(body_json(serde_json::json!({
            "username": "demoted_user",
            "role": null
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok_admin")
        .build()
        .unwrap();

    client
        .admin()
        .roles()
        .set("trusted_user", Some("moderator"))
        .await
        .unwrap();

    client
        .admin()
        .roles()
        .set("demoted_user", None)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_admin_actions_list_and_stream() {
    let server = MockServer::start().await;

    // Page 2
    Mock::given(method("GET"))
        .and(path("/admin/actions"))
        .and(query_param("limit", "1"))
        .and(query_param("cursor", "cur_act2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "actions": [mock_action("act_2", "admin_bob", "ban_create")],
            "next_cursor": null
        })))
        .mount(&server)
        .await;

    // Page 1
    Mock::given(method("GET"))
        .and(path("/admin/actions"))
        .and(query_param("limit", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "actions": [mock_action("act_1", "admin_alice", "content_delete")],
            "next_cursor": "cur_act2"
        })))
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok_mod")
        .build()
        .unwrap();

    let page1 = client
        .admin()
        .actions()
        .list()
        .limit(1)
        .send()
        .await
        .unwrap();
    assert_eq!(page1.items.len(), 1);
    assert_eq!(page1.items[0].id, "act_1");

    let stream = client.admin().actions().list().limit(1).stream();
    let all_actions: Vec<actos::Result<actos::AdminActionSummary>> = stream.collect().await;
    assert_eq!(all_actions.len(), 2);
    assert_eq!(all_actions[0].as_ref().unwrap().id, "act_1");
    assert_eq!(all_actions[1].as_ref().unwrap().id, "act_2");
}

#[tokio::test]
async fn test_admin_forbidden_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/admin/reports"))
        .and(header("authorization", "Bearer tok_regular_user"))
        .respond_with(
            ResponseTemplate::new(403)
                .insert_header("content-type", "application/problem+json")
                .set_body_json(serde_json::json!({
                    "code": "FORBIDDEN",
                    "detail": "Moderator privileges required"
                })),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok_regular_user")
        .build()
        .unwrap();

    let err = client.admin().reports().list().send().await.unwrap_err();
    assert!(err.is_forbidden());
    assert_eq!(err.code(), Some(ErrorCode::Forbidden));
    assert_eq!(err.status(), Some(403));
}
