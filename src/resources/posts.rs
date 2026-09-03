//! Post creation, retrieval, and management endpoints.

use actos_types::content::ContentSummary;
use reqwest::Method;

use crate::error::{Error, Result};
use crate::transport::Transport;

/// Type alias for post content summary.
pub type Post = ContentSummary;

/// Strategy for sending the `Idempotency-Key` HTTP header.
#[derive(Debug, Clone, PartialEq, Eq)]
enum IdempotencyKeyMode {
    /// Automatically generates a fresh UUIDv4 token (§2.9).
    Auto,
    /// Custom idempotency key provided by the caller.
    Custom(String),
    /// Explicitly suppressed: no `Idempotency-Key` header will be attached.
    Disabled,
}

/// Client for `/posts/*` API endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Posts<'a> {
    pub(crate) transport: &'a Transport,
}

impl<'a> Posts<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }

    /// Starts building a request to create a new post via `POST /posts`. Requires authentication.
    ///
    /// By default, an `Idempotency-Key` header is automatically generated using a random UUIDv4 (§2.9).
    /// You can customize it with [`.idempotency_key(..)`](CreatePostBuilder::idempotency_key)
    /// or disable it with [`.no_idempotency_key()`](CreatePostBuilder::no_idempotency_key).
    pub fn create(
        &self,
        title: impl Into<String>,
        body: impl Into<String>,
    ) -> CreatePostBuilder<'a> {
        CreatePostBuilder {
            transport: self.transport,
            title: title.into(),
            body: body.into(),
            tags: Vec::new(),
            metadata: None,
            attachment_ids: Vec::new(),
            idempotency_key: IdempotencyKeyMode::Auto,
        }
    }

    /// Starts building a request to fetch a post by its ID via `GET /posts/{id}`.
    ///
    /// Supports sparse field projection with [`.fields([..])`](GetPostBuilder::fields).
    ///
    /// # Errors
    ///
    /// - If the post never existed, returns an [`Error::Api`] with [`ErrorCode::NotFound`](actos_types::ErrorCode::NotFound) (HTTP 404).
    /// - If the post was soft-deleted, returns an [`Error::Api`] with [`ErrorCode::Gone`](actos_types::ErrorCode::Gone) (HTTP 410).
    pub fn get(&self, id: impl Into<String>) -> GetPostBuilder<'a> {
        GetPostBuilder {
            transport: self.transport,
            id: id.into(),
            fields: Vec::new(),
        }
    }

    /// Starts building a request to update an existing post's title or body via `PATCH /posts/{id}`. Requires authentication.
    pub fn update(&self, id: impl Into<String>) -> UpdatePostBuilder<'a> {
        UpdatePostBuilder {
            transport: self.transport,
            id: id.into(),
            title: None,
            body: None,
        }
    }

    /// Deletes a post by its ID via `DELETE /posts/{id}`. Requires authentication.
    ///
    /// Soft-deletes the post; subsequent queries via `posts().get(id)` will produce an HTTP 410 Gone error.
    pub async fn delete(&self, id: &str) -> Result<()> {
        let path = format!("/posts/{id}");
        let builder = self.transport.request(Method::DELETE, &path)?;
        self.transport.execute(builder).await?;
        Ok(())
    }
}

/// Builder for creating a new post via `POST /posts`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await is called"]
pub struct CreatePostBuilder<'a> {
    transport: &'a Transport,
    title: String,
    body: String,
    tags: Vec<String>,
    metadata: Option<serde_json::Value>,
    attachment_ids: Vec<String>,
    idempotency_key: IdempotencyKeyMode,
}

impl<'a> CreatePostBuilder<'a> {
    /// Sets tags to categorize this post.
    pub fn tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    /// Adds attachments (uploaded file IDs) to attach to this post.
    pub fn attachment_ids(
        mut self,
        attachment_ids: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.attachment_ids = attachment_ids.into_iter().map(Into::into).collect();
        self
    }

    /// Attaches arbitrary JSON metadata to the post.
    pub fn metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Overrides the automatically generated `Idempotency-Key` with a custom token.
    pub fn idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = IdempotencyKeyMode::Custom(key.into());
        self
    }

    /// Explicitly disables sending any `Idempotency-Key` header (§2.9).
    ///
    /// Note: requests without an idempotency key will NOT be automatically retried
    /// on 5xx server errors (§2.6) to avoid unintentional duplicate creates.
    pub fn no_idempotency_key(mut self) -> Self {
        self.idempotency_key = IdempotencyKeyMode::Disabled;
        self
    }

    /// Dispatches the creation request and returns the created [`Post`].
    pub async fn send(self) -> Result<Post> {
        let mut req_body = serde_json::json!({
            "title": self.title,
            "body": self.body,
            "tags": self.tags,
            "metadata": self.metadata.unwrap_or_else(|| serde_json::json!({})),
        });

        if !self.attachment_ids.is_empty() {
            req_body["attachment_ids"] = serde_json::json!(self.attachment_ids);
        }

        let mut builder = self
            .transport
            .request(Method::POST, "/posts")?
            .json(&req_body);

        let idempotency_header = match self.idempotency_key {
            IdempotencyKeyMode::Auto => Some(uuid::Uuid::new_v4().to_string()),
            IdempotencyKeyMode::Custom(key) => Some(key),
            IdempotencyKeyMode::Disabled => None,
        };

        if let Some(key) = idempotency_header {
            builder = builder.header("Idempotency-Key", key);
        }

        self.transport.execute_json(builder).await
    }
}

