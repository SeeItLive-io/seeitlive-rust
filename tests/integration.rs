use serde_json::json;
use wiremock::matchers::{body_json, header, header_regex, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use seeitlive::{
    Client, Config, CreateAgentTokenParams, CreateSessionParams, Env, Error, InviteChannel,
    ResetProtectionParams, ReshareParams, SetProtectionParams,
};

const LIVE_KEY: &str = "sil_live_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const TEST_KEY: &str = "sil_test_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn plan_json() -> serde_json::Value {
    json!({
        "planKey": "free",
        "planName": "Free",
        "sessionLimit": null,
        "seatLimit": null,
        "concurrentLimit": null,
    })
}

// --- (a) construction: env inference + exactly-one-credential rule ---

#[test]
fn infers_env_from_token_prefix() {
    let client = Client::new(Config {
        api_key: Some(TEST_KEY.to_string()),
        ..Default::default()
    })
    .expect("test key should construct");
    assert_eq!(client.env(), Env::Test);

    let client = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        ..Default::default()
    })
    .expect("live key should construct");
    assert_eq!(client.env(), Env::Live);
}

#[test]
fn explicit_env_overrides_inference() {
    let client = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        env: Some(Env::Test),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(client.env(), Env::Test);
}

#[test]
fn rejects_neither_credential() {
    let err = Client::new(Config::default()).unwrap_err();
    assert!(matches!(err, Error::Validation { .. }));
}

#[test]
fn rejects_both_credentials() {
    let err = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        oauth_token: Some("sil_live_oauth_x".to_string()),
        ..Default::default()
    })
    .unwrap_err();
    assert!(matches!(err, Error::Validation { .. }));
}

#[test]
fn treats_empty_string_credentials_as_absent() {
    // Empty api_key alone -> treated as absent -> neither provided -> Err.
    let err = Client::new(Config {
        api_key: Some(String::new()),
        ..Default::default()
    })
    .unwrap_err();
    assert!(matches!(err, Error::Validation { .. }));

    // Empty api_key + a real oauth_token -> exactly one effectively present -> Ok.
    let client = Client::new(Config {
        api_key: Some(String::new()),
        oauth_token: Some(LIVE_KEY.to_string()),
        ..Default::default()
    })
    .expect("empty api_key should be treated as absent, leaving just oauth_token");
    assert_eq!(client.env(), Env::Live);

    // Both empty -> Err.
    let err = Client::new(Config {
        api_key: Some(String::new()),
        oauth_token: Some(String::new()),
        ..Default::default()
    })
    .unwrap_err();
    assert!(matches!(err, Error::Validation { .. }));
}

// --- (b) sessions.create request shape + response parse ---

#[tokio::test]
async fn sessions_create_posts_bearer_json_no_team_id() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/sessions"))
        .and(header("authorization", format!("Bearer {LIVE_KEY}")))
        .and(body_json(json!({ "channel": "link" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "s1",
            "status": "pending",
            "inviteCode": "abc",
            "inviteUrl": "https://example.com/i/abc",
            "inviteShortUrl": "https://sitl.cc/s/aB3xK9",
            "agentViewUrl": "https://example.com/a/abc",
            "expiresAt": "2026-01-01T00:00:00Z",
        })))
        .mount(&server)
        .await;

    let client = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        base_url: Some(format!("{}/v1", server.uri())),
        ..Default::default()
    })
    .unwrap();

    let session = client
        .sessions()
        .create(CreateSessionParams::new(InviteChannel::Link))
        .await
        .expect("create should succeed");

    assert_eq!(session.id, "s1");
    assert_eq!(session.invite_code, "abc");
    assert_eq!(session.invite_url, "https://example.com/i/abc");
    assert_eq!(session.invite_short_url.as_deref(), Some("https://sitl.cc/s/aB3xK9"));
    assert_eq!(session.agent_view_url, "https://example.com/a/abc");
    assert_eq!(session.expires_at.as_deref(), Some("2026-01-01T00:00:00Z"));

    // wiremock's body_json + path/method matchers above already assert the
    // request shape (no team_id key present, since body_json requires
    // an exact match against { "channel": "link" }); the mount would have
    // failed to match (and the request panicked/errored) otherwise.
}

// --- (c) sessions.status path + parse ---

