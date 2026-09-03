//! RFC 9457 Problem Details error handling and SDK error representations.

use std::time::Duration;

use reqwest::StatusCode;
use reqwest::header::HeaderMap;

/// Rate limit information extracted from HTTP response headers (`X-RateLimit-*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RateLimit {
    /// Maximum number of requests allowed in the current rate limit window.
    pub limit: u32,
    /// Number of remaining requests permitted in the current rate limit window.
    pub remaining: u32,
    /// Seconds remaining until the rate limit window resets.
    pub reset: u64,
}

impl RateLimit {
    /// Parses rate limit headers (`x-ratelimit-limit`, `x-ratelimit-remaining`, `x-ratelimit-reset`)
    /// from an HTTP [`HeaderMap`].
    #[must_use]
    pub fn from_headers(headers: &HeaderMap) -> Option<Self> {
        let limit = headers
            .get("x-ratelimit-limit")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<u32>().ok())?;

        let remaining = headers
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<u32>().ok())?;

        let reset = headers
            .get("x-ratelimit-reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| {
                let trimmed = s.trim();
                trimmed
                    .parse::<u64>()
                    .ok()
                    .or_else(|| trimmed.parse::<f64>().ok().map(|f| f.ceil() as u64))
            })?;

        Some(Self {
            limit,
            remaining,
            reset,
        })
    }
}

/// Fallback mapping from an HTTP status code to an [`actos_types::ErrorCode`] when no machine-readable
/// code is returned by the server.
#[must_use]
pub const fn fallback_error_code_for_status(status: u16) -> actos_types::ErrorCode {
    match status {
        400 => actos_types::ErrorCode::ValidationFailed,
        401 => actos_types::ErrorCode::MissingCredentials,
        403 => actos_types::ErrorCode::Forbidden,
        404 => actos_types::ErrorCode::NotFound,
        409 => actos_types::ErrorCode::Conflict,
        410 => actos_types::ErrorCode::Gone,
        415 => actos_types::ErrorCode::UnsupportedMedia,
        429 => actos_types::ErrorCode::RateLimited,
        500..=599 => actos_types::ErrorCode::Internal,
        _ => actos_types::ErrorCode::Internal,
    }
}

/// Helper function to format RFC 9457 API error messages.
fn format_api_error(
    status: &u16,
    code: &actos_types::ErrorCode,
    detail: Option<&str>,
    request_id: Option<&str>,
) -> String {
    let mut s = format!("[{status} {code:?}]");
    if let Some(detail) = detail {
        s.push(' ');
        s.push_str(detail);
    }
    if let Some(req_id) = request_id {
        s.push_str(" (request_id=");
        s.push_str(req_id);
        s.push(')');
    }
    s
}

