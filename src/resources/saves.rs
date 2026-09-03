//! Bookmarks and saved contents endpoints.

pub use actos_types::interaction::SaveListResponse;
use reqwest::Method;

use crate::error::{Error, Result};
use crate::pagination::{Page, paginate_stream_with_cursor};
use crate::resources::posts::{Post, synthesize_partial_post};
use crate::transport::Transport;

/// Client for `/contents/{id}/save` and `/me/saves` API endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Saves<'a> {
    pub(crate) transport: &'a Transport,
}

impl<'a> Saves<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }

    /// Saves (bookmarks) a content item via `PUT /contents/{id}/save`. Requires authentication.
    ///
    /// This operation is **idempotent**: repeating the call on an already saved item succeeds cleanly (204).
    pub async fn add(&self, content_id: &str) -> Result<()> {
        let path = format!("/contents/{content_id}/save");
        let builder = self.transport.request(Method::PUT, &path)?;
        self.transport.execute(builder).await?;
        Ok(())
    }

    /// Removes a saved bookmark via `DELETE /contents/{id}/save`. Requires authentication.
    ///
    /// This operation is **idempotent**: repeating the call on an unsaved item succeeds cleanly (204).
    pub async fn remove(&self, content_id: &str) -> Result<()> {
        let path = format!("/contents/{content_id}/save");
        let builder = self.transport.request(Method::DELETE, &path)?;
        self.transport.execute(builder).await?;
        Ok(())
    }

    /// Starts building a query to list the authenticated actor's saved bookmarks via `GET /me/saves`. Requires authentication.
    pub fn list(&self) -> ListSavesBuilder<'a> {
        ListSavesBuilder {
            transport: self.transport,
            fields: Vec::new(),
            limit: None,
            cursor: None,
        }
    }

    /// Convenience shortcut to stream saved bookmarks across pages.
    pub fn stream(&self) -> impl futures_core::Stream<Item = Result<Post, Error>> + Send {
        self.list().stream()
    }
}

/// Builder for querying saved bookmarks via `GET /me/saves`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await or .stream() is called"]
pub struct ListSavesBuilder<'a> {
    transport: &'a Transport,
    fields: Vec<String>,
    limit: Option<u32>,
    cursor: Option<String>,
}

impl<'a> ListSavesBuilder<'a> {
    /// Selects specific fields to project in the saved content items.
    pub fn fields(mut self, fields: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.fields.extend(fields.into_iter().map(Into::into));
        self
    }

    /// Appends a single field to the projection list.
    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.fields.push(field.into());
        self
    }

    /// Limits the maximum number of items returned per page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the pagination cursor for fetching subsequent pages.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Dispatches the request and returns a single page of saved [`Post`] items.
    pub async fn send(self) -> Result<Page<Post>> {
        let mut builder = self.transport.request(Method::GET, "/me/saves")?;

        if !self.fields.is_empty() {
            builder = builder.query(&[("fields", self.fields.join(","))]);
        }
        if let Some(lim) = self.limit {
            builder = builder.query(&[("limit", lim.to_string())]);
        }
        if let Some(ref c) = self.cursor {
            builder = builder.query(&[("cursor", c.as_str())]);
        }

        if self.fields.is_empty() {
            let res: SaveListResponse = self.transport.execute_json(builder).await?;
            Ok(Page::new(res.saves, res.next_cursor))
        } else {
            let val: serde_json::Value = self.transport.execute_json(builder).await?;
            let next_cursor = val
                .get("next_cursor")
                .and_then(|c| c.as_str())
                .map(ToString::to_string);
            let posts = val
                .get("saves")
                .and_then(|p| p.as_array())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(synthesize_partial_post)
                .collect::<Result<Vec<Post>>>()?;
            Ok(Page::new(posts, next_cursor))
        }
    }

    /// Produces a stream that yields saved bookmarks across pages.
    pub fn stream(self) -> impl futures_core::Stream<Item = Result<Post, Error>> + Send {
        let transport = self.transport.clone();
        let fields = self.fields;
        let limit = self.limit;

        paginate_stream_with_cursor(self.cursor, move |cursor| {
            let transport = transport.clone();
            let fields = fields.clone();
            async move {
                let mut builder = transport.request(Method::GET, "/me/saves")?;

                if !fields.is_empty() {
                    builder = builder.query(&[("fields", fields.join(","))]);
                }
                if let Some(lim) = limit {
                    builder = builder.query(&[("limit", lim.to_string())]);
                }
                if let Some(ref c) = cursor {
                    builder = builder.query(&[("cursor", c.as_str())]);
                }

                if fields.is_empty() {
                    let res: SaveListResponse = transport.execute_json(builder).await?;
                    Ok(Page::new(res.saves, res.next_cursor))
                } else {
                    let val: serde_json::Value = transport.execute_json(builder).await?;
                    let next_cursor = val
                        .get("next_cursor")
                        .and_then(|c| c.as_str())
                        .map(ToString::to_string);
                    let posts = val
                        .get("saves")
                        .and_then(|p| p.as_array())
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .map(synthesize_partial_post)
                        .collect::<Result<Vec<Post>>>()?;
                    Ok(Page::new(posts, next_cursor))
                }
            }
        })
    }
}
