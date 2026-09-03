//! HTTP transport layer for the Actos SDK.
//!
//! Provides a resilient wrapper over [`reqwest::Client`] featuring:
//! - Default headers (`User-Agent`, `Accept: application/json`, `Authorization: Bearer <key>`)
//! - Retry logic with exponential backoff and full jitter (§2.6, §2.7, §2.8)
//! - `Retry-After` header priority on HTTP 429
//! - Idempotency protection (non-idempotent `POST` requests are never retried on 5xx)
//! - Shared `X-RateLimit-*` tracking across client clones

use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use reqwest::{Method, Request, RequestBuilder, Response, Url};

use crate::error::{Error, RateLimit, Result};

/// HTTP header name for idempotency keys.
pub const IDEMPOTENCY_KEY_HEADER: &str = "idempotency-key";

/// Normalizes a base URL by ensuring it terminates with a trailing slash `/`.
///
/// This guarantees that [`Url::join`] resolves relative endpoints (e.g. `posts`, `/posts`)
/// without inadvertently stripping path segments from the base URL.
pub fn normalize_base_url(url: Url) -> Result<Url> {
    let mut s = url.as_str().trim_end_matches('/').to_string();
    s.push('/');
    Url::parse(&s).map_err(|e| Error::Config(format!("geçersiz base_url: {e}")))
}

/// Masks an API key for safe debug logging, displaying only the first 10 characters followed by `…`.
pub(crate) fn mask_api_key(key: &str) -> String {
    if key.len() > 10 {
        format!("{}…", &key[..10])
    } else {
        "***".to_string()
    }
}

/// HTTP transport client for dispatching requests to the Actos API.
///
/// Cheap to clone: inner state wraps a shared connection pool and rate limit lock.
#[derive(Clone)]
pub struct Transport {
    client: reqwest::Client,
    base_url: Url,
    api_key: Option<String>,
    max_retries: u32,
    rate_limit: Arc<Mutex<Option<RateLimit>>>,
    user_agent: String,
}

impl fmt::Debug for Transport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Transport")
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_deref().map(mask_api_key))
            .field("max_retries", &self.max_retries)
            .field("rate_limit", &self.rate_limit)
            .field("user_agent", &self.user_agent)
            .finish()
    }
}

impl Transport {
    /// Creates a new [`Transport`] instance with the specified configuration.
    pub fn new(
        base_url: Url,
        api_key: Option<String>,
        max_retries: Option<u32>,
        timeout: Option<Duration>,
        client: Option<reqwest::Client>,
    ) -> Result<Self> {
        let base_url = normalize_base_url(base_url)?;
        let max_retries = max_retries.unwrap_or(2);
        let client = match client {
            Some(c) => c,
            None => reqwest::Client::builder()
                .timeout(timeout.unwrap_or(Duration::from_secs(30)))
                .build()
                .map_err(|e| Error::Config(e.to_string()))?,
        };
        let user_agent = format!("actos-rust/{}", crate::VERSION);

        Ok(Self {
            client,
            base_url,
            api_key,
            max_retries,
            rate_limit: Arc::new(Mutex::new(None)),
            user_agent,
        })
    }

    /// Creates a new [`Transport`] with an existing [`reqwest::Client`].
    pub fn with_client(
        client: reqwest::Client,
        base_url: Url,
        api_key: Option<String>,
    ) -> Result<Self> {
        Self::new(base_url, api_key, None, None, Some(client))
    }

    /// Returns the normalized base URL.
    #[must_use]
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Returns the configured API key, if any.
    #[must_use]
    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    /// Returns the maximum number of retry attempts.
    #[must_use]
    pub const fn max_retries(&self) -> u32 {
        self.max_retries
    }

    /// Returns the active `User-Agent` header value.
    #[must_use]
    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    /// Returns a reference to the underlying [`reqwest::Client`].
    #[must_use]
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Returns the latest rate limit snapshot parsed from HTTP response headers.
    #[must_use]
    pub fn rate_limit(&self) -> Option<RateLimit> {
        self.rate_limit.lock().ok().and_then(|guard| *guard)
    }

    /// Appends a custom suffix to the `User-Agent` header.
    #[must_use]
    pub fn with_user_agent_suffix(mut self, suffix: &str) -> Self {
        self.user_agent = format!("actos-rust/{} ({suffix})", crate::VERSION);
        self
    }

