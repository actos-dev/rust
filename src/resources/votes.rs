//! Voting on contents (upvote, downvote, clear).

use std::collections::BTreeMap;

pub use actos_types::interaction::{VoteMapResponse, VoteResponse};
use reqwest::Method;

use crate::error::Result;
use crate::transport::Transport;

/// Client for `/contents/{id}/vote` and `/me/votes` API endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Votes<'a> {
    pub(crate) transport: &'a Transport,
}

impl<'a> Votes<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }

    /// Sets the vote on a content item (post or comment) via `PUT /contents/{id}/vote`. Requires authentication.
    ///
    /// Accepts:
    /// - `1` for upvote
    /// - `-1` for downvote
    /// - `0` for clearing / withdrawing the vote
    ///
    /// Returns the updated vote counters and user vote state in [`VoteResponse`].
    ///
    /// This operation is **idempotent**: repeating the call with the same value succeeds without error.
    pub async fn set(&self, content_id: &str, value: i16) -> Result<VoteResponse> {
        let path = format!("/contents/{content_id}/vote");
        let req_body = serde_json::json!({
            "value": value,
        });
        let builder = self.transport.request(Method::PUT, &path)?.json(&req_body);
        self.transport.execute_json(builder).await
    }

    /// Convenience helper to upvote a content item (+1). Requires authentication.
    pub async fn up(&self, content_id: &str) -> Result<VoteResponse> {
        self.set(content_id, 1).await
    }

    /// Convenience helper to downvote a content item (-1). Requires authentication.
    pub async fn down(&self, content_id: &str) -> Result<VoteResponse> {
        self.set(content_id, -1).await
    }

    /// Convenience helper to clear / withdraw a vote on a content item (0). Requires authentication.
    pub async fn clear(&self, content_id: &str) -> Result<VoteResponse> {
        self.set(content_id, 0).await
    }

    /// Lists the authenticated actor's current votes via `GET /me/votes`. Requires authentication.
    ///
    /// Optionally filters the result to a specific slice of content IDs.
    /// Returns a map of `content_id -> vote_value` (`1` or `-1`). Unvoted contents are omitted from the map.
    pub async fn list(&self, content_ids: Option<&[&str]>) -> Result<BTreeMap<String, i16>> {
        let mut builder = self.transport.request(Method::GET, "/me/votes")?;
        if let Some(ids) = content_ids
            && !ids.is_empty()
        {
            builder = builder.query(&[("content_ids", ids.join(","))]);
        }

        let res: VoteMapResponse = self.transport.execute_json(builder).await?;
        Ok(res.votes)
    }
}
