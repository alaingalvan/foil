use axum::{
    http::header::LOCATION,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use hmac::{Hmac, Mac};
use serde_derive::Deserialize;
use serde_json::from_str;
use sha2::Sha256;
use std::env;

// Create alias for HMAC-SHA256
type HmacSha256 = Hmac<Sha256>;

mod update;
use update::update;

// 🧸 Redirect GET requests
pub async fn redirect() -> Response {
    (StatusCode::TEMPORARY_REDIRECT, [(LOCATION, "/")]).into_response()
}

#[derive(Clone, Deserialize)]
pub struct APIRequest {
    #[serde(rename = "ref")]
    refs: String,
    //commits: Vec<Commit>,
}

#[derive(Clone, Deserialize)]
pub struct Commit {
    //added: Vec<String>,
    //removed: Vec<String>,
    //modified: Vec<String>,
}

pub async fn github_hook(headers: HeaderMap, body: String) -> Response {
    let header = match headers.get("X-Hub-Signature-256") {
        Some(h) => h,
        None => {
            return (
                StatusCode::NOT_FOUND,
                "Could not find GitHub Signature Header.",
            )
                .into_response()
        }
    };

    // Compare secret with GitHub header:
    let header_str = String::from_utf8(header.as_bytes().to_vec()).unwrap();
    let header_split: Vec<&str> = header_str.split("sha256=").collect();
    let header_hex_str = header_split[1];
    let header_hex = hex::decode(header_hex_str);
    let sent_result = header_hex.unwrap();
    let secret = env::var("FOIL_GITHUB_SECRET").unwrap_or_default();
    let mut mac: HmacSha256 =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("Failed to generate HMAC from key.");
    mac.update(body.as_bytes());

    let verify = mac.clone().verify_slice(&sent_result[..]);
    if verify.is_err() {
        println!(
            "Sent: {:?}\nTest: {:?}",
            &header_str,
            hex::encode(mac.clone().finalize().into_bytes())
        );
        return (
            StatusCode::NOT_FOUND,
            "GitHub header secret didn't match config secret.",
        )
            .into_response();
    } else {
        // Get APIRequest
        let data: APIRequest = match from_str(&body) {
            Ok(r) => r,
            Err(_) => {
                return (StatusCode::NOT_FOUND, "Could not read payload body.").into_response()
            }
        };
        update(data).await;
    }
    (StatusCode::ACCEPTED).into_response()
}