    /// Resolves an endpoint path against the base URL, handling leading slashes safely.
    pub fn build_url(&self, path: &str) -> Result<Url> {
        let clean_path = path.trim_start_matches('/');
        self.base_url
            .join(clean_path)
            .map_err(|e| Error::Config(format!("URL oluşturulamadı ({path}): {e}")))
    }

    /// Creates a new [`RequestBuilder`] for the specified HTTP method and relative path.
    pub fn request(&self, method: Method, path: &str) -> Result<RequestBuilder> {
        let url = self.build_url(path)?;
        Ok(self.client.request(method, url))
    }

    /// Convenience wrapper for a `GET` request.
    pub async fn get(&self, path: &str) -> Result<Response> {
        let builder = self.request(Method::GET, path)?;
        self.execute(builder).await
    }

    /// Convenience wrapper for a `POST` request.
    pub async fn post(&self, path: &str) -> Result<Response> {
        let builder = self.request(Method::POST, path)?;
        self.execute(builder).await
    }

    /// Convenience wrapper for a `DELETE` request.
    pub async fn delete(&self, path: &str) -> Result<Response> {
        let builder = self.request(Method::DELETE, path)?;
        self.execute(builder).await
    }

    /// Convenience wrapper for a `PATCH` request.
    pub async fn patch(&self, path: &str) -> Result<Response> {
        let builder = self.request(Method::PATCH, path)?;
        self.execute(builder).await
    }

    /// Convenience wrapper for a `PUT` request.
    pub async fn put(&self, path: &str) -> Result<Response> {
        let builder = self.request(Method::PUT, path)?;
        self.execute(builder).await
    }

    /// Executes a request constructed by a [`RequestBuilder`], applying default headers and retries.
    pub async fn execute(&self, builder: RequestBuilder) -> Result<Response> {
        let req = builder.build().map_err(Error::Transport)?;
        self.send(req).await
    }

    /// Executes a request and deserializes the JSON response body into `T`.
    pub async fn execute_json<T: serde::de::DeserializeOwned>(
        &self,
        builder: RequestBuilder,
    ) -> Result<T> {
        let res = self.execute(builder).await?;
        let bytes = res.bytes().await.map_err(Error::Transport)?;
        serde_json::from_slice::<T>(&bytes).map_err(|e| Error::Decode(e.to_string()))
    }

    /// Sends an HTTP request with automatic retry, exponential backoff, and header tracking.
    pub async fn send(&self, mut req: Request) -> Result<Response> {
        self.prepare_request(&mut req);

        let method = req.method().clone();
        let headers = req.headers().clone();
        let mut attempt: u32 = 0;
        let mut maybe_req = Some(req);

        loop {
            let is_last_attempt = attempt >= self.max_retries;
            let req_to_send = if is_last_attempt {
                maybe_req
                    .take()
                    .ok_or_else(|| Error::Config("İstek gövdesi tükendi".to_string()))?
            } else {
                match maybe_req.as_ref().and_then(Request::try_clone) {
                    Some(cloned) => cloned,
                    None => {
                        // Cannot clone body; consume original request and prevent retries
                        maybe_req
                            .take()
                            .ok_or_else(|| Error::Config("İstek gövdesi tükendi".to_string()))?
                    }
                }
            };

            let send_result = self.client.execute(req_to_send).await;

            match send_result {
                Ok(res) => {
                    // Update rate limit on every response
                    if let Some(rl) = RateLimit::from_headers(res.headers())
                        && let Ok(mut guard) = self.rate_limit.lock()
                    {
                        *guard = Some(rl);
                    }

                    if res.status().is_success() {
                        return Ok(res);
                    }

                    let status = res.status();
                    let res_headers = res.headers().clone();
                    let bytes = res.bytes().await.map_err(Error::Transport)?;
                    let err = Error::from_response_parts(status, &res_headers, &bytes);

                    // Check retry eligibility
                    if maybe_req.is_some()
                        && attempt < self.max_retries
                        && Self::is_request_retryable(&method, &headers, &err)
                    {
                        let delay = Self::calculate_backoff(attempt, &err);
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                        continue;
                    }

                    return Err(err);
                }
                Err(transport_err) => {
                    let err = Error::Transport(transport_err);

                    // Check retry eligibility
                    if maybe_req.is_some()
                        && attempt < self.max_retries
                        && Self::is_request_retryable(&method, &headers, &err)
                    {
                        let delay = Self::calculate_backoff(attempt, &err);
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                        continue;
                    }

                    return Err(err);
                }
            }
        }
    }

