//! Tag discovery and tagged post listing endpoints.

use actos_types::content::PostListResponse;
pub use actos_types::tag::{TagMatch, TagSearchResponse, TagSummary};
use reqwest::Method;

use crate::error::{Error, Result};
use crate::pagination::{Page, paginate_stream_with_cursor};
use crate::resources::Sort;
use crate::resources::posts::{Post, synthesize_partial_post};
use crate::transport::Transport;

/// Client for `/tags/*` API endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Tags<'a> {
    pub(crate) transport: &'a Transport,
}

impl<'a> Tags<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }

    /// Starts building a query to list popular platform tags via `GET /tags`.
    ///
    /// Note: `fields` projection is not supported on this endpoint.
    pub fn list(&self) -> ListTagsBuilder<'a> {
        ListTagsBuilder {
            transport: self.transport,
            limit: None,
            cursor: None,
        }
    }

    /// Convenience shortcut to stream popular platform tags across pages.
    pub fn stream(&self) -> impl futures_core::Stream<Item = Result<TagSummary, Error>> + Send {
        self.list().stream()
    }

    /// Searches for tags matching an auto-completion prefix via `GET /tags/search?q=prefix`.
    pub async fn search(&self, prefix: &str) -> Result<Vec<TagMatch>> {
        let builder = self
            .transport
            .request(Method::GET, "/tags/search")?
            .query(&[("q", prefix)]);
        let res: TagSearchResponse = self.transport.execute_json(builder).await?;
        Ok(res.tags)
    }

    /// Starts building a query to list posts associated with a given tag name via `GET /tags/{name}/posts`.
    pub fn posts(&self, name: &str) -> TagPostsBuilder<'a> {
        TagPostsBuilder {
            transport: self.transport,
            name: name.to_string(),
            sort: None,
            fields: Vec::new(),
            limit: None,
            cursor: None,
        }
    }

    /// Convenience shortcut to stream posts associated with a tag across pages.
    pub fn stream_posts(
        &self,
        name: &str,
    ) -> impl futures_core::Stream<Item = Result<Post, Error>> + Send {
        self.posts(name).stream()
    }
}

/// Builder for listing tags via `GET /tags`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await or .stream() is called"]
pub struct ListTagsBuilder<'a> {
    transport: &'a Transport,
    limit: Option<u32>,
    cursor: Option<String>,
}

impl<'a> ListTagsBuilder<'a> {
    /// Limits the maximum number of tags returned in a page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the cursor token for pagination.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Dispatches the request and returns a single page of [`TagSummary`].
    pub async fn send(self) -> Result<Page<TagSummary>> {
        let mut builder = self.transport.request(Method::GET, "/tags")?;
        if let Some(lim) = self.limit {
            builder = builder.query(&[("limit", lim.to_string())]);
        }
        if let Some(ref c) = self.cursor {
            builder = builder.query(&[("cursor", c.as_str())]);
        }

        let res: actos_types::tag::TagListResponse = self.transport.execute_json(builder).await?;
        Ok(Page::new(res.tags, res.next_cursor))
    }

    /// Produces a stream that yields tags across all pages.
    pub fn stream(self) -> impl futures_core::Stream<Item = Result<TagSummary, Error>> + Send {
        let transport = self.transport.clone();
        let limit = self.limit;

        paginate_stream_with_cursor(self.cursor, move |cursor| {
            let transport = transport.clone();
            async move {
                let mut builder = transport.request(Method::GET, "/tags")?;
                if let Some(lim) = limit {
                    builder = builder.query(&[("limit", lim.to_string())]);
                }
                if let Some(ref c) = cursor {
                    builder = builder.query(&[("cursor", c.as_str())]);
                }

                let res: actos_types::tag::TagListResponse =
                    transport.execute_json(builder).await?;
                Ok(Page::new(res.tags, res.next_cursor))
            }
        })
    }
}

/// Builder for listing tagged posts via `GET /tags/{name}/posts`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await or .stream() is called"]
pub struct TagPostsBuilder<'a> {
    transport: &'a Transport,
    name: String,
    sort: Option<Sort>,
    fields: Vec<String>,
    limit: Option<u32>,
    cursor: Option<String>,
}

impl<'a> TagPostsBuilder<'a> {
    /// Sets the sort ordering using the typed [`Sort`] enum.
    pub fn sort(mut self, sort: Sort) -> Self {
        self.sort = Some(sort);
        self
    }

    /// Specifies post fields to project in the response.
    pub fn fields(mut self, fields: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.fields.extend(fields.into_iter().map(Into::into));
        self
    }

    /// Appends a single field to project in the response.
    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.fields.push(field.into());
        self
    }

    /// Limits the maximum number of posts per page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the cursor token for pagination.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Dispatches the request and returns a single page of [`Post`].
    pub async fn send(self) -> Result<Page<Post>> {
        let path = format!("/tags/{}/posts", self.name);
        let mut builder = self.transport.request(Method::GET, &path)?;

        if let Some(s) = self.sort {
            builder = builder.query(&[("sort", s.to_string())]);
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

    /// Produces a stream that yields posts tagged with this name across pages.
    pub fn stream(self) -> impl futures_core::Stream<Item = Result<Post, Error>> + Send {
        let transport = self.transport.clone();
        let name = self.name;
        let sort = self.sort;
        let fields = self.fields;
        let limit = self.limit;

        paginate_stream_with_cursor(self.cursor, move |cursor| {
            let transport = transport.clone();
            let path = format!("/tags/{name}/posts");
            let sort = sort;
            let fields = fields.clone();
            async move {
                let mut builder = transport.request(Method::GET, &path)?;
                if let Some(s) = sort {
                    builder = builder.query(&[("sort", s.to_string())]);
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
