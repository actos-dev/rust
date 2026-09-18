//! Community directory, membership, moderation, and admission endpoints.
//!
//! Communities are containers with an owner, moderators and members; a post
//! may belong to one, and membership is required to post in it. Reading a
//! public community needs nothing, while a private community is unlisted and
//! serves a cover page to readers who cannot see inside.

use actos_types::community::{
    ApplicationListResponse, ApplicationSummary, CommunityListResponse,
    CommunityMemberListResponse, CommunityMemberSummary, CommunitySummary, InvitationListResponse,
    InvitationSummary,
};
use actos_types::content::PostListResponse;
use reqwest::Method;

use crate::error::{Error, Result};
use crate::pagination::{Page, paginate_stream_with_cursor};
use crate::resources::Sort;
use crate::resources::posts::{Post, synthesize_partial_post};
use crate::transport::Transport;

/// Client for the `/communities/*` and `/me/invitations` API endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Communities<'a> {
    pub(crate) transport: &'a Transport,
}

impl<'a> Communities<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }

    /// Starts building a query for the public community directory via `GET /communities`.
    ///
    /// Private communities are never listed.
    pub fn list(&self) -> ListCommunitiesBuilder<'a> {
        ListCommunitiesBuilder {
            transport: self.transport,
            limit: None,
            cursor: None,
        }
    }

    /// Convenience shortcut to stream the community directory across pages.
    pub fn stream(
        &self,
    ) -> impl futures_core::Stream<Item = Result<CommunitySummary, Error>> + Send {
        self.list().stream()
    }

    /// Starts building a request to create a community via `POST /communities`. Requires authentication.
    ///
    /// The creator becomes the owner and first member. An actor may own at
    /// most three communities. `.visibility("private")` creates an unlisted
    /// community; the default is `"public"`.
    pub fn create(
        &self,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> CreateCommunityBuilder<'a> {
        CreateCommunityBuilder {
            transport: self.transport,
            name: name.into(),
            description: description.into(),
            visibility: None,
        }
    }

    /// Reads a single community by name via `GET /communities/{name}`.
    ///
    /// When authentication is present, `is_member` reflects the caller;
    /// anonymous requests get `false`. A private community the viewer may not
    /// see inside returns a cover (same name and description, zeroed counts).
    pub async fn get(&self, name: &str) -> Result<CommunitySummary> {
        let path = format!("/communities/{name}");
        let builder = self.transport.request(Method::GET, &path)?;
        self.transport.execute_json(builder).await
    }

    /// Starts building a request to edit a community via `PATCH /communities/{name}`. Requires authentication.
    ///
    /// Callable by the owner or a holder of `community.edit` for this
    /// community. Visibility moves one way only: public to private.
    pub fn update(&self, name: impl Into<String>) -> UpdateCommunityBuilder<'a> {
        UpdateCommunityBuilder {
            transport: self.transport,
            name: name.into(),
            description: None,
            visibility: None,
        }
    }

    /// Joins a community via `POST /communities/{name}/join`. Requires authentication.
    ///
    /// Instant and idempotent for public communities.
    pub async fn join(&self, name: &str) -> Result<()> {
        let path = format!("/communities/{name}/join");
        let builder = self.transport.request(Method::POST, &path)?;
        self.transport.execute(builder).await?;
        Ok(())
    }

    /// Leaves a community via `DELETE /communities/{name}/join`. Requires authentication.
    ///
    /// Idempotent. An owner leaving triggers succession or closure.
    pub async fn leave(&self, name: &str) -> Result<()> {
        let path = format!("/communities/{name}/join");
        let builder = self.transport.request(Method::DELETE, &path)?;
        self.transport.execute(builder).await?;
        Ok(())
    }

    /// Starts building a query to list a community's members via `GET /communities/{name}/members`.
    ///
    /// Longest-serving member first.
    pub fn members(&self, name: impl Into<String>) -> CommunityMembersBuilder<'a> {
        CommunityMembersBuilder {
            transport: self.transport,
            name: name.into(),
            limit: None,
            cursor: None,
        }
    }

    /// Convenience shortcut to stream a community's members across pages.
    pub fn stream_members(
        &self,
        name: &str,
    ) -> impl futures_core::Stream<Item = Result<CommunityMemberSummary, Error>> + Send {
        self.members(name).stream()
    }

    /// Kicks a member from a community via `DELETE /communities/{name}/members/{username}`. Requires authentication.
    ///
    /// Requires `member.kick` scoped to this community. The owner cannot be
    /// kicked.
    pub async fn kick(&self, name: &str, username: &str) -> Result<()> {
        let path = format!("/communities/{name}/members/{username}");
        let builder = self.transport.request(Method::DELETE, &path)?;
        self.transport.execute(builder).await?;
        Ok(())
    }

    /// Starts building a query to list a community's posts via `GET /communities/{name}/posts`.
    ///
    /// Supports the three sorts (`new`, `top`, `hot`) and sparse field
    /// projection with [`.fields([..])`](CommunityPostsBuilder::fields).
    pub fn posts(&self, name: impl Into<String>) -> CommunityPostsBuilder<'a> {
        CommunityPostsBuilder {
            transport: self.transport,
            name: name.into(),
            sort: None,
            fields: Vec::new(),
            limit: None,
            cursor: None,
        }
    }

    /// Convenience shortcut to stream a community's posts across pages.
    pub fn stream_posts(
        &self,
        name: &str,
    ) -> impl futures_core::Stream<Item = Result<Post, Error>> + Send {
        self.posts(name).stream()
    }

    /// Closes a community via `POST /communities/{name}/close`. Requires authentication.
    ///
    /// Requires `community.close` scoped to this community. A public
    /// community's posts become independent; a private community's posts are
    /// deleted.
    pub async fn close(&self, name: &str) -> Result<()> {
        let path = format!("/communities/{name}/close");
        let builder = self.transport.request(Method::POST, &path)?;
        self.transport.execute(builder).await?;
        Ok(())
    }

    /// Designates a community's successor via `PUT /communities/{name}/successor`. Requires authentication.
    ///
    /// Owner only. The named actor inherits the community when the owner
    /// leaves or deletes their account.
    pub async fn set_successor(&self, name: &str, username: &str) -> Result<()> {
        let path = format!("/communities/{name}/successor");
        let req_body = actos_types::community::SuccessorRequest {
            username: username.to_string(),
        };
        let builder = self.transport.request(Method::PUT, &path)?.json(&req_body);
        self.transport.execute(builder).await?;
        Ok(())
    }

    /// Invites an actor to a private community via `POST /communities/{name}/invitations`. Requires authentication.
    ///
    /// Requires `member.invite` scoped to this community. Private communities
    /// only; the invitee is not a member until they accept.
    pub async fn invite(&self, name: &str, username: &str) -> Result<()> {
        let path = format!("/communities/{name}/invitations");
        let req_body = actos_types::community::CreateInvitationRequest {
            username: username.to_string(),
        };
        let builder = self.transport.request(Method::POST, &path)?.json(&req_body);
        self.transport.execute(builder).await?;
        Ok(())
    }

    /// Starts building a query for the caller's pending invitations via `GET /me/invitations`. Requires authentication.
    pub fn invitations(&self) -> ListInvitationsBuilder<'a> {
        ListInvitationsBuilder {
            transport: self.transport,
            limit: None,
            cursor: None,
        }
    }

    /// Convenience shortcut to stream the caller's pending invitations across pages.
    pub fn stream_invitations(
        &self,
    ) -> impl futures_core::Stream<Item = Result<InvitationSummary, Error>> + Send {
        self.invitations().stream()
    }

    /// Accepts an invitation via `POST /me/invitations/{id}/accept`. Requires authentication.
    ///
    /// On success the actor becomes a member of the community.
    pub async fn accept_invitation(&self, id: &str) -> Result<()> {
        let path = format!("/me/invitations/{id}/accept");
        let builder = self.transport.request(Method::POST, &path)?;
        self.transport.execute(builder).await?;
        Ok(())
    }

    /// Declines an invitation via `POST /me/invitations/{id}/decline`. Requires authentication.
    pub async fn decline_invitation(&self, id: &str) -> Result<()> {
        let path = format!("/me/invitations/{id}/decline");
        let builder = self.transport.request(Method::POST, &path)?;
        self.transport.execute(builder).await?;
        Ok(())
    }

    /// Applies to a private community via `POST /communities/{name}/applications`. Requires authentication.
    ///
    /// The reason is 1-2000 characters. Every holder of `member.approve`
    /// scoped to the community is notified.
    pub async fn apply(&self, name: &str, reason: impl Into<String>) -> Result<()> {
        let path = format!("/communities/{name}/applications");
        let req_body = actos_types::community::CreateApplicationRequest {
            reason: reason.into(),
        };
        let builder = self.transport.request(Method::POST, &path)?.json(&req_body);
        self.transport.execute(builder).await?;
        Ok(())
    }

    /// Starts building a query for a community's application queue via `GET /communities/{name}/applications`. Requires authentication.
    ///
    /// Requires `member.approve` scoped to this community. Oldest first.
    pub fn applications(&self, name: impl Into<String>) -> ListApplicationsBuilder<'a> {
        ListApplicationsBuilder {
            transport: self.transport,
            name: name.into(),
            status: None,
            limit: None,
            cursor: None,
        }
    }

    /// Convenience shortcut to stream a community's applications across pages.
    pub fn stream_applications(
        &self,
        name: &str,
    ) -> impl futures_core::Stream<Item = Result<ApplicationSummary, Error>> + Send {
        self.applications(name).stream()
    }

    /// Accepts an application via `POST /communities/{name}/applications/{id}/accept`. Requires authentication.
    ///
    /// Requires `member.approve` scoped to this community. The applicant
    /// becomes a member.
    pub async fn accept_application(&self, name: &str, id: &str) -> Result<()> {
        let path = format!("/communities/{name}/applications/{id}/accept");
        let builder = self.transport.request(Method::POST, &path)?;
        self.transport.execute(builder).await?;
        Ok(())
    }

    /// Rejects an application via `POST /communities/{name}/applications/{id}/reject`. Requires authentication.
    ///
    /// Requires `member.approve` scoped to this community.
    pub async fn reject_application(&self, name: &str, id: &str) -> Result<()> {
        let path = format!("/communities/{name}/applications/{id}/reject");
        let builder = self.transport.request(Method::POST, &path)?;
        self.transport.execute(builder).await?;
        Ok(())
    }
}

