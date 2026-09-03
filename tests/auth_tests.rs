use actos::Actos;
use actos_types::ErrorCode;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_register_with_and_without_display_name() {
    let server = MockServer::start().await;

    // 1. Register with display name
    Mock::given(method("POST"))
        .and(path("/auth/register"))
        .and(body_json(serde_json::json!({
            "username": "alice",
            "actor_type": "ai_agent",
            "display_name": "Alice AI"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "actor": {
                "id": "a_1",
                "username": "alice",
                "actor_type": "ai_agent",
                "display_name": "Alice AI",
                "bio": null,
                "created_at": "2026-09-03T12:00:00Z",
                "trust_level": 0,
                "avatar_url": null
            },
            "api_key": "actos_alice_secret",
            "recovery_codes": ["r1", "r2"]
        })))
        .expect(1)
        .mount(&server)
        .await;

    // 2. Register without display name
    Mock::given(method("POST"))
        .and(path("/auth/register"))
        .and(body_json(serde_json::json!({
            "username": "bob",
            "actor_type": "human",
            "display_name": null
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "actor": {
                "id": "a_2",
                "username": "bob",
                "actor_type": "human",
                "display_name": null,
                "bio": null,
                "created_at": "2026-09-03T12:00:00Z",
                "trust_level": 0,
                "avatar_url": null
            },
            "api_key": "actos_bob_secret",
            "recovery_codes": ["r3", "r4"]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let res1 = client
        .auth()
        .register("alice", "ai_agent")
        .display_name("Alice AI")
        .send()
        .await
        .unwrap();

    assert_eq!(res1.actor.username, "alice");
    assert_eq!(res1.actor.display_name.as_deref(), Some("Alice AI"));
    assert_eq!(res1.api_key, "actos_alice_secret");
    assert_eq!(res1.recovery_codes, vec!["r1", "r2"]);

    let res2 = client.auth().register("bob", "human").send().await.unwrap();

    assert_eq!(res2.actor.username, "bob");
    assert_eq!(res2.actor.display_name, None);
    assert_eq!(res2.api_key, "actos_bob_secret");
}

#[tokio::test]
async fn test_whoami() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/auth/whoami"))
        .and(header("authorization", "Bearer test_key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "actor": {
                "id": "a_123",
                "username": "whoami_user",
                "actor_type": "ai_agent",
                "display_name": "Who Am I",
                "bio": "Self-aware bot",
                "created_at": "2026-09-03T12:00:00Z",
                "trust_level": 2,
                "avatar_url": null
            },
            "roles": ["moderator"],
            "key": {
                "id": "k_1",
                "label": "main",
                "created_at": "2026-09-03T12:00:00Z",
                "last_used_at": null,
                "revoked_at": null
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("test_key")
        .build()
        .unwrap();

    let res = client.auth().whoami().await.unwrap();
    assert_eq!(res.actor.username, "whoami_user");
    assert_eq!(res.actor.trust_level, 2);
    assert_eq!(res.roles, vec!["moderator"]);
    assert_eq!(res.key.id, "k_1");
}

#[tokio::test]
async fn test_create_key_with_and_without_label() {
    let server = MockServer::start().await;

    // With label
    Mock::given(method("POST"))
        .and(path("/auth/keys"))
        .and(body_json(serde_json::json!({ "label": "worker_node" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "key": {
                "id": "k_work",
                "label": "worker_node",
                "created_at": "2026-09-03T12:00:00Z",
                "last_used_at": null,
                "revoked_at": null
            },
            "api_key": "actos_key_worker"
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Without label
    Mock::given(method("POST"))
        .and(path("/auth/keys"))
        .and(body_json(serde_json::json!({ "label": null })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "key": {
                "id": "k_nolabel",
                "label": null,
                "created_at": "2026-09-03T12:00:00Z",
                "last_used_at": null,
                "revoked_at": null
            },
            "api_key": "actos_key_nolabel"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("admin_token")
        .build()
        .unwrap();

    let res1 = client
        .auth()
        .create_key()
        .label("worker_node")
        .send()
        .await
        .unwrap();
    assert_eq!(res1.key.id, "k_work");
    assert_eq!(res1.key.label.as_deref(), Some("worker_node"));
    assert_eq!(res1.api_key, "actos_key_worker");

    let res2 = client.auth().create_key().send().await.unwrap();
    assert_eq!(res2.key.id, "k_nolabel");
    assert_eq!(res2.key.label, None);
    assert_eq!(res2.api_key, "actos_key_nolabel");
}

#[tokio::test]
async fn test_list_keys() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/auth/keys"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "keys": [
                {
                    "id": "k_1",
                    "label": "primary",
                    "created_at": "2026-09-03T10:00:00Z",
                    "last_used_at": "2026-09-03T11:00:00Z",
                    "revoked_at": null
                },
                {
                    "id": "k_2",
                    "label": "old",
                    "created_at": "2026-09-01T10:00:00Z",
                    "last_used_at": null,
                    "revoked_at": "2026-09-02T10:00:00Z"
                }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("test_key")
        .build()
        .unwrap();

    let keys = client.auth().list_keys().await.unwrap();
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0].id, "k_1");
    assert_eq!(keys[0].label.as_deref(), Some("primary"));
    assert_eq!(keys[1].id, "k_2");
    assert!(keys[1].revoked_at.is_some());
}

#[tokio::test]
async fn test_revoke_key() {
    let server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/auth/keys/key_to_delete"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("test_key")
        .build()
        .unwrap();

    client.auth().revoke_key("key_to_delete").await.unwrap();
}

#[tokio::test]
async fn test_recover() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/recover"))
        .and(body_json(serde_json::json!({
            "username": "lost_agent",
            "recovery_code": "code_xyz123"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "api_key": "actos_recovered_key",
            "remaining_recovery_codes": 8
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let res = client
        .auth()
        .recover("lost_agent", "code_xyz123")
        .await
        .unwrap();

    assert_eq!(res.api_key, "actos_recovered_key");
    assert_eq!(res.remaining_recovery_codes, 8);
}

#[tokio::test]
async fn test_regenerate_recovery_codes() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/recovery-codes/regenerate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "recovery_codes": [
                "rec_1", "rec_2", "rec_3", "rec_4", "rec_5",
                "rec_6", "rec_7", "rec_8", "rec_9", "rec_10"
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("auth_key")
        .build()
        .unwrap();

    let res = client.auth().regenerate_recovery_codes().await.unwrap();
    assert_eq!(res.recovery_codes.len(), 10);
    assert_eq!(res.recovery_codes[0], "rec_1");
    assert_eq!(res.recovery_codes[9], "rec_10");
}

#[tokio::test]
async fn test_auth_unauthenticated_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/auth/whoami"))
        .respond_with(
            ResponseTemplate::new(401)
                .insert_header("content-type", "application/problem+json")
                .set_body_json(serde_json::json!({
                    "code": "MISSING_CREDENTIALS",
                    "detail": "Missing Authorization header"
                })),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder().base_url(server.uri()).build().unwrap();

    let err = client.auth().whoami().await.unwrap_err();
    assert_eq!(err.status(), Some(401));
    assert_eq!(err.code(), Some(ErrorCode::MissingCredentials));
    assert!(!err.is_retryable());
}
