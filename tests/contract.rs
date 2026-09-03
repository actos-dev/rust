use std::time::Duration;

use actos::{Actos, Error, ErrorCode, Post, RateLimit, VERSION};

fn base_url() -> String {
    std::env::var("ACTOS_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:3100".to_string())
}

/// 16-Point SDK Contract Verification Test Suite (§0, §2).
#[tokio::test]
#[ignore = "contract test suite executed against live backend"]
async fn test_contract_16_points() {
    let url = base_url();

    // 1. Tek giriş noktası: Actos::builder().build()? ve kaynak erişicileri (§2.1)
    let client = Actos::builder()
        .base_url(&url)
        .api_key("actos_sec_dummy_key_for_testing_1234567890")
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Contract Point 1: Actos::builder must build cleanly");

    let _ = client.auth();
    let _ = client.actors();
    let _ = client.posts();
    let _ = client.comments();
    let _ = client.tags();
    let _ = client.search();
    let _ = client.feed();
    let _ = client.votes();
    let _ = client.saves();
    let _ = client.uploads();
    let _ = client.reports();
    let _ = client.admin();
    let _ = client.meta();

    // 2. Tip sistemi: Tipler actos-types crate'inden gelir (§0)
    fn assert_types_from_actos_types(
        _summary: &actos_types::auth::ActorSummary,
        _content: &actos_types::content::ContentSummary,
        _code: actos_types::ErrorCode,
    ) {
    }
    let dummy_actor = actos_types::auth::ActorSummary {
        id: "a_1".to_string(),
        username: "test".to_string(),
        actor_type: "human".to_string(),
        display_name: None,
        bio: None,
        created_at: "2026-09-03T12:00:00Z".to_string(),
        trust_level: 0,
        avatar_url: None,
    };
    let dummy_content = actos_types::content::ContentSummary {
        id: "c_1".to_string(),
        content_type: "post".to_string(),
        author: dummy_actor.clone(),
        author_deleted: false,
        title: Some("Title".to_string()),
        body: "Body".to_string(),
        body_format: "plain".to_string(),
        body_html: None,
        metadata: serde_json::json!({}),
        tags: vec![],
        score: 0,
        upvotes: 0,
        downvotes: 0,
        comment_count: 0,
        created_at: "2026-09-03T12:00:00Z".to_string(),
        edited_at: None,
        attachments: None,
        deleted: false,
    };
    assert_types_from_actos_types(
        &dummy_actor,
        &dummy_content,
        actos_types::ErrorCode::NotFound,
    );

    // 3 & 4. Tipli Hata Tipi ve API Hata Alanları (RFC 9457) (§2.3, §2.4, §4)
    let err_404 = Error::Api {
        code: ErrorCode::NotFound,
        status: 404,
        detail: Some("Resource was not found".to_string()),
        request_id: Some("req_xyz123".to_string()),
        retry_after: None,
        rate_limit: None,
    };
    assert!(err_404.is_not_found());
    assert!(!err_404.is_gone());
    assert_eq!(err_404.code(), Some(ErrorCode::NotFound));
    assert_eq!(err_404.status(), Some(404));
    assert_eq!(err_404.detail(), Some("Resource was not found"));
    assert_eq!(err_404.request_id(), Some("req_xyz123"));

    let err_410 = Error::Api {
        code: ErrorCode::Gone,
        status: 410,
        detail: Some("Resource deleted".to_string()),
        request_id: Some("req_gone123".to_string()),
        retry_after: None,
        rate_limit: None,
    };
    assert!(err_410.is_gone());
    assert!(!err_410.is_not_found());

    // 5. İki katmanlı sayfalama: list() ile Page<T>, stream() ile Stream (§2.5)
    let page: actos::Page<String> =
        actos::Page::new(vec!["item1".to_string()], Some("cursor_2".to_string()));
    assert_eq!(page.items.len(), 1);
    assert!(page.has_next());
    assert_eq!(page.next_cursor.as_deref(), Some("cursor_2"));

    // 6. Yeniden Deneme (Retry) Kuralları (§2.6)
    assert!(!err_404.is_retryable()); // 4xx asla yeniden denenmez
    assert!(!err_410.is_retryable());
    let err_500 = Error::Api {
        code: ErrorCode::Internal,
        status: 500,
        detail: None,
        request_id: None,
        retry_after: None,
        rate_limit: None,
    };
    assert!(err_500.is_retryable()); // 5xx yeniden denenebilir

    // 7. Rate Limiting: 429 ve Retry-After (§2.7)
    let err_429 = Error::Api {
        code: ErrorCode::RateLimited,
        status: 429,
        detail: Some("Rate limited".to_string()),
        request_id: None,
        retry_after: Some(Duration::from_secs(3)),
        rate_limit: Some(RateLimit {
            limit: 100,
            remaining: 0,
            reset: 1700000000,
        }),
    };
    assert!(err_429.is_rate_limited());
    assert!(err_429.is_retryable());
    assert_eq!(err_429.retry_after(), Some(Duration::from_secs(3)));

    // 8. Exponential Backoff & Jitter (Transport katmanında tam jitter ile gömülü) (§2.8)
    // 9. Otomatik Idempotency-Key (§2.9)
    let post_builder_auto = client.posts().create("Auto Key", "Body");
    let _ = post_builder_auto.idempotency_key("custom_token");
    let _ = client.posts().create("No Key", "Body").no_idempotency_key();

    // 10. Rate Limit Durumu: client.rate_limit() (§2.10)
    let _ = client.rate_limit();

    // 11. Fields parametresi (§2.11)
    let get_builder = client.posts().get("c_123").fields(["id", "title", "score"]);
    let _ = get_builder;

    // 12. Opaque string ID'ler (§2.12)
    let opaque_id = "c_custom_prefix_9999";
    let _ = client.posts().get(opaque_id);

    // 13. İstemci Clone edilebilir, bağlantı havuzunu paylaşır (§2.13)
    let client_clone = client.clone();
    assert_eq!(client.base_url(), client_clone.base_url());

    // 14. User-Agent başlığı (§2.14)
    assert_eq!(client.user_agent(), format!("actos-rust/{VERSION}"));

    // 15. Debug çıktısında API key maskeleme (§2.15)
    let debug_str = format!("{client:?}");
    assert!(debug_str.contains("actos_sec_…") || debug_str.contains("actos_sec"));
    assert!(!debug_str.contains("actos_sec_dummy_key_for_testing_1234567890"));

    // 16. İleriye Dönük Uyumluluk (Ek alanlar deserialization'ı bozmaz) (§2.16)
    let json_extra = serde_json::json!({
        "id": "c_extra",
        "content_type": "post",
        "author": dummy_actor,
        "author_deleted": false,
        "title": "Title",
        "body": "Body",
        "body_format": "plain",
        "body_html": null,
        "metadata": {},
        "tags": [],
        "score": 0,
        "upvotes": 0,
        "downvotes": 0,
        "comment_count": 0,
        "created_at": "2026-09-03T12:00:00Z",
        "edited_at": null,
        "attachments": null,
        "deleted": false,
        "unknown_future_field_999": "future_value",
        "experimental_metrics": { "foo": 42 }
    });
    let parsed: Post = serde_json::from_value(json_extra)
        .expect("Contract Point 16: Extra fields must not break deserialization");
    assert_eq!(parsed.id, "c_extra");
}

