# seeitlive-sdk

Official SeeItLive SDK for Rust — an idiomatic port of the reference
[`@seeitlive/sdk`](../client-node) Node client. Same behaviour, same wire
contract, Rust conventions.

## Install

```toml
[dependencies]
seeitlive-sdk = { version = "0.1", package = "seeitlive-sdk" }
```

The crate name on crates.io is `seeitlive-sdk`; the Rust module/import
path is `seeitlive`:

```rust
use seeitlive::{Client, Config};
```

## Quick start

```rust
use seeitlive::{Client, Config, CreateSessionParams, InviteChannel};

#[tokio::main]
async fn main() -> Result<(), seeitlive::Error> {
    let client = Client::new(Config {
        api_key: Some(std::env::var("SEEITLIVE_API_KEY").unwrap()),
        ..Default::default()
    })?;

    // SMS channel requires recipient_phone + sms_consent (the recipient
    // agreed to receive a text; carrier fees may apply).
    let session = client
        .sessions()
        .create(CreateSessionParams {
            recipient_phone: Some("+15551234567".to_string()),
            sms_consent: Some(true),
            ..CreateSessionParams::new(InviteChannel::Sms)
        })
        .await?;
    println!("{} {}", session.invite_url, session.invite_code);

    // email channel requires recipient_email
    let _ = client
        .sessions()
        .create(CreateSessionParams {
            recipient_email: Some("customer@example.com".to_string()),
            ..CreateSessionParams::new(InviteChannel::Email)
        })
        .await?;

    // link channel needs neither — just returns a shareable link
    let link_session = client
        .sessions()
        .create(CreateSessionParams::new(InviteChannel::Link))
        .await?;

    let status = client.sessions().status(&link_session.id).await?;
    println!("{} {} {}", status.status, status.producer_count, status.has_video);

    Ok(())
}
```

The client infers `Env` (`Env::Live` or `Env::Test`) from the API key's
prefix (`sil_test_...` -> test, otherwise live). Override with
`Config { env: Some(Env::Test), .. }`, and override the API base URL
with `Config { base_url: Some(...), .. }` (default:
`https://api.seeitlive.io/v1`).

Provide **exactly one** of `Config.api_key` or `Config.oauth_token` —
supplying both, or neither, makes `Client::new` return
`Err(Error::Validation)`. A present-but-empty string is treated the same
as absent.

### Reading a session

`sessions().get` returns a stored session, including how its invitation was
delivered — useful when re-opening a session an integration created earlier:

```rust
let s = client.sessions().get(&session.id).await?;
println!("{} {:?}", s.status, s.invitation.map(|i| i.channel));
```

### Guest protection

A session can require the customer to enter a secret before their camera is
shared — a static password, or a rotating code. Read the current one to them:

```rust
let p = client.sessions().get_protection(&session.id).await?;
match p.r#type.as_str() {
    "password" => println!("{:?}", p.password),
    "totp" => println!("{:?} ({:?}s left)", p.code, p.seconds_remaining),
    _ => {}
}
```

Change it after creation — including adding one to a session that had none.
Leave `protection_secret` as `None` and one is generated for you:

```rust
use seeitlive::SetProtectionParams;

let added = client
    .sessions()
    .set_protection(&session.id, SetProtectionParams {
        protection: "password".to_string(),
        protection_secret: None,
    })
    .await?;
println!("{:?}", added.password); // read this to the customer
```

Five wrong answers lock the guest out. Clear the lockout — rotation is opt-in,
since the usual cause is a mistyped secret:

```rust
use seeitlive::ResetProtectionParams;

client.sessions().reset_protection(&session.id, ResetProtectionParams::default()).await?;

let fresh = client
    .sessions()
    .reset_protection(&session.id, ResetProtectionParams { rotate: Some(true) })
    .await?;
```

For `totp`, `get_protection` returns the **current code**, never the seed — the
seed leaves the server only once, in the create response.

### Embedded viewer tokens

`sessions().create_agent_token` mints a fresh agent-view grant so your own UI
can embed the live viewer. Tokens expire after 120 minutes — mint another to
re-open a viewer:

```rust
use seeitlive::CreateAgentTokenParams;

let viewer = client
    .sessions()
    .create_agent_token(&session.id, CreateAgentTokenParams {
        label: Some("Odoo — crm.lead 42".to_string()), // shown in the viewer list
    })
    .await?;
println!("{}", viewer.agent_view_url);
```

### Snapshots

Frames the agent captured during a session. Both calls require a plan with
snapshot history:

