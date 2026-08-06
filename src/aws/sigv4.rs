//! AWS SigV4 signing — for native Bedrock API requests.
//!
//! Signs an HTTP request with AWS Signature Version 4 using the
//! Authorization header (HMAC-SHA256).
//!
//! Required env vars:
//!   AWS_ACCESS_KEY_ID
//!   AWS_SECRET_ACCESS_KEY
//!   AWS_REGION (default: us-east-1)
//!   AWS_SESSION_TOKEN (optional — for STS temporary credentials)
//!
//! Reference: https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_InvokeModel.html

use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use reqwest::RequestBuilder;

type HmacSha256 = Hmac<Sha256>;

const SERVICE: &str = "bedrock";

pub struct SigV4Credentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: Option<String>,
    pub region: String,
}

impl SigV4Credentials {
    pub fn from_env() -> Option<Self> {
        let access_key_id = std::env::var("AWS_ACCESS_KEY_ID").ok()?;
        let secret_access_key = std::env::var("AWS_SECRET_ACCESS_KEY").ok()?;
        let session_token = std::env::var("AWS_SESSION_TOKEN").ok().filter(|s| !s.is_empty());
        let region = std::env::var("AWS_REGION")
            .or_else(|_| std::env::var("AWS_DEFAULT_REGION"))
            .unwrap_or_else(|_| "us-east-1".into());
        Some(Self { access_key_id, secret_access_key, session_token, region })
    }
}

/// Sign a reqwest RequestBuilder with SigV4. Mutates the request in-place
/// by adding the Authorization, X-Amz-Date, and (optional) X-Amz-Security-Token headers.
///
/// `method` is "GET" | "POST" | etc.
/// `url` is the full URL (must match what reqwest will send).
/// `body` is the request body bytes (empty for GET).
pub fn sign(
    creds: &SigV4Credentials,
    method: &str,
    url: &str,
    body: &[u8],
    builder: RequestBuilder,
) -> RequestBuilder {
    let parsed = match url::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return builder,
    };
    let host = parsed.host_str().unwrap_or("");
    let path = parsed.path();
    let query = parsed.query().unwrap_or("");

    // Timestamps
    let now = Utc::now();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let date_stamp = now.format("%Y%m%d").to_string();

    // 1. Canonical request
    let payload_hash = hex::encode(Sha256::digest(body));
    let canonical_headers = format!(
        "host:{}\nx-amz-content-sha256:{}\nx-amz-date:{}\n",
        host, payload_hash, amz_date
    );
    let signed_headers = "host;x-amz-content-sha256;x-amz-date";
    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        method.to_uppercase(),
        path,
        query,
        canonical_headers,
        signed_headers,
        payload_hash
    );

    // 2. String to sign
    let credential_scope = format!("{}/{}/{}/aws4_request", date_stamp, creds.region, SERVICE);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        amz_date,
        credential_scope,
        hex::encode(Sha256::digest(canonical_request.as_bytes()))
    );

    // 3. Signing key derivation
    let signing_key = derive_signing_key(&creds.secret_access_key, &date_stamp, &creds.region, SERVICE);

    // 4. Signature
    let signature = hex::encode(hmac_sha256(&signing_key, string_to_sign.as_bytes()));

    // 5. Authorization header
    let auth_header = format!(
        "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
        creds.access_key_id, credential_scope, signed_headers, signature
    );

    let mut builder = builder
        .header("Authorization", &auth_header)
        .header("X-Amz-Date", &amz_date)
        .header("X-Amz-Content-Sha256", &payload_hash)
        .header("Host", host);

    if let Some(token) = &creds.session_token {
        builder = builder.header("X-Amz-Security-Token", token);
    }

    builder
}

fn derive_signing_key(secret: &str, date: &str, region: &str, service: &str) -> Vec<u8> {
    let k_date = hmac_sha256(format!("AWS4{}", secret).as_bytes(), date.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    hmac_sha256(&k_service, b"aws4_request")
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signing_key_derivation_matches_aws_doc_example() {
        // AWS doc example — known test vector
        // https://docs.aws.amazon.com/general/latest/gr/sigv4_signing.html
        let secret = "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY";
        let date = "20150830";
        let region = "us-east-1";
        let service = "iam";
        let key = derive_signing_key(secret, date, region, service);
        // Expected (hex): c4afb1cc5771d871763a393e44b703571b55cc28424d1a5e86da6ed3c154a4b9
        let expected_hex = "c4afb1cc5771d871763a393e44b703571b55cc28424d1a5e86da6ed3c154a4b9";
        assert_eq!(hex::encode(&key), expected_hex);
    }
}
