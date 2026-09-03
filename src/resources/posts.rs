//! Post creation, retrieval, and management endpoints.

use crate::transport::Transport;

/// Client for `/posts/*` API endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Posts<'a> {
    #[allow(dead_code)]
    pub(crate) transport: &'a Transport,
}

impl<'a> Posts<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }
}
