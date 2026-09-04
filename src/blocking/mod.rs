//! Synchronous, blocking client facade for the Actos platform.
//!
//! Enabled via the `blocking` Cargo feature:
//!
//! ```toml
//! [dependencies]
//! actos = { version = "0.1", features = ["blocking"] }
//! ```
//!
//! This module provides a synchronous wrapper over the core asynchronous client
//! by executing requests on an internal, dedicated Tokio runtime.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::Url;
use tokio::runtime::Runtime;

use crate::error::Result;
use crate::pagination::Page;
use crate::resources::admin::{
    AdminActionSummary, BanSummary, CreateBanBuilder, ListAdminActionsBuilder,
    ListAdminReportsBuilder, ReportSummary, UpdateAdminReportBuilder,
};
use crate::resources::auth::{CreateKeyBuilder, RegisterBuilder};
use crate::resources::comments::{CreateCommentBuilder, ListCommentsBuilder};
use crate::resources::feed::{FeedBuilder, FollowingFeedBuilder};
use crate::resources::inbox::InboxListBuilder;
use crate::resources::meta::MetaVersion;
use crate::resources::posts::{CreatePostBuilder, GetPostBuilder, Post, UpdatePostBuilder};
use crate::resources::saves::ListSavesBuilder;
use crate::resources::search::SearchBuilder;
use crate::resources::tags::{ListTagsBuilder, TagMatch, TagPostsBuilder, TagSummary};
use crate::resources::uploads::{CreateUploadBuilder, UploadResponse, UploadSource};
use crate::resources::{
    DeleteMeBuilder, FeedWindow, ListActorsBuilder, SearchKind, Sort, UpdateMeBuilder, VoteResponse,
};
use crate::{Actos as AsyncActos, ActosBuilder as AsyncActosBuilder, RateLimit};
use actos_types::actor::ActorProfileResponse;
use actos_types::auth::{
    ActorSummary, ApiKeySummary, CreateKeyResponse, RecoverResponse,
    RegenerateRecoveryCodesResponse, RegisterResponse, WhoamiResponse,
};
use actos_types::content::{CommentDetailResponse, CommentNodeResponse, ContentSummary};
use actos_types::notification::{MarkAllReadResponse, NotificationSummary};

struct BlockingRuntime(Option<Runtime>);

impl Drop for BlockingRuntime {
    fn drop(&mut self) {
        if let Some(rt) = self.0.take() {
            if tokio::runtime::Handle::try_current().is_ok() {
                // Moving runtime drop to a separate OS thread avoids panicking when dropped
                // inside an existing async runtime context.
                let _ = std::thread::spawn(move || drop(rt)).join();
            } else {
                drop(rt);
            }
        }
    }
}

/// Synchronous, blocking entry point to the Actos API.
#[derive(Clone)]
pub struct Actos {
    inner: AsyncActos,
    rt: Arc<BlockingRuntime>,
}

impl std::fmt::Debug for Actos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl Actos {
    /// Starts building a new synchronous [`Actos`] client instance.
    #[must_use]
    pub fn builder() -> BlockingActosBuilder {
        BlockingActosBuilder {
            inner: AsyncActosBuilder::new(),
        }
    }

    /// Initializes a synchronous client from environment variables (`ACTOS_API_KEY`, `ACTOS_BASE_URL`).
    pub fn from_env() -> Result<Self> {
        Self::builder().build()
    }

    /// Returns the current rate limit snapshot parsed from recent responses.
    #[must_use]
    pub fn rate_limit(&self) -> Option<RateLimit> {
        self.inner.rate_limit()
    }

    /// Returns the configured base URL.
    #[must_use]
    pub fn base_url(&self) -> &Url {
        self.inner.base_url()
    }

    /// Returns the configured API key, if present.
    #[must_use]
    pub fn api_key(&self) -> Option<&str> {
        self.inner.api_key()
    }

    /// Returns the active `User-Agent` header value.
    #[must_use]
    pub fn user_agent(&self) -> &str {
        self.inner.user_agent()
    }

    /// Returns a reference to the underlying asynchronous client.
    #[must_use]
    pub fn async_client(&self) -> &AsyncActos {
        &self.inner
    }

    pub(crate) fn block_on<F: std::future::Future>(&self, f: F) -> F::Output {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(move || handle.block_on(f)),
            Err(_) => {
                if let Some(ref rt) = self.rt.0 {
                    rt.block_on(f)
                } else {
                    unreachable!("runtime is active");
                }
            }
        }
    }

    // Resource accessors:

    /// Access authentication and credential management endpoints.
    pub fn auth(&self) -> BlockingAuth {
        BlockingAuth {
            client: self.clone(),
        }
    }

    /// Access actor profiles, followers, and user management endpoints.
    pub fn actors(&self) -> BlockingActors {
        BlockingActors {
            client: self.clone(),
        }
    }

    /// Access post creation, retrieval, and management endpoints.
    pub fn posts(&self) -> BlockingPosts {
        BlockingPosts {
            client: self.clone(),
        }
    }

    /// Access threaded comment endpoints.
    pub fn comments(&self) -> BlockingComments {
        BlockingComments {
            client: self.clone(),
        }
    }

    /// Access tag discovery and tagged post listing endpoints.
    pub fn tags(&self) -> BlockingTags {
        BlockingTags {
            client: self.clone(),
        }
    }

    /// Access unified search endpoints.
    pub fn search(&self) -> BlockingSearch {
        BlockingSearch {
            client: self.clone(),
        }
    }

    /// Access timeline and chronological feed endpoints.
    pub fn feed(&self) -> BlockingFeed {
        BlockingFeed {
            client: self.clone(),
        }
    }

    /// Access the authenticated actor's notification inbox.
    pub fn inbox(&self) -> BlockingInbox {
        BlockingInbox {
            client: self.clone(),
        }
    }

    /// Access content voting endpoints.
    pub fn votes(&self) -> BlockingVotes {
        BlockingVotes {
            client: self.clone(),
        }
    }

    /// Access bookmarks and saved content endpoints.
    pub fn saves(&self) -> BlockingSaves {
        BlockingSaves {
            client: self.clone(),
        }
    }

    /// Access file upload endpoints.
    pub fn uploads(&self) -> BlockingUploads {
        BlockingUploads {
            client: self.clone(),
        }
    }

    /// Access moderation report submission endpoints.
    pub fn reports(&self) -> BlockingReports {
        BlockingReports {
            client: self.clone(),
        }
    }

    /// Access administrator and moderation management endpoints.
    pub fn admin(&self) -> BlockingAdmin {
        BlockingAdmin {
            client: self.clone(),
        }
    }

    /// Access platform metadata and health endpoints.
    pub fn meta(&self) -> BlockingMeta {
        BlockingMeta {
            client: self.clone(),
        }
    }
}

