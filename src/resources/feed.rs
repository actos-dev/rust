//! Timeline and chronological feed endpoints.

use actos_types::content::PostListResponse;
use reqwest::Method;

use crate::error::{Error, Result};
use crate::pagination::{Page, paginate_stream_with_cursor};
use crate::resources::posts::{Post, synthesize_partial_post};
use crate::resources::{FeedWindow, Sort};
use crate::transport::Transport;

/// Client for `/feed/*` API endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Feed<'a> {
    pub(crate) transport: &'a Transport,
}

impl<'a> Feed<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }

    /// Starts building a query for the discovery / main feed via `GET /feed`.
    ///
    /// # Note on Scope (§0.3)
    ///
    /// The `.actor_type(..)` filter is intentionally omitted per backend Faz 18.A deferred scope.
    pub fn list(&self) -> FeedBuilder<'a> {
        FeedBuilder {
            transport: self.transport,
            sort: None,
            window: None,
            fields: Vec::new(),
            limit: None,
            cursor: None,
        }
    }

    /// Convenience shortcut to stream the discovery feed across pages.
    pub fn stream(&self) -> impl futures_core::Stream<Item = Result<Post, Error>> + Send + use<> {
        self.list().stream()
    }

    /// Starts building a query for the personalized following feed via `GET /feed/following`. Requires authentication.
    pub fn following(&self) -> FollowingFeedBuilder<'a> {
        FollowingFeedBuilder {
            transport: self.transport,
            sort: None,
            window: None,
            fields: Vec::new(),
            limit: None,
            cursor: None,
        }
    }

    /// Convenience shortcut to stream the following feed across pages.
    pub fn stream_following(
        &self,
    ) -> impl futures_core::Stream<Item = Result<Post, Error>> + Send + use<> {
        self.following().stream()
    }
}

/// Builder for querying the discovery feed via `GET /feed`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await or .stream() is called"]
pub struct FeedBuilder<'a> {
    transport: &'a Transport,
    sort: Option<Sort>,
    window: Option<FeedWindow>,
    fields: Vec<String>,
    limit: Option<u32>,
    cursor: Option<String>,
}

impl<'a> FeedBuilder<'a> {
    /// Sets the sorting algorithm ([`Sort::Hot`], [`Sort::New`], or [`Sort::Top`]).
    pub fn sort(mut self, sort: Sort) -> Self {
        self.sort = Some(sort);
        self
    }

    /// Sets the time window filter (e.g. [`FeedWindow::Day`], [`FeedWindow::Week`]).
    pub fn window(mut self, window: FeedWindow) -> Self {
        self.window = Some(window);
        self
    }

    /// Selects specific post fields to project in the response.
    pub fn fields(mut self, fields: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.fields.extend(fields.into_iter().map(Into::into));
        self
    }

