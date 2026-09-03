//! Authentication and identity endpoints.

use crate::transport::Transport;

/// Client for `/auth/*` API endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Auth<'a> {
    #[allow(dead_code)]
    pub(crate) transport: &'a Transport,
}

impl<'a> Auth<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }
}
