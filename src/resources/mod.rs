//! API resource clients exposed through the [`crate::Actos`] facade.

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

pub use actors::Actors;
pub use admin::Admin;
pub use auth::Auth;
pub use comments::Comments;
pub use feed::Feed;
pub use meta::Meta;
pub use posts::Posts;
pub use reports::Reports;
pub use saves::Saves;
pub use search::Search;
pub use tags::Tags;
pub use uploads::Uploads;
pub use votes::Votes;
