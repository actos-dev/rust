//! Media and attachment upload endpoints.

use crate::transport::Transport;

/// Client for `/uploads/*` API endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Uploads<'a> {
    #[allow(dead_code)]
    pub(crate) transport: &'a Transport,
}

impl<'a> Uploads<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }
}