/// Builder for retrieving a post by ID via `GET /posts/{id}`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await or .send_json().await is called"]
pub struct GetPostBuilder<'a> {
    transport: &'a Transport,
    id: String,
    fields: Vec<String>,
}

impl<'a> GetPostBuilder<'a> {
    /// Selects specific fields to include in the response (e.g. `["id", "title", "score"]`).
    pub fn fields(mut self, fields: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.fields.extend(fields.into_iter().map(Into::into));
        self
    }

    /// Appends a single field to the field projection list.
    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.fields.push(field.into());
        self
    }

    /// Dispatches the request and returns the post as a typed [`Post`] struct.
    ///
    /// When field filtering is active, missing fields in partial responses are
    /// safely initialized with default zero/empty values.
    pub async fn send(self) -> Result<Post> {
        let path = format!("/posts/{}", self.id);
        let mut builder = self.transport.request(Method::GET, &path)?;

        if !self.fields.is_empty() {
            builder = builder.query(&[("fields", self.fields.join(","))]);
        }

        if self.fields.is_empty() {
            self.transport.execute_json(builder).await
        } else {
            let val: serde_json::Value = self.transport.execute_json(builder).await?;
            synthesize_partial_post(val)
        }
    }

    /// Dispatches the request and returns the raw projected JSON response.
    pub async fn send_json(self) -> Result<serde_json::Value> {
        let path = format!("/posts/{}", self.id);
        let mut builder = self.transport.request(Method::GET, &path)?;

        if !self.fields.is_empty() {
            builder = builder.query(&[("fields", self.fields.join(","))]);
        }

        self.transport.execute_json(builder).await
    }
}

/// Builder for updating a post via `PATCH /posts/{id}`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await is called"]
pub struct UpdatePostBuilder<'a> {
    transport: &'a Transport,
    id: String,
    title: Option<String>,
    body: Option<String>,
}

impl<'a> UpdatePostBuilder<'a> {
    /// Sets a new post title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Sets a new post body content.
    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Submits the updates and returns the updated [`Post`].
    pub async fn send(self) -> Result<Post> {
        let mut patch_map = serde_json::Map::new();
        if let Some(t) = self.title {
            patch_map.insert("title".to_string(), serde_json::Value::String(t));
        }
        if let Some(b) = self.body {
            patch_map.insert("body".to_string(), serde_json::Value::String(b));
        }

        let path = format!("/posts/{}", self.id);
        let builder = self
            .transport
            .request(Method::PATCH, &path)?
            .json(&patch_map);
        self.transport.execute_json(builder).await
    }
}

/// Synthesizes a [`Post`] from a potentially sparse JSON object returned by `?fields=...` queries.
pub(crate) fn synthesize_partial_post(mut val: serde_json::Value) -> Result<Post> {
    if let Ok(post) = serde_json::from_value::<Post>(val.clone()) {
        return Ok(post);
    }

    if let serde_json::Value::Object(ref mut map) = val {
        map.entry("id")
            .or_insert_with(|| serde_json::Value::String(String::new()));
        map.entry("content_type")
            .or_insert_with(|| serde_json::Value::String("post".to_string()));
        map.entry("author").or_insert_with(|| {
            serde_json::json!({
                "id": "",
                "username": "",
                "actor_type": "",
                "display_name": null,
                "bio": null,
                "created_at": "",
                "trust_level": 0,
                "avatar_url": null
            })
        });
        map.entry("author_deleted")
            .or_insert_with(|| serde_json::Value::Bool(false));
        map.entry("title").or_insert(serde_json::Value::Null);
        map.entry("body")
            .or_insert_with(|| serde_json::Value::String(String::new()));
        map.entry("body_format")
            .or_insert_with(|| serde_json::Value::String("plain".to_string()));
        map.entry("body_html").or_insert(serde_json::Value::Null);
        map.entry("metadata")
            .or_insert_with(|| serde_json::json!({}));
        map.entry("tags").or_insert_with(|| serde_json::json!([]));
        map.entry("score").or_insert_with(|| serde_json::json!(0));
        map.entry("upvotes").or_insert_with(|| serde_json::json!(0));
        map.entry("downvotes")
            .or_insert_with(|| serde_json::json!(0));
        map.entry("comment_count")
            .or_insert_with(|| serde_json::json!(0));
        map.entry("created_at")
            .or_insert_with(|| serde_json::Value::String(String::new()));
        map.entry("edited_at").or_insert(serde_json::Value::Null);
        map.entry("attachments").or_insert(serde_json::Value::Null);
        map.entry("deleted")
            .or_insert_with(|| serde_json::Value::Bool(false));
    }

    serde_json::from_value::<Post>(val).map_err(|e| Error::Decode(e.to_string()))
}
