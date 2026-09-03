//! Tag discovery and tagged post listing endpoints.

use crate::transport::Transport;

/// Client for `/tags/*` API endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Tags<'a> {
    #[allow(dead_code)]
    pub(crate) transport: &'a Transport,
}

impl<'a> Tags<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }
}
