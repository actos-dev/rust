//! Unified search endpoints.

use crate::transport::Transport;

/// Client for `/search` API endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Search<'a> {
    #[allow(dead_code)]
    pub(crate) transport: &'a Transport,
}

impl<'a> Search<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }
}
