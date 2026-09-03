//! Moderation and administration endpoints.

pub use actos_types::moderation::{
    AdminActionListResponse, AdminActionSummary, BanSummary, ReportListResponse, ReportSummary,
};
use reqwest::Method;

use crate::error::{Error, Result};
use crate::pagination::{Page, paginate_stream_with_cursor};
use crate::transport::Transport;

/// Root client for `/admin/*` moderation and platform management endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Admin<'a> {
    pub(crate) transport: &'a Transport,
}

impl<'a> Admin<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }

    /// Access moderation reports management endpoints (`/admin/reports`).
    pub fn reports(&self) -> AdminReports<'a> {
        AdminReports {
            transport: self.transport,
        }
    }

    /// Access content moderation endpoints (`/admin/contents`).
    pub fn contents(&self) -> AdminContents<'a> {
        AdminContents {
            transport: self.transport,
        }
    }

    /// Access account bans management endpoints (`/admin/bans`).
    pub fn bans(&self) -> AdminBans<'a> {
        AdminBans {
            transport: self.transport,
        }
    }

    /// Access role management endpoints (`/admin/roles`).
    pub fn roles(&self) -> AdminRoles<'a> {
        AdminRoles {
            transport: self.transport,
        }
    }

    /// Access admin audit log actions endpoints (`/admin/actions`).
    pub fn actions(&self) -> AdminActions<'a> {
        AdminActions {
            transport: self.transport,
        }
    }
}

/// Moderation reports management sub-client.
#[derive(Debug, Clone, Copy)]
pub struct AdminReports<'a> {
    transport: &'a Transport,
}

impl<'a> AdminReports<'a> {
    /// Starts building a query to list reports via `GET /admin/reports`. Requires moderator auth `[M]`.
    pub fn list(&self) -> ListAdminReportsBuilder<'a> {
        ListAdminReportsBuilder {
            transport: self.transport,
            status: None,
            limit: None,
            cursor: None,
        }
    }

    /// Convenience shortcut to stream reports across pages.
    pub fn stream(&self) -> impl futures_core::Stream<Item = Result<ReportSummary, Error>> + Send {
        self.list().stream()
    }

    /// Starts building a request to update a report's resolution status via `PATCH /admin/reports/{id}`. Requires moderator auth `[M]`.
    pub fn update(
        &self,
        id: impl Into<String>,
        status: impl Into<String>,
    ) -> UpdateAdminReportBuilder<'a> {
        UpdateAdminReportBuilder {
            transport: self.transport,
            id: id.into(),
            status: status.into(),
            notes: None,
        }
    }
}

/// Builder for listing moderation reports via `GET /admin/reports`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await or .stream() is called"]
pub struct ListAdminReportsBuilder<'a> {
    transport: &'a Transport,
    status: Option<String>,
    limit: Option<u32>,
    cursor: Option<String>,
}

impl<'a> ListAdminReportsBuilder<'a> {
    /// Filters reports by status (`"pending"`, `"resolved"`, or `"dismissed"`).
    pub fn status(mut self, status: impl Into<String>) -> Self {
        self.status = Some(status.into());
        self
    }

    /// Limits the maximum number of reports per page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the pagination cursor for fetching subsequent pages.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Dispatches the request and returns a single page of [`ReportSummary`].
    pub async fn send(self) -> Result<Page<ReportSummary>> {
        let mut builder = self.transport.request(Method::GET, "/admin/reports")?;
        if let Some(ref s) = self.status {
            builder = builder.query(&[("status", s.as_str())]);
        }
        if let Some(lim) = self.limit {
            builder = builder.query(&[("limit", lim.to_string())]);
        }
        if let Some(ref c) = self.cursor {
            builder = builder.query(&[("cursor", c.as_str())]);
        }

        let res: ReportListResponse = self.transport.execute_json(builder).await?;
        Ok(Page::new(res.reports, res.next_cursor))
    }

