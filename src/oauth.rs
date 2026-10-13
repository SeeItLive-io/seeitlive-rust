use serde::Deserialize;

use crate::errors::Error;
use crate::http::HttpClient;
use crate::types::{Plan, TokenInfo};

/// Raw (snake_case) wire shape of `GET /oauth/verify`.
#[derive(Deserialize)]
struct RawTokenInfo {
    valid: bool,
    client_id: String,
    team_id: String,
    scopes: Vec<String>,
    plan: Plan,
    expires_at: Option<String>,
}

/// The `oauth` resource: `verify`.
pub struct OAuth<'a> {
    http: &'a HttpClient,
}

impl<'a> OAuth<'a> {
    pub(crate) fn new(http: &'a HttpClient) -> Self {
        Self { http }
    }

    /// `GET /oauth/verify`, using the client's bearer token. Maps the
    /// snake_case wire response onto the idiomatic [`TokenInfo`].
    pub async fn verify(&self) -> Result<TokenInfo, Error> {
        let raw: RawTokenInfo = self.http.get("/oauth/verify").await?;
        Ok(TokenInfo {
            valid: raw.valid,
            client_id: raw.client_id,
            team_id: raw.team_id,
            scopes: raw.scopes,
            plan: raw.plan,
            expires_at: raw.expires_at,
        })
    }
}
