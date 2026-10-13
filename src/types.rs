use serde::{Deserialize, Serialize};

/// Which SeeItLive environment a credential belongs to, inferred from the
/// token prefix (`sil_test_...` -> `Test`, else `Live`) unless
/// [`Config::env`] overrides it explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Env {
    Live,
    Test,
}

/// Configuration for [`crate::Client::new`]. Exactly one of `api_key` /
/// `oauth_token` must be set (a present-but-empty string is treated as
/// absent); providing neither, or both, is a construction-time error.
#[derive(Debug, Clone, Default)]
pub struct Config {
    pub api_key: Option<String>,
    pub oauth_token: Option<String>,
    /// Defaults to `https://api.seeitlive.io/v1`. Trailing slashes are
    /// trimmed.
    pub base_url: Option<String>,
    /// Overrides env inference from the token prefix.
    pub env: Option<Env>,
    /// Optional shared secret used to HMAC-sign every outgoing request
    /// (`X-SeeItLive-Signature` / `X-SeeItLive-Timestamp` headers). A
    /// present-but-empty string is treated the same as absent (unset), and
    /// the SDK stays backwards-compatible: requests are unsigned by
    /// default.
    pub signing_secret: Option<String>,
}

/// Delivery channel for a session invite.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InviteChannel {
    Sms,
    Email,
    Link,
}

/// Parameters for [`crate::Sessions::create`]. The SDK does **not** send a
/// team id — the team is implicit from the API key / OAuth token.
/// The API requires `recipient_phone` for the `Sms` channel and
/// `recipient_email` for the `Email` channel; the SDK forwards whatever
/// is given and lets the API validate.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionParams {
    pub channel: InviteChannel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipient_phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipient_email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_minutes: Option<u32>,
    /// Attests the recipient agreed to receive a text. The API requires it
    /// to be `true` for the `Sms` channel (carrier fees may apply).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sms_consent: Option<bool>,
    /// Optional flat map of string keys to string values attached to the
    /// session; omitted from the request body when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, String>>,
    /// Requests that the API also mint a `sitl.cc/s/<code>` short link for
    /// the session invite; omitted from the request body when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_link: Option<bool>,
    /// Requires the guest to answer a challenge before sharing camera:
    /// `"password"` or `"totp"`. Omitted from the body when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protection: Option<String>,
    /// The password or base32 TOTP seed; the API generates one when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protection_secret: Option<String>,
}

impl CreateSessionParams {
    /// Convenience constructor for the common case of just a channel.
    pub fn new(channel: InviteChannel) -> Self {
        Self {
            channel,
            recipient_phone: None,
            recipient_email: None,
            note: None,
            ttl_minutes: None,
            sms_consent: None,
            metadata: None,
            short_link: None,
            protection: None,
            protection_secret: None,
        }
    }
}

/// Response from `POST /sessions`. Wire format is camelCase.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub status: String,
    pub invite_code: String,
    pub invite_url: String,
    pub invite_short_url: Option<String>,
    pub agent_view_url: String,
    pub metadata: Option<std::collections::HashMap<String, String>>,
    pub expires_at: Option<String>,
    pub protection: Option<String>,
    pub protection_secret: Option<String>,
    pub protection_otpauth_url: Option<String>,
}

/// Response from `GET /sessions/{id}/status`. Wire format is camelCase.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatus {
    pub id: String,
    pub status: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub producer_count: i64,
    pub has_video: bool,
}

/// The agent's view of a session's guest challenge. Wire format is camelCase.
///
/// For `totp`, `code` is the CURRENT code, never the seed — the seed leaves the
/// server only once, in the create response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectionInfo {
    pub r#type: String,
    pub password: Option<String>,
    pub code: Option<String>,
    pub seconds_remaining: Option<i64>,
    pub locked: bool,
    pub failed_attempts: i64,
}

/// Changes a session's guest challenge after creation.
///
/// `protection_secret` is skipped when `None`: absence is what tells the API to
/// generate one, so it must not be serialized as an explicit null.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetProtectionParams {
    pub protection: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protection_secret: Option<String>,
}