    /// Produces a stream that yields reports across pages.
    pub fn stream(self) -> impl futures_core::Stream<Item = Result<ReportSummary, Error>> + Send {
        let transport = self.transport.clone();
        let status = self.status;
        let limit = self.limit;

        paginate_stream_with_cursor(self.cursor, move |cursor| {
            let transport = transport.clone();
            let status = status.clone();
            async move {
                let mut builder = transport.request(Method::GET, "/admin/reports")?;
                if let Some(ref s) = status {
                    builder = builder.query(&[("status", s.as_str())]);
                }
                if let Some(lim) = limit {
                    builder = builder.query(&[("limit", lim.to_string())]);
                }
                if let Some(ref c) = cursor {
                    builder = builder.query(&[("cursor", c.as_str())]);
                }

                let res: ReportListResponse = transport.execute_json(builder).await?;
                Ok(Page::new(res.reports, res.next_cursor))
            }
        })
    }
}

/// Builder for updating a moderation report via `PATCH /admin/reports/{id}`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await is called"]
pub struct UpdateAdminReportBuilder<'a> {
    transport: &'a Transport,
    id: String,
    status: String,
    notes: Option<String>,
}

impl<'a> UpdateAdminReportBuilder<'a> {
    /// Adds moderator resolution notes.
    pub fn notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }

    /// Dispatches the update request and returns the updated [`ReportSummary`].
    pub async fn send(self) -> Result<ReportSummary> {
        let path = format!("/admin/reports/{}", self.id);
        let req_body = serde_json::json!({
            "status": self.status,
            "notes": self.notes,
        });
        let builder = self
            .transport
            .request(Method::PATCH, &path)?
            .json(&req_body);
        self.transport.execute_json(builder).await
    }
}

/// Content moderation sub-client.
#[derive(Debug, Clone, Copy)]
pub struct AdminContents<'a> {
    transport: &'a Transport,
}

impl<'a> AdminContents<'a> {
    /// Moderates (hard-deletes) a content item via `DELETE /admin/contents/{id}`. Requires moderator auth `[M]`.
    ///
    /// The `reason` parameter is required and logged to the admin audit trail.
    pub async fn delete(&self, id: &str, reason: impl Into<String>) -> Result<()> {
        let path = format!("/admin/contents/{id}");
        let req_body = serde_json::json!({
            "reason": reason.into(),
        });
        let builder = self
            .transport
            .request(Method::DELETE, &path)?
            .json(&req_body);
        self.transport.execute(builder).await?;
        Ok(())
    }
}

/// Account bans management sub-client.
#[derive(Debug, Clone, Copy)]
pub struct AdminBans<'a> {
    transport: &'a Transport,
}

impl<'a> AdminBans<'a> {
    /// Starts building a request to ban a user via `POST /admin/bans`. Requires moderator auth `[M]`.
    pub fn create(
        &self,
        username: impl Into<String>,
        reason: impl Into<String>,
    ) -> CreateBanBuilder<'a> {
        CreateBanBuilder {
            transport: self.transport,
            username: username.into(),
            reason: reason.into(),
            expires_at: None,
        }
    }

    /// Unbans a user via `DELETE /admin/bans/{username}`. Requires moderator auth `[M]`.
    ///
    /// Expects 204 No Content.
    pub async fn remove(&self, username: &str) -> Result<()> {
        let path = format!("/admin/bans/{username}");
        let builder = self.transport.request(Method::DELETE, &path)?;
        self.transport.execute(builder).await?;
        Ok(())
    }
}

/// Builder for creating a ban via `POST /admin/bans`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await is called"]
pub struct CreateBanBuilder<'a> {
    transport: &'a Transport,
    username: String,
    reason: String,
    expires_at: Option<String>,
}

impl<'a> CreateBanBuilder<'a> {
    /// Sets an expiration timestamp for the ban in RFC 3339 format. If omitted, the ban is permanent.
    pub fn expires_at(mut self, expires_at: impl Into<String>) -> Self {
        self.expires_at = Some(expires_at.into());
        self
    }