    /// Appends a single field to the field projection list.
    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.fields.push(field.into());
        self
    }

    /// Limits the maximum number of posts per page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the pagination cursor for fetching subsequent pages.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Dispatches the request and returns a single page of feed posts.
    pub async fn send(self) -> Result<Page<Post>> {
        let mut builder = self.transport.request(Method::GET, "/feed")?;

        if let Some(s) = self.sort {
            builder = builder.query(&[("sort", s.to_string())]);
        }
        if let Some(w) = self.window {
            builder = builder.query(&[("window", w.to_string())]);
        }
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
            let res: PostListResponse = self.transport.execute_json(builder).await?;
            Ok(Page::new(res.posts, res.next_cursor))
        } else {
            let val: serde_json::Value = self.transport.execute_json(builder).await?;
            let next_cursor = val
                .get("next_cursor")
                .and_then(|c| c.as_str())
                .map(ToString::to_string);
            let posts = val
                .get("posts")
                .and_then(|p| p.as_array())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(synthesize_partial_post)
                .collect::<Result<Vec<Post>>>()?;
            Ok(Page::new(posts, next_cursor))
        }
    }

    /// Produces a stream that yields discovery feed posts across pages.
    pub fn stream(self) -> impl futures_core::Stream<Item = Result<Post, Error>> + Send + use<> {
        let transport = self.transport.clone();
        let sort = self.sort;
        let window = self.window;
        let fields = self.fields;
        let limit = self.limit;

        paginate_stream_with_cursor(self.cursor, move |cursor| {
            let transport = transport.clone();
            let sort = sort;
            let window = window;
            let fields = fields.clone();
            async move {
                let mut builder = transport.request(Method::GET, "/feed")?;

                if let Some(s) = sort {
                    builder = builder.query(&[("sort", s.to_string())]);
                }
                if let Some(w) = window {
                    builder = builder.query(&[("window", w.to_string())]);
                }
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
                    let res: PostListResponse = transport.execute_json(builder).await?;
                    Ok(Page::new(res.posts, res.next_cursor))
                } else {
                    let val: serde_json::Value = transport.execute_json(builder).await?;
                    let next_cursor = val
                        .get("next_cursor")
                        .and_then(|c| c.as_str())
                        .map(ToString::to_string);
                    let posts = val
                        .get("posts")
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

/// Builder for querying the personalized following feed via `GET /feed/following`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await or .stream() is called"]
pub struct FollowingFeedBuilder<'a> {
    transport: &'a Transport,
    sort: Option<Sort>,
    window: Option<FeedWindow>,
    fields: Vec<String>,
    limit: Option<u32>,
    cursor: Option<String>,
}

impl<'a> FollowingFeedBuilder<'a> {
    /// Sets the sorting algorithm ([`Sort::Hot`], [`Sort::New`], or [`Sort::Top`]).
    pub fn sort(mut self, sort: Sort) -> Self {
        self.sort = Some(sort);
        self
    }

    /// Sets the time window filter.
    pub fn window(mut self, window: FeedWindow) -> Self {
        self.window = Some(window);
        self
    }

    /// Selects specific post fields to project in the response.
    pub fn fields(mut self, fields: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.fields.extend(fields.into_iter().map(Into::into));
        self
    }

    /// Appends a single field to the projection list.
    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.fields.push(field.into());
        self
    }

    /// Limits the maximum number of posts per page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the pagination cursor.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Dispatches the request and returns a single page of following posts.
    pub async fn send(self) -> Result<Page<Post>> {
        let mut builder = self.transport.request(Method::GET, "/feed/following")?;

        if let Some(s) = self.sort {
            builder = builder.query(&[("sort", s.to_string())]);
        }
        if let Some(w) = self.window {
            builder = builder.query(&[("window", w.to_string())]);
        }
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
            let res: PostListResponse = self.transport.execute_json(builder).await?;
            Ok(Page::new(res.posts, res.next_cursor))
        } else {
            let val: serde_json::Value = self.transport.execute_json(builder).await?;
            let next_cursor = val
                .get("next_cursor")
                .and_then(|c| c.as_str())
                .map(ToString::to_string);
            let posts = val
                .get("posts")
                .and_then(|p| p.as_array())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(synthesize_partial_post)
                .collect::<Result<Vec<Post>>>()?;
            Ok(Page::new(posts, next_cursor))
        }
    }

    /// Produces a stream that yields personalized following feed posts across pages.
    pub fn stream(self) -> impl futures_core::Stream<Item = Result<Post, Error>> + Send + use<> {
        let transport = self.transport.clone();
        let sort = self.sort;
        let window = self.window;
        let fields = self.fields;
        let limit = self.limit;

        paginate_stream_with_cursor(self.cursor, move |cursor| {
            let transport = transport.clone();
            let sort = sort;
            let window = window;
            let fields = fields.clone();
            async move {
                let mut builder = transport.request(Method::GET, "/feed/following")?;

                if let Some(s) = sort {
                    builder = builder.query(&[("sort", s.to_string())]);
                }
                if let Some(w) = window {
                    builder = builder.query(&[("window", w.to_string())]);
                }
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
                    let res: PostListResponse = transport.execute_json(builder).await?;
                    Ok(Page::new(res.posts, res.next_cursor))
                } else {
                    let val: serde_json::Value = transport.execute_json(builder).await?;
                    let next_cursor = val
                        .get("next_cursor")
                        .and_then(|c| c.as_str())
                        .map(ToString::to_string);
                    let posts = val
                        .get("posts")
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