/// Builder for listing the community directory via `GET /communities`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await or .stream() is called"]
pub struct ListCommunitiesBuilder<'a> {
    transport: &'a Transport,
    limit: Option<u32>,
    cursor: Option<String>,
}

impl<'a> ListCommunitiesBuilder<'a> {
    /// Limits the maximum number of communities per page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the pagination cursor for fetching subsequent pages.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Dispatches the request and returns a single page of [`CommunitySummary`].
    pub async fn send(self) -> Result<Page<CommunitySummary>> {
        let mut builder = self.transport.request(Method::GET, "/communities")?;
        if let Some(lim) = self.limit {
            builder = builder.query(&[("limit", lim.to_string())]);
        }
        if let Some(ref c) = self.cursor {
            builder = builder.query(&[("cursor", c.as_str())]);
        }

        let res: CommunityListResponse = self.transport.execute_json(builder).await?;
        Ok(Page::new(res.communities, res.next_cursor))
    }

    /// Produces a stream that yields the community directory across pages.
    pub fn stream(
        self,
    ) -> impl futures_core::Stream<Item = Result<CommunitySummary, Error>> + Send {
        let transport = self.transport.clone();
        let limit = self.limit;

        paginate_stream_with_cursor(self.cursor, move |cursor| {
            let transport = transport.clone();
            async move {
                let mut builder = transport.request(Method::GET, "/communities")?;
                if let Some(lim) = limit {
                    builder = builder.query(&[("limit", lim.to_string())]);
                }
                if let Some(ref c) = cursor {
                    builder = builder.query(&[("cursor", c.as_str())]);
                }

                let res: CommunityListResponse = transport.execute_json(builder).await?;
                Ok(Page::new(res.communities, res.next_cursor))
            }
        })
    }
}

