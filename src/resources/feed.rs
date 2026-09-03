//! Timeline and chronological feed endpoints.

use crate::transport::Transport;

/// Client for `/feed/*` API endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Feed<'a> {
    #[allow(dead_code)]
    pub(crate) transport: &'a Transport,
}

impl<'a> Feed<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }
}
