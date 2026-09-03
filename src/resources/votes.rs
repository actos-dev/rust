//! Voting on contents (upvote, downvote, clear).

use crate::transport::Transport;

/// Client for `/contents/{id}/vote` and `/me/votes` API endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Votes<'a> {
    #[allow(dead_code)]
    pub(crate) transport: &'a Transport,
}

impl<'a> Votes<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }
}
