use crate::errors::Error;
use crate::http::HttpClient;
use crate::sessions::encode_path_segment;
use crate::types::{SnapshotImage, SnapshotSummary};

/// The `snapshots` resource: `list` and `get`. Both require the team's plan
/// to include snapshot history.
pub struct Snapshots<'a> {
    http: &'a HttpClient,
}

impl<'a> Snapshots<'a> {
    pub(crate) fn new(http: &'a HttpClient) -> Self {
        Self { http }
    }

    /// `GET /sessions/{id}/snapshots`. A session's snapshots, newest first.
    pub async fn list(&self, session_id: &str) -> Result<Vec<SnapshotSummary>, Error> {
        let path = format!("/sessions/{}/snapshots", encode_path_segment(session_id));
        self.http.get(&path).await
    }

    /// `GET /snapshots/{id}?format=base64`. The endpoint serves raw PNG bytes
    /// by default; the SDK asks for base64 JSON so the response stays JSON —
    /// and signs the query along with the path, which is what the server
    /// verifies. Decode `data` to get the PNG bytes.
    pub async fn get(&self, id: &str) -> Result<SnapshotImage, Error> {
        let path = format!("/snapshots/{}?format=base64", encode_path_segment(id));
        self.http.get(&path).await
    }
}
