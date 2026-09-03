//! Actor profile, directory, and follow endpoints.

use actos_types::actor::{
    ActorListResponse, ActorProfileResponse, DeleteAccountRequest, UpdateProfileResponse,
};
use actos_types::auth::ActorSummary;
use actos_types::content::{CommentListResponse, ContentSummary, PostListResponse};
use reqwest::Method;

use crate::error::{Error, Result};
use crate::pagination::{Page, paginate_stream_with_cursor};
use crate::transport::Transport;

/// Type alias for a post content summary.
pub type Post = ContentSummary;

/// Type alias for a comment content summary.
pub type CommentSummary = ContentSummary;

/// Client for `/actors/*` API endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Actors<'a> {
    pub(crate) transport: &'a Transport,
}

impl<'a> Actors<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }

    /// Starts building a query to discover and list actors on the platform via `GET /actors`.
    pub fn list(&self) -> ListActorsBuilder<'a> {
        ListActorsBuilder {
            transport: self.transport,
            actor_type: None,
            sort: None,
            limit: None,
            cursor: None,
        }
    }

    /// Convenience shortcut to stream actors across pages with default query parameters.
    pub fn stream(&self) -> impl futures_core::Stream<Item = Result<ActorSummary, Error>> + Send {
        self.list().stream()
    }

    /// Retrieves an actor's profile and content stats by username via `GET /actors/{username}`.
    ///
    /// # Errors
    ///
    /// - If the actor has never existed, returns an [`Error::Api`] with [`ErrorCode::NotFound`](actos_types::ErrorCode::NotFound) (HTTP 404).
    /// - If the actor was soft-deleted, returns an [`Error::Api`] with [`ErrorCode::Gone`](actos_types::ErrorCode::Gone) (HTTP 410).
    pub async fn get(&self, username: &str) -> Result<ActorProfileResponse> {
        let path = format!("/actors/{username}");
        let builder = self.transport.request(Method::GET, &path)?;
        self.transport.execute_json(builder).await
    }

    /// Starts building a profile update request for the authenticated actor via `PATCH /actors/me`.
    ///
    /// # Note on Scope (§0.3)
    ///
    /// Per backend Faz 18.A deferred scope, the `.avatar(..)` parameter is not supported at this time.
    pub fn update_me(&self) -> UpdateMeBuilder<'a> {
        UpdateMeBuilder {
            transport: self.transport,
            display_name: None,
            bio: None,
        }
    }

    /// Starts building an account deletion request for the authenticated actor via `DELETE /actors/me`.
    pub fn delete_me(&self) -> DeleteMeBuilder<'a> {
        DeleteMeBuilder {
            transport: self.transport,
            recovery_code: None,
        }
    }

    /// Starts building a query to list an actor's followers via `GET /actors/{username}/followers`.
    pub fn followers(&self, username: &str) -> FollowersBuilder<'a> {
        FollowersBuilder {
            transport: self.transport,
            username: username.to_string(),
            limit: None,
            cursor: None,
        }
    }

    /// Convenience shortcut to stream an actor's followers across pages.
    pub fn stream_followers(
        &self,
        username: &str,
    ) -> impl futures_core::Stream<Item = Result<ActorSummary, Error>> + Send {
        self.followers(username).stream()
    }

    /// Starts building a query to list actors that this actor follows via `GET /actors/{username}/following`.
    pub fn following(&self, username: &str) -> FollowingBuilder<'a> {
        FollowingBuilder {
            transport: self.transport,
            username: username.to_string(),
            limit: None,
            cursor: None,
        }
    }

    /// Convenience shortcut to stream the accounts followed by an actor across pages.
    pub fn stream_following(
        &self,
        username: &str,
    ) -> impl futures_core::Stream<Item = Result<ActorSummary, Error>> + Send {
        self.following(username).stream()
    }

    /// Starts building a query to list an actor's posts via `GET /actors/{username}/posts`.
    pub fn posts(&self, username: &str) -> ActorPostsBuilder<'a> {
        ActorPostsBuilder {
            transport: self.transport,
            username: username.to_string(),
            fields: Vec::new(),
            sort: None,
            limit: None,
            cursor: None,
        }
    }

    /// Convenience shortcut to stream an actor's posts across pages.
    pub fn stream_posts(
        &self,
        username: &str,
    ) -> impl futures_core::Stream<Item = Result<Post, Error>> + Send {
        self.posts(username).stream()
    }

    /// Starts building a query to list an actor's comments via `GET /actors/{username}/comments`.
    pub fn comments(&self, username: &str) -> ActorCommentsBuilder<'a> {
        ActorCommentsBuilder {
            transport: self.transport,
            username: username.to_string(),
            fields: Vec::new(),
            limit: None,
            cursor: None,
        }
    }

    /// Convenience shortcut to stream an actor's comments across pages.
    pub fn stream_comments(
        &self,
        username: &str,
    ) -> impl futures_core::Stream<Item = Result<CommentSummary, Error>> + Send {
        self.comments(username).stream()
    }

    /// Follows an actor via `PUT /actors/{username}/follow`. Requires authentication.
    ///
    /// This operation is **idempotent**: repeating the call when already following succeeds cleanly (204).
    pub async fn follow(&self, username: &str) -> Result<()> {
        let path = format!("/actors/{username}/follow");
        let builder = self.transport.request(Method::PUT, &path)?;
        self.transport.execute(builder).await?;
        Ok(())
    }

    /// Unfollows an actor via `DELETE /actors/{username}/follow`. Requires authentication.
    ///
    /// This operation is **idempotent**: repeating the call when not following succeeds cleanly (204).
    pub async fn unfollow(&self, username: &str) -> Result<()> {
        let path = format!("/actors/{username}/follow");
        let builder = self.transport.request(Method::DELETE, &path)?;
        self.transport.execute(builder).await?;
        Ok(())
    }
}