/// Builder for creating a community via `POST /communities`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await is called"]
pub struct CreateCommunityBuilder<'a> {
    transport: &'a Transport,
    name: String,
    description: String,
    visibility: Option<String>,
}

impl<'a> CreateCommunityBuilder<'a> {
    /// Sets the visibility (`"public"`, the default, or `"private"`).
    pub fn visibility(mut self, visibility: impl Into<String>) -> Self {
        self.visibility = Some(visibility.into());
        self
    }

    /// Dispatches the request and returns the created [`CommunitySummary`].
    pub async fn send(self) -> Result<CommunitySummary> {
        let req_body = actos_types::community::CreateCommunityRequest {
            name: self.name,
            description: self.description,
            visibility: self.visibility,
        };
        let builder = self
            .transport
            .request(Method::POST, "/communities")?
            .json(&req_body);
        self.transport.execute_json(builder).await
    }
}

/// Builder for editing a community via `PATCH /communities/{name}`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await is called"]
pub struct UpdateCommunityBuilder<'a> {
    transport: &'a Transport,
    name: String,
    description: Option<String>,
    visibility: Option<String>,
}

impl<'a> UpdateCommunityBuilder<'a> {
    /// Replaces the community description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Changes visibility (public to private only).
    pub fn visibility(mut self, visibility: impl Into<String>) -> Self {
        self.visibility = Some(visibility.into());
        self
    }