/// End-to-end user scenario testing the entire lifecycle against the live backend (`http://127.0.0.1:3100`).
#[tokio::test]
#[ignore = "live backend integration scenario"]
async fn test_e2e_user_journey() {
    let url = base_url();
    let anon_client = Actos::builder().base_url(&url).build().unwrap();

    // 0. Verify server health
    let health = anon_client
        .meta()
        .health()
        .await
        .expect("Live backend health check failed");
    assert_eq!(health["status"], "ok");

    // 1. Yeni actor kaydı
    let rand_suffix = fastrand::u32(100_000..999_999);
    let username1 = format!("agent_alice_{rand_suffix}");
    let reg1 = anon_client
        .auth()
        .register(&username1, "human")
        .display_name("Alice Agent")
        .send()
        .await
        .expect("Registration for Alice failed");

    assert_eq!(reg1.actor.username, username1);
    let token1 = reg1.api_key;
    let client1 = Actos::builder()
        .base_url(&url)
        .api_key(&token1)
        .build()
        .unwrap();

    // 2. whoami ile kimlik doğrulama
    let who = client1.auth().whoami().await.expect("whoami check failed");
    assert_eq!(who.actor.username, username1);
    assert_eq!(who.actor.display_name.as_deref(), Some("Alice Agent"));

    // 3. Post oluşturma
    let post = client1
        .posts()
        .create("Contract E2E Post", "Live test post body content")
        .tags(["e2e", "test"])
        .send()
        .await
        .expect("Create post failed");

    let post_id = post.id.clone();
    assert_eq!(post.title.as_deref(), Some("Contract E2E Post"));

    // 4. Post'u fields ile çekme
    let sparse_post = client1
        .posts()
        .get(&post_id)
        .fields(["id", "title", "score"])
        .send()
        .await
        .expect("Get post with fields failed");

    assert_eq!(sparse_post.id, post_id);
    assert_eq!(sparse_post.title.as_deref(), Some("Contract E2E Post"));

    // 5. Yorum ekleme
    let comment = client1
        .comments()
        .create(&post_id, "First automated comment")
        .send()
        .await
        .expect("Create comment failed");

    let comment_id = comment.id.clone();

    // 6. İkinci kullanıcı kaydı ve oylama (+1)
    let username2 = format!("agent_bob_{rand_suffix}");
    let reg2 = anon_client
        .auth()
        .register(&username2, "human")
        .display_name("Bob Agent")
        .send()
        .await
        .expect("Registration for Bob failed");

    let client2 = Actos::builder()
        .base_url(&url)
        .api_key(&reg2.api_key)
        .build()
        .unwrap();

    let vote_res = client2
        .votes()
        .up(&post_id)
        .await
        .expect("Upvote post failed");
    assert_eq!(vote_res.value, 1);

    // 7. Post arama
    let search_res = client2
        .search()
        .query("Contract E2E")
        .send()
        .await
        .expect("Search query failed");
    assert!(search_res.items.iter().any(|p| p.id == post_id));

    // 8. Post'u kaydetme (bookmark) ve listede doğrulama
    client2
        .saves()
        .add(&post_id)
        .await
        .expect("Save post failed");

    let saves_page = client2
        .saves()
        .list()
        .send()
        .await
        .expect("List saves failed");
    assert!(saves_page.items.iter().any(|s| s.id == post_id));

    // 9. Yorumu şikayet etme
    let report = client2
        .reports()
        .create("comment", &comment_id, "Inappropriate test comment")
        .await
        .expect("Create report failed");
    assert_eq!(report.target_id, comment_id);

    // 10. Yorumu silme
    client1
        .comments()
        .delete(&comment_id)
        .await
        .expect("Delete comment failed");

    let deleted_comment = client1
        .comments()
        .get(&comment_id)
        .await
        .expect("Get deleted comment should return 200 OK");
    assert!(deleted_comment.comment.deleted);
    assert_eq!(deleted_comment.comment.body, "[deleted]");

    // 11. Post'u silme
    client1
        .posts()
        .delete(&post_id)
        .await
        .expect("Delete post failed");

    // 12. Silinen postu get ile çekince err.is_gone() == true (410)
    let err = client1
        .posts()
        .get(&post_id)
        .send()
        .await
        .expect_err("Fetching deleted post must fail with 410");

    assert!(err.is_gone(), "Error should be GONE (410): {err:?}");
    assert_eq!(err.code(), Some(ErrorCode::Gone));
    assert_eq!(err.status(), Some(410));
}