    /// Dispatches the request and returns the created [`BanSummary`].
    pub async fn send(self) -> Result<BanSummary> {
        let req_body = serde_json::json!({
            "username": self.username,
            "reason": self.reason,
            "expires_at": self.expires_at,
        });
        let builder = self
            .transport
            .request(Method::POST, "/admin/bans")?
            .json(&req_body);
        self.transport.execute_json(builder).await
    }
}

/// Role management sub-client.
#[derive(Debug, Clone, Copy)]
pub struct AdminRoles<'a> {
    transport: &'a Transport,
}

impl<'a> AdminRoles<'a> {
    /// Sets or removes an administrative role for a user via `POST /admin/roles`. Requires admin auth `[X]`.
    ///
    /// - `role`: `Some("admin")`, `Some("moderator")`, or `None` to revoke privileges.
    pub async fn set(&self, username: impl Into<String>, role: Option<&str>) -> Result<()> {
        let req_body = serde_json::json!({
            "username": username.into(),
            "role": role,
        });
        let builder = self
            .transport
            .request(Method::POST, "/admin/roles")?
            .json(&req_body);
        self.transport.execute(builder).await?;
        Ok(())
    }
}

/// Admin audit log actions sub-client.
#[derive(Debug, Clone, Copy)]
pub struct AdminActions<'a> {
    transport: &'a Transport,
}

impl<'a> AdminActions<'a> {
    /// Starts building a query to list admin audit trail actions via `GET /admin/actions`. Requires moderator auth `[M]`.
    pub fn list(&self) -> ListAdminActionsBuilder<'a> {
        ListAdminActionsBuilder {
            transport: self.transport,
            limit: None,
            cursor: None,
        }
    }

    /// Convenience shortcut to stream admin audit trail actions across pages.
    pub fn stream(
        &self,
    ) -> impl futures_core::Stream<Item = Result<AdminActionSummary, Error>> + Send {
        self.list().stream()
    }
}

/// Builder for querying admin actions via `GET /admin/actions`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await or .stream() is called"]
pub struct ListAdminActionsBuilder<'a> {
    transport: &'a Transport,
    limit: Option<u32>,
    cursor: Option<String>,
}

impl<'a> ListAdminActionsBuilder<'a> {
    /// Limits the maximum number of actions per page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the pagination cursor for fetching subsequent pages.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Dispatches the request and returns a single page of [`AdminActionSummary`].
    pub async fn send(self) -> Result<Page<AdminActionSummary>> {
        let mut builder = self.transport.request(Method::GET, "/admin/actions")?;
        if let Some(lim) = self.limit {
            builder = builder.query(&[("limit", lim.to_string())]);
        }
        if let Some(ref c) = self.cursor {
            builder = builder.query(&[("cursor", c.as_str())]);
        }

        let res: AdminActionListResponse = self.transport.execute_json(builder).await?;
        Ok(Page::new(res.actions, res.next_cursor))
    }

    /// Produces a stream that yields admin audit trail actions across pages.
    pub fn stream(
        self,
    ) -> impl futures_core::Stream<Item = Result<AdminActionSummary, Error>> + Send {
        let transport = self.transport.clone();
        let limit = self.limit;

        paginate_stream_with_cursor(self.cursor, move |cursor| {
            let transport = transport.clone();
            async move {
                let mut builder = transport.request(Method::GET, "/admin/actions")?;
                if let Some(lim) = limit {
                    builder = builder.query(&[("limit", lim.to_string())]);
                }
                if let Some(ref c) = cursor {
                    builder = builder.query(&[("cursor", c.as_str())]);
                }

                let res: AdminActionListResponse = transport.execute_json(builder).await?;
                Ok(Page::new(res.actions, res.next_cursor))
            }
        })
    }
}