    /// Dispatches the request and returns the updated [`CommunitySummary`].
    pub async fn send(self) -> Result<CommunitySummary> {
        let mut body = serde_json::Map::new();
        if let Some(description) = self.description {
            body.insert(
                "description".to_string(),
                serde_json::Value::String(description),
            );
        }
        if let Some(visibility) = self.visibility {
            body.insert(
                "visibility".to_string(),
                serde_json::Value::String(visibility),
            );
        }

        let path = format!("/communities/{}", self.name);
        let builder = self.transport.request(Method::PATCH, &path)?.json(&body);
        self.transport.execute_json(builder).await
    }
}

/// Builder for listing a community's members via `GET /communities/{name}/members`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await or .stream() is called"]
pub struct CommunityMembersBuilder<'a> {
    transport: &'a Transport,
    name: String,
    limit: Option<u32>,
    cursor: Option<String>,
}

impl<'a> CommunityMembersBuilder<'a> {
    /// Limits the maximum number of members per page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the pagination cursor for fetching subsequent pages.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Dispatches the request and returns a single page of [`CommunityMemberSummary`].
    pub async fn send(self) -> Result<Page<CommunityMemberSummary>> {
        let path = format!("/communities/{}/members", self.name);
        let mut builder = self.transport.request(Method::GET, &path)?;
        if let Some(lim) = self.limit {
            builder = builder.query(&[("limit", lim.to_string())]);
        }
        if let Some(ref c) = self.cursor {
            builder = builder.query(&[("cursor", c.as_str())]);
        }

        let res: CommunityMemberListResponse = self.transport.execute_json(builder).await?;
        Ok(Page::new(res.members, res.next_cursor))
    }

    /// Produces a stream that yields a community's members across pages.
    pub fn stream(
        self,
    ) -> impl futures_core::Stream<Item = Result<CommunityMemberSummary, Error>> + Send {
        let transport = self.transport.clone();
        let name = self.name;
        let limit = self.limit;

        paginate_stream_with_cursor(self.cursor, move |cursor| {
            let transport = transport.clone();
            let path = format!("/communities/{name}/members");
            async move {
                let mut builder = transport.request(Method::GET, &path)?;
                if let Some(lim) = limit {
                    builder = builder.query(&[("limit", lim.to_string())]);
                }
                if let Some(ref c) = cursor {
                    builder = builder.query(&[("cursor", c.as_str())]);
                }

                let res: CommunityMemberListResponse = transport.execute_json(builder).await?;
                Ok(Page::new(res.members, res.next_cursor))
            }
        })
    }
}

/// Builder for listing a community's posts via `GET /communities/{name}/posts`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await or .stream() is called"]
pub struct CommunityPostsBuilder<'a> {
    transport: &'a Transport,
    name: String,
    sort: Option<Sort>,
    fields: Vec<String>,
    limit: Option<u32>,
    cursor: Option<String>,
}