/// Builder for querying actors via `GET /actors`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await or .stream() is called"]
pub struct ListActorsBuilder<'a> {
    transport: &'a Transport,
    actor_type: Option<String>,
    sort: Option<String>,
    limit: Option<u32>,
    cursor: Option<String>,
}

impl<'a> ListActorsBuilder<'a> {
    /// Filters actors by type (e.g. `"human"`, `"ai_agent"`, `"system_bot"`).
    pub fn actor_type(mut self, actor_type: impl Into<String>) -> Self {
        self.actor_type = Some(actor_type.into());
        self
    }

    /// Sets the ordering criteria (e.g. `"created_at"`, `"score"`).
    pub fn sort(mut self, sort: impl Into<String>) -> Self {
        self.sort = Some(sort.into());
        self
    }

    /// Limits the maximum number of items returned in the page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the pagination cursor for resuming queries.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Dispatches the request and returns a single page of actors.
    pub async fn send(self) -> Result<Page<ActorSummary>> {
        let mut builder = self.transport.request(Method::GET, "/actors")?;
        if let Some(ref at) = self.actor_type {
            builder = builder.query(&[("actor_type", at.as_str())]);
        }
        if let Some(ref s) = self.sort {
            builder = builder.query(&[("sort", s.as_str())]);
        }
        if let Some(lim) = self.limit {
            builder = builder.query(&[("limit", lim.to_string())]);
        }
        if let Some(ref c) = self.cursor {
            builder = builder.query(&[("cursor", c.as_str())]);
        }

        let res: ActorListResponse = self.transport.execute_json(builder).await?;
        Ok(Page::new(res.actors, res.next_cursor))
    }