/// Options for reviving (or replacing) an expired invitation. Every field is
/// optional; a `None` field is skipped from the request body entirely and
/// means "reuse what the invitation already recorded".
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReshareParams {
    /// `"revive"` (the default) extends the existing link in place; `"new"`
    /// issues a fresh one and kills the old.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// Changes how the customer is reached. Omit to reuse the recorded
    /// channel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<InviteChannel>,
    /// Changes the phone number or email address. Omit to reuse the
    /// recorded one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipient: Option<String>,
    /// Required and must be `true` when `channel` is `Sms`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sms_consent: Option<bool>,
}

/// Reports how a revived invitation was (or wasn't) delivered.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReshareResult {
    pub expires_at: String,
    /// `None` when the session has no short link — it can be extended but
    /// not re-sent.
    pub invite_short_url: Option<String>,
    pub resend: String,
    /// `true` when a new code/link was issued (`mode: "new"`).
    pub rotated: bool,
}

/// A session's invitation as the agent side sees it — never the token hash
/// and never the guest secret. Returned nested in [`SessionDetail`].
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInvitation {
    pub code: String,
    /// `"sms"`, `"email"` or `"link"`. A plain `String` rather than
    /// [`InviteChannel`] so a channel added server-side later cannot fail
    /// deserialization of the whole session.
    pub channel: String,
    pub recipient: Option<String>,
    pub expires_at: String,
    pub opened_at: Option<String>,
}

/// Response from `GET /sessions/{id}` — a stored session read back by id,
/// unlike [`Session`] which is the one-time create response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    pub id: String,
    pub team_id: String,
    pub env: String,
    pub status: String,
    #[serde(default)]
    pub created_by_client_id: Option<String>,
    #[serde(default)]
    pub created_by_api_key_id: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub metadata: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub ended_at: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub invitation: Option<SessionInvitation>,
}

/// Parameters for [`crate::Sessions::reset_protection`]. Rotation is opt-in:
/// a lockout is usually the customer mistyping, and a new secret would just
/// make the agent read it out again. `Default` keeps the existing secret.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ResetProtectionParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotate: Option<bool>,
}

/// Parameters for [`crate::Sessions::create_agent_token`]. `label` names the
/// viewer in the dashboard's list (e.g. `"Odoo — crm.lead 42"`); leave it
/// `None` and the API applies its own default, branched by principal.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CreateAgentTokenParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// A freshly minted agent-view grant. Tokens expire after 120 minutes; mint
/// another to re-open a viewer.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentToken {
    pub code: String,
    pub token: String,
    pub agent_view_url: String,
}

/// One entry of [`crate::Snapshots::list`]. `url` is the API path the bytes
/// are served from.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotSummary {
    pub id: String,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub captured_at: String,
    pub url: String,
}

/// One snapshot fetched by [`crate::Snapshots::get`]. `data` is the
/// base64-encoded PNG.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotImage {
    pub id: String,
    pub content_type: String,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub captured_at: String,
    pub data: String,
}

/// Plan limits nested in OAuth responses. Wire format is camelCase;
/// unlimited tiers report `null` (`None`) for the limit fields.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub plan_key: String,
    pub plan_name: String,
    pub session_limit: Option<i64>,
    pub seat_limit: Option<i64>,
    pub concurrent_limit: Option<i64>,
}

/// Idiomatic (camelCase-mapped) result of [`crate::OAuth::verify`]. The
/// wire response is snake_case (`client_id`, `team_id`, `expires_at`);
/// this type is hand-mapped from the raw wire shape rather than derived.
#[derive(Debug, Clone)]
pub struct TokenInfo {
    pub valid: bool,
    pub client_id: String,
    pub team_id: String,
    pub scopes: Vec<String>,
    pub plan: Plan,
    pub expires_at: Option<String>,
}

/// Parameters for the standalone [`crate::exchange_code`] function.
#[derive(Debug, Clone)]
pub struct ExchangeCodeParams {
    pub code: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    /// PKCE code verifier. Omitted from the request body when `None` or
    /// empty.
    pub code_verifier: Option<String>,
    /// Defaults to `https://api.seeitlive.io/v1`.
    pub base_url: Option<String>,
}

/// Idiomatic (camelCase-mapped) result of [`crate::exchange_code`]. The
/// wire response is snake_case (`access_token`, `token_type`, etc.).
#[derive(Debug, Clone)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub client_id: String,
    pub team_id: String,
    pub scopes: Vec<String>,
    pub plan: Plan,
}