/// Comprehensive error type for the Actos SDK.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The server returned an RFC 9457 error response.
    #[error("{}", format_api_error(.status, .code, .detail.as_deref(), .request_id.as_deref()))]
    Api {
        /// Machine-readable error code.
        code: actos_types::ErrorCode,
        /// HTTP status code.
        status: u16,
        /// Human-readable problem detail description.
        detail: Option<String>,
        /// Correlation identifier for debugging and support requests.
        request_id: Option<String>,
        /// Delay before retry, populated for rate limiting (HTTP 429).
        retry_after: Option<Duration>,
        /// Rate limit quota parsed from response headers.
        rate_limit: Option<RateLimit>,
    },

    /// Network or transport failure (no HTTP response received).
    #[error("taşıma hatası: {0}")]
    Transport(#[from] reqwest::Error),

    /// Server response could not be decoded into expected shape.
    #[error("yanıt çözümlenemedi: {0}")]
    Decode(String),

    /// Invalid client configuration (e.g. malformed base URL).
    #[error("yapılandırma hatası: {0}")]
    Config(String),

    /// File I/O error during upload operations (§4).
    #[error("dosya okuma hatası: {0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    /// Returns the API error code if this is an [`Error::Api`].
    #[must_use]
    pub const fn code(&self) -> Option<actos_types::ErrorCode> {
        match self {
            Self::Api { code, .. } => Some(*code),
            _ => None,
        }
    }

    /// Returns the HTTP status code if this is an [`Error::Api`].
    #[must_use]
    pub const fn status(&self) -> Option<u16> {
        match self {
            Self::Api { status, .. } => Some(*status),
            _ => None,
        }
    }

    /// Returns the detail message if this is an [`Error::Api`].
    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        match self {
            Self::Api { detail, .. } => detail.as_deref(),
            _ => None,
        }
    }

    /// Returns the request ID if available.
    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Api { request_id, .. } => request_id.as_deref(),
            _ => None,
        }
    }

    /// Returns the retry delay if available.
    #[must_use]
    pub const fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Api { retry_after, .. } => *retry_after,
            _ => None,
        }
    }

    /// Returns the rate limit metadata if available.
    #[must_use]
    pub const fn rate_limit(&self) -> Option<RateLimit> {
        match self {
            Self::Api { rate_limit, .. } => *rate_limit,
            _ => None,
        }
    }

    /// Checks if the error represents `NOT_FOUND` (404).
    #[must_use]
    pub fn is_not_found(&self) -> bool {
        self.code() == Some(actos_types::ErrorCode::NotFound)
    }

    /// Checks if the error represents `GONE` (410, resource was deleted).
    #[must_use]
    pub fn is_gone(&self) -> bool {
        self.code() == Some(actos_types::ErrorCode::Gone)
    }

    /// Checks if the error represents `RATE_LIMITED` (429).
    #[must_use]
    pub fn is_rate_limited(&self) -> bool {
        self.code() == Some(actos_types::ErrorCode::RateLimited)
    }

    /// Checks if the error represents `FORBIDDEN` (403).
    #[must_use]
    pub fn is_forbidden(&self) -> bool {
        self.code() == Some(actos_types::ErrorCode::Forbidden) || self.status() == Some(403)
    }

    /// Returns `true` if this error can safely be retried per SDK Contract §2.6:
    /// - Transport errors (network disconnect, timeout)
    /// - 5xx server errors
    /// - 429 rate limit errors
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Transport(_) => true,
            Self::Api { status, code, .. } => {
                *status == 429
                    || *code == actos_types::ErrorCode::RateLimited
                    || (*status >= 500 && *status <= 599)
            }
            Self::Decode(_) | Self::Config(_) | Self::Io(_) => false,
        }
    }

    /// Constructs an [`Error::Api`] by parsing RFC 9457 Problem Details from HTTP response components.
    #[must_use]
    pub fn from_response_parts(status: StatusCode, headers: &HeaderMap, body: &[u8]) -> Self {
        #[derive(serde::Deserialize)]
        struct ProblemPayload {
            #[serde(default)]
            code: Option<serde_json::Value>,
            #[serde(default)]
            detail: Option<String>,
            #[serde(default)]
            request_id: Option<String>,
            #[serde(default)]
            title: Option<String>,
        }

        let rate_limit = RateLimit::from_headers(headers);

        let retry_after = headers
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| {
                let trimmed = s.trim();
                trimmed
                    .parse::<u64>()
                    .ok()
                    .or_else(|| trimmed.parse::<f64>().ok().map(|f| f.ceil() as u64))
            })
            .map(Duration::from_secs);

        let header_request_id = headers
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .map(ToOwned::to_owned);

        let status_u16 = status.as_u16();

        if let Ok(payload) = serde_json::from_slice::<ProblemPayload>(body) {
            let code = payload
                .code
                .and_then(|v| serde_json::from_value::<actos_types::ErrorCode>(v).ok())
                .unwrap_or_else(|| fallback_error_code_for_status(status_u16));

            let detail = payload
                .detail
                .filter(|s| !s.is_empty())
                .or_else(|| payload.title.filter(|s| !s.is_empty()))
                .or_else(|| status.canonical_reason().map(ToString::to_string));

            let request_id = payload.request_id.or(header_request_id);

            Self::Api {
                code,
                status: status_u16,
                detail,
                request_id,
                retry_after,
                rate_limit,
            }
        } else {
            let text = std::str::from_utf8(body)
                .ok()
                .map(str::trim)
                .filter(|s| !s.is_empty());

            let detail = match text {
                Some(t)
                    if t.starts_with('<') || t.to_ascii_lowercase().starts_with("<!doctype") =>
                {
                    status.canonical_reason().map(ToString::to_string)
                }
                Some(t) => Some(t.to_string()),
                None => status.canonical_reason().map(ToString::to_string),
            };

            let code = fallback_error_code_for_status(status_u16);

            Self::Api {
                code,
                status: status_u16,
                detail,
                request_id: header_request_id,
                retry_after,
                rate_limit,
            }
        }
    }
}