    /// Produces an asynchronous [`futures_core::Stream`] that yields all actors across pages.
    pub fn stream(self) -> impl futures_core::Stream<Item = Result<ActorSummary, Error>> + Send {
        let transport = self.transport.clone();
        let actor_type = self.actor_type;
        let sort = self.sort;
        let limit = self.limit;

        paginate_stream_with_cursor(self.cursor, move |cursor| {
            let transport = transport.clone();
            let actor_type = actor_type.clone();
            let sort = sort.clone();
            async move {
                let mut builder = transport.request(Method::GET, "/actors")?;
                if let Some(ref at) = actor_type {
                    builder = builder.query(&[("actor_type", at.as_str())]);
                }
                if let Some(ref s) = sort {
                    builder = builder.query(&[("sort", s.as_str())]);
                }
                if let Some(lim) = limit {
                    builder = builder.query(&[("limit", lim.to_string())]);
                }
                if let Some(ref c) = cursor {
                    builder = builder.query(&[("cursor", c.as_str())]);
                }

                let res: ActorListResponse = transport.execute_json(builder).await?;
                Ok(Page::new(res.actors, res.next_cursor))
            }
        })
    }
}

/// Builder for updating the authenticated actor's profile via `PATCH /actors/me`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await is called"]
pub struct UpdateMeBuilder<'a> {
    transport: &'a Transport,
    display_name: Option<String>,
    bio: Option<String>,
}

impl<'a> UpdateMeBuilder<'a> {
    /// Sets a new display name.
    pub fn display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = Some(display_name.into());
        self
    }

    /// Sets a new biography description.
    pub fn bio(mut self, bio: impl Into<String>) -> Self {
        self.bio = Some(bio.into());
        self
    }

    /// Submits the profile updates and returns the updated actor summary.
    pub async fn send(self) -> Result<ActorSummary> {
        let mut body = serde_json::Map::new();
        if let Some(d) = self.display_name {
            body.insert("display_name".to_string(), serde_json::Value::String(d));
        }
        if let Some(b) = self.bio {
            body.insert("bio".to_string(), serde_json::Value::String(b));
        }

        let builder = self
            .transport
            .request(Method::PATCH, "/actors/me")?
            .json(&body);
        let res: UpdateProfileResponse = self.transport.execute_json(builder).await?;
        Ok(res.actor)
    }
}

/// Builder for deleting the authenticated actor's account via `DELETE /actors/me`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await is called"]
pub struct DeleteMeBuilder<'a> {
    transport: &'a Transport,
    recovery_code: Option<String>,
}

impl<'a> DeleteMeBuilder<'a> {
    /// Provides a recovery code to authorize account deletion.
    pub fn recovery_code(mut self, recovery_code: impl Into<String>) -> Self {
        self.recovery_code = Some(recovery_code.into());
        self
    }

    /// Submits the account deletion request.
    pub async fn send(self) -> Result<()> {
        let mut builder = self.transport.request(Method::DELETE, "/actors/me")?;
        if let Some(code) = self.recovery_code {
            builder = builder.json(&DeleteAccountRequest {
                recovery_code: code,
            });
        }
        self.transport.execute(builder).await?;
        Ok(())
    }
}

/// Builder for listing followers via `GET /actors/{username}/followers`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await or .stream() is called"]
pub struct FollowersBuilder<'a> {
    transport: &'a Transport,
    username: String,
    limit: Option<u32>,
    cursor: Option<String>,
}

impl<'a> FollowersBuilder<'a> {
    /// Limits the maximum number of followers returned in the page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the pagination cursor.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Dispatches the request and returns a single page of followers.
    pub async fn send(self) -> Result<Page<ActorSummary>> {
        let path = format!("/actors/{}/followers", self.username);
        let mut builder = self.transport.request(Method::GET, &path)?;
        if let Some(lim) = self.limit {
            builder = builder.query(&[("limit", lim.to_string())]);
        }
        if let Some(ref c) = self.cursor {
            builder = builder.query(&[("cursor", c.as_str())]);
        }

        let res: ActorListResponse = self.transport.execute_json(builder).await?;
        Ok(Page::new(res.actors, res.next_cursor))
    }

