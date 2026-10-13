use crate::errors::Error;
use crate::http::HttpClient;
use crate::oauth::OAuth;
use crate::sessions::Sessions;
use crate::snapshots::Snapshots;
use crate::types::{Config, Env};
use crate::DEFAULT_BASE_URL;

/// The SeeItLive API client. Construct with [`Client::new`].
pub struct Client {
    env: Env,
    http: HttpClient,
}

/// Manual `Debug` impl: `http` (`HttpClient`) already redacts its own
/// auth header, but we implement this explicitly (rather than deriving)
/// so a future field holding secret material doesn't silently start
/// leaking via an auto-derived `Debug`.
impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("env", &self.env)
            .field("http", &self.http)
            .finish()
    }
}

impl Client {
    /// Construct a `Client`. Exactly one of `config.api_key` or
    /// `config.oauth_token` must be set — a present-but-empty string is
    /// treated the same as absent (matching the Node reference SDK's
    /// falsy check). Providing neither, or both, returns
    /// `Err(Error::Validation)`.
    ///
    /// `env` is inferred from the token prefix (a token starting
    /// `sil_test_` -> `Env::Test`, else `Env::Live`) unless
    /// `config.env` is set explicitly. `base_url` defaults to
    /// `https://api.seeitlive.io/v1` (trailing slashes trimmed).
    pub fn new(config: Config) -> Result<Self, Error> {
        let api_key = config.api_key.filter(|s| !s.is_empty());
        let oauth_token = config.oauth_token.filter(|s| !s.is_empty());

        let token = match (api_key, oauth_token) {
            (Some(_), Some(_)) => {
                return Err(Error::Validation {
                    message: "Provide only one of apiKey or oauthToken".to_string(),
                    field: None,
                    code: None,
                })
            }
            (None, None) => {
                return Err(Error::Validation {
                    message: "Either apiKey or oauthToken must be provided".to_string(),
                    field: None,
                    code: None,
                })
            }
            (Some(t), None) => t,
            (None, Some(t)) => t,
        };

        let env = config.env.unwrap_or(if token.starts_with("sil_test_") {
            Env::Test
        } else {
            Env::Live
        });

        let base_url = config
            .base_url
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
        let base_url = base_url.trim_end_matches('/').to_string();

        // Same empty-string-means-absent convention as api_key/oauth_token above.
        let signing_secret = config.signing_secret.filter(|s| !s.is_empty());
        let http = HttpClient::new(base_url, Some(format!("Bearer {token}")), signing_secret);

        Ok(Self { env, http })
    }

    /// The inferred (or overridden) environment for this client.
    pub fn env(&self) -> Env {
        self.env
    }

    /// Access the `sessions` resource (`create`, `status`).
    pub fn sessions(&self) -> Sessions<'_> {
        Sessions::new(&self.http)
    }

    /// Access the `snapshots` resource (`list`, `get`).
    pub fn snapshots(&self) -> Snapshots<'_> {
        Snapshots::new(&self.http)
    }

    /// Access the `oauth` resource (`verify`). Only meaningful when the
    /// client was constructed with an `oauth_token`.
    pub fn oauth(&self) -> OAuth<'_> {
        OAuth::new(&self.http)
    }
}
