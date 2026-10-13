use serde::{Deserialize, Serialize};

use crate::errors::Error;
use crate::http::HttpClient;
use crate::types::{ExchangeCodeParams, Plan, TokenResponse};
use crate::DEFAULT_BASE_URL;

#[derive(Serialize)]
struct ExchangeBody<'a> {
    grant_type: &'static str,
    code: &'a str,
    client_id: &'a str,
    client_secret: &'a str,
    redirect_uri: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    code_verifier: Option<&'a str>,
}

/// Raw (snake_case) wire shape of `POST /oauth/token`.
#[derive(Deserialize)]
struct RawTokenResponse {
    access_token: String,
    token_type: String,
    client_id: String,
    team_id: String,
    scopes: Vec<String>,
    plan: Plan,
}

/// Standalone / free function (not a `Client` method), because it is
/// called *before* a token exists and needs no bearer auth — it
/// authenticates via the confidential client's `client_id` +
/// `client_secret` in the request body, per `POST {base_url}/oauth/token`.
///
/// `code_verifier` is omitted from the request body when `None` or an
/// empty string.
pub async fn exchange_code(params: ExchangeCodeParams) -> Result<TokenResponse, Error> {
    let base_url = params
        .base_url
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
    let base_url = base_url.trim_end_matches('/').to_string();
    let http = HttpClient::new(base_url, None, None);

    let code_verifier = params.code_verifier.as_deref().filter(|s| !s.is_empty());
    let body = ExchangeBody {
        grant_type: "authorization_code",
        code: &params.code,
        client_id: &params.client_id,
        client_secret: &params.client_secret,
        redirect_uri: &params.redirect_uri,
        code_verifier,
    };

    let raw: RawTokenResponse = http.post("/oauth/token", &body).await?;
    Ok(TokenResponse {
        access_token: raw.access_token,
        token_type: raw.token_type,
        client_id: raw.client_id,
        team_id: raw.team_id,
        scopes: raw.scopes,
        plan: raw.plan,
    })
}
