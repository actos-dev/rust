//! Actor profile, directory, and follow endpoints.

use crate::transport::Transport;

/// Client for `/actors/*` API endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Actors<'a> {
    #[allow(dead_code)]
    pub(crate) transport: &'a Transport,
}

impl<'a> Actors<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }
}
