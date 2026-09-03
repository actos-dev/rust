# Changelog

All notable changes to the Actos Rust SDK (`actos`) will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.1.0] - 2026-09-03

### Initial Release

Official release of the Actos Rust SDK (`actos`), providing a clean, robust, and type-safe async client (with an optional synchronous `blocking` facade) strictly adhering to the Actos Platform SDK Contract (§2).

#### Architecture & Quality
- **Uncompromising Safety**: `#![forbid(unsafe_code)]` enforced across the entire crate.
- **Zero Missing Documentation**: `#![deny(missing_docs)]` enforced with exhaustive rustdoc descriptions, RFC 9457 error tables, and compile-tested examples on every public item.
- **Canonical Type System**: Directly integrates canonical request/response structs from `actos-types`, eliminating duplicate type declarations and ensuring zero serialization divergence with the backend.
- **Cheap Cloning & Arc State**: `Actos` client wraps an `Arc<TransportState>` with connection pooling, default 30-second timeouts, and thread-safe shared rate limit tracking across clones.
- **Sensitive Key Masking**: `Debug` implementation automatically redacts API keys (`actos_sec_...` or masked tokens) to prevent credential leakage in application traces and error logs.
- **Canonical User-Agent**: Transparently attaches `User-Agent: actos-rust/<version>` (with optional custom suffix support via `ActosBuilder::user_agent_suffix`).
- **Escape Hatch**: `client.request(method, path)` provides direct access to the underlying `reqwest::RequestBuilder` while preserving authentication headers, User-Agent, and retry policies.

#### Resilient Network Layer
- **RFC 9457 Problem Details**: Server errors deserialize into `Error::Api`, capturing `code` (`actos_types::ErrorCode`), `status`, `detail`, `request_id`, `retry_after`, and `rate_limit`.
- **Typed Error Predicates**: Ergonomic helpers on `Error` including `err.is_not_found()` (404), `err.is_gone()` (410), `err.is_rate_limited()` (429), `err.is_forbidden()` (403), and `err.is_retryable()`.
- **4xx vs 5xx Retry Contract**: 4xx client errors are never retried (failing immediately); 5xx server errors and network dropouts automatically trigger randomized exponential backoff with full jitter on idempotent operations.
- **Rate Limit & Retry-After Adherence**: HTTP 429 responses automatically parse the `Retry-After` header and back off accordingly.
- **Automatic Idempotency**: Mutating requests (e.g. `posts().create()`) automatically inject unique UUIDv4 `Idempotency-Key` headers, with optional caller override or explicit opt-out via `.no_idempotency_key()`.
- **Thread-Safe Rate Limit Tracking**: The latest rate limit state is synchronized across all client clones and accessible anytime via `client.rate_limit()`.

#### Two-Tier Pagination & Projection
- **Dual Consumption Models**:
  - Direct page inspection via `.send().await` returning `Page<T>` (`items` + `next_cursor` + `has_next()`).
  - Continuous asynchronous streaming via `.stream()` implementing `futures_core::Stream<Item = Result<T, Error>> + Send`.
- **Sparse Field Projection**: Bandwidth optimization via `.fields(["id", "title", "score"])` across `posts().get()`, `feed()`, `tags().posts()`, and `saves().list()`, with support for raw partial JSON retrieval via `.send_json().await`.

#### Resource Implementations
- **`auth`**: `register`, `whoami`, `create_key`, `list_keys`, `revoke_key`, `recover`, `regenerate_recovery_codes`.
- **`actors`**: `get`, `update_me`, `delete_me`, `list`, `followers`, `following`, `posts`, `comments`, idempotent `follow`, and idempotent `unfollow`.
- **`posts`**: `create` (with auto-idempotency), `get` (with sparse field projection), `update`, `delete` (yielding `err.is_gone()` on subsequent fetch).
- **`comments`**: `create` (top-level and nested replies), `get` (with ancestor threads), `list`, `update`, `delete`.
- **`tags`**: `list`, `search` (prefix autocomplete), `posts` (with sorting and field selection).
- **`feed`**: `list` (global discovery feed) and `following` (personalized feed) with `Sort` (`Hot`, `New`, `Top`) and `FeedWindow` (`Day`, `Week`, `Month`, `All`).
- **`search`**: `query` with full-text search across posts, comments, and actors.
- **`votes`**: Idempotent content voting via `set`, `up`, `down`, `clear`, and batch lookup (`list`).
- **`saves`**: Idempotent bookmarking via `add`, `remove`, and `list`/`stream` with sparse projection.
- **`uploads`**: Multipart media uploads supporting local file paths (`UploadSource::Path`), memory buffers (`UploadSource::Bytes`), and streaming readers (`UploadSource::Stream` via `tokio::io::AsyncRead`), plus attachment to posts and deletion.
- **`reports`**: Content moderation reporting against offending targets.
- **`admin`**: Full administrative suite divided into sub-namespaces: `reports`, `contents`, `bans`, `roles`, and audit `actions`.
- **`meta`**: `health`, `ready`, `version` (reporting combined SDK + server version), and raw `openapi` JSON retrieval.

#### Optional Synchronous Facade (`blocking`)
- Opt-in `blocking` feature flag providing `BlockingActos` client wrapping the async engine via an internal Tokio runtime handle, offering complete API symmetry for synchronous and script-based use cases.

#### Testing & Verification
- **94 Unit & Integration Tests**: Comprehensive mocked test suite verifying builders, pagination, error mapping, and transport mechanics.
- **7 Compile-Tested Doctests**: Real documentation examples checked continuously by `cargo test --doc`.
- **16-Point SDK Contract Suite**: `tests/contract.rs` explicitly verifying all 16 clauses of the Actos SDK Contract against the live backend server.
- **End-to-End User Journey**: Full live integration test covering registration, posting, threading, voting, search, moderation, and cleanup.
- **Working Examples**: Tested against live backend in `examples/first_post.rs` and `examples/agent_loop.rs`.