    /// Prepares a request by injecting default headers if not already present.
    fn prepare_request(&self, req: &mut Request) {
        let headers = req.headers_mut();

        if !headers.contains_key(USER_AGENT)
            && let Ok(val) = HeaderValue::from_str(&self.user_agent)
        {
            headers.insert(USER_AGENT, val);
        }

        if !headers.contains_key(ACCEPT) {
            headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        }

        if !headers.contains_key(AUTHORIZATION)
            && let Some(ref key) = self.api_key
        {
            let auth_header = format!("Bearer {key}");
            if let Ok(val) = HeaderValue::from_str(&auth_header) {
                headers.insert(AUTHORIZATION, val);
            }
        }
    }

    /// Evaluates retry eligibility per SDK Contract §2.6.
    ///
    /// - 4xx errors (except 429) are NEVER retried.
    /// - POST requests without `Idempotency-Key` are NEVER retried on 5xx or transport errors.
    /// - 5xx, 429, and transport errors are retryable.
    fn is_request_retryable(method: &Method, headers: &HeaderMap, err: &Error) -> bool {
        if !err.is_retryable() {
            return false;
        }

        // Idempotency rule: non-idempotent POST requests must not be retried on 5xx or transport errors
        if method == Method::POST
            && !headers.contains_key(IDEMPOTENCY_KEY_HEADER)
            && (matches!(err, Error::Transport(_))
                || err
                    .status()
                    .is_some_and(|status| (500..=599).contains(&status)))
        {
            return false;
        }

        true
    }

    /// Calculates backoff delay with exponential scaling and full jitter (§2.8).
    ///
    /// If the error is a 429 and includes a `Retry-After` header, that duration is prioritized.
    fn calculate_backoff(attempt: u32, err: &Error) -> Duration {
        if let Some(retry_after) = err.retry_after() {
            return retry_after;
        }

        let exp_factor = 2_u32.saturating_pow(attempt.min(10)) as f64;
        let max_delay = 30.0_f64.min(0.25 * exp_factor);
        let delay_secs = fastrand::f64() * max_delay;
        Duration::from_secs_f64(delay_secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_headers_applied() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/test"))
            .and(header("user-agent", "actos-rust/0.1.0"))
            .and(header("accept", "application/json"))
            .and(header("authorization", "Bearer test_secret_key_12345"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"status": "ok"})),
            )
            .expect(1)
            .mount(&server)
            .await;

        let base_url = Url::parse(&server.uri()).expect("valid url");
        let transport = Transport::new(
            base_url,
            Some("test_secret_key_12345".to_string()),
            None,
            None,
            None,
        )
        .expect("valid transport");

