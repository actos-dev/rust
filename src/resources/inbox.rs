//! Notification inbox endpoints.

use std::time::Duration;

use actos_types::notification::{InboxResponse, MarkAllReadResponse, NotificationSummary};
use reqwest::Method;

use crate::error::{Error, Result};
use crate::pagination::{Page, paginate_stream_with_cursor};
use crate::transport::Transport;

/// Client for `/me/inbox` notification endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Inbox<'a> {
    pub(crate) transport: &'a Transport,
}

impl<'a> Inbox<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }

    /// Starts building a query to list the authenticated actor's inbox via `GET /me/inbox`.
    pub fn list(&self) -> InboxListBuilder<'a> {
        InboxListBuilder {
            transport: self.transport,
            unread: None,
            limit: None,
            cursor: None,
        }
    }

    /// Convenience shortcut to stream all inbox notifications across pages.
    pub fn stream(
        &self,
    ) -> impl futures_core::Stream<Item = Result<NotificationSummary, Error>> + Send + use<'a> {
        self.list().stream()
    }

    /// Marks a single notification as read via `PATCH /me/inbox/{id}/read`. Requires authentication.
    ///
    /// # Note
    ///
    /// The wire method is **`PATCH`** (the SDK's plan briefly assumed PUT/POST; the live spec
    /// confirms PATCH). Returns 204, and is idempotent — marking an already-read notification
    /// still succeeds. Returns `NOT_FOUND` (404) if the notification ID does not exist.
    pub async fn read(&self, notification_id: impl Into<String>) -> Result<()> {
        let path = format!("/me/inbox/{}/read", notification_id.into());
        let builder = self.transport.request(Method::PATCH, &path)?;
        self.transport.execute(builder).await?;
        Ok(())
    }

    /// Bulk-marks notifications as read via `POST /me/inbox/read`. Requires authentication.
    ///
    /// If `up_to_cursor` is `None`, **all** unread notifications are marked read; otherwise only
    /// those up to the cursor returned by `GET /me/inbox` are marked. Returns the number of
    /// notifications **newly** marked by this call (already-read ones are not recounted).
    pub async fn read_all(
        &self,
        up_to_cursor: Option<impl Into<String>>,
    ) -> Result<MarkAllReadResponse> {
        let mut builder = self.transport.request(Method::POST, "/me/inbox/read")?;
        if let Some(cursor) = up_to_cursor {
            builder = builder.query(&[("cursor", cursor.into())]);
        }
        self.transport.execute_json(builder).await
    }

    /// Returns the caller's total unread notification count.
    ///
    /// Derived from the `unread_count` field returned by `GET /me/inbox` (a single-page probe
    /// requested with `limit=1&unread=true`), without needing to fetch the full inbox.
    pub async fn unread_count(&self) -> Result<i64> {
        let mut builder = self.transport.request(Method::GET, "/me/inbox")?;
        builder = builder.query(&[("limit", "1"), ("unread", "true")]);
        let res: InboxResponse = self.transport.execute_json(builder).await?;
        Ok(res.unread_count)
    }

    /// Polls the inbox for unread notifications at a fixed `poll_interval`, yielding the caller's
    /// total unread count after each poll.
    ///
    /// This is an infinite (unbounded) polling stream — stop consuming it to halt. Transient
    /// per-request errors are yielded and polling continues; on HTTP 429 the transport's built-in
    /// `Retry-After` backoff is honored before the next poll.
    pub fn watch(
        &self,
        poll_interval: Duration,
    ) -> impl futures_core::Stream<Item = Result<i64, Error>> + Send + use<'a> {
        let transport = self.transport.clone();
        futures_util::stream::unfold((), move |()| {
            let transport = transport.clone();
            async move {
                let result: Result<i64> = async {
                    let mut builder = transport.request(Method::GET, "/me/inbox")?;
                    builder = builder.query(&[("limit", "1"), ("unread", "true")]);
                    let res: InboxResponse = transport.execute_json(builder).await?;
                    Ok(res.unread_count)
                }
                .await;
                tokio::time::sleep(poll_interval).await;
                Some((result, ()))
            }
        })
    }
}

/// Builder for querying the authenticated actor's inbox via `GET /me/inbox`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await or .stream() is called"]
pub struct InboxListBuilder<'a> {
    transport: &'a Transport,
    unread: Option<bool>,
    limit: Option<u32>,
    cursor: Option<String>,
}

impl<'a> InboxListBuilder<'a> {
    /// Filters the page to unread notifications only.
    pub fn unread(mut self, unread: bool) -> Self {
        self.unread = Some(unread);
        self
    }

    /// Limits the maximum number of notifications returned in the page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the pagination cursor for fetching subsequent pages.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Dispatches the request and returns a single page of inbox notifications.
    pub async fn send(self) -> Result<Page<NotificationSummary>> {
        let mut builder = self.transport.request(Method::GET, "/me/inbox")?;

        if let Some(u) = self.unread {
            builder = builder.query(&[("unread", u.to_string())]);
        }
        if let Some(lim) = self.limit {
            builder = builder.query(&[("limit", lim.to_string())]);
        }
        if let Some(ref c) = self.cursor {
            builder = builder.query(&[("cursor", c.as_str())]);
        }

        let res: InboxResponse = self.transport.execute_json(builder).await?;
        Ok(Page::new(res.notifications, res.next_cursor))
    }

    /// Produces a stream that yields all inbox notifications across pages.
    pub fn stream(
        self,
    ) -> impl futures_core::Stream<Item = Result<NotificationSummary, Error>> + Send {
        let transport = self.transport.clone();
        let unread = self.unread;
        let limit = self.limit;

        paginate_stream_with_cursor(self.cursor, move |cursor| {
            let transport = transport.clone();
            async move {
                let mut builder = transport.request(Method::GET, "/me/inbox")?;
                if let Some(u) = unread {
                    builder = builder.query(&[("unread", u.to_string())]);
                }
                if let Some(lim) = limit {
                    builder = builder.query(&[("limit", lim.to_string())]);
                }
                if let Some(ref c) = cursor {
                    builder = builder.query(&[("cursor", c.as_str())]);
                }

                let res: InboxResponse = transport.execute_json(builder).await?;
                Ok(Page::new(res.notifications, res.next_cursor))
            }
        })
    }
}