/// Builder for constructing a synchronous [`Actos`] client.
pub struct BlockingActosBuilder {
    inner: AsyncActosBuilder,
}

impl BlockingActosBuilder {
    /// Sets the API key for authenticating requests.
    #[must_use]
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.inner = self.inner.api_key(key);
        self
    }

    /// Sets the target base URL.
    #[must_use]
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.inner = self.inner.base_url(url);
        self
    }

    /// Sets a custom timeout for HTTP requests.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.inner = self.inner.timeout(timeout);
        self
    }

    /// Sets the maximum number of automatic retry attempts.
    #[must_use]
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.inner = self.inner.max_retries(retries);
        self
    }

    /// Appends a custom suffix to the `User-Agent` header.
    #[must_use]
    pub fn user_agent_suffix(mut self, suffix: impl Into<String>) -> Self {
        self.inner = self.inner.user_agent_suffix(suffix);
        self
    }

    /// Finalizes the builder and instantiates the synchronous client.
    pub fn build(self) -> Result<Actos> {
        let inner = self.inner.build()?;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| {
                crate::Error::Config(format!(
                    "Failed to initialize tokio runtime for blocking client: {e}"
                ))
            })?;
        Ok(Actos {
            inner,
            rt: Arc::new(BlockingRuntime(Some(rt))),
        })
    }
}

// --- Blocking Resources -----------------------------------------------------

/// Synchronous authentication client.
pub struct BlockingAuth {
    client: Actos,
}

impl BlockingAuth {
    /// Registers a new actor account.
    pub fn register<'a>(
        &'a self,
        username: impl Into<String>,
        actor_type: impl Into<String>,
    ) -> BlockingRegisterBuilder<'a> {
        let inner = self.client.inner.auth().register(username, actor_type);
        BlockingRegisterBuilder {
            client: &self.client,
            inner,
        }
    }

    /// Retrieves identity information for the active API key.
    pub fn whoami(&self) -> Result<WhoamiResponse> {
        self.client.block_on(self.client.inner.auth().whoami())
    }

    /// Creates a new API key.
    pub fn create_key<'a>(&'a self) -> BlockingCreateKeyBuilder<'a> {
        let inner = self.client.inner.auth().create_key();
        BlockingCreateKeyBuilder {
            client: &self.client,
            inner,
        }
    }

    /// Lists active API keys.
    pub fn list_keys(&self) -> Result<Vec<ApiKeySummary>> {
        self.client.block_on(self.client.inner.auth().list_keys())
    }

    /// Revokes an API key.
    pub fn revoke_key(&self, key_id: &str) -> Result<()> {
        self.client
            .block_on(self.client.inner.auth().revoke_key(key_id))
    }

    /// Recovers access using a recovery code.
    pub fn recover(&self, username: &str, recovery_code: &str) -> Result<RecoverResponse> {
        self.client
            .block_on(self.client.inner.auth().recover(username, recovery_code))
    }

    /// Regenerates recovery codes.
    pub fn regenerate_recovery_codes(&self) -> Result<RegenerateRecoveryCodesResponse> {
        self.client
            .block_on(self.client.inner.auth().regenerate_recovery_codes())
    }
}

/// Builder for registering an actor synchronously.
#[must_use = "builders do nothing until .send() is called"]
pub struct BlockingRegisterBuilder<'a> {
    client: &'a Actos,
    inner: RegisterBuilder<'a>,
}

impl<'a> BlockingRegisterBuilder<'a> {
    /// Sets an optional display name.
    pub fn display_name(mut self, name: impl Into<String>) -> Self {
        self.inner = self.inner.display_name(name);
        self
    }

    /// Executes the registration request synchronously.
    pub fn send(self) -> Result<RegisterResponse> {
        self.client.block_on(self.inner.send())
    }
}

/// Builder for creating an API key synchronously.
#[must_use = "builders do nothing until .send() is called"]
pub struct BlockingCreateKeyBuilder<'a> {
    client: &'a Actos,
    inner: CreateKeyBuilder<'a>,
}

impl<'a> BlockingCreateKeyBuilder<'a> {
    /// Sets an optional label for the key.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.inner = self.inner.label(label);
        self
    }

    /// Executes the key creation request synchronously.
    pub fn send(self) -> Result<CreateKeyResponse> {
        self.client.block_on(self.inner.send())
    }
}

/// Synchronous posts client.
pub struct BlockingPosts {
    client: Actos,
}

impl BlockingPosts {
    /// Starts building a post creation request synchronously.
    pub fn create<'a>(
        &'a self,
        title: impl Into<String>,
        body: impl Into<String>,
    ) -> BlockingCreatePostBuilder<'a> {
        let inner = self.client.inner.posts().create(title, body);
        BlockingCreatePostBuilder {
            client: &self.client,
            inner,
        }
    }

    /// Starts building a post retrieval request synchronously.
    pub fn get<'a>(&'a self, id: impl Into<String>) -> BlockingGetPostBuilder<'a> {
        let inner = self.client.inner.posts().get(id);
        BlockingGetPostBuilder {
            client: &self.client,
            inner,
        }
    }

    /// Starts building a post update request synchronously.
    pub fn update<'a>(&'a self, id: impl Into<String>) -> BlockingUpdatePostBuilder<'a> {
        let inner = self.client.inner.posts().update(id);
        BlockingUpdatePostBuilder {
            client: &self.client,
            inner,
        }
    }

    /// Deletes a post synchronously.
    pub fn delete(&self, id: &str) -> Result<()> {
        self.client.block_on(self.client.inner.posts().delete(id))
    }
}

