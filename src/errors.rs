use thiserror::Error;

/// The unified error type for all SeeItLive SDK operations.
///
/// Every fallible operation (client construction, session calls, OAuth
/// verification, code exchange) returns `Result<T, Error>`. Non-2xx API
/// responses map to a status-specific variant carrying the API's
/// `message` and optional `code`; construction-time validation problems
/// (missing/duplicate credentials) surface as `Error::Validation`;
/// network/transport failures (DNS, connection refused, timeouts,
/// response decoding) surface as `Error::Transport`, unwrapped.
#[derive(Debug, Error)]
pub enum Error {
    /// HTTP 400, or a construction-time validation failure (e.g. missing
    /// or duplicate credentials). `field` is populated when the API
    /// response body includes a `field` property.
    #[error("{message}")]
    Validation {
        message: String,
        field: Option<String>,
        code: Option<String>,
    },

    /// HTTP 401.
    #[error("{message}")]
    Authentication {
        message: String,
        code: Option<String>,
    },

    /// HTTP 403.
    #[error("{message}")]
    Permission {
        message: String,
        code: Option<String>,
    },

    /// HTTP 404.
    #[error("{message}")]
    NotFound {
        message: String,
        code: Option<String>,
    },

    /// HTTP 429. `retry_after` is `Some(n)` when the response carried a
    /// numeric `Retry-After: n` header (seconds), and `None` when the
    /// header was absent or unparsable. A legitimate `Retry-After: 0`
    /// (retry immediately) is preserved as `Some(0)` — do not treat
    /// `None` as "no need to wait"; check explicitly.
    #[error("{message}")]
    RateLimit {
        message: String,
        code: Option<String>,
        retry_after: Option<u64>,
    },

    /// Any other non-2xx status not covered by a dedicated variant above.
    #[error("API error {status}: {message}")]
    Api {
        status: u16,
        code: Option<String>,
        message: String,
    },

    /// Network/transport failure (DNS, connection refused, timeout,
    /// response body decoding, etc.) — the underlying `reqwest::Error`,
    /// not wrapped in a SeeItLive-specific type.
    #[error(transparent)]
    Transport(#[from] reqwest::Error),

    /// Request body failed to serialize to JSON. In practice this should
    /// not happen for the SDK's own request types, but is surfaced rather
    /// than panicking.
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
}
