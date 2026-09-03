//! Administrative and moderation endpoints.

use crate::transport::Transport;

/// Client for `/admin/*` API endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Admin<'a> {
    #[allow(dead_code)]
    pub(crate) transport: &'a Transport,
}

impl<'a> Admin<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }
}