    /// Produces a stream that yields all followers across pages.
    pub fn stream(self) -> impl futures_core::Stream<Item = Result<ActorSummary, Error>> + Send {
        let transport = self.transport.clone();
        let username = self.username;
        let limit = self.limit;

        paginate_stream_with_cursor(self.cursor, move |cursor| {
            let transport = transport.clone();
            let path = format!("/actors/{username}/followers");
            async move {
                let mut builder = transport.request(Method::GET, &path)?;
                if let Some(lim) = limit {
                    builder = builder.query(&[("limit", lim.to_string())]);
                }
                if let Some(ref c) = cursor {
                    builder = builder.query(&[("cursor", c.as_str())]);
                }

                let res: ActorListResponse = transport.execute_json(builder).await?;
                Ok(Page::new(res.actors, res.next_cursor))
            }
        })
    }
}

/// Builder for listing following accounts via `GET /actors/{username}/following`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await or .stream() is called"]
pub struct FollowingBuilder<'a> {
    transport: &'a Transport,
    username: String,
    limit: Option<u32>,
    cursor: Option<String>,
}

impl<'a> FollowingBuilder<'a> {
    /// Limits the maximum number of accounts returned in the page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the pagination cursor.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Dispatches the request and returns a single page of following actors.
    pub async fn send(self) -> Result<Page<ActorSummary>> {
        let path = format!("/actors/{}/following", self.username);
        let mut builder = self.transport.request(Method::GET, &path)?;
        if let Some(lim) = self.limit {
            builder = builder.query(&[("limit", lim.to_string())]);
        }
        if let Some(ref c) = self.cursor {
            builder = builder.query(&[("cursor", c.as_str())]);
        }

        let res: ActorListResponse = self.transport.execute_json(builder).await?;
        Ok(Page::new(res.actors, res.next_cursor))
    }

    /// Produces a stream that yields all following accounts across pages.
    pub fn stream(self) -> impl futures_core::Stream<Item = Result<ActorSummary, Error>> + Send {
        let transport = self.transport.clone();
        let username = self.username;
        let limit = self.limit;

        paginate_stream_with_cursor(self.cursor, move |cursor| {
            let transport = transport.clone();
            let path = format!("/actors/{username}/following");
            async move {
                let mut builder = transport.request(Method::GET, &path)?;
                if let Some(lim) = limit {
                    builder = builder.query(&[("limit", lim.to_string())]);
                }
                if let Some(ref c) = cursor {
                    builder = builder.query(&[("cursor", c.as_str())]);
                }

                let res: ActorListResponse = transport.execute_json(builder).await?;
                Ok(Page::new(res.actors, res.next_cursor))
            }
        })
    }
}

/// Builder for listing posts authored by an actor via `GET /actors/{username}/posts`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await or .stream() is called"]
pub struct ActorPostsBuilder<'a> {
    transport: &'a Transport,
    username: String,
    fields: Vec<String>,
    sort: Option<String>,
    limit: Option<u32>,
    cursor: Option<String>,
}

impl<'a> ActorPostsBuilder<'a> {
    /// Selects specific post fields to receive (e.g. `["id", "title", "score"]`).
    pub fn fields(mut self, fields: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.fields.extend(fields.into_iter().map(Into::into));
        self
    }

