use actos::{Actos, UploadSource};
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Match, Mock, MockServer, Request, ResponseTemplate};

struct MultipartFileMatcher;

impl Match for MultipartFileMatcher {
    fn matches(&self, request: &Request) -> bool {
        let content_type = request
            .headers
            .get(&wiremock::http::HeaderName::from_static("content-type"));
        let is_multipart = content_type
            .and_then(|v| v.to_str().ok())
            .map_or(false, |s| s.starts_with("multipart/form-data"));
        let body_str = String::from_utf8_lossy(&request.body);
        is_multipart && body_str.contains("name=\"file\"")
    }
}

fn mock_upload(id: &str, url: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "url": url,
        "thumbnail_url": format!("{url}_thumb.webp"),
        "mime_type": "image/webp",
        "byte_size": 1024,
        "width": 800,
        "height": 600,
        "checksum_sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "created_at": "2026-09-03T12:00:00Z"
    })
}

#[tokio::test]
async fn test_upload_bytes() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/uploads"))
        .and(MultipartFileMatcher)
        .and(header("authorization", "Bearer tok_123"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_upload("u_bytes", "https://cdn.actos.dev/u_bytes.webp")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok_123")
        .build()
        .unwrap();

    let bytes = b"sample byte content for image".to_vec();
    let upload = client
        .uploads()
        .create(bytes)
        .filename("avatar.png")
        .mime_type("image/png")
        .send()
        .await
        .unwrap();

    assert_eq!(upload.id, "u_bytes");
    assert_eq!(upload.mime_type, "image/webp");
}

#[tokio::test]
async fn test_upload_file_path() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/uploads"))
        .and(MultipartFileMatcher)
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_upload("u_file", "https://cdn.actos.dev/u_file.webp")),
        )
        .expect(1)
        .mount(&server)
        .await;

    // Create temporary file on disk
    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join(format!("test_upload_{}.png", uuid::Uuid::new_v4()));
    tokio::fs::write(&temp_path, b"file bytes on disk")
        .await
        .unwrap();

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok_123")
        .build()
        .unwrap();

    let upload = client.uploads().create(&temp_path).send().await.unwrap();

    assert_eq!(upload.id, "u_file");

    // Clean up temp file
    let _ = tokio::fs::remove_file(&temp_path).await;
}

#[tokio::test]
async fn test_upload_stream() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/uploads"))
        .and(MultipartFileMatcher)
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_upload(
            "u_stream",
            "https://cdn.actos.dev/u_stream.webp",
        )))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok_123")
        .build()
        .unwrap();

    let cursor = std::io::Cursor::new(vec![1, 2, 3, 4, 5, 6, 7, 8]);
    let source = UploadSource::from_stream(cursor, Some(8));

    let upload = client
        .uploads()
        .create(source)
        .filename("streamed_data.bin")
        .send()
        .await
        .unwrap();

    assert_eq!(upload.id, "u_stream");
}

#[tokio::test]
async fn test_upload_delete() {
    let server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/uploads/u_delete_me"))
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

    client.uploads().delete("u_delete_me").await.unwrap();
}

#[tokio::test]
async fn test_upload_and_attach_to_post() {
    let server = MockServer::start().await;

    // 1. Upload
    Mock::given(method("POST"))
        .and(path("/uploads"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_upload(
            "u_attach1",
            "https://cdn.actos.dev/u_attach1.webp",
        )))
        .expect(1)
        .mount(&server)
        .await;

    // 2. Post creation with attachment_ids: ["u_attach1"]
    Mock::given(method("POST"))
        .and(path("/posts"))
        .and(body_json(serde_json::json!({
            "title": "Post With Attachment",
            "body": "See attached image",
            "tags": [],
            "metadata": {},
            "attachment_ids": ["u_attach1"]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "c_post_att",
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
            "title": "Post With Attachment",
            "body": "See attached image",
            "body_format": "markdown",
            "body_html": null,
            "metadata": {},
            "tags": [],
            "score": 1,
            "upvotes": 1,
            "downvotes": 0,
            "comment_count": 0,
            "created_at": "2026-09-03T12:00:00Z",
            "edited_at": null,
            "attachments": [
                mock_upload("u_attach1", "https://cdn.actos.dev/u_attach1.webp")
            ],
            "deleted": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = Actos::builder()
        .base_url(server.uri())
        .api_key("tok_123")
        .build()
        .unwrap();

    // 1. Upload file
    let upload = client
        .uploads()
        .create(b"image binary payload".to_vec())
        .filename("chart.png")
        .send()
        .await
        .unwrap();
    assert_eq!(upload.id, "u_attach1");

    // 2. Attach to post
    let post = client
        .posts()
        .create("Post With Attachment", "See attached image")
        .attachment_ids([upload.id])
        .send()
        .await
        .unwrap();

    assert_eq!(post.id, "c_post_att");
    assert!(post.attachments.is_some());
    assert_eq!(post.attachments.as_ref().unwrap().len(), 1);
    assert_eq!(post.attachments.as_ref().unwrap()[0].id, "u_attach1");
}