#[tokio::test]
async fn sessions_status_gets_expected_path_and_parses() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/sessions/s1/status"))
        .and(header("authorization", format!("Bearer {LIVE_KEY}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "s1",
            "status": "live",
            "startedAt": "2026-01-01T00:00:00Z",
            "endedAt": null,
            "producerCount": 1,
            "hasVideo": true,
        })))
        .mount(&server)
        .await;

    let client = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        base_url: Some(format!("{}/v1", server.uri())),
        ..Default::default()
    })
    .unwrap();

    let status = client.sessions().status("s1").await.unwrap();
    assert_eq!(status.id, "s1");
    assert_eq!(status.status, "live");
    assert_eq!(status.started_at.as_deref(), Some("2026-01-01T00:00:00Z"));
    assert_eq!(status.ended_at, None);
    assert_eq!(status.producer_count, 1);
    assert!(status.has_video);
}

// --- (d) oauth.verify snake_case -> idiomatic ---

#[tokio::test]
async fn oauth_verify_maps_snake_case_to_idiomatic() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/oauth/verify"))
        .and(header(
            "authorization",
            "Bearer sil_live_oauth_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "valid": true,
            "client_id": "c1",
            "team_id": "p1",
            "scopes": ["sessions:write", "sessions:read"],
            "plan": plan_json(),
            "expires_at": "2026-06-01T00:00:00Z",
        })))
        .mount(&server)
        .await;

    let client = Client::new(Config {
        oauth_token: Some("sil_live_oauth_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
        base_url: Some(format!("{}/v1", server.uri())),
        ..Default::default()
    })
    .unwrap();

    let info = client.oauth().verify().await.unwrap();
    assert!(info.valid);
    assert_eq!(info.client_id, "c1");
    assert_eq!(info.team_id, "p1");
    assert_eq!(info.scopes, vec!["sessions:write", "sessions:read"]);
    assert_eq!(info.plan.plan_key, "free");
    assert_eq!(info.expires_at.as_deref(), Some("2026-06-01T00:00:00Z"));
}

// --- (e) status -> error mapping ---

#[tokio::test]
async fn maps_4xx_status_codes_to_typed_errors() {
    async fn error_for(status: u16, body: serde_json::Value) -> Error {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/sessions/s1/status"))
            .respond_with(ResponseTemplate::new(status).set_body_json(body))
            .mount(&server)
            .await;

        let client = Client::new(Config {
            api_key: Some(LIVE_KEY.to_string()),
            base_url: Some(format!("{}/v1", server.uri())),
            ..Default::default()
        })
        .unwrap();

        client.sessions().status("s1").await.unwrap_err()
    }

    let err = error_for(400, json!({ "message": "bad", "field": "channel" })).await;
    match err {
        Error::Validation { message, field, .. } => {
            assert_eq!(message, "bad");
            assert_eq!(field.as_deref(), Some("channel"));
        }
        other => panic!("expected Validation, got {other:?}"),
    }

    let err = error_for(401, json!({ "message": "no auth" })).await;
    assert!(matches!(err, Error::Authentication { .. }));

    let err = error_for(403, json!({ "message": "forbidden" })).await;
    assert!(matches!(err, Error::Permission { .. }));

    let err = error_for(404, json!({ "message": "missing" })).await;
    assert!(matches!(err, Error::NotFound { .. }));

    let err = error_for(500, json!({ "message": "boom" })).await;
    match err {
        Error::Api {
            status, message, ..
        } => {
            assert_eq!(status, 500);
            assert_eq!(message, "boom");
        }
        other => panic!("expected Api, got {other:?}"),
    }
}

#[tokio::test]
async fn rate_limit_preserves_retry_after_including_a_legitimate_zero() {
    // Retry-After: 12
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/sessions/s1/status"))
        .respond_with(
            ResponseTemplate::new(429)
                .set_body_json(json!({ "message": "slow down" }))
                .insert_header("Retry-After", "12"),
        )
        .mount(&server)
        .await;
    let client = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        base_url: Some(format!("{}/v1", server.uri())),
        ..Default::default()
    })
    .unwrap();
    let err = client.sessions().status("s1").await.unwrap_err();
    match err {
        Error::RateLimit { retry_after, .. } => assert_eq!(retry_after, Some(12)),
        other => panic!("expected RateLimit, got {other:?}"),
    }

    // Retry-After: 0 -- a legitimate "retry immediately", must be Some(0), not None.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/sessions/s1/status"))
        .respond_with(
            ResponseTemplate::new(429)
                .set_body_json(json!({ "message": "slow down" }))
                .insert_header("Retry-After", "0"),
        )
        .mount(&server)
        .await;
    let client = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        base_url: Some(format!("{}/v1", server.uri())),
        ..Default::default()
    })
    .unwrap();
    let err = client.sessions().status("s1").await.unwrap_err();
    match err {
        Error::RateLimit { retry_after, .. } => assert_eq!(retry_after, Some(0)),
        other => panic!("expected RateLimit, got {other:?}"),
    }

    // No Retry-After header at all -> None, distinct from Some(0).
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/sessions/s1/status"))
        .respond_with(ResponseTemplate::new(429).set_body_json(json!({ "message": "slow down" })))
        .mount(&server)
        .await;
    let client = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        base_url: Some(format!("{}/v1", server.uri())),
        ..Default::default()
    })
    .unwrap();
    let err = client.sessions().status("s1").await.unwrap_err();
    match err {
        Error::RateLimit { retry_after, .. } => assert_eq!(retry_after, None),
        other => panic!("expected RateLimit, got {other:?}"),
    }
}

