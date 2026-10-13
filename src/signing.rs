use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

/// The two headers produced by [`sign_request`].
pub(crate) struct SignatureHeaders {
    /// Value for `X-SeeItLive-Signature`, e.g. `sha256=<hex>`.
    pub signature: String,
    /// Value for `X-SeeItLive-Timestamp`: the unix-seconds timestamp, as a string.
    pub timestamp: String,
}

/// Computes the HMAC request-signature headers per the SeeItLive wire contract:
///
///   signature = HMAC_SHA256(secret, `${timestamp}.${METHOD}.${path}.${sha256hex(rawBody)}`)
///
/// - `timestamp` is unix seconds.
/// - `method` is upper-cased (case-insensitive input).
/// - `path` is exactly the request path (no base URL, no `/api/v1` prefix).
/// - `body` must be the exact serialized bytes sent as the request body; pass
///   an empty slice when the request has no body, which hashes as the empty
///   string (matching the Node reference SDK's `sha256("")`).
pub(crate) fn sign_request(
    secret: &str,
    method: &str,
    path: &str,
    body: &[u8],
    timestamp: i64,
) -> SignatureHeaders {
    let body_hash = hex::encode(Sha256::digest(body));
    let message = format!("{timestamp}.{}.{path}.{body_hash}", method.to_uppercase());

    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts a key of any length");
    mac.update(message.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());

    SignatureHeaders {
        signature: format!("sha256={signature}"),
        timestamp: timestamp.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn matches_known_answer_vector_for_post_with_body() {
        let headers = sign_request(
            SECRET,
            "POST",
            "/sessions",
            br#"{"channel":"demo"}"#,
            1800000000,
        );
        assert_eq!(
            headers.signature,
            "sha256=a59401f3e80ae2e50f4b2517c8e4708de856bb93eadb1f4cfc59100db2b6e7f7"
        );
        assert_eq!(headers.timestamp, "1800000000");
    }

    #[test]
    fn matches_known_answer_vector_for_get_with_no_body() {
        let headers = sign_request(SECRET, "GET", "/sessions/x/status", b"", 1800000000);
        assert_eq!(
            headers.signature,
            "sha256=3499dfb7f9581f996d537dfb108f775cb329f1f3f490f3bcb791820b4d81d5ea"
        );
        assert_eq!(headers.timestamp, "1800000000");
    }

    #[test]
    fn method_is_case_insensitive() {
        let upper = sign_request(
            SECRET,
            "POST",
            "/sessions",
            br#"{"channel":"demo"}"#,
            1800000000,
        );
        let lower = sign_request(
            SECRET,
            "post",
            "/sessions",
            br#"{"channel":"demo"}"#,
            1800000000,
        );
        assert_eq!(upper.signature, lower.signature);
        assert_eq!(upper.timestamp, lower.timestamp);
    }
}
