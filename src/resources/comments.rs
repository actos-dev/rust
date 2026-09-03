//! Threaded comments endpoints.

use crate::transport::Transport;

/// Client for comment endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Comments<'a> {
    #[allow(dead_code)]
    pub(crate) transport: &'a Transport,
}

impl<'a> Comments<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }
}