        let res = transport.get("/test").await.expect("request ok");
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_rate_limit_shared_across_clones() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rate-limited"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("x-ratelimit-limit", "120")
                    .insert_header("x-ratelimit-remaining", "119")
                    .insert_header("x-ratelimit-reset", "60")
                    .set_body_json(serde_json::json!({"ok": true})),
            )
            .expect(1)
            .mount(&server)
            .await;

        let base_url = Url::parse(&server.uri()).expect("valid url");
        let transport1 = Transport::new(base_url, None, None, None, None).expect("valid transport");
        let transport2 = transport1.clone();

        assert!(transport1.rate_limit().is_none());
        assert!(transport2.rate_limit().is_none());

        transport1.get("/rate-limited").await.expect("request ok");

        let expected_rl = RateLimit {
            limit: 120,
            remaining: 119,
            reset: 60,
        };
        assert_eq!(transport1.rate_limit(), Some(expected_rl));
        assert_eq!(transport2.rate_limit(), Some(expected_rl));
    }

    #[tokio::test]
    async fn test_retry_5xx_success() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/transient-error"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Error"))
            .up_to_n_times(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/transient-error"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"recovered": true})),
            )
            .mount(&server)
            .await;

        let base_url = Url::parse(&server.uri()).expect("valid url");
        let transport =
            Transport::new(base_url, None, Some(2), None, None).expect("valid transport");

        let res = transport
            .get("/transient-error")
            .await
            .expect("should recover on retry");
        assert_eq!(res.status(), StatusCode::OK);

        let received = server.received_requests().await.expect("requests list");
        assert_eq!(received.len(), 2);
    }

    #[tokio::test]
    async fn test_4xx_never_retried() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/not-found"))
            .respond_with(
                ResponseTemplate::new(404)
                    .insert_header("content-type", "application/problem+json")
                    .set_body_json(serde_json::json!({
                        "code": "NOT_FOUND",
                        "detail": "Resource not found"
                    })),
            )
            .expect(1)
            .mount(&server)
            .await;

        let base_url = Url::parse(&server.uri()).expect("valid url");
        let transport =
            Transport::new(base_url, None, Some(2), None, None).expect("valid transport");

        let err = transport
            .get("/not-found")
            .await
            .expect_err("should return 404 error");
        assert!(err.is_not_found());
        assert_eq!(err.status(), Some(404));

        let received = server.received_requests().await.expect("requests list");
        assert_eq!(received.len(), 1);
    }

    #[tokio::test]
    async fn test_post_without_idempotency_key_not_retried_on_5xx() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/posts"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal DB Error"))
            .expect(1)
            .mount(&server)
            .await;

        let base_url = Url::parse(&server.uri()).expect("valid url");
        let transport =
            Transport::new(base_url, None, Some(2), None, None).expect("valid transport");

        let err = transport
            .post("/posts")
            .await
            .expect_err("should fail without retry");
        assert_eq!(err.status(), Some(500));

        let received = server.received_requests().await.expect("requests list");
        assert_eq!(received.len(), 1);
    }

    #[tokio::test]
    async fn test_post_with_idempotency_key_retried_on_5xx() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/posts"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Server glitch"))
            .up_to_n_times(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/posts"))
            .respond_with(
                ResponseTemplate::new(201).set_body_json(serde_json::json!({"id": "p_123"})),
            )
            .mount(&server)
            .await;

        let base_url = Url::parse(&server.uri()).expect("valid url");
        let transport =
            Transport::new(base_url, None, Some(2), None, None).expect("valid transport");

        let builder = transport
            .request(Method::POST, "/posts")
            .expect("valid request")
            .header("idempotency-key", "idem-uuid-test");

        let res = transport
            .execute(builder)
            .await
            .expect("should retry and succeed");
        assert_eq!(res.status(), StatusCode::CREATED);

        let received = server.received_requests().await.expect("requests list");
        assert_eq!(received.len(), 2);
    }

    #[tokio::test]
    async fn test_retry_429_respects_retry_after() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/throttled"))
            .respond_with(
                ResponseTemplate::new(429)
                    .insert_header("retry-after", "0")
                    .insert_header("content-type", "application/problem+json")
                    .set_body_json(serde_json::json!({
                        "code": "RATE_LIMITED",
                        "detail": "Slow down"
                    })),
            )
            .up_to_n_times(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/throttled"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
            .mount(&server)
            .await;

        let base_url = Url::parse(&server.uri()).expect("valid url");
        let transport =
            Transport::new(base_url, None, Some(2), None, None).expect("valid transport");

        let res = transport
            .get("/throttled")
            .await
            .expect("should retry and succeed");
        assert_eq!(res.status(), StatusCode::OK);

        let received = server.received_requests().await.expect("requests list");
        assert_eq!(received.len(), 2);
    }

    #[test]
    fn test_base_url_normalization() {
        let url1 = Url::parse("http://127.0.0.1:3100").expect("valid url");
        let transport1 = Transport::new(url1, None, None, None, None).expect("valid transport");
        assert_eq!(transport1.base_url().as_str(), "http://127.0.0.1:3100/");

        assert_eq!(
            transport1.build_url("posts").expect("url").as_str(),
            "http://127.0.0.1:3100/posts"
        );
        assert_eq!(
            transport1.build_url("/posts").expect("url").as_str(),
            "http://127.0.0.1:3100/posts"
        );

        let url2 = Url::parse("http://127.0.0.1:3100/api/v1").expect("valid url");
        let transport2 = Transport::new(url2, None, None, None, None).expect("valid transport");
        assert_eq!(
            transport2.base_url().as_str(),
            "http://127.0.0.1:3100/api/v1/"
        );

        assert_eq!(
            transport2.build_url("posts").expect("url").as_str(),
            "http://127.0.0.1:3100/api/v1/posts"
        );
        assert_eq!(
            transport2.build_url("/posts").expect("url").as_str(),
            "http://127.0.0.1:3100/api/v1/posts"
        );
    }

    #[test]
    fn test_debug_masks_api_key() {
        let url = Url::parse("http://127.0.0.1:3100").expect("valid url");
        let transport = Transport::new(
            url,
            Some("actos_1iga_very_secret_key_12345".to_string()),
            None,
            None,
            None,
        )
        .expect("valid transport");

        let debug_str = format!("{transport:?}");
        assert!(!debug_str.contains("very_secret_key"));
        assert!(debug_str.contains("actos_1iga…"));
    }
}
