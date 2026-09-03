//! Unified search endpoints.

use actos_types::search::ContentSearchResponse;
use reqwest::Method;

use crate::error::{Error, Result};
use crate::pagination::{Page, paginate_stream_with_cursor};
use crate::resources::SearchKind;
use crate::resources::posts::{Post, synthesize_partial_post};
use crate::transport::Transport;

/// Client for `/search` API endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Search<'a> {
    pub(crate) transport: &'a Transport,
}

impl<'a> Search<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }

    /// Starts building a search query across platform content via `GET /search`.
    pub fn query(&self, q: impl Into<String>) -> SearchBuilder<'a> {
        SearchBuilder {
            transport: self.transport,
            q: q.into(),
            kind: SearchKind::Post,
            fields: Vec::new(),
            limit: None,
            cursor: None,
        }
    }

    /// Convenience shortcut to stream search results matching a query.
    pub fn stream(
        &self,
        q: impl Into<String>,
    ) -> impl futures_core::Stream<Item = Result<Post, Error>> + Send {
        self.query(q).stream()
    }
}

/// Builder for querying search results via `GET /search`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await or .stream() is called"]
pub struct SearchBuilder<'a> {
    transport: &'a Transport,
    q: String,
    kind: SearchKind,
    fields: Vec<String>,
    limit: Option<u32>,
    cursor: Option<String>,
}

impl<'a> SearchBuilder<'a> {
    /// Sets the target entity kind for search (default is [`SearchKind::Post`]).
    pub fn kind(mut self, kind: SearchKind) -> Self {
        self.kind = kind;
        self
    }

    /// Specifies fields to project in the search results.
    pub fn fields(mut self, fields: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.fields.extend(fields.into_iter().map(Into::into));
        self
    }

    /// Appends a single field to project in the response.
    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.fields.push(field.into());
        self
    }

    /// Limits the maximum number of items returned in a page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the pagination cursor for fetching subsequent pages.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Dispatches the search query and returns a single page of results.
    pub async fn send(self) -> Result<Page<Post>> {
        let mut builder = self.transport.request(Method::GET, "/search")?.query(&[
            ("q", self.q.as_str()),
            ("type", self.kind.to_string().as_str()),
        ]);

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
            let res: ContentSearchResponse = self.transport.execute_json(builder).await?;
            Ok(Page::new(res.results, res.next_cursor))
        } else {
            let val: serde_json::Value = self.transport.execute_json(builder).await?;
            let next_cursor = val
                .get("next_cursor")
                .and_then(|c| c.as_str())
                .map(ToString::to_string);
            let results = val
                .get("results")
                .and_then(|p| p.as_array())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(synthesize_partial_post)
                .collect::<Result<Vec<Post>>>()?;
            Ok(Page::new(results, next_cursor))
        }
    }

    /// Produces a stream that yields matching search results across all pages.
    pub fn stream(self) -> impl futures_core::Stream<Item = Result<Post, Error>> + Send {
        let transport = self.transport.clone();
        let q = self.q;
        let kind = self.kind;
        let fields = self.fields;
        let limit = self.limit;

        paginate_stream_with_cursor(self.cursor, move |cursor| {
            let transport = transport.clone();
            let q = q.clone();
            let kind = kind;
            let fields = fields.clone();
            async move {
                let mut builder = transport
                    .request(Method::GET, "/search")?
                    .query(&[("q", q.as_str()), ("type", kind.to_string().as_str())]);

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
                    let res: ContentSearchResponse = transport.execute_json(builder).await?;
                    Ok(Page::new(res.results, res.next_cursor))
                } else {
                    let val: serde_json::Value = transport.execute_json(builder).await?;
                    let next_cursor = val
                        .get("next_cursor")
                        .and_then(|c| c.as_str())
                        .map(ToString::to_string);
                    let results = val
                        .get("results")
                        .and_then(|p| p.as_array())
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .map(synthesize_partial_post)
                        .collect::<Result<Vec<Post>>>()?;
                    Ok(Page::new(results, next_cursor))
                }
            }
        })
    }
}