/// Convenient type alias for operations returning [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;
    use actos_types::ErrorCode;
    use reqwest::header::{HeaderMap, HeaderValue};

    #[test]
    fn test_all_12_error_codes_exhaustive_mapping() {
        let codes = [
            (ErrorCode::ValidationFailed, "VALIDATION_FAILED", 400),
            (ErrorCode::MissingCredentials, "MISSING_CREDENTIALS", 401),
            (ErrorCode::InvalidKey, "INVALID_KEY", 401),
            (ErrorCode::Forbidden, "FORBIDDEN", 403),
            (ErrorCode::Banned, "BANNED", 403),
            (ErrorCode::NotFound, "NOT_FOUND", 404),
            (ErrorCode::Gone, "GONE", 410),
            (ErrorCode::Conflict, "CONFLICT", 409),
            (ErrorCode::RateLimited, "RATE_LIMITED", 429),
            (ErrorCode::UnsupportedMedia, "UNSUPPORTED_MEDIA", 415),
            (ErrorCode::InvalidCursor, "INVALID_CURSOR", 400),
            (ErrorCode::Internal, "INTERNAL", 500),
        ];

        for (expected_code, code_str, status_code) in codes {
            let json = format!(
                r#"{{"code":"{code_str}","detail":"Test error detail","request_id":"req_test_123"}}"#
            );
            let headers = HeaderMap::new();
            let status = StatusCode::from_u16(status_code).expect("valid status code");

            let err = Error::from_response_parts(status, &headers, json.as_bytes());

            match err {
                Error::Api {
                    code,
                    status: s,
                    detail,
                    request_id,
                    ..
                } => {
                    assert_eq!(code, expected_code);
                    assert_eq!(s, status_code);
                    assert_eq!(detail.as_deref(), Some("Test error detail"));
                    assert_eq!(request_id.as_deref(), Some("req_test_123"));
                }
                _ => panic!("Expected Error::Api variant"),
            }

            // Also test exhaustive match on ErrorCode directly
            match expected_code {
                ErrorCode::ValidationFailed => assert_eq!(expected_code.http_status(), 400),
                ErrorCode::MissingCredentials => assert_eq!(expected_code.http_status(), 401),
                ErrorCode::InvalidKey => assert_eq!(expected_code.http_status(), 401),
                ErrorCode::Forbidden => assert_eq!(expected_code.http_status(), 403),
                ErrorCode::Banned => assert_eq!(expected_code.http_status(), 403),
                ErrorCode::NotFound => assert_eq!(expected_code.http_status(), 404),
                ErrorCode::Gone => assert_eq!(expected_code.http_status(), 410),
                ErrorCode::Conflict => assert_eq!(expected_code.http_status(), 409),
                ErrorCode::RateLimited => assert_eq!(expected_code.http_status(), 429),
                ErrorCode::UnsupportedMedia => assert_eq!(expected_code.http_status(), 415),
                ErrorCode::InvalidCursor => assert_eq!(expected_code.http_status(), 400),
                ErrorCode::Internal => assert_eq!(expected_code.http_status(), 500),
            }
        }
    }

    #[test]
    fn test_error_predicates() {
        let headers = HeaderMap::new();

        let not_found_err = Error::from_response_parts(
            StatusCode::NOT_FOUND,
            &headers,
            br#"{"code":"NOT_FOUND","detail":"Actor not found"}"#,
        );
        assert!(not_found_err.is_not_found());
        assert!(!not_found_err.is_gone());
        assert!(!not_found_err.is_rate_limited());
        assert!(!not_found_err.is_retryable());

        let gone_err = Error::from_response_parts(
            StatusCode::GONE,
            &headers,
            br#"{"code":"GONE","detail":"Post was deleted"}"#,
        );
        assert!(gone_err.is_gone());
        assert!(!gone_err.is_not_found());
        assert!(!gone_err.is_rate_limited());
        assert!(!gone_err.is_retryable());

        let rate_limit_err = Error::from_response_parts(
            StatusCode::TOO_MANY_REQUESTS,
            &headers,
            br#"{"code":"RATE_LIMITED","detail":"Slow down"}"#,
        );
        assert!(rate_limit_err.is_rate_limited());
        assert!(rate_limit_err.is_retryable());

        let server_err = Error::from_response_parts(
            StatusCode::INTERNAL_SERVER_ERROR,
            &headers,
            br#"{"code":"INTERNAL","detail":"DB down"}"#,
        );
        assert!(server_err.is_retryable());

        let config_err = Error::Config("bad url".to_string());
        assert!(!config_err.is_retryable());

        let decode_err = Error::Decode("invalid json".to_string());
        assert!(!decode_err.is_retryable());
    }

    #[test]
    fn test_fallback_mapping_for_empty_and_html() {
        let headers = HeaderMap::new();

        // Empty body fallback
        let empty_404 = Error::from_response_parts(StatusCode::NOT_FOUND, &headers, b"");
        assert_eq!(empty_404.code(), Some(ErrorCode::NotFound));
        assert_eq!(empty_404.status(), Some(404));
        assert_eq!(empty_404.detail(), Some("Not Found"));

        let empty_400 = Error::from_response_parts(StatusCode::BAD_REQUEST, &headers, b"");
        assert_eq!(empty_400.code(), Some(ErrorCode::ValidationFailed));

        let empty_401 = Error::from_response_parts(StatusCode::UNAUTHORIZED, &headers, b"");
        assert_eq!(empty_401.code(), Some(ErrorCode::MissingCredentials));

        let empty_403 = Error::from_response_parts(StatusCode::FORBIDDEN, &headers, b"");
        assert_eq!(empty_403.code(), Some(ErrorCode::Forbidden));

        let empty_409 = Error::from_response_parts(StatusCode::CONFLICT, &headers, b"");
        assert_eq!(empty_409.code(), Some(ErrorCode::Conflict));

        let empty_410 = Error::from_response_parts(StatusCode::GONE, &headers, b"");
        assert_eq!(empty_410.code(), Some(ErrorCode::Gone));

        let empty_415 =
            Error::from_response_parts(StatusCode::UNSUPPORTED_MEDIA_TYPE, &headers, b"");
        assert_eq!(empty_415.code(), Some(ErrorCode::UnsupportedMedia));

        let empty_429 = Error::from_response_parts(StatusCode::TOO_MANY_REQUESTS, &headers, b"");
        assert_eq!(empty_429.code(), Some(ErrorCode::RateLimited));

        let empty_500 =
            Error::from_response_parts(StatusCode::INTERNAL_SERVER_ERROR, &headers, b"");
        assert_eq!(empty_500.code(), Some(ErrorCode::Internal));

        // HTML 502 error fallback
        let html_502 = b"<html><head><title>502 Bad Gateway</title></head><body><h1>Bad Gateway</h1></body></html>";
        let err_502 = Error::from_response_parts(StatusCode::BAD_GATEWAY, &headers, html_502);
        assert_eq!(err_502.code(), Some(ErrorCode::Internal));
        assert_eq!(err_502.status(), Some(502));
        assert_eq!(err_502.detail(), Some("Bad Gateway"));
        assert!(err_502.is_retryable());

        // HTML 504 error fallback
        let html_504 = b"<!DOCTYPE html><html><body>Gateway Timeout</body></html>";
        let err_504 = Error::from_response_parts(StatusCode::GATEWAY_TIMEOUT, &headers, html_504);
        assert_eq!(err_504.code(), Some(ErrorCode::Internal));
        assert_eq!(err_504.status(), Some(504));
        assert_eq!(err_504.detail(), Some("Gateway Timeout"));
        assert!(err_504.is_retryable());
    }

    #[test]
    fn test_retry_after_and_rate_limit_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", HeaderValue::from_static("30"));
        headers.insert("x-ratelimit-limit", HeaderValue::from_static("100"));
        headers.insert("x-ratelimit-remaining", HeaderValue::from_static("0"));
        headers.insert("x-ratelimit-reset", HeaderValue::from_static("15"));
        headers.insert("x-request-id", HeaderValue::from_static("req_header_789"));

        let err = Error::from_response_parts(
            StatusCode::TOO_MANY_REQUESTS,
            &headers,
            br#"{"code":"RATE_LIMITED","detail":"Limit exceeded"}"#,
        );

        assert_eq!(err.code(), Some(ErrorCode::RateLimited));
        assert_eq!(err.status(), Some(429));
        assert_eq!(err.retry_after(), Some(Duration::from_secs(30)));
        assert_eq!(
            err.rate_limit(),
            Some(RateLimit {
                limit: 100,
                remaining: 0,
                reset: 15,
            })
        );
        assert_eq!(err.request_id(), Some("req_header_789"));
    }

    #[test]
    fn test_display_formatting() {
        let err_with_all = Error::Api {
            code: ErrorCode::NotFound,
            status: 404,
            detail: Some("Actor not found".to_string()),
            request_id: Some("req_abc123".to_string()),
            retry_after: None,
            rate_limit: None,
        };
        assert_eq!(
            err_with_all.to_string(),
            "[404 NotFound] Actor not found (request_id=req_abc123)"
        );

        let err_no_req_id = Error::Api {
            code: ErrorCode::ValidationFailed,
            status: 400,
            detail: Some("Invalid email".to_string()),
            request_id: None,
            retry_after: None,
            rate_limit: None,
        };
        assert_eq!(
            err_no_req_id.to_string(),
            "[400 ValidationFailed] Invalid email"
        );

        let err_no_detail = Error::Api {
            code: ErrorCode::Internal,
            status: 500,
            detail: None,
            request_id: None,
            retry_after: None,
            rate_limit: None,
        };
        assert_eq!(err_no_detail.to_string(), "[500 Internal]");

        let config_err = Error::Config("invalid base URL".to_string());
        assert_eq!(
            config_err.to_string(),
            "yapılandırma hatası: invalid base URL"
        );

        let decode_err = Error::Decode("JSON error".to_string());
        assert_eq!(decode_err.to_string(), "yanıt çözümlenemedi: JSON error");
    }
}