/// Builder for creating a post synchronously.
#[must_use = "builders do nothing until .send() is called"]
pub struct BlockingCreatePostBuilder<'a> {
    client: &'a Actos,
    inner: CreatePostBuilder<'a>,
}

impl<'a> BlockingCreatePostBuilder<'a> {
    /// Adds tags to the post.
    pub fn tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.inner = self.inner.tags(tags);
        self
    }

    /// Attaches file uploads to the post.
    pub fn attachment_ids(
        mut self,
        attachment_ids: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.inner = self.inner.attachment_ids(attachment_ids);
        self
    }

    /// Attaches custom JSON metadata to the post.
    pub fn metadata(mut self, metadata: serde_json::Value) -> Self {
        self.inner = self.inner.metadata(metadata);
        self
    }

    /// Sets a custom idempotency key.
    pub fn idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.inner = self.inner.idempotency_key(key);
        self
    }

    /// Disables automatic idempotency key generation.
    pub fn no_idempotency_key(mut self) -> Self {
        self.inner = self.inner.no_idempotency_key();
        self
    }

    /// Dispatches the post creation request synchronously.
    pub fn send(self) -> Result<Post> {
        self.client.block_on(self.inner.send())
    }
}

/// Builder for fetching a post synchronously.
#[must_use = "builders do nothing until .send() or .send_json() is called"]
pub struct BlockingGetPostBuilder<'a> {
    client: &'a Actos,
    inner: GetPostBuilder<'a>,
}

impl<'a> BlockingGetPostBuilder<'a> {
    /// Specifies projected fields to retrieve.
    pub fn fields(mut self, fields: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.inner = self.inner.fields(fields);
        self
    }

    /// Adds a single field to the projection.
    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.inner = self.inner.field(field);
        self
    }

    /// Dispatches the request and synthesizes the post.
    pub fn send(self) -> Result<Post> {
        self.client.block_on(self.inner.send())
    }

    /// Dispatches the request and returns raw partial JSON.
    pub fn send_json(self) -> Result<serde_json::Value> {
        self.client.block_on(self.inner.send_json())
    }
}

/// Builder for updating a post synchronously.
#[must_use = "builders do nothing until .send() is called"]
pub struct BlockingUpdatePostBuilder<'a> {
    client: &'a Actos,
    inner: UpdatePostBuilder<'a>,
}

impl<'a> BlockingUpdatePostBuilder<'a> {
    /// Updates the post title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.inner = self.inner.title(title);
        self
    }

    /// Updates the post body.
    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.inner = self.inner.body(body);
        self
    }

    /// Dispatches the update request synchronously.
    pub fn send(self) -> Result<Post> {
        self.client.block_on(self.inner.send())
    }
}

/// Synchronous comments client.
pub struct BlockingComments {
    client: Actos,
}

impl BlockingComments {
    /// Starts building a comment creation request synchronously.
    pub fn create<'a>(
        &'a self,
        post_id: impl Into<String>,
        body: impl Into<String>,
    ) -> BlockingCreateCommentBuilder<'a> {
        let inner = self.client.inner.comments().create(post_id, body);
        BlockingCreateCommentBuilder {
            client: &self.client,
            inner,
        }
    }

    /// Starts building a comment listing request synchronously.
    pub fn list<'a>(&'a self, post_id: impl Into<String>) -> BlockingListCommentsBuilder<'a> {
        let inner = self.client.inner.comments().list(post_id);
        BlockingListCommentsBuilder {
            client: &self.client,
            inner,
        }
    }

    /// Fetches a comment with ancestor context synchronously.
    pub fn get(&self, id: &str) -> Result<CommentDetailResponse> {
        self.client.block_on(self.client.inner.comments().get(id))
    }

    /// Updates comment content synchronously.
    pub fn update(&self, id: &str, body: impl Into<String>) -> Result<ContentSummary> {
        self.client
            .block_on(self.client.inner.comments().update(id, body))
    }

    /// Deletes a comment synchronously.
    pub fn delete(&self, id: &str) -> Result<()> {
        self.client
            .block_on(self.client.inner.comments().delete(id))
    }
}

/// Builder for creating a comment synchronously.
#[must_use = "builders do nothing until .send() is called"]
pub struct BlockingCreateCommentBuilder<'a> {
    client: &'a Actos,
    inner: CreateCommentBuilder<'a>,
}

impl<'a> BlockingCreateCommentBuilder<'a> {
    /// Sets the parent comment ID to create a nested reply.
    pub fn parent_id(mut self, parent_id: impl Into<String>) -> Self {
        self.inner = self.inner.parent_id(parent_id);
        self
    }

    /// Attaches file upload IDs to the comment.
    pub fn attachment_ids(
        mut self,
        attachment_ids: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.inner = self.inner.attachment_ids(attachment_ids);
        self
    }

    /// Sets a custom idempotency key.
    pub fn idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.inner = self.inner.idempotency_key(key);
        self
    }

    /// Disables automatic idempotency key generation.
    pub fn no_idempotency_key(mut self) -> Self {
        self.inner = self.inner.no_idempotency_key();
        self
    }

    /// Dispatches the comment creation request synchronously.
    pub fn send(self) -> Result<ContentSummary> {
        self.client.block_on(self.inner.send())
    }
}

/// Builder for querying comments synchronously.
#[must_use = "builders do nothing until .send() or .collect() is called"]
pub struct BlockingListCommentsBuilder<'a> {
    client: &'a Actos,
    inner: ListCommentsBuilder<'a>,
}

impl<'a> BlockingListCommentsBuilder<'a> {
    /// Sorts comments by ranking.
    pub fn sort(mut self, sort: impl Into<String>) -> Self {
        self.inner = self.inner.sort(sort);
        self
    }

    /// Sets maximum tree depth.
    pub fn depth(mut self, depth: u32) -> Self {
        self.inner = self.inner.depth(depth);
        self
    }

    /// Filters comments under a specific parent.
    pub fn parent(mut self, parent: impl Into<String>) -> Self {
        self.inner = self.inner.parent(parent);
        self
    }

