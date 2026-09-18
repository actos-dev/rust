//! API resource clients exposed through the [`crate::Actos`] facade.

use std::fmt;

pub mod actors;
pub mod admin;
pub mod auth;
pub mod comments;
pub mod communities;
pub mod feed;
pub mod inbox;
pub mod meta;
pub mod posts;
pub mod reports;
pub mod saves;
pub mod search;
pub mod tags;
pub mod votes;

use crate::error::{Error, Result};

/// A binary file travelling with a post, a comment, or an avatar.
///
/// Actos has no standalone image host: an image is only ever created as a
/// side effect of the content (or avatar) request that carries it. This type
/// is the SDK-side carrier for that payload — raw bytes plus the optional
/// metadata used in the multipart `Content-Disposition`/`Content-Type`
/// headers.
///
/// The server detects the real image type from its magic bytes; the declared
/// [`content_type`](FileUpload::content_type) is advisory only.
#[derive(Debug, Clone)]
pub struct FileUpload {
    pub(crate) data: Vec<u8>,
    pub(crate) filename: Option<String>,
    pub(crate) content_type: Option<String>,
}

impl FileUpload {
    /// Creates a file from an in-memory byte buffer.
    #[must_use]
    pub fn new(data: impl Into<Vec<u8>>) -> Self {
        Self {
            data: data.into(),
            filename: None,
            content_type: None,
        }
    }

    /// Creates a file from a filesystem path, reading it into memory.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the file cannot be read.
    pub fn from_path(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let path = path.as_ref();
        let data = std::fs::read(path)?;
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(ToString::to_string);
        Ok(Self {
            data,
            filename,
            content_type: None,
        })
    }

    /// Sets the filename reported to the server in the multipart part.
    #[must_use]
    pub fn filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = Some(filename.into());
        self
    }

    /// Sets the advisory MIME type of the file (e.g. `"image/png"`).
    #[must_use]
    pub fn content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }
}

impl From<Vec<u8>> for FileUpload {
    fn from(bytes: Vec<u8>) -> Self {
        Self::new(bytes)
    }
}

impl From<&[u8]> for FileUpload {
    fn from(bytes: &[u8]) -> Self {
        Self::new(bytes.to_vec())
    }
}

impl<const N: usize> From<&[u8; N]> for FileUpload {
    fn from(bytes: &[u8; N]) -> Self {
        Self::new(bytes.to_vec())
    }
}

/// Builds the `multipart/form-data` body used by `POST /posts` and
/// `POST /posts/{id}/comments`: one JSON part named `payload` plus one or
/// more file parts named `files`.
pub(crate) fn build_content_form(
    payload: serde_json::Value,
    files: Vec<FileUpload>,
) -> Result<reqwest::multipart::Form> {
    let json = serde_json::to_string(&payload).map_err(|e| Error::Config(e.to_string()))?;
    let payload_part = reqwest::multipart::Part::text(json)
        .mime_str("application/json")
        .map_err(|e| Error::Config(e.to_string()))?;
    let mut form = reqwest::multipart::Form::new().part("payload", payload_part);

    for (index, file) in files.into_iter().enumerate() {
        let filename = file.filename.unwrap_or_else(|| format!("file-{index}"));
        let mut part = reqwest::multipart::Part::bytes(file.data).file_name(filename);
        if let Some(content_type) = file.content_type {
            part = part
                .mime_str(&content_type)
                .map_err(|e| Error::Config(e.to_string()))?;
        }
        form = form.part("files", part);
    }

    Ok(form)
}

/// Builds the `multipart/form-data` body used by `POST /actors/me/avatar`:
/// a single file part named `file`.
pub(crate) fn build_avatar_form(file: FileUpload) -> Result<reqwest::multipart::Form> {
    let filename = file.filename.unwrap_or_else(|| "avatar".to_string());
    let mut part = reqwest::multipart::Part::bytes(file.data).file_name(filename);
    if let Some(content_type) = file.content_type {
        part = part
            .mime_str(&content_type)
            .map_err(|e| Error::Config(e.to_string()))?;
    }
    Ok(reqwest::multipart::Form::new().part("file", part))
}

/// Sorting algorithm for posts and feeds (§Faz 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    /// Sort items by popularity / activity ranking.
    Hot,
    /// Sort items by creation time descending (newest first).
    New,
    /// Sort items by overall score descending.
    Top,
}

impl fmt::Display for Sort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hot => write!(f, "hot"),
            Self::New => write!(f, "new"),
            Self::Top => write!(f, "top"),
        }
    }
}

/// Time window for feed queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedWindow {
    /// Query items created within the last 24 hours.
    Day,
    /// Query items created within the last 7 days.
    Week,
    /// Query items created within the last 30 days.
    Month,
    /// Query items across all time.
    All,
}

impl fmt::Display for FeedWindow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Day => write!(f, "day"),
            Self::Week => write!(f, "week"),
            Self::Month => write!(f, "month"),
            Self::All => write!(f, "all"),
        }
    }
}

/// Entity type filter for unified search queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchKind {
    /// Search post entries.
    Post,
    /// Search comment entries.
    Comment,
    /// Search actor accounts.
    Actor,
}

impl fmt::Display for SearchKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Post => write!(f, "post"),
            Self::Comment => write!(f, "comment"),
            Self::Actor => write!(f, "actor"),
        }
    }
}

pub use actors::{
    ActorCommentsBuilder, ActorPostsBuilder, Actors, CommentSummary, DeleteMeBuilder, FieldUpdate,
    FollowersBuilder, FollowingBuilder, ListActorsBuilder, Post, UpdateMeBuilder,
};
pub use actos_types::actor::AvatarResponse;
pub use actos_types::auth::PermissionSummary;
pub use actos_types::content::{
    CommentDetailResponse, CommentNodeResponse, CommentThreadResponse, CommunityRefSummary,
    CrossPostPreviewSummary,
};
pub use actos_types::notification::{InboxResponse, MarkAllReadResponse, NotificationSummary};
pub use actos_types::upload::UploadResponse;
pub use admin::{
    Admin, AdminActionListResponse, AdminActionSummary, AdminActions, AdminBans, AdminContents,
    AdminPermissions, AdminReports, BanSummary, CreateBanBuilder, ListAdminActionsBuilder,
    ListAdminReportsBuilder, ReportListResponse, UpdateAdminReportBuilder,
};
pub use auth::{Auth, CreateKeyBuilder, RegisterBuilder};
pub use comments::{Comments, CreateCommentBuilder, ListCommentsBuilder};
pub use communities::{
    Communities, CommunityMembersBuilder, CommunityPostsBuilder, CreateCommunityBuilder,
    ListApplicationsBuilder, ListCommunitiesBuilder, ListInvitationsBuilder,
    UpdateCommunityBuilder,
};
pub use feed::{Feed, FeedBuilder, FollowingFeedBuilder};
pub use inbox::{Inbox, InboxListBuilder};
pub use meta::{Meta, MetaVersion};
pub use posts::{CreatePostBuilder, GetPostBuilder, Posts, UpdatePostBuilder};
pub use reports::{ReportSummary, Reports};
pub use saves::{ListSavesBuilder, SaveListResponse, Saves};
pub use search::{Search, SearchBuilder};
pub use tags::{ListTagsBuilder, TagMatch, TagPostsBuilder, TagSummary, Tags};
pub use votes::{VoteMapResponse, VoteResponse, Votes};

pub use actos_types::community::{
    ApplicationSummary, CommunityMemberSummary, CommunitySummary, InvitationSummary,
};
