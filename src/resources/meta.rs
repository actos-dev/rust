//! Platform metadata, health, and OpenAPI specification endpoints.

use crate::transport::Transport;

/// Client for platform metadata endpoints (`/health`, `/version`, `/openapi.json`).
#[derive(Debug, Clone, Copy)]
pub struct Meta<'a> {
    #[allow(dead_code)]
    pub(crate) transport: &'a Transport,
}

impl<'a> Meta<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }
}