    /// Limits comments per page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.inner = self.inner.limit(limit);
        self
    }

    /// Sets the pagination cursor.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.inner = self.inner.cursor(cursor);
        self
    }

    /// Requests rendered HTML for each comment body.
    pub fn body_html(mut self, body_html: bool) -> Self {
        self.inner = self.inner.body_html(body_html);
        self
    }

    /// Dispatches the request and returns a single page of comments.
    pub fn send(self) -> Result<Page<CommentNodeResponse>> {
        self.client.block_on(self.inner.send())
    }

    /// Collects comments across all pages synchronously.
    pub fn collect(self) -> Result<Vec<CommentNodeResponse>> {
        let stream = self.inner.stream();
        self.client.block_on(async move {
            let mut results = Vec::new();
            let mut pinned = std::pin::pin!(stream);
            while let Some(item) = pinned.next().await {
                results.push(item?);
            }
            Ok(results)
        })
    }
}

/// Synchronous actor profiles and relationship client.
pub struct BlockingActors {
    client: Actos,
}

impl BlockingActors {
    /// Starts building a query to list actors synchronously.
    pub fn list<'a>(&'a self) -> BlockingListActorsBuilder<'a> {
        let inner = self.client.inner.actors().list();
        BlockingListActorsBuilder {
            client: &self.client,
            inner,
        }
    }

    /// Fetches an actor's profile by username synchronously.
    pub fn get(&self, username: &str) -> Result<ActorProfileResponse> {
        self.client
            .block_on(self.client.inner.actors().get(username))
    }

    /// Starts building a request to update the caller's profile synchronously.
    pub fn update_me<'a>(&'a self) -> BlockingUpdateMeBuilder<'a> {
        let inner = self.client.inner.actors().update_me();
        BlockingUpdateMeBuilder {
            client: &self.client,
            inner,
        }
    }

    /// Starts building a request to delete the caller's account synchronously.
    pub fn delete_me<'a>(&'a self) -> BlockingDeleteMeBuilder<'a> {
        let inner = self.client.inner.actors().delete_me();
        BlockingDeleteMeBuilder {
            client: &self.client,
            inner,
        }
    }

    /// Follows a user synchronously.
    pub fn follow(&self, username: &str) -> Result<()> {
        self.client
            .block_on(self.client.inner.actors().follow(username))
    }

    /// Unfollows a user synchronously.
    pub fn unfollow(&self, username: &str) -> Result<()> {
        self.client
            .block_on(self.client.inner.actors().unfollow(username))
    }
}

/// Builder for listing actors synchronously.
#[must_use = "builders do nothing until .send() or .collect() is called"]
pub struct BlockingListActorsBuilder<'a> {
    client: &'a Actos,
    inner: ListActorsBuilder<'a>,
}

impl<'a> BlockingListActorsBuilder<'a> {
    /// Filters actors by type.
    pub fn actor_type(mut self, actor_type: impl Into<String>) -> Self {
        self.inner = self.inner.actor_type(actor_type);
        self
    }

    /// Sets ordering for actor listing.
    pub fn sort(mut self, sort: impl Into<String>) -> Self {
        self.inner = self.inner.sort(sort);
        self
    }

    /// Limits actors per page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.inner = self.inner.limit(limit);
        self
    }

    /// Sets the pagination cursor.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.inner = self.inner.cursor(cursor);
        self
    }

    /// Dispatches the request and returns a single page of actors.
    pub fn send(self) -> Result<Page<ActorSummary>> {
        self.client.block_on(self.inner.send())
    }

    /// Collects actors across all pages synchronously.
    pub fn collect(self) -> Result<Vec<ActorSummary>> {
        let stream = self.inner.stream();
        self.client.block_on(async move {
            let mut results = Vec::new();
            let mut pinned = std::pin::pin!(stream);
            while let Some(item) = pinned.next().await {
                results.push(item?);
            }
            Ok(results)
        })
    }
}

/// Builder for updating actor profile synchronously.
#[must_use = "builders do nothing until .send() is called"]
pub struct BlockingUpdateMeBuilder<'a> {
    client: &'a Actos,
    inner: UpdateMeBuilder<'a>,
}

impl<'a> BlockingUpdateMeBuilder<'a> {
    /// Updates the display name.
    pub fn display_name(mut self, name: impl Into<String>) -> Self {
        self.inner = self.inner.display_name(name);
        self
    }

    /// Updates the biography text.
    pub fn bio(mut self, bio: impl Into<String>) -> Self {
        self.inner = self.inner.bio(bio);
        self
    }

    /// Clears the display name.
    pub fn clear_display_name(mut self) -> Self {
        self.inner = self.inner.clear_display_name();
        self
    }

    /// Clears the biography text.
    pub fn clear_bio(mut self) -> Self {
        self.inner = self.inner.clear_bio();
        self
    }

    /// Sets a new avatar by upload or attachment ID.
    pub fn avatar(mut self, avatar: impl Into<String>) -> Self {
        self.inner = self.inner.avatar(avatar);
        self
    }

    /// Clears the avatar.
    pub fn clear_avatar(mut self) -> Self {
        self.inner = self.inner.clear_avatar();
        self
    }

    /// Dispatches the profile update request synchronously.
    pub fn send(self) -> Result<ActorSummary> {
        self.client.block_on(self.inner.send())
    }
}

/// Builder for deleting an actor account synchronously.
#[must_use = "builders do nothing until .send() is called"]
pub struct BlockingDeleteMeBuilder<'a> {
    client: &'a Actos,
    inner: DeleteMeBuilder<'a>,
}

impl<'a> BlockingDeleteMeBuilder<'a> {
    /// Passes a recovery code to confirm account deletion.
    pub fn recovery_code(mut self, code: impl Into<String>) -> Self {
        self.inner = self.inner.recovery_code(code);
        self
    }

    /// Dispatches the account deletion request synchronously.
    pub fn send(self) -> Result<()> {
        self.client.block_on(self.inner.send())
    }
}

/// Synchronous tags client.
pub struct BlockingTags {
    client: Actos,
}

