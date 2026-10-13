use crate::errors::Error;
use crate::http::HttpClient;
use crate::types::{
    AgentToken, CreateAgentTokenParams, CreateSessionParams, ProtectionInfo, ResetProtectionParams,
    ReshareParams, ReshareResult, Session, SessionDetail, SessionStatus, SetProtectionParams,
};

/// The `sessions` resource: `create` and `status`.
pub struct Sessions<'a> {
    http: &'a HttpClient,
}

impl<'a> Sessions<'a> {
    pub(crate) fn new(http: &'a HttpClient) -> Self {
        Self { http }
    }

    /// `POST /sessions`. Does not send a team id — the team is
    /// implicit from the client's credential.
    pub async fn create(&self, params: CreateSessionParams) -> Result<Session, Error> {
        self.http.post("/sessions", &params).await
    }

    /// `GET /sessions/{id}`. Reads a stored session, including how its
    /// invitation was delivered (channel/recipient).
    pub async fn get(&self, id: &str) -> Result<SessionDetail, Error> {
        let path = format!("/sessions/{}", encode_path_segment(id));
        self.http.get(&path).await
    }

    /// `GET /sessions/{id}/status`.
    pub async fn status(&self, id: &str) -> Result<SessionStatus, Error> {
        let path = format!("/sessions/{}/status", encode_path_segment(id));
        self.http.get(&path).await
    }

    /// `GET /sessions/{id}/protection`. The secret to read to the customer:
    /// the password, or the rotating code of the moment.
    pub async fn get_protection(&self, id: &str) -> Result<ProtectionInfo, Error> {
        let path = format!("/sessions/{}/protection", encode_path_segment(id));
        self.http.get(&path).await
    }

    /// `PUT /sessions/{id}/protection`. Changes a session's guest challenge
    /// after creation — including adding one to a session created without any,
    /// which is otherwise impossible. `protection: "none"` clears the challenge
    /// and any lockout with it.
    pub async fn set_protection(
        &self,
        id: &str,
        params: SetProtectionParams,
    ) -> Result<ProtectionInfo, Error> {
        let path = format!("/sessions/{}/protection", encode_path_segment(id));
        self.http.put(&path, &params).await
    }

    /// `POST /sessions/{id}/protection/reset`. Clears a guest lockout,
    /// rotating the secret only when `rotate: Some(true)` is given.
    pub async fn reset_protection(
        &self,
        id: &str,
        params: ResetProtectionParams,
    ) -> Result<ProtectionInfo, Error> {
        let path = format!("/sessions/{}/protection/reset", encode_path_segment(id));
        self.http.post(&path, &params).await
    }

    /// `POST /sessions/{id}/agent-token`. Mints a fresh agent-view token so an
    /// embedded viewer can watch the session. Tokens expire after 120 minutes.
    pub async fn create_agent_token(
        &self,
        id: &str,
        params: CreateAgentTokenParams,
    ) -> Result<AgentToken, Error> {
        let path = format!("/sessions/{}/agent-token", encode_path_segment(id));
        self.http.post(&path, &params).await
    }

    /// `POST /sessions/{id}/reshare`. Revives an expired invitation. `mode:
    /// "revive"` (the default) extends the SAME link and code in place, so a
    /// message the customer already has starts working again; `mode: "new"`
    /// issues a fresh link and kills the old one.
    ///
    /// A session created without a short link can be extended but not
    /// re-sent — the plaintext token survives only in the short link's
    /// encrypted fragment. `resend` on the result reports which happened.
    pub async fn reshare(
        &self,
        id: &str,
        params: ReshareParams,
    ) -> Result<ReshareResult, Error> {
        let path = format!("/sessions/{}/reshare", encode_path_segment(id));
        self.http.post(&path, &params).await
    }
}

/// Minimal percent-encoding for a single path segment, mirroring
/// JavaScript's `encodeURIComponent` closely enough for session ids
/// (which are opaque server-generated identifiers).
pub(crate) fn encode_path_segment(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{byte:02X}"));
            }
        }
    }
    out
}
