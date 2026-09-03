//! API resource clients exposed through the [`crate::Actos`] facade.

use std::fmt;

pub mod actors;
pub mod admin;
pub mod auth;
pub mod comments;
pub mod feed;
pub mod meta;
pub mod posts;
pub mod reports;
pub mod saves;
pub mod search;
pub mod tags;
pub mod uploads;
pub mod votes;

/// Sorting algorithm for posts and feeds (§Faz 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    Hot,
    New,
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
    Day,
    Week,
    Month,
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
    Post,
    Comment,
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
    ActorCommentsBuilder, ActorPostsBuilder, Actors, CommentSummary, DeleteMeBuilder,
    FollowersBuilder, FollowingBuilder, ListActorsBuilder, Post, UpdateMeBuilder,
};
pub use actos_types::content::{CommentDetailResponse, CommentNodeResponse, CommentThreadResponse};
pub use admin::Admin;
pub use auth::{Auth, CreateKeyBuilder, RegisterBuilder};
pub use comments::{Comments, CreateCommentBuilder, ListCommentsBuilder};
pub use feed::{Feed, FeedBuilder, FollowingFeedBuilder};
pub use meta::Meta;
pub use posts::{CreatePostBuilder, GetPostBuilder, Posts, UpdatePostBuilder};
pub use reports::Reports;
pub use saves::{ListSavesBuilder, SaveListResponse, Saves};
pub use search::{Search, SearchBuilder};
pub use tags::{ListTagsBuilder, TagMatch, TagPostsBuilder, TagSummary, Tags};
pub use uploads::Uploads;
pub use votes::{VoteMapResponse, VoteResponse, Votes};