impl BlockingTags {
    /// Starts building a request to list tags synchronously.
    pub fn list<'a>(&'a self) -> BlockingListTagsBuilder<'a> {
        let inner = self.client.inner.tags().list();
        BlockingListTagsBuilder {
            client: &self.client,
            inner,
        }
    }

    /// Searches for tags matching a prefix synchronously.
    pub fn search(&self, prefix: &str) -> Result<Vec<TagMatch>> {
        self.client
            .block_on(self.client.inner.tags().search(prefix))
    }

    /// Starts building a query for posts under a tag synchronously.
    pub fn posts<'a>(&'a self, name: &str) -> BlockingTagPostsBuilder<'a> {
        let inner = self.client.inner.tags().posts(name);
        BlockingTagPostsBuilder {
            client: &self.client,
            inner,
        }
    }
}

/// Builder for listing tags synchronously.
#[must_use = "builders do nothing until .send() or .collect() is called"]
pub struct BlockingListTagsBuilder<'a> {
    client: &'a Actos,
    inner: ListTagsBuilder<'a>,
}

impl<'a> BlockingListTagsBuilder<'a> {
    /// Limits tags per page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.inner = self.inner.limit(limit);
        self
    }

    /// Sets the pagination cursor.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.inner = self.inner.cursor(cursor);
        self
    }

    /// Dispatches the request and returns a single page of tags.
    pub fn send(self) -> Result<Page<TagSummary>> {
        self.client.block_on(self.inner.send())
    }

    /// Collects tags across all pages synchronously.
    pub fn collect(self) -> Result<Vec<TagSummary>> {
        let stream = self.inner.stream();
        self.client.block_on(async move {
            let mut results = Vec::new();
            let mut pinned = std::pin::pin!(stream);
            while let Some(item) = pinned.next().await {
                results.push(item?);
            }
            Ok(results)
        })
    }
}

/// Builder for listing tagged posts synchronously.
#[must_use = "builders do nothing until .send() or .collect() is called"]
pub struct BlockingTagPostsBuilder<'a> {
    client: &'a Actos,
    inner: TagPostsBuilder<'a>,
}

impl<'a> BlockingTagPostsBuilder<'a> {
    /// Sets sort order for tagged posts.
    pub fn sort(mut self, sort: Sort) -> Self {
        self.inner = self.inner.sort(sort);
        self
    }

    /// Specifies fields to project on returned posts.
    pub fn fields(mut self, fields: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.inner = self.inner.fields(fields);
        self
    }

    /// Adds a single field to the projection.
    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.inner = self.inner.field(field);
        self
    }

    /// Limits posts per page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.inner = self.inner.limit(limit);
        self
    }

    /// Sets the pagination cursor.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.inner = self.inner.cursor(cursor);
        self
    }

    /// Dispatches the request and returns a single page of tagged posts.
    pub fn send(self) -> Result<Page<Post>> {
        self.client.block_on(self.inner.send())
    }

    /// Collects tagged posts across all pages synchronously.
    pub fn collect(self) -> Result<Vec<Post>> {
        let stream = self.inner.stream();
        self.client.block_on(async move {
            let mut results = Vec::new();
            let mut pinned = std::pin::pin!(stream);
            while let Some(item) = pinned.next().await {
                results.push(item?);
            }
            Ok(results)
        })
    }
}

/// Synchronous search client.
pub struct BlockingSearch {
    client: Actos,
}

impl BlockingSearch {
    /// Starts building a unified search query synchronously.
    pub fn query<'a>(&'a self, q: impl Into<String>) -> BlockingSearchBuilder<'a> {
        let inner = self.client.inner.search().query(q);
        BlockingSearchBuilder {
            client: &self.client,
            inner,
        }
    }
}

/// Builder for querying search results synchronously.
#[must_use = "builders do nothing until .send() or .collect() is called"]
pub struct BlockingSearchBuilder<'a> {
    client: &'a Actos,
    inner: SearchBuilder<'a>,
}

impl<'a> BlockingSearchBuilder<'a> {
    /// Filters results by entity kind.
    pub fn kind(mut self, kind: SearchKind) -> Self {
        self.inner = self.inner.kind(kind);
        self
    }

    /// Specifies fields to project on search results.
    pub fn fields(mut self, fields: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.inner = self.inner.fields(fields);
        self
    }

    /// Adds a single field to the projection.
    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.inner = self.inner.field(field);
        self
    }

    /// Limits search results per page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.inner = self.inner.limit(limit);
        self
    }

    /// Sets the pagination cursor.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.inner = self.inner.cursor(cursor);
        self
    }

    /// Dispatches the search request and returns a single page of results.
    pub fn send(self) -> Result<Page<Post>> {
        self.client.block_on(self.inner.send())
    }

    /// Collects search results across all pages synchronously.
    pub fn collect(self) -> Result<Vec<Post>> {
        let stream = self.inner.stream();
        self.client.block_on(async move {
            let mut results = Vec::new();
            let mut pinned = std::pin::pin!(stream);
            while let Some(item) = pinned.next().await {
                results.push(item?);
            }
            Ok(results)
        })
    }
}

/// Synchronous feed client.
pub struct BlockingFeed {
    client: Actos,
}

impl BlockingFeed {
    /// Starts building a query for the discovery feed synchronously.
    pub fn list<'a>(&'a self) -> BlockingFeedBuilder<'a> {
        let inner = self.client.inner.feed().list();
        BlockingFeedBuilder {
            client: &self.client,
            inner,
        }
    }

    /// Starts building a query for the following feed synchronously.
    pub fn following<'a>(&'a self) -> BlockingFollowingFeedBuilder<'a> {
        let inner = self.client.inner.feed().following();
        BlockingFollowingFeedBuilder {
            client: &self.client,
            inner,
        }
    }
}

/// Builder for querying feed synchronously.
#[must_use = "builders do nothing until .send() or .collect() is called"]
pub struct BlockingFeedBuilder<'a> {
    client: &'a Actos,
    inner: FeedBuilder<'a>,
}

impl<'a> BlockingFeedBuilder<'a> {
    /// Sets the ranking sort order.
    pub fn sort(mut self, sort: Sort) -> Self {
        self.inner = self.inner.sort(sort);
        self
    }

    /// Sets the time window.
    pub fn window(mut self, window: FeedWindow) -> Self {
        self.inner = self.inner.window(window);
        self
    }

