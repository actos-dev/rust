//! Main client facade and builder for the Actos SDK.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use reqwest::{Method, RequestBuilder, Url};

use crate::error::{Error, RateLimit, Result};
use crate::resources::*;
use crate::transport::{Transport, mask_api_key};

/// Default API endpoint for local development and backend execution.
pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:3100";

/// Fluent builder for constructing an [`Actos`] client instance.
#[derive(Debug, Default, Clone)]
pub struct ActosBuilder {
    api_key: Option<String>,
    base_url: Option<String>,
    timeout: Option<Duration>,
    max_retries: Option<u32>,
    user_agent_suffix: Option<String>,
    http_client: Option<reqwest::Client>,
}

impl ActosBuilder {
    /// Creates a new builder with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Initializes a builder, inspecting `ACTOS_API_KEY` and `ACTOS_BASE_URL` environment variables.
    #[must_use]
    pub fn from_env() -> Self {
        let api_key = std::env::var("ACTOS_API_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let base_url = std::env::var("ACTOS_BASE_URL")
            .ok()
            .filter(|s| !s.trim().is_empty());

        Self {
            api_key,
            base_url,
            timeout: None,
            max_retries: None,
            user_agent_suffix: None,
            http_client: None,
        }
    }

    /// Sets the API key for authenticating requests.
    #[must_use]
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Sets the target base URL (e.g. `http://127.0.0.1:3100` or `https://api.actos.dev`).
    #[must_use]
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Sets the request timeout (default is 30 seconds).
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Sets the maximum number of retry attempts for transient errors (default is 2).
    #[must_use]
    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = Some(max_retries);
        self
    }

    /// Appends a custom suffix to the client's `User-Agent` header.
    #[must_use]
    pub fn user_agent_suffix(mut self, suffix: impl Into<String>) -> Self {
        self.user_agent_suffix = Some(suffix.into());
        self
    }

    /// Injects a custom [`reqwest::Client`].
    #[must_use]
    pub fn http_client(mut self, client: reqwest::Client) -> Self {
        self.http_client = Some(client);
        self
    }

    /// Builds and validates the [`Actos`] client.
    pub fn build(self) -> Result<Actos> {
        let raw_base_url = self
            .base_url
            .or_else(|| {
                std::env::var("ACTOS_BASE_URL")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
            })
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());

        let base_url = Url::parse(&raw_base_url)
            .map_err(|e| Error::Config(format!("invalid base_url '{raw_base_url}': {e}")))?;

        let api_key = self.api_key.or_else(|| {
            std::env::var("ACTOS_API_KEY")
                .ok()
                .filter(|s| !s.trim().is_empty())
        });

        let mut transport = Transport::new(
            base_url,
            api_key,
            self.max_retries,
            self.timeout,
            self.http_client,
        )?;

        if let Some(suffix) = self.user_agent_suffix {
            transport = transport.with_user_agent_suffix(&suffix);
        }

        Ok(Actos {
            transport: Arc::new(transport),
        })
    }
}

/// The official asynchronous Actos SDK client.
///
/// Serves as the single entry point (§2.1) to all platform resources.
/// Cheap to clone: all instances share the underlying connection pool and rate limit lock.
#[derive(Clone)]
pub struct Actos {
    transport: Arc<Transport>,
}

impl fmt::Debug for Actos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Actos")
            .field("base_url", self.transport.base_url())
            .field("api_key", &self.transport.api_key().map(mask_api_key))
            .field("rate_limit", &self.transport.rate_limit())
            .finish()
    }
}

impl Actos {
    /// Starts building a new [`Actos`] client instance.
    #[must_use]
    pub fn builder() -> ActosBuilder {
        ActosBuilder::new()
    }

    /// Initializes a new [`Actos`] client from environment variables (`ACTOS_API_KEY`, `ACTOS_BASE_URL`).
    pub fn from_env() -> Result<Self> {
        ActosBuilder::from_env().build()
    }

    /// Returns the current rate limit snapshot parsed from recent responses.
    #[must_use]
    pub fn rate_limit(&self) -> Option<RateLimit> {
        self.transport.rate_limit()
    }

    /// Returns the configured base URL.
    #[must_use]
    pub fn base_url(&self) -> &Url {
        self.transport.base_url()
    }

    /// Returns the configured API key, if present.
    #[must_use]
    pub fn api_key(&self) -> Option<&str> {
        self.transport.api_key()
    }

    /// Returns the active `User-Agent` header value (§2.14).
    #[must_use]
    pub fn user_agent(&self) -> &str {
        self.transport.user_agent()
    }

    /// Returns a reference to the shared transport layer.
    #[must_use]
    pub fn transport(&self) -> &Arc<Transport> {
        &self.transport
    }