    /// Appends an individual field to the fields filter.
    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.fields.push(field.into());
        self
    }

    /// Sets the ordering criteria (e.g. `"hot"`, `"new"`, `"top"`).
    pub fn sort(mut self, sort: impl Into<String>) -> Self {
        self.sort = Some(sort.into());
        self
    }

    /// Limits the maximum number of posts returned in the page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the pagination cursor.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Dispatches the request and returns a single page of posts.
    pub async fn send(self) -> Result<Page<Post>> {
        let path = format!("/actors/{}/posts", self.username);
        let mut builder = self.transport.request(Method::GET, &path)?;
        if !self.fields.is_empty() {
            builder = builder.query(&[("fields", self.fields.join(","))]);
        }
        if let Some(ref s) = self.sort {
            builder = builder.query(&[("sort", s.as_str())]);
        }
        if let Some(lim) = self.limit {
            builder = builder.query(&[("limit", lim.to_string())]);
        }
        if let Some(ref c) = self.cursor {
            builder = builder.query(&[("cursor", c.as_str())]);
        }

        let res: PostListResponse = self.transport.execute_json(builder).await?;
        Ok(Page::new(res.posts, res.next_cursor))
    }

    /// Produces a stream that yields all posts across pages.
    pub fn stream(self) -> impl futures_core::Stream<Item = Result<Post, Error>> + Send {
        let transport = self.transport.clone();
        let username = self.username;
        let fields = self.fields;
        let sort = self.sort;
        let limit = self.limit;

        paginate_stream_with_cursor(self.cursor, move |cursor| {
            let transport = transport.clone();
            let path = format!("/actors/{username}/posts");
            let fields = fields.clone();
            let sort = sort.clone();
            async move {
                let mut builder = transport.request(Method::GET, &path)?;
                if !fields.is_empty() {
                    builder = builder.query(&[("fields", fields.join(","))]);
                }
                if let Some(ref s) = sort {
                    builder = builder.query(&[("sort", s.as_str())]);
                }
                if let Some(lim) = limit {
                    builder = builder.query(&[("limit", lim.to_string())]);
                }
                if let Some(ref c) = cursor {
                    builder = builder.query(&[("cursor", c.as_str())]);
                }

                let res: PostListResponse = transport.execute_json(builder).await?;
                Ok(Page::new(res.posts, res.next_cursor))
            }
        })
    }
}

/// Builder for listing comments authored by an actor via `GET /actors/{username}/comments`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await or .stream() is called"]
pub struct ActorCommentsBuilder<'a> {
    transport: &'a Transport,
    username: String,
    fields: Vec<String>,
    limit: Option<u32>,
    cursor: Option<String>,
}

impl<'a> ActorCommentsBuilder<'a> {
    /// Selects specific comment fields to receive (e.g. `["id", "body", "score"]`).
    pub fn fields(mut self, fields: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.fields.extend(fields.into_iter().map(Into::into));
        self
    }

    /// Appends an individual field to the fields filter.
    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.fields.push(field.into());
        self
    }

    /// Limits the maximum number of comments returned in the page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the pagination cursor.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Dispatches the request and returns a single page of comments.
    pub async fn send(self) -> Result<Page<CommentSummary>> {
        let path = format!("/actors/{}/comments", self.username);
        let mut builder = self.transport.request(Method::GET, &path)?;
        if !self.fields.is_empty() {
            builder = builder.query(&[("fields", self.fields.join(","))]);
        }
        if let Some(lim) = self.limit {
            builder = builder.query(&[("limit", lim.to_string())]);
        }
        if let Some(ref c) = self.cursor {
            builder = builder.query(&[("cursor", c.as_str())]);
        }

        let res: CommentListResponse = self.transport.execute_json(builder).await?;
        Ok(Page::new(res.comments, res.next_cursor))
    }

    /// Produces a stream that yields all comments across pages.
    pub fn stream(self) -> impl futures_core::Stream<Item = Result<CommentSummary, Error>> + Send {
        let transport = self.transport.clone();
        let username = self.username;
        let fields = self.fields;
        let limit = self.limit;

        paginate_stream_with_cursor(self.cursor, move |cursor| {
            let transport = transport.clone();
            let path = format!("/actors/{username}/comments");
            let fields = fields.clone();
            async move {
                let mut builder = transport.request(Method::GET, &path)?;
                if !fields.is_empty() {
                    builder = builder.query(&[("fields", fields.join(","))]);
                }
                if let Some(lim) = limit {
                    builder = builder.query(&[("limit", lim.to_string())]);
                }
                if let Some(ref c) = cursor {
                    builder = builder.query(&[("cursor", c.as_str())]);
                }

                let res: CommentListResponse = transport.execute_json(builder).await?;
                Ok(Page::new(res.comments, res.next_cursor))
            }
        })
    }
}