```rust
let snapshots = client.snapshots().list(&session.id).await?; // newest first
let image = client.snapshots().get(&snapshots[0].id).await?;
// `image.data` is base64 PNG — the endpoint's raw-bytes mode is not used here.
```

## OAuth usage

For OAuth-based (multi-tenant / marketplace) integrations, exchange an
authorization code for an access token using the standalone
`exchange_code` function (this call is unauthenticated — it uses your
client credentials in the request body, not a bearer token, and is a
free function because it runs *before* any token exists):

```rust
use seeitlive::{exchange_code, Client, Config, ExchangeCodeParams};

let token = exchange_code(ExchangeCodeParams {
    code: authorization_code_from_redirect,
    client_id: std::env::var("SEEITLIVE_CLIENT_ID").unwrap(),
    client_secret: std::env::var("SEEITLIVE_CLIENT_SECRET").unwrap(),
    redirect_uri: "https://your-app.example.com/oauth/callback".to_string(),
    code_verifier: None,
    base_url: None,
})
.await?;

let client = Client::new(Config {
    oauth_token: Some(token.access_token),
    ..Default::default()
})?;

let info = client.oauth().verify().await?;
println!("{} {} {:?}", info.client_id, info.team_id, info.scopes);
```

## Request signing

Set `Config.signing_secret` to have every outgoing request HMAC-signed
automatically:

```rust
let client = Client::new(Config {
    api_key: Some(std::env::var("SEEITLIVE_API_KEY").unwrap()),
    signing_secret: Some(std::env::var("SEEITLIVE_SIGNING_SECRET").unwrap()),
    ..Default::default()
})?;
```

When set, each request carries:

- `X-SeeItLive-Signature: sha256=<hex>` — `HMAC_SHA256(signing_secret,
  "<unix_ts>.<METHOD>.<path>.<sha256hex(raw_body)>")`, where `path` is the
  request path only (e.g. `/sessions`, no base URL) and `raw_body` is the
  exact bytes sent (hashed as the empty string when there is no body).
- `X-SeeItLive-Timestamp: <unix_ts>` — unix seconds.

`path` is signed **including any query string** the SDK sends (e.g.
`/snapshots/<id>?format=base64`) — the server verifies path *and* query,
so the two must match exactly.

A present-but-empty string is treated the same as an absent secret:
requests are unsigned by default, so this is fully backwards-compatible.

## Errors

Every fallible call returns `Result<T, seeitlive::Error>`. `Error` is a
`thiserror` enum:

| Variant | HTTP status |
| --- | --- |
| `Error::Validation { message, field, code }` (also raised on bad `Config`, e.g. missing/duplicate credentials) | 400 |
| `Error::Authentication { message, code }` | 401 |
| `Error::Permission { message, code }` | 403 |
| `Error::NotFound { message, code }` | 404 |
| `Error::RateLimit { message, code, retry_after: Option<u64> }` | 429 |
| `Error::Api { status, code, message }` | any other non-2xx status |
| `Error::Transport(reqwest::Error)` | network/transport failure |
| `Error::Serialization(serde_json::Error)` | request body failed to serialize (not expected for the SDK's own types) |

```rust
match client.sessions().status("missing").await {
    Ok(status) => println!("{status:?}"),
    Err(seeitlive::Error::RateLimit { retry_after: Some(secs), .. }) => {
        eprintln!("retry after {secs} seconds");
    }
    Err(seeitlive::Error::Api { status, message, .. }) => {
        eprintln!("{status} {message}");
    }
    Err(seeitlive::Error::Transport(e)) => {
        eprintln!("network error: {e}"); // not a SeeItLive API error
    }
    Err(e) => eprintln!("{e}"),
}
```

`RateLimit.retry_after` is `Option<u64>`, not a bare `u64`: the
`Retry-After` header can legitimately be `0` (retry immediately), which
must be distinguished from the header being absent (`None`). Don't treat
`None` as "no need to wait" — match on it explicitly.

Network/transport failures (DNS, connection refused, timeouts, response
decoding) surface as `Error::Transport(reqwest::Error)` — the underlying
`reqwest` error, not re-wrapped into a SeeItLive-specific shape.

## Security

- **Never log API keys, OAuth access tokens, or refresh tokens.**
- **Never ship `client_secret` to the browser or a mobile app.**
  `exchange_code` must only be called from a trusted server-side context
  — it is a confidential-client operation. Client-side code should
  redirect the user through your own backend, which performs the code
  exchange and hands the browser a short-lived session, never the raw
  OAuth token or client secret.
- The default `base_url` is HTTPS; the SDK never disables TLS
  verification.

## Development

```bash
cargo build
cargo test
cargo fmt
```
