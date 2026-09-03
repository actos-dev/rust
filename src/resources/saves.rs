//! Bookmarks and saved contents endpoints.

use crate::transport::Transport;

/// Client for `/contents/{id}/save` and `/me/saves` API endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Saves<'a> {
    #[allow(dead_code)]
    pub(crate) transport: &'a Transport,
}

impl<'a> Saves<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }
}
