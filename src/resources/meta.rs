//! Platform metadata, health, and OpenAPI specification endpoints.

use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::VERSION;
use crate::error::Result;
use crate::transport::Transport;

/// Combined SDK and server version metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetaVersion {
    /// Actos Rust SDK package version.
    pub sdk: String,
    /// Live backend server version response payload.
    pub server: serde_json::Value,
}

/// Client for platform metadata endpoints (`/health`, `/version`, `/openapi.json`).
#[derive(Debug, Clone, Copy)]
pub struct Meta<'a> {
    pub(crate) transport: &'a Transport,
}

impl<'a> Meta<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }

    /// Checks the basic liveness of the platform server via `GET /health`.
    pub async fn health(&self) -> Result<serde_json::Value> {
        let builder = self.transport.request(Method::GET, "/health")?;
        self.transport.execute_json(builder).await
    }

    /// Checks deep readiness (database, cache, storage connectivity) via `GET /health/ready`.
    pub async fn ready(&self) -> Result<serde_json::Value> {
        let builder = self.transport.request(Method::GET, "/health/ready")?;
        self.transport.execute_json(builder).await
    }

    /// Retrieves version metadata for both this SDK and the remote server via `GET /version`.
    pub async fn version(&self) -> Result<MetaVersion> {
        let builder = self.transport.request(Method::GET, "/version")?;
        let server_version: serde_json::Value = self.transport.execute_json(builder).await?;
        Ok(MetaVersion {
            sdk: VERSION.to_string(),
            server: server_version,
        })
    }

    /// Fetches the live OpenAPI 3.1 schema specification via `GET /openapi.json`.
    pub async fn openapi(&self) -> Result<serde_json::Value> {
        let builder = self.transport.request(Method::GET, "/openapi.json")?;
        self.transport.execute_json(builder).await
    }
}