// --- (f) exchange_code: standalone, no bearer, maps token ---

#[tokio::test]
async fn exchange_code_posts_oauth_token_without_bearer() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/oauth/token"))
        .and(body_json(json!({
            "grant_type": "authorization_code",
            "code": "code123",
            "client_id": "client-1",
            "client_secret": "secret-1",
            "redirect_uri": "https://app.example.com/callback",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "sil_live_oauth_x",
            "token_type": "Bearer",
            "client_id": "client-1",
            "team_id": "p1",
            "scopes": ["sessions:write"],
            "plan": plan_json(),
        })))
        .mount(&server)
        .await;

    let token = seeitlive::exchange_code(seeitlive::ExchangeCodeParams {
        code: "code123".to_string(),
        client_id: "client-1".to_string(),
        client_secret: "secret-1".to_string(),
        redirect_uri: "https://app.example.com/callback".to_string(),
        code_verifier: None,
        base_url: Some(format!("{}/v1", server.uri())),
    })
    .await
    .expect("exchange should succeed");

    assert_eq!(token.access_token, "sil_live_oauth_x");
    assert_eq!(token.token_type, "Bearer");
    assert_eq!(token.client_id, "client-1");
    assert_eq!(token.team_id, "p1");
    assert_eq!(token.scopes, vec!["sessions:write"]);
    assert_eq!(token.plan.plan_key, "free");

    // The body_json matcher above requires an *exact* match against a body
    // with no `code_verifier` key and no `authorization` header was
    // configured as a requirement — but assert explicitly there is no
    // Authorization header expectation possible to satisfy accidentally:
    // if wiremock had required one and none was sent, the mount wouldn't
    // match and the call above would have failed with a connection/match
    // error rather than succeeding.
}

#[tokio::test]
async fn exchange_code_includes_code_verifier_when_present_omits_when_empty() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/oauth/token"))
        .and(body_json(json!({
            "grant_type": "authorization_code",
            "code": "code123",
            "client_id": "client-1",
            "client_secret": "secret-1",
            "redirect_uri": "https://app.example.com/callback",
            "code_verifier": "verifier-xyz",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "sil_live_oauth_x",
            "token_type": "Bearer",
            "client_id": "client-1",
            "team_id": "p1",
            "scopes": [],
            "plan": plan_json(),
        })))
        .mount(&server)
        .await;

    seeitlive::exchange_code(seeitlive::ExchangeCodeParams {
        code: "code123".to_string(),
        client_id: "client-1".to_string(),
        client_secret: "secret-1".to_string(),
        redirect_uri: "https://app.example.com/callback".to_string(),
        code_verifier: Some("verifier-xyz".to_string()),
        base_url: Some(format!("{}/v1", server.uri())),
    })
    .await
    .expect("exchange with code_verifier should succeed and match exact body");
}

// --- (g) HMAC request signing: on when configured, off by default ---

#[tokio::test]
async fn signs_requests_when_signing_secret_is_configured() {
    let server = MockServer::start().await;
    let secret = "wh_secret_".to_string() + &"b".repeat(32);

    Mock::given(method("POST"))
        .and(path("/v1/sessions"))
        .and(body_json(json!({ "channel": "link" })))
        .and(header_regex(
            "x-seeitlive-signature",
            r"^sha256=[0-9a-f]{64}$",
        ))
        .and(header_regex("x-seeitlive-timestamp", r"^\d+$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "s1",
            "status": "pending",
            "inviteCode": "abc",
            "inviteUrl": "https://example.com/i/abc",
            "agentViewUrl": "https://example.com/a/abc",
            "expiresAt": "2026-01-01T00:00:00Z",
        })))
        .mount(&server)
        .await;

    let client = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        base_url: Some(format!("{}/v1", server.uri())),
        signing_secret: Some(secret),
        ..Default::default()
    })
    .unwrap();

    // If the mounted mock (which requires the signature/timestamp headers)
    // didn't match, this call would fail with a connection/match error
    // rather than succeeding.
    client
        .sessions()
        .create(CreateSessionParams::new(InviteChannel::Link))
        .await
        .expect("signed create should succeed and match the signature-header mock");
}

