use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::header::CONTENT_TYPE;
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::errors::Error;
use crate::signing::sign_request;

/// Shared low-level transport used by both the authenticated `Client`
/// resources and the standalone `exchange_code` function. `auth_header`
/// is `None` for unauthenticated calls (e.g. the OAuth token exchange,
/// which authenticates via client credentials in the body).
///
/// `signing_secret` is `None` for unauthenticated calls (mirroring the
/// Node reference: `exchange_code` never signs). When set on the
/// authenticated `Client` path, every request is HMAC-signed
/// automatically — see [`crate::signing::sign_request`].
pub(crate) struct HttpClient {
    base_url: String,
    auth_header: Option<String>,
    signing_secret: Option<String>,
    client: reqwest::Client,
}

/// Manual `Debug` impl: `auth_header` holds the literal `Authorization:
/// Bearer <token>` value, so it must never be printed verbatim (spec §9 —
/// never log tokens). `{:?}`/`dbg!()`/`#[tracing::instrument]` on an
/// `HttpClient` (or the `Client` that wraps it) must not leak the token.
impl std::fmt::Debug for HttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpClient")
            .field("base_url", &self.base_url)
            .field(
                "auth_header",
                &self.auth_header.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(serde::Deserialize, Default)]
struct ErrorBody {
    message: Option<String>,
    code: Option<String>,
    field: Option<String>,
}

impl HttpClient {
    pub(crate) fn new(
        base_url: String,
        auth_header: Option<String>,
        signing_secret: Option<String>,
    ) -> Self {
        Self {
            base_url,
            auth_header,
            signing_secret,
            client: reqwest::Client::new(),
        }
    }

    pub(crate) async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
        self.request::<T, ()>(Method::GET, path, None).await
    }

    pub(crate) async fn post<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, Error> {
        self.request(Method::POST, path, Some(body)).await
    }

    pub(crate) async fn put<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, Error> {
        self.request(Method::PUT, path, Some(body)).await
    }

    async fn request<T: DeserializeOwned, B: Serialize>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<T, Error> {
        // Serialize the body ourselves (rather than letting `.json()` do it
        // internally) so the bytes we sign are byte-for-byte identical to
        // the bytes actually sent on the wire.
        let bytes: Vec<u8> = match body {
            Some(b) => serde_json::to_vec(b).map_err(Error::from)?,
            None => Vec::new(),
        };

        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.request(method.clone(), url);
        if let Some(auth) = &self.auth_header {
            req = req.header("Authorization", auth.as_str());
        }
        if body.is_some() {
            req = req.header(CONTENT_TYPE, "application/json");
        }
        if let Some(secret) = &self.signing_secret {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after the unix epoch")
                .as_secs() as i64;
            let headers = sign_request(secret, method.as_str(), path, &bytes, timestamp);
            req = req
                .header("X-SeeItLive-Signature", headers.signature)
                .header("X-SeeItLive-Timestamp", headers.timestamp);
        }
        if body.is_some() {
            req = req.body(bytes);
        }

        let res = req.send().await?;
        if !res.status().is_success() {
            return Err(Self::handle_error(res).await);
        }
        let parsed = res.json::<T>().await?;
        Ok(parsed)
    }

    async fn handle_error(res: reqwest::Response) -> Error {
        let status = res.status();
        let status_code = status.as_u16();

        // Preserve a legitimate `Retry-After: 0` (retry now) as Some(0),
        // distinct from an absent/unparsable header (None).
        let retry_after = if status == StatusCode::TOO_MANY_REQUESTS {
            res.headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
        } else {
            None
        };

        let body: ErrorBody = res.json().await.unwrap_or_default();
        let message = body
            .message
            .unwrap_or_else(|| format!("HTTP {status_code}"));

        match status_code {
            400 => Error::Validation {
                message,
                field: body.field,
                code: body.code,
            },
            401 => Error::Authentication {
                message,
                code: body.code,
            },
            403 => Error::Permission {
                message,
                code: body.code,
            },
            404 => Error::NotFound {
                message,
                code: body.code,
            },
            429 => Error::RateLimit {
                message,
                code: body.code,
                retry_after,
            },
            _ => Error::Api {
                status: status_code,
                code: body.code,
                message,
            },
        }
    }
}
