//! Official SeeItLive SDK for Rust — an idiomatic port of the reference
//! `@seeitlive/sdk` Node client. Same behaviour, same wire contract, Rust
//! conventions.
//!
//! ```no_run
//! use seeitlive::{Client, Config, CreateSessionParams, InviteChannel};
//!
//! # async fn run() -> Result<(), seeitlive::Error> {
//! let client = Client::new(Config {
//!     api_key: Some(std::env::var("SEEITLIVE_API_KEY").unwrap_or_default()),
//!     ..Default::default()
//! })?;
//!
//! let session = client
//!     .sessions()
//!     .create(CreateSessionParams::new(InviteChannel::Link))
//!     .await?;
//! println!("{}", session.invite_url);
//! # Ok(())
//! # }
//! ```

mod client;
mod errors;
mod exchange;
mod http;
mod oauth;
mod sessions;
mod signing;
mod snapshots;
mod types;

/// Default API base URL used when `Config::base_url` / `ExchangeCodeParams::base_url`
/// is not set. Shared by `Client::new` and `exchange_code`.
pub(crate) const DEFAULT_BASE_URL: &str = "https://api.seeitlive.io/v1";

pub use client::Client;
pub use errors::Error;
pub use exchange::exchange_code;
pub use oauth::OAuth;
pub use sessions::Sessions;
pub use snapshots::Snapshots;
pub use types::{
    AgentToken, Config, CreateAgentTokenParams, CreateSessionParams, Env, ExchangeCodeParams,
    InviteChannel, Plan, ProtectionInfo, ResetProtectionParams, ReshareParams, ReshareResult,
    Session, SessionDetail, SessionInvitation, SessionStatus, SetProtectionParams, SnapshotImage,
    SnapshotSummary, TokenInfo, TokenResponse,
};