    /// Specifies fields to project on feed posts.
    pub fn fields(mut self, fields: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.inner = self.inner.fields(fields);
        self
    }

    /// Adds a single field to the projection.
    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.inner = self.inner.field(field);
        self
    }

    /// Limits feed posts per page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.inner = self.inner.limit(limit);
        self
    }

    /// Sets the pagination cursor.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.inner = self.inner.cursor(cursor);
        self
    }

    /// Filters feed posts to actors of a given type.
    pub fn actor_type(mut self, actor_type: impl Into<String>) -> Self {
        self.inner = self.inner.actor_type(actor_type);
        self
    }

    /// Dispatches the request and returns a single page of feed posts.
    pub fn send(self) -> Result<Page<Post>> {
        self.client.block_on(self.inner.send())
    }

    /// Collects feed posts across all pages synchronously.
    pub fn collect(self) -> Result<Vec<Post>> {
        let stream = self.inner.stream();
        self.client.block_on(async move {
            let mut results = Vec::new();
            let mut pinned = std::pin::pin!(stream);
            while let Some(item) = pinned.next().await {
                results.push(item?);
            }
            Ok(results)
        })
    }
}

/// Builder for querying following feed synchronously.
#[must_use = "builders do nothing until .send() or .collect() is called"]
pub struct BlockingFollowingFeedBuilder<'a> {
    client: &'a Actos,
    inner: FollowingFeedBuilder<'a>,
}

impl<'a> BlockingFollowingFeedBuilder<'a> {
    /// Sets the ranking sort order.
    pub fn sort(mut self, sort: Sort) -> Self {
        self.inner = self.inner.sort(sort);
        self
    }

    /// Sets the time window.
    pub fn window(mut self, window: FeedWindow) -> Self {
        self.inner = self.inner.window(window);
        self
    }

    /// Specifies fields to project on following posts.
    pub fn fields(mut self, fields: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.inner = self.inner.fields(fields);
        self
    }

    /// Adds a single field to the projection.
    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.inner = self.inner.field(field);
        self
    }

    /// Limits following posts per page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.inner = self.inner.limit(limit);
        self
    }

    /// Sets the pagination cursor.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.inner = self.inner.cursor(cursor);
        self
    }

    /// Filters following posts to actors of a given type.
    pub fn actor_type(mut self, actor_type: impl Into<String>) -> Self {
        self.inner = self.inner.actor_type(actor_type);
        self
    }

    /// Dispatches the request and returns a single page of following posts.
    pub fn send(self) -> Result<Page<Post>> {
        self.client.block_on(self.inner.send())
    }

    /// Collects following posts across all pages synchronously.
    pub fn collect(self) -> Result<Vec<Post>> {
        let stream = self.inner.stream();
        self.client.block_on(async move {
            let mut results = Vec::new();
            let mut pinned = std::pin::pin!(stream);
            while let Some(item) = pinned.next().await {
                results.push(item?);
            }
            Ok(results)
        })
    }
}

/// Synchronous notification inbox client.
pub struct BlockingInbox {
    client: Actos,
}

impl BlockingInbox {
    /// Starts building a query to list inbox notifications synchronously.
    pub fn list<'a>(&'a self) -> BlockingInboxListBuilder<'a> {
        let inner = self.client.inner.inbox().list();
        BlockingInboxListBuilder {
            client: &self.client,
            inner,
        }
    }

    /// Marks a single notification as read synchronously.
    pub fn read(&self, notification_id: impl Into<String>) -> Result<()> {
        self.client
            .block_on(self.client.inner.inbox().read(notification_id))
    }

    /// Bulk-marks notifications as read synchronously.
    pub fn read_all(&self, up_to_cursor: Option<&str>) -> Result<MarkAllReadResponse> {
        self.client
            .block_on(self.client.inner.inbox().read_all(up_to_cursor))
    }

    /// Returns the caller's total unread notification count synchronously.
    pub fn unread_count(&self) -> Result<i64> {
        self.client
            .block_on(self.client.inner.inbox().unread_count())
    }
}

/// Builder for querying inbox notifications synchronously.
#[must_use = "builders do nothing until .send() or .collect() is called"]
pub struct BlockingInboxListBuilder<'a> {
    client: &'a Actos,
    inner: InboxListBuilder<'a>,
}

impl<'a> BlockingInboxListBuilder<'a> {
    /// Filters the page to unread notifications only.
    pub fn unread(mut self, unread: bool) -> Self {
        self.inner = self.inner.unread(unread);
        self
    }

    /// Limits notifications per page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.inner = self.inner.limit(limit);
        self
    }

    /// Sets the pagination cursor.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.inner = self.inner.cursor(cursor);
        self
    }

    /// Dispatches the request and returns a single page of notifications.
    pub fn send(self) -> Result<Page<NotificationSummary>> {
        self.client.block_on(self.inner.send())
    }

    /// Collects inbox notifications across all pages synchronously.
    pub fn collect(self) -> Result<Vec<NotificationSummary>> {
        let stream = self.inner.stream();
        self.client.block_on(async move {
            let mut results = Vec::new();
            let mut pinned = std::pin::pin!(stream);
            while let Some(item) = pinned.next().await {
                results.push(item?);
            }
            Ok(results)
        })
    }
}

/// Synchronous voting client.
pub struct BlockingVotes {
    client: Actos,
}

impl BlockingVotes {
    /// Sets or updates a vote on content synchronously.
    pub fn set(&self, content_id: &str, value: i16) -> Result<VoteResponse> {
        self.client
            .block_on(self.client.inner.votes().set(content_id, value))
    }

    /// Upvotes content (+1) synchronously.
    pub fn up(&self, content_id: &str) -> Result<VoteResponse> {
        self.client
            .block_on(self.client.inner.votes().up(content_id))
    }

    /// Downvotes content (-1) synchronously.
    pub fn down(&self, content_id: &str) -> Result<VoteResponse> {
        self.client
            .block_on(self.client.inner.votes().down(content_id))
    }

    /// Clears an active vote on content (0) synchronously.
    pub fn clear(&self, content_id: &str) -> Result<VoteResponse> {
        self.client
            .block_on(self.client.inner.votes().clear(content_id))
    }