impl<'a> CommunityPostsBuilder<'a> {
    /// Sets the sort ordering using the typed [`Sort`] enum.
    pub fn sort(mut self, sort: Sort) -> Self {
        self.sort = Some(sort);
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

    /// Sets the pagination cursor for fetching subsequent pages.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Dispatches the request and returns a single page of [`Post`].
    pub async fn send(self) -> Result<Page<Post>> {
        let path = format!("/communities/{}/posts", self.name);
        let mut builder = self.transport.request(Method::GET, &path)?;

        if let Some(sort) = self.sort {
            builder = builder.query(&[("sort", sort.to_string())]);
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

    /// Produces a stream that yields a community's posts across pages.
    pub fn stream(self) -> impl futures_core::Stream<Item = Result<Post, Error>> + Send {
        let transport = self.transport.clone();
        let name = self.name;
        let sort = self.sort;
        let fields = self.fields;
        let limit = self.limit;

        paginate_stream_with_cursor(self.cursor, move |cursor| {
            let transport = transport.clone();
            let path = format!("/communities/{name}/posts");
            let fields = fields.clone();
            async move {
                let mut builder = transport.request(Method::GET, &path)?;
                if let Some(sort) = sort {
                    builder = builder.query(&[("sort", sort.to_string())]);
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

/// Builder for listing the caller's pending invitations via `GET /me/invitations`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await or .stream() is called"]
pub struct ListInvitationsBuilder<'a> {
    transport: &'a Transport,
    limit: Option<u32>,
    cursor: Option<String>,
}

impl<'a> ListInvitationsBuilder<'a> {
    /// Limits the maximum number of invitations per page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the pagination cursor for fetching subsequent pages.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Dispatches the request and returns a single page of [`InvitationSummary`].
    pub async fn send(self) -> Result<Page<InvitationSummary>> {
        let mut builder = self.transport.request(Method::GET, "/me/invitations")?;
        if let Some(lim) = self.limit {
            builder = builder.query(&[("limit", lim.to_string())]);
        }
        if let Some(ref c) = self.cursor {
            builder = builder.query(&[("cursor", c.as_str())]);
        }

        let res: InvitationListResponse = self.transport.execute_json(builder).await?;
        Ok(Page::new(res.invitations, res.next_cursor))
    }

    /// Produces a stream that yields pending invitations across pages.
    pub fn stream(
        self,
    ) -> impl futures_core::Stream<Item = Result<InvitationSummary, Error>> + Send {
        let transport = self.transport.clone();
        let limit = self.limit;

        paginate_stream_with_cursor(self.cursor, move |cursor| {
            let transport = transport.clone();
            async move {
                let mut builder = transport.request(Method::GET, "/me/invitations")?;
                if let Some(lim) = limit {
                    builder = builder.query(&[("limit", lim.to_string())]);
                }
                if let Some(ref c) = cursor {
                    builder = builder.query(&[("cursor", c.as_str())]);
                }

                let res: InvitationListResponse = transport.execute_json(builder).await?;
                Ok(Page::new(res.invitations, res.next_cursor))
            }
        })
    }
}

/// Builder for listing a community's application queue via `GET /communities/{name}/applications`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await or .stream() is called"]
pub struct ListApplicationsBuilder<'a> {
    transport: &'a Transport,
    name: String,
    status: Option<String>,
    limit: Option<u32>,
    cursor: Option<String>,
}

impl<'a> ListApplicationsBuilder<'a> {
    /// Filters applications by status (`"pending"`, `"accepted"`, or `"rejected"`).
    pub fn status(mut self, status: impl Into<String>) -> Self {
        self.status = Some(status.into());
        self
    }

    /// Limits the maximum number of applications per page.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the pagination cursor for fetching subsequent pages.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Dispatches the request and returns a single page of [`ApplicationSummary`].
    pub async fn send(self) -> Result<Page<ApplicationSummary>> {
        let path = format!("/communities/{}/applications", self.name);
        let mut builder = self.transport.request(Method::GET, &path)?;
        if let Some(ref status) = self.status {
            builder = builder.query(&[("status", status.as_str())]);
        }
        if let Some(lim) = self.limit {
            builder = builder.query(&[("limit", lim.to_string())]);
        }
        if let Some(ref c) = self.cursor {
            builder = builder.query(&[("cursor", c.as_str())]);
        }

        let res: ApplicationListResponse = self.transport.execute_json(builder).await?;
        Ok(Page::new(res.applications, res.next_cursor))
    }

    /// Produces a stream that yields applications across pages.
    pub fn stream(
        self,
    ) -> impl futures_core::Stream<Item = Result<ApplicationSummary, Error>> + Send {
        let transport = self.transport.clone();
        let name = self.name;
        let status = self.status;
        let limit = self.limit;

        paginate_stream_with_cursor(self.cursor, move |cursor| {
            let transport = transport.clone();
            let path = format!("/communities/{name}/applications");
            let status = status.clone();
            async move {
                let mut builder = transport.request(Method::GET, &path)?;
                if let Some(ref status) = status {
                    builder = builder.query(&[("status", status.as_str())]);
                }
                if let Some(lim) = limit {
                    builder = builder.query(&[("limit", lim.to_string())]);
                }
                if let Some(ref c) = cursor {
                    builder = builder.query(&[("cursor", c.as_str())]);
                }

                let res: ApplicationListResponse = transport.execute_json(builder).await?;
                Ok(Page::new(res.applications, res.next_cursor))
            }
        })
    }
}
