//! Threaded comments endpoints.

use actos_types::content::{
    CommentDetailResponse, CommentNodeResponse, CommentThreadResponse, ContentSummary,
};
use reqwest::Method;

use crate::error::{Error, Result};
use crate::pagination::{Page, paginate_stream_with_cursor};
use crate::transport::Transport;

/// Strategy for sending the `Idempotency-Key` HTTP header.
#[derive(Debug, Clone, PartialEq, Eq)]
enum IdempotencyKeyMode {
    Auto,
    Custom(String),
    Disabled,
}

/// Client for comment endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Comments<'a> {
    pub(crate) transport: &'a Transport,
}

impl<'a> Comments<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }

    /// Starts building a request to post a comment on a post via `POST /posts/{id}/comments`. Requires authentication.
    ///
    /// Supports threaded / nested comments via [`.parent_id(..)`](CreateCommentBuilder::parent_id).
    /// The maximum reply tree depth (32) is enforced by the server; the SDK forwards any server validation error.
    pub fn create(
        &self,
        post_id: impl Into<String>,
        body: impl Into<String>,
    ) -> CreateCommentBuilder<'a> {
        CreateCommentBuilder {
            transport: self.transport,
            post_id: post_id.into(),
            body: body.into(),
            parent_id: None,
            attachment_ids: Vec::new(),
            idempotency_key: IdempotencyKeyMode::Auto,
        }
    }

    /// Starts building a query to list comments on a post via `GET /posts/{id}/comments`.
    pub fn list(&self, post_id: impl Into<String>) -> ListCommentsBuilder<'a> {
        ListCommentsBuilder {
            transport: self.transport,
            post_id: post_id.into(),
            sort: None,
            depth: None,
            parent: None,
            limit: None,
            cursor: None,
        }
    }

    /// Convenience shortcut to stream top-level comments for a post across pages.
    pub fn stream(
        &self,
        post_id: impl Into<String>,
    ) -> impl futures_core::Stream<Item = Result<CommentNodeResponse, Error>> + Send {
        self.list(post_id).stream()
    }

    /// Retrieves a comment and its thread ancestor breadcrumb chain via `GET /comments/{id}`.
    ///
    /// # Soft-Deleted Comments
    ///
    /// When a comment is soft-deleted, this endpoint still returns `200 OK` with `comment.deleted = true`
    /// and `comment.body = "[silindi]"` (instead of an HTTP 410 Gone error) so that existing nested
    /// replies remain intact and navigable in the thread tree.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::NotFound`](actos_types::ErrorCode::NotFound) (HTTP 404) if the comment ID does not exist.
    pub async fn get(&self, id: &str) -> Result<CommentDetailResponse> {
        let path = format!("/comments/{id}");
        let builder = self.transport.request(Method::GET, &path)?;
        self.transport.execute_json(builder).await
    }

    /// Updates the body text of a comment via `PATCH /comments/{id}`. Requires authentication.
    pub async fn update(&self, id: &str, body: impl Into<String>) -> Result<ContentSummary> {
        let path = format!("/comments/{id}");
        let req_body = serde_json::json!({
            "body": body.into(),
        });
        let builder = self
            .transport
            .request(Method::PATCH, &path)?
            .json(&req_body);
        self.transport.execute_json(builder).await
    }

    /// Soft-deletes a comment via `DELETE /comments/{id}`. Requires authentication.
    ///
    /// Expects 204 No Content. Child replies remain accessible under the placeholder text.
    pub async fn delete(&self, id: &str) -> Result<()> {
        let path = format!("/comments/{id}");
        let builder = self.transport.request(Method::DELETE, &path)?;
        self.transport.execute(builder).await?;
        Ok(())
    }
}

/// Builder for creating a comment via `POST /posts/{id}/comments`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await is called"]
pub struct CreateCommentBuilder<'a> {
    transport: &'a Transport,
    post_id: String,
    body: String,
    parent_id: Option<String>,
    attachment_ids: Vec<String>,
    idempotency_key: IdempotencyKeyMode,
}

impl<'a> CreateCommentBuilder<'a> {
    /// Sets the parent comment ID to create a nested reply.
    pub fn parent_id(mut self, parent_id: impl Into<String>) -> Self {
        self.parent_id = Some(parent_id.into());
        self
    }

    /// Adds media attachment IDs to the comment.
    pub fn attachment_ids(
        mut self,
        attachment_ids: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.attachment_ids = attachment_ids.into_iter().map(Into::into).collect();
        self
    }