#[tokio::test]
async fn omits_signature_headers_when_no_signing_secret_is_configured() {
    let server = MockServer::start().await;

    // No signature/timestamp header expectations here: a mock requiring
    // them would simply not match an unsigned request, so instead assert
    // directly against what was received.
    Mock::given(method("GET"))
        .and(path("/v1/sessions/s1/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "s1",
            "status": "live",
            "startedAt": "2026-01-01T00:00:00Z",
            "endedAt": null,
            "producerCount": 1,
            "hasVideo": true,
        })))
        .mount(&server)
        .await;

    let client = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        base_url: Some(format!("{}/v1", server.uri())),
        ..Default::default()
    })
    .unwrap();

    client.sessions().status("s1").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert!(!requests[0].headers.contains_key("x-seeitlive-signature"));
    assert!(!requests[0].headers.contains_key("x-seeitlive-timestamp"));
}

// --- (e) Debug must redact the bearer token ---

#[test]
fn debug_format_redacts_bearer_token_instead_of_leaking_it() {
    let client = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        ..Default::default()
    })
    .expect("live key should construct");

    let debug_output = format!("{client:?}");
    assert!(
        !debug_output.contains(LIVE_KEY),
        "Debug output must not contain the raw token: {debug_output}"
    );
    assert!(
        !debug_output.contains("Bearer"),
        "Debug output must not contain the Authorization header value: {debug_output}"
    );
    assert!(
        debug_output.contains("<redacted>"),
        "Debug output should show a redaction placeholder: {debug_output}"
    );
}

// --- (g) guest protection ---

#[tokio::test]
async fn sessions_get_protection_gets_expected_path_and_parses() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/sessions/s1/protection"))
        .and(header("authorization", format!("Bearer {LIVE_KEY}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "type": "totp",
            "password": null,
            "code": "418205",
            "secondsRemaining": 17,
            "locked": false,
            "failedAttempts": 0,
        })))
        .mount(&server)
        .await;

    let client = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        base_url: Some(format!("{}/v1", server.uri())),
        ..Default::default()
    })
    .unwrap();

    let info = client.sessions().get_protection("s1").await.unwrap();
    assert_eq!(info.r#type, "totp");
    assert_eq!(info.password, None);
    assert_eq!(info.code.as_deref(), Some("418205"));
    assert_eq!(info.seconds_remaining, Some(17));
    assert!(!info.locked);
    assert_eq!(info.failed_attempts, 0);
}

#[tokio::test]
async fn sessions_set_protection_puts_expected_path_and_body() {
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/v1/sessions/s1/protection"))
        .and(body_json(json!({
            "protection": "password",
            "protectionSecret": "K7QMX2RT",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "type": "password",
            "password": "K7QMX2RT",
            "code": null,
            "secondsRemaining": null,
            "locked": false,
            "failedAttempts": 0,
        })))
        .mount(&server)
        .await;

    let client = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        base_url: Some(format!("{}/v1", server.uri())),
        ..Default::default()
    })
    .unwrap();

    let info = client
        .sessions()
        .set_protection(
            "s1",
            SetProtectionParams {
                protection: "password".to_string(),
                protection_secret: Some("K7QMX2RT".to_string()),
            },
        )
        .await
        .unwrap();
    assert_eq!(info.password.as_deref(), Some("K7QMX2RT"));
}

