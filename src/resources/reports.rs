//! User content reporting endpoints.

pub use actos_types::moderation::ReportSummary;
use reqwest::Method;

use crate::error::Result;
use crate::transport::Transport;

/// Client for `/reports` API endpoint.
#[derive(Debug, Clone, Copy)]
pub struct Reports<'a> {
    pub(crate) transport: &'a Transport,
}

impl<'a> Reports<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }

    /// Submits a new moderation report against a post or comment via `POST /reports`. Requires authentication.
    ///
    /// - `target_type`: `"post"` or `"comment"`
    /// - `target_id`: The ID of the reported content
    /// - `reason`: Description of why the content violates guidelines
    pub async fn create(
        &self,
        target_type: impl Into<String>,
        target_id: impl Into<String>,
        reason: impl Into<String>,
    ) -> Result<ReportSummary> {
        let req_body = serde_json::json!({
            "target_type": target_type.into(),
            "target_id": target_id.into(),
            "reason": reason.into(),
        });
        let builder = self
            .transport
            .request(Method::POST, "/reports")?
            .json(&req_body);
        self.transport.execute_json(builder).await
    }
}