    /// Overrides the auto-generated `Idempotency-Key` header with a custom token.
    pub fn idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = IdempotencyKeyMode::Custom(key.into());
        self
    }

    /// Explicitly disables sending any `Idempotency-Key` header (§2.9).
    pub fn no_idempotency_key(mut self) -> Self {
        self.idempotency_key = IdempotencyKeyMode::Disabled;
        self
    }

    /// Sends the request to create the comment and returns the created [`ContentSummary`].
    pub async fn send(self) -> Result<ContentSummary> {
        let path = format!("/posts/{}/comments", self.post_id);
        let mut req_body = serde_json::json!({
            "body": self.body,
        });

        if let Some(parent) = self.parent_id {
            req_body["parent_id"] = serde_json::Value::String(parent);
        }

        if !self.attachment_ids.is_empty() {
            req_body["attachment_ids"] = serde_json::json!(self.attachment_ids);
        }

        let mut builder = self.transport.request(Method::POST, &path)?.json(&req_body);

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

/// Builder for querying comments on a post via `GET /posts/{id}/comments`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await or .stream() is called"]
pub struct ListCommentsBuilder<'a> {
    transport: &'a Transport,
    post_id: String,
    sort: Option<String>,
    depth: Option<u32>,
    parent: Option<String>,
    limit: Option<u32>,
    cursor: Option<String>,
}

impl<'a> ListCommentsBuilder<'a> {
    /// Sets the sorting order (e.g. `"top"`, `"new"`).
    pub fn sort(mut self, sort: impl Into<String>) -> Self {
        self.sort = Some(sort.into());
        self
    }

    /// Sets the tree nesting depth to fetch in replies.
    pub fn depth(mut self, depth: u32) -> Self {
        self.depth = Some(depth);
        self
    }

    /// Fetches replies rooted at a specific parent comment ID.
    pub fn parent(mut self, parent: impl Into<String>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    /// Limits the maximum number of top-level comments per page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the pagination cursor for fetching subsequent pages.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Dispatches the request and returns a single page of [`CommentNodeResponse`] tree nodes.
    pub async fn send(self) -> Result<Page<CommentNodeResponse>> {
        let path = format!("/posts/{}/comments", self.post_id);
        let mut builder = self.transport.request(Method::GET, &path)?;

        if let Some(ref s) = self.sort {
            builder = builder.query(&[("sort", s.as_str())]);
        }
        if let Some(d) = self.depth {
            builder = builder.query(&[("depth", d.to_string())]);
        }
        if let Some(ref p) = self.parent {
            builder = builder.query(&[("parent", p.as_str())]);
        }
        if let Some(lim) = self.limit {
            builder = builder.query(&[("limit", lim.to_string())]);
        }
        if let Some(ref c) = self.cursor {
            builder = builder.query(&[("cursor", c.as_str())]);
        }

        let res: CommentThreadResponse = self.transport.execute_json(builder).await?;
        Ok(Page::new(res.comments, res.next_cursor))
    }

    /// Produces a stream that yields top-level [`CommentNodeResponse`] nodes across pages.
    pub fn stream(
        self,
    ) -> impl futures_core::Stream<Item = Result<CommentNodeResponse, Error>> + Send {
        let transport = self.transport.clone();
        let post_id = self.post_id;
        let sort = self.sort;
        let depth = self.depth;
        let parent = self.parent;
        let limit = self.limit;

        paginate_stream_with_cursor(self.cursor, move |cursor| {
            let transport = transport.clone();
            let path = format!("/posts/{post_id}/comments");
            let sort = sort.clone();
            let parent = parent.clone();
            async move {
                let mut builder = transport.request(Method::GET, &path)?;
                if let Some(ref s) = sort {
                    builder = builder.query(&[("sort", s.as_str())]);
                }
                if let Some(d) = depth {
                    builder = builder.query(&[("depth", d.to_string())]);
                }
                if let Some(ref p) = parent {
                    builder = builder.query(&[("parent", p.as_str())]);
                }
                if let Some(lim) = limit {
                    builder = builder.query(&[("limit", lim.to_string())]);
                }
                if let Some(ref c) = cursor {
                    builder = builder.query(&[("cursor", c.as_str())]);
                }

                let res: CommentThreadResponse = transport.execute_json(builder).await?;
                Ok(Page::new(res.comments, res.next_cursor))
            }
        })
    }
}