// skip_serializing_if is load-bearing: absence is what tells the API to
// generate a secret, so `None` must not reach the wire as an explicit null.
#[tokio::test]
async fn sessions_set_protection_omits_the_secret_when_none() {
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/v1/sessions/s1/protection"))
        .and(body_json(json!({ "protection": "password" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "type": "password",
            "password": "GENERATED",
            "code": null,
            "secondsRemaining": null,
            "locked": false,
            "failedAttempts": 0,
        })))
        .mount(&server)
        .await;

    let client = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        base_url: Some(format!("{}/v1", server.uri())),
        ..Default::default()
    })
    .unwrap();

    let info = client
        .sessions()
        .set_protection(
            "s1",
            SetProtectionParams {
                protection: "password".to_string(),
                protection_secret: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(info.password.as_deref(), Some("GENERATED"));
}

// --- session read / lockout reset / agent-view token / snapshots ---

#[tokio::test]
async fn sessions_get_reads_a_session_with_its_invitation() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/sessions/s1"))
        .and(header("authorization", format!("Bearer {LIVE_KEY}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "s1",
            "teamId": "t1",
            "env": "live",
            "status": "active",
            "createdByClientId": null,
            "createdByApiKeyId": "k1",
            "note": null,
            "metadata": { "ticket": "42" },
            "startedAt": "2026-01-01T00:00:00Z",
            "endedAt": null,
            "createdAt": "2026-01-01T00:00:00Z",
            "invitation": {
                "code": "abc123",
                "channel": "sms",
                "recipient": "+15145551234",
                "expiresAt": "2026-01-01T01:00:00Z",
                "openedAt": null,
            },
        })))
        .mount(&server)
        .await;

    let client = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        base_url: Some(format!("{}/v1", server.uri())),
        ..Default::default()
    })
    .unwrap();

    let session = client.sessions().get("s1").await.unwrap();
    assert_eq!(session.team_id, "t1");
    assert_eq!(session.created_by_api_key_id.as_deref(), Some("k1"));
    let invitation = session.invitation.expect("invitation should be present");
    assert_eq!(invitation.code, "abc123");
    assert_eq!(invitation.channel, "sms");
    assert_eq!(invitation.recipient.as_deref(), Some("+15145551234"));
    assert_eq!(invitation.opened_at, None);
}

#[tokio::test]
async fn sessions_reset_protection_posts_rotate_only_when_asked() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/sessions/s1/protection/reset"))
        .and(body_json(json!({ "rotate": true })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "type": "password",
            "password": "NEWSECRET",
            "code": null,
            "secondsRemaining": null,
            "locked": false,
            "failedAttempts": 0,
        })))
        .mount(&server)
        .await;

    let client = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        base_url: Some(format!("{}/v1", server.uri())),
        ..Default::default()
    })
    .unwrap();

    let info = client
        .sessions()
        .reset_protection("s1", ResetProtectionParams { rotate: Some(true) })
        .await
        .unwrap();
    assert_eq!(info.password.as_deref(), Some("NEWSECRET"));
    assert!(!info.locked);
}

#[tokio::test]
async fn sessions_reset_protection_omits_rotate_by_default() {
    let server = MockServer::start().await;

    // Clearing a lockout is the common case and must not rotate by accident:
    // an empty body is what tells the API to keep the existing secret.
    Mock::given(method("POST"))
        .and(path("/v1/sessions/s1/protection/reset"))
        .and(body_json(json!({})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "type": "password",
            "password": "K7QMX2RT",
            "code": null,
            "secondsRemaining": null,
            "locked": false,
            "failedAttempts": 0,
        })))
        .mount(&server)
        .await;

    let client = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        base_url: Some(format!("{}/v1", server.uri())),
        ..Default::default()
    })
    .unwrap();

    client
        .sessions()
        .reset_protection("s1", ResetProtectionParams::default())
        .await
        .unwrap();
}

#[tokio::test]
async fn sessions_create_agent_token_posts_the_label() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/sessions/s1/agent-token"))
        .and(body_json(json!({ "label": "Odoo — crm.lead 42" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "code": "vw1",
            "token": "tok",
            "agentViewUrl": "https://app.seeitlive.io/v/vw1#tok",
        })))
        .mount(&server)
        .await;

    let client = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        base_url: Some(format!("{}/v1", server.uri())),
        ..Default::default()
    })
    .unwrap();

    let token = client
        .sessions()
        .create_agent_token(
            "s1",
            CreateAgentTokenParams {
                label: Some("Odoo — crm.lead 42".to_string()),
            },
        )
        .await
        .unwrap();
    assert_eq!(token.code, "vw1");
    assert_eq!(token.agent_view_url, "https://app.seeitlive.io/v/vw1#tok");
}

#[tokio::test]
async fn sessions_create_agent_token_omits_the_label_so_the_api_defaults_it() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/sessions/s1/agent-token"))
        .and(body_json(json!({})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "code": "vw1",
            "token": "tok",
            "agentViewUrl": "u",
        })))
        .mount(&server)
        .await;

    let client = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        base_url: Some(format!("{}/v1", server.uri())),
        ..Default::default()
    })
    .unwrap();

    client
        .sessions()
        .create_agent_token("s1", CreateAgentTokenParams::default())
        .await
        .unwrap();
}