    /// Escape hatch (§2): constructs a raw [`reqwest::RequestBuilder`] pointing to the relative path,
    /// pre-configured with default headers (`User-Agent`, `Accept`, `Authorization`).
    pub fn request(&self, method: Method, path: &str) -> RequestBuilder {
        let url = self
            .transport
            .build_url(path)
            .unwrap_or_else(|_| self.transport.base_url().clone());

        let mut builder = self.transport.client().request(method, url);
        builder = builder.header(USER_AGENT, self.transport.user_agent());
        builder = builder.header(ACCEPT, "application/json");

        if let Some(api_key) = self.transport.api_key() {
            builder = builder.header(AUTHORIZATION, format!("Bearer {api_key}"));
        }

        builder
    }

    /// Accessor for authentication and identity endpoints.
    #[must_use]
    pub fn auth(&self) -> Auth<'_> {
        Auth::new(&self.transport)
    }

    /// Accessor for actor profiles, directories, and relationships.
    #[must_use]
    pub fn actors(&self) -> Actors<'_> {
        Actors::new(&self.transport)
    }

    /// Accessor for posts and content creation.
    #[must_use]
    pub fn posts(&self) -> Posts<'_> {
        Posts::new(&self.transport)
    }

    /// Accessor for threaded comments.
    #[must_use]
    pub fn comments(&self) -> Comments<'_> {
        Comments::new(&self.transport)
    }

    /// Accessor for hashtags and topic discovery.
    #[must_use]
    pub fn tags(&self) -> Tags<'_> {
        Tags::new(&self.transport)
    }

    /// Accessor for full-text and kind-filtered search.
    #[must_use]
    pub fn search(&self) -> Search<'_> {
        Search::new(&self.transport)
    }

    /// Accessor for feed and timeline queries.
    #[must_use]
    pub fn feed(&self) -> Feed<'_> {
        Feed::new(&self.transport)
    }

    /// Accessor for the authenticated actor's notification inbox.
    #[must_use]
    pub fn inbox(&self) -> Inbox<'_> {
        Inbox::new(&self.transport)
    }

    /// Accessor for content voting.
    #[must_use]
    pub fn votes(&self) -> Votes<'_> {
        Votes::new(&self.transport)
    }

    /// Accessor for bookmarks and saved content.
    #[must_use]
    pub fn saves(&self) -> Saves<'_> {
        Saves::new(&self.transport)
    }

    /// Accessor for media and file uploads.
    #[must_use]
    pub fn uploads(&self) -> Uploads<'_> {
        Uploads::new(&self.transport)
    }

    /// Accessor for abuse reports.
    #[must_use]
    pub fn reports(&self) -> Reports<'_> {
        Reports::new(&self.transport)
    }

    /// Accessor for administrative moderation actions.
    #[must_use]
    pub fn admin(&self) -> Admin<'_> {
        Admin::new(&self.transport)
    }

    /// Accessor for platform health and spec metadata.
    #[must_use]
    pub fn meta(&self) -> Meta<'_> {
        Meta::new(&self.transport)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_defaults_and_custom() {
        let client = Actos::builder()
            .base_url("http://localhost:3100")
            .api_key("test_key_123")
            .timeout(Duration::from_secs(10))
            .max_retries(3)
            .user_agent_suffix("my-agent/1.0")
            .build()
            .expect("valid client");

        assert_eq!(client.base_url().as_str(), "http://localhost:3100/");
        assert_eq!(client.api_key(), Some("test_key_123"));
        assert_eq!(client.transport().max_retries(), 3);
        assert!(client.transport().user_agent().contains("my-agent/1.0"));
    }

    #[test]
    fn test_builder_from_env_defaults() {
        let builder = ActosBuilder::from_env();
        let client = builder.build().expect("valid client");
        assert!(client.base_url().as_str().contains("3100"));
    }

    #[test]
    fn test_debug_masks_api_key() {
        let client = Actos::builder()
            .api_key("actos_1iga_top_secret_token")
            .build()
            .expect("valid client");

        let debug_repr = format!("{client:?}");
        assert!(!debug_repr.contains("top_secret_token"));
        assert!(debug_repr.contains("actos_1iga…"));
    }

    #[test]
    fn test_request_escape_hatch() {
        let client = Actos::builder()
            .base_url("http://localhost:3100")
            .api_key("key_abc")
            .build()
            .expect("valid client");

        let req_builder = client.request(Method::GET, "/custom-path");
        let req = req_builder.build().expect("valid request");

        assert_eq!(req.url().as_str(), "http://localhost:3100/custom-path");
        assert_eq!(req.method(), Method::GET);
        assert_eq!(
            req.headers()
                .get("authorization")
                .unwrap()
                .to_str()
                .unwrap(),
            "Bearer key_abc"
        );
        assert_eq!(
            req.headers().get("accept").unwrap().to_str().unwrap(),
            "application/json"
        );
    }

    #[test]
    fn test_resource_accessors() {
        let client = Actos::builder().build().expect("valid client");

        let _auth = client.auth();
        let _actors = client.actors();
        let _posts = client.posts();
        let _comments = client.comments();
        let _tags = client.tags();
        let _search = client.search();
        let _feed = client.feed();
        let _votes = client.votes();
        let _saves = client.saves();
        let _uploads = client.uploads();
        let _inbox = client.inbox();
        let _reports = client.reports();
        let _admin = client.admin();
        let _meta = client.meta();
    }
}
