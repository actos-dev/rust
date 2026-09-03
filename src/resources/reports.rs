//! Moderation report submission endpoints.

use crate::transport::Transport;

/// Client for `/reports` API endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Reports<'a> {
    #[allow(dead_code)]
    pub(crate) transport: &'a Transport,
}

impl<'a> Reports<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }
}