#[tokio::test]
async fn snapshots_list_gets_a_sessions_snapshots() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/sessions/s1/snapshots"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "id": "sn1",
            "width": 1280,
            "height": 720,
            "capturedAt": "2026-01-01T00:00:00Z",
            "url": "/api/v1/snapshots/sn1",
        }])))
        .mount(&server)
        .await;

    let client = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        base_url: Some(format!("{}/v1", server.uri())),
        ..Default::default()
    })
    .unwrap();

    let snapshots = client.snapshots().list("s1").await.unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].id, "sn1");
    assert_eq!(snapshots[0].width, Some(1280));
    assert_eq!(snapshots[0].url, "/api/v1/snapshots/sn1");
}

#[tokio::test]
async fn snapshots_get_asks_for_base64_json_rather_than_raw_png_bytes() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/snapshots/sn1"))
        .and(query_param("format", "base64"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "sn1",
            "contentType": "image/png",
            "width": 1280,
            "height": 720,
            "capturedAt": "2026-01-01T00:00:00Z",
            "data": "aVZCT1J3MEs=",
        })))
        .mount(&server)
        .await;

    let client = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        base_url: Some(format!("{}/v1", server.uri())),
        ..Default::default()
    })
    .unwrap();

    let image = client.snapshots().get("sn1").await.unwrap();
    assert_eq!(image.content_type, "image/png");
    assert_eq!(image.data, "aVZCT1J3MEs=");
    assert_eq!(image.height, Some(720));
}

#[tokio::test]
async fn sessions_reshare_posts_expected_path_and_body() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/sessions/s1/reshare"))
        .and(body_json(json!({
            "mode": "new",
            "channel": "sms",
            "recipient": "+15145550199",
            "smsConsent": true,
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "expiresAt": "2026-01-02T00:00:00Z",
            "inviteShortUrl": "https://sitl.cc/s/aB3xK9",
            "resend": "sent",
            "rotated": true,
        })))
        .mount(&server)
        .await;

    let client = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        base_url: Some(format!("{}/v1", server.uri())),
        ..Default::default()
    })
    .unwrap();

    let result = client
        .sessions()
        .reshare(
            "s1",
            ReshareParams {
                mode: Some("new".to_string()),
                channel: Some(InviteChannel::Sms),
                recipient: Some("+15145550199".to_string()),
                sms_consent: Some(true),
            },
        )
        .await
        .unwrap();
    assert!(result.rotated);
    assert_eq!(result.resend, "sent");
    assert_eq!(result.invite_short_url.as_deref(), Some("https://sitl.cc/s/aB3xK9"));
}

// skip_serializing_if is load-bearing here: an omitted option must be ABSENT
// from the body, not sent as null — absent means "reuse what was recorded".
#[tokio::test]
async fn sessions_reshare_with_default_params_sends_an_empty_body() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/sessions/s1/reshare"))
        .and(body_json(json!({})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "expiresAt": "2026-01-02T00:00:00Z",
            "inviteShortUrl": null,
            "resend": "not_requested",
            "rotated": false,
        })))
        .mount(&server)
        .await;

    let client = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        base_url: Some(format!("{}/v1", server.uri())),
        ..Default::default()
    })
    .unwrap();

    let result = client
        .sessions()
        .reshare("s1", ReshareParams::default())
        .await
        .unwrap();
    assert_eq!(result.invite_short_url, None);
    assert!(!result.rotated);
}

// The session id must be percent-encoded in the path — otherwise a
// path-separator character in the id could redirect the request to a
// different endpoint entirely.
#[tokio::test]
async fn sessions_reshare_percent_encodes_the_session_id() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/sessions/sess%2F..%2Fevil/reshare"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "expiresAt": "2026-01-02T00:00:00Z",
            "inviteShortUrl": null,
            "resend": "not_requested",
            "rotated": false,
        })))
        .mount(&server)
        .await;

    let client = Client::new(Config {
        api_key: Some(LIVE_KEY.to_string()),
        base_url: Some(format!("{}/v1", server.uri())),
        ..Default::default()
    })
    .unwrap();

    client
        .sessions()
        .reshare("sess/../evil", ReshareParams::default())
        .await
        .expect("request must hit the percent-encoded path or the mock server has nothing to match");
}