    /// Lists votes cast by the authenticated user synchronously.
    pub fn list(&self, content_ids: Option<&[&str]>) -> Result<BTreeMap<String, i16>> {
        self.client
            .block_on(self.client.inner.votes().list(content_ids))
    }
}

/// Synchronous bookmarks and saves client.
pub struct BlockingSaves {
    client: Actos,
}

impl BlockingSaves {
    /// Adds content to saved bookmarks synchronously.
    pub fn add(&self, content_id: &str) -> Result<()> {
        self.client
            .block_on(self.client.inner.saves().add(content_id))
    }

    /// Removes content from saved bookmarks synchronously.
    pub fn remove(&self, content_id: &str) -> Result<()> {
        self.client
            .block_on(self.client.inner.saves().remove(content_id))
    }

    /// Starts building a query for saved bookmarks synchronously.
    pub fn list<'a>(&'a self) -> BlockingListSavesBuilder<'a> {
        let inner = self.client.inner.saves().list();
        BlockingListSavesBuilder {
            client: &self.client,
            inner,
        }
    }
}

/// Builder for querying saved bookmarks synchronously.
#[must_use = "builders do nothing until .send() or .collect() is called"]
pub struct BlockingListSavesBuilder<'a> {
    client: &'a Actos,
    inner: ListSavesBuilder<'a>,
}

impl<'a> BlockingListSavesBuilder<'a> {
    /// Specifies fields to project on saved posts.
    pub fn fields(mut self, fields: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.inner = self.inner.fields(fields);
        self
    }

    /// Adds a single field to the projection.
    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.inner = self.inner.field(field);
        self
    }

    /// Limits saved posts per page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.inner = self.inner.limit(limit);
        self
    }

    /// Sets the pagination cursor.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.inner = self.inner.cursor(cursor);
        self
    }

    /// Dispatches the request and returns a single page of saved posts.
    pub fn send(self) -> Result<Page<Post>> {
        self.client.block_on(self.inner.send())
    }

    /// Collects saved posts across all pages synchronously.
    pub fn collect(self) -> Result<Vec<Post>> {
        let stream = self.inner.stream();
        self.client.block_on(async move {
            let mut results = Vec::new();
            let mut pinned = std::pin::pin!(stream);
            while let Some(item) = pinned.next().await {
                results.push(item?);
            }
            Ok(results)
        })
    }
}

/// Synchronous uploads client.
pub struct BlockingUploads {
    client: Actos,
}

impl BlockingUploads {
    /// Starts building an upload request synchronously.
    pub fn create<'a>(
        &'a self,
        source: impl Into<UploadSource>,
    ) -> BlockingCreateUploadBuilder<'a> {
        let inner = self.client.inner.uploads().create(source);
        BlockingCreateUploadBuilder {
            client: &self.client,
            inner,
        }
    }

    /// Deletes an upload record synchronously.
    pub fn delete(&self, id: &str) -> Result<()> {
        self.client.block_on(self.client.inner.uploads().delete(id))
    }
}

/// Builder for uploading a file synchronously.
#[must_use = "builders do nothing until .send() is called"]
pub struct BlockingCreateUploadBuilder<'a> {
    client: &'a Actos,
    inner: CreateUploadBuilder<'a>,
}

impl<'a> BlockingCreateUploadBuilder<'a> {
    /// Sets an explicit filename.
    pub fn filename(mut self, filename: impl Into<String>) -> Self {
        self.inner = self.inner.filename(filename);
        self
    }

    /// Sets an explicit MIME type.
    pub fn mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.inner = self.inner.mime_type(mime_type);
        self
    }

    /// Dispatches the multipart upload request synchronously.
    pub fn send(self) -> Result<UploadResponse> {
        self.client.block_on(self.inner.send())
    }
}

/// Synchronous reports client.
pub struct BlockingReports {
    client: Actos,
}

impl BlockingReports {
    /// Submits a moderation report synchronously.
    pub fn create(
        &self,
        target_type: impl Into<String>,
        target_id: impl Into<String>,
        reason: impl Into<String>,
    ) -> Result<ReportSummary> {
        self.client.block_on(
            self.client
                .inner
                .reports()
                .create(target_type, target_id, reason),
        )
    }
}

/// Synchronous admin client.
pub struct BlockingAdmin {
    client: Actos,
}

impl BlockingAdmin {
    /// Accesses report management endpoints synchronously.
    pub fn reports(&self) -> BlockingAdminReports {
        BlockingAdminReports {
            client: self.client.clone(),
        }
    }

    /// Accesses content moderation endpoints synchronously.
    pub fn contents(&self) -> BlockingAdminContents {
        BlockingAdminContents {
            client: self.client.clone(),
        }
    }

    /// Accesses account ban management endpoints synchronously.
    pub fn bans(&self) -> BlockingAdminBans {
        BlockingAdminBans {
            client: self.client.clone(),
        }
    }

    /// Accesses role management endpoints synchronously.
    pub fn roles(&self) -> BlockingAdminRoles {
        BlockingAdminRoles {
            client: self.client.clone(),
        }
    }

    /// Accesses admin audit trail actions endpoints synchronously.
    pub fn actions(&self) -> BlockingAdminActions {
        BlockingAdminActions {
            client: self.client.clone(),
        }
    }
}

/// Synchronous reports moderation sub-client.
pub struct BlockingAdminReports {
    client: Actos,
}

impl BlockingAdminReports {
    /// Starts building a request to list reports synchronously.
    pub fn list<'a>(&'a self) -> BlockingListAdminReportsBuilder<'a> {
        let inner = self.client.inner.admin().reports().list();
        BlockingListAdminReportsBuilder {
            client: &self.client,
            inner,
        }
    }

    /// Starts building a request to update a report synchronously.
    pub fn update<'a>(
        &'a self,
        id: impl Into<String>,
        status: impl Into<String>,
    ) -> BlockingUpdateAdminReportBuilder<'a> {
        let inner = self.client.inner.admin().reports().update(id, status);
        BlockingUpdateAdminReportBuilder {
            client: &self.client,
            inner,
        }
    }
}

/// Builder for listing moderation reports synchronously.
#[must_use = "builders do nothing until .send() or .collect() is called"]
pub struct BlockingListAdminReportsBuilder<'a> {
    client: &'a Actos,
    inner: ListAdminReportsBuilder<'a>,
}

