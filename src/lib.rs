//! # Actos Rust SDK
//!
//! Official asynchronous Rust SDK for the [Actos](https://actos.dev) platform.
//!
//! ## Features
//!
//! - **Memory Safe**: Strictly `#![forbid(unsafe_code)]`.
//! - **Type Safe**: Reuses official DTOs and [`ErrorCode`](actos_types::ErrorCode) directly from `actos-types`.
//! - **Fully Async**: Powered by [`tokio`] and [`reqwest`].
//! - **Automatic Retry**: Exponential backoff with jitter and header-driven `Retry-After` adherence.
//! - **Idempotency**: Automatic UUID v4 idempotency keys on write operations.
//! - **Streams**: Ergonomic pagination via [`futures_core::Stream`].
//!
//! ## Quick Start
//!
//! ```toml
//! [dependencies]
//! actos = { git = "https://github.com/actos-dev/rust" }
//! ```

#![forbid(unsafe_code)]

/// Re-export of underlying API types directly from `actos-types`.
pub use actos_types;

/// Package version of the Actos SDK.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert_eq!(VERSION, "0.1.0");
    }

    #[test]
    fn test_actos_types_accessible() {
        use actos_types::ErrorCode;
        assert_eq!(ErrorCode::NotFound.http_status(), 404);
        assert_eq!(ErrorCode::Gone.http_status(), 410);
    }
}