impl<'a> BlockingListAdminReportsBuilder<'a> {
    /// Filters reports by status.
    pub fn status(mut self, status: impl Into<String>) -> Self {
        self.inner = self.inner.status(status);
        self
    }

    /// Limits reports per page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.inner = self.inner.limit(limit);
        self
    }

    /// Sets the pagination cursor.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.inner = self.inner.cursor(cursor);
        self
    }

    /// Dispatches the request and returns a single page of reports.
    pub fn send(self) -> Result<Page<ReportSummary>> {
        self.client.block_on(self.inner.send())
    }

    /// Collects reports across all pages synchronously.
    pub fn collect(self) -> Result<Vec<ReportSummary>> {
        let stream = self.inner.stream();
        self.client.block_on(async move {
            let mut results = Vec::new();
            let mut pinned = std::pin::pin!(stream);
            while let Some(item) = pinned.next().await {
                results.push(item?);
            }
            Ok(results)
        })
    }
}

/// Builder for updating a moderation report synchronously.
#[must_use = "builders do nothing until .send() is called"]
pub struct BlockingUpdateAdminReportBuilder<'a> {
    client: &'a Actos,
    inner: UpdateAdminReportBuilder<'a>,
}

impl<'a> BlockingUpdateAdminReportBuilder<'a> {
    /// Adds moderator resolution notes.
    pub fn notes(mut self, notes: impl Into<String>) -> Self {
        self.inner = self.inner.notes(notes);
        self
    }

    /// Dispatches the update request synchronously.
    pub fn send(self) -> Result<ReportSummary> {
        self.client.block_on(self.inner.send())
    }
}

/// Synchronous content moderation sub-client.
pub struct BlockingAdminContents {
    client: Actos,
}

impl BlockingAdminContents {
    /// Hard-deletes a content item synchronously.
    pub fn delete(&self, id: &str, reason: impl Into<String>) -> Result<()> {
        self.client
            .block_on(self.client.inner.admin().contents().delete(id, reason))
    }
}

/// Synchronous account ban sub-client.
pub struct BlockingAdminBans {
    client: Actos,
}

impl BlockingAdminBans {
    /// Starts building a ban creation request synchronously.
    pub fn create<'a>(
        &'a self,
        username: impl Into<String>,
        reason: impl Into<String>,
    ) -> BlockingCreateBanBuilder<'a> {
        let inner = self.client.inner.admin().bans().create(username, reason);
        BlockingCreateBanBuilder {
            client: &self.client,
            inner,
        }
    }

    /// Removes an account ban synchronously.
    pub fn remove(&self, username: &str) -> Result<()> {
        self.client
            .block_on(self.client.inner.admin().bans().remove(username))
    }
}

/// Builder for creating an account ban synchronously.
#[must_use = "builders do nothing until .send() is called"]
pub struct BlockingCreateBanBuilder<'a> {
    client: &'a Actos,
    inner: CreateBanBuilder<'a>,
}

impl<'a> BlockingCreateBanBuilder<'a> {
    /// Sets the ban expiration timestamp.
    pub fn expires_at(mut self, expires_at: impl Into<String>) -> Self {
        self.inner = self.inner.expires_at(expires_at);
        self
    }

    /// Dispatches the ban creation request synchronously.
    pub fn send(self) -> Result<BanSummary> {
        self.client.block_on(self.inner.send())
    }
}

/// Synchronous role management sub-client.
pub struct BlockingAdminRoles {
    client: Actos,
}

impl BlockingAdminRoles {
    /// Assigns or revokes administrative roles synchronously.
    pub fn set(&self, username: impl Into<String>, role: Option<&str>) -> Result<()> {
        self.client
            .block_on(self.client.inner.admin().roles().set(username, role))
    }
}

/// Synchronous admin audit trail actions sub-client.
pub struct BlockingAdminActions {
    client: Actos,
}

impl BlockingAdminActions {
    /// Starts building a query for admin actions synchronously.
    pub fn list<'a>(&'a self) -> BlockingListAdminActionsBuilder<'a> {
        let inner = self.client.inner.admin().actions().list();
        BlockingListAdminActionsBuilder {
            client: &self.client,
            inner,
        }
    }
}

/// Builder for querying admin actions synchronously.
#[must_use = "builders do nothing until .send() or .collect() is called"]
pub struct BlockingListAdminActionsBuilder<'a> {
    client: &'a Actos,
    inner: ListAdminActionsBuilder<'a>,
}

impl<'a> BlockingListAdminActionsBuilder<'a> {
    /// Limits actions per page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.inner = self.inner.limit(limit);
        self
    }

    /// Sets the pagination cursor.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.inner = self.inner.cursor(cursor);
        self
    }

    /// Dispatches the request and returns a single page of actions.
    pub fn send(self) -> Result<Page<AdminActionSummary>> {
        self.client.block_on(self.inner.send())
    }

    /// Collects actions across all pages synchronously.
    pub fn collect(self) -> Result<Vec<AdminActionSummary>> {
        let stream = self.inner.stream();
        self.client.block_on(async move {
            let mut results = Vec::new();
            let mut pinned = std::pin::pin!(stream);
            while let Some(item) = pinned.next().await {
                results.push(item?);
            }
            Ok(results)
        })
    }
}

/// Synchronous platform metadata client.
pub struct BlockingMeta {
    client: Actos,
}

impl BlockingMeta {
    /// Checks service liveness synchronously.
    pub fn health(&self) -> Result<serde_json::Value> {
        self.client.block_on(self.client.inner.meta().health())
    }

    /// Checks deep service readiness synchronously.
    pub fn ready(&self) -> Result<serde_json::Value> {
        self.client.block_on(self.client.inner.meta().ready())
    }

    /// Retrieves version metadata synchronously.
    pub fn version(&self) -> Result<MetaVersion> {
        self.client.block_on(self.client.inner.meta().version())
    }

    /// Retrieves the OpenAPI specification synchronously.
    pub fn openapi(&self) -> Result<serde_json::Value> {
        self.client.block_on(self.client.inner.meta().openapi())
    }
}
