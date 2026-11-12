//! Dashboard HMAC token（对齐 Python `_issue_token` / `_verify_token`）。

use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug)]
pub struct AuthConfig {
    pub user: String,
    pub password: String,
    pub secret: String,
    pub ttl_secs: i64,
}

impl AuthConfig {
    pub fn from_env() -> Self {
        Self {
            user: env::var("DASHBOARD_USER").unwrap_or_else(|_| "admin".into()),
            password: env::var("DASHBOARD_PASSWORD").unwrap_or_else(|_| "admin".into()),
            secret: env::var("DASHBOARD_SECRET_KEY").unwrap_or_else(|_| {
                "fac8cf149bdd616c07c1a675c4571ccacc40d7f7fe16914cfe0f9f9d966bb773".into()
            }),
            ttl_secs: 30 * 24 * 60 * 60,
        }
    }
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn sign(secret: &str, message: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(message.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

pub fn issue_token(cfg: &AuthConfig, username: &str) -> String {
    let timestamp = now_secs().to_string();
    let message = format!("{username}:{timestamp}");
    let signature = sign(&cfg.secret, &message);
    format!("{message}:{signature}")
}

pub fn verify_token(cfg: &AuthConfig, token: &str) -> Option<String> {
    let mut parts = token.splitn(3, ':');
    let username = parts.next()?;
    let timestamp = parts.next()?;
    let signature = parts.next()?;
    let message = format!("{username}:{timestamp}");
    let expected = sign(&cfg.secret, &message);
    if !constant_time_eq(expected.as_bytes(), signature.as_bytes()) {
        return None;
    }
    let ts: i64 = timestamp.parse().ok()?;
    if now_secs() - ts > cfg.ttl_secs {
        return None;
    }
    if username != cfg.user {
        return None;
    }
    Some(username.to_string())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_token() {
        let cfg = AuthConfig {
            user: "admin".into(),
            password: "admin".into(),
            secret: "s".into(),
            ttl_secs: 3600,
        };
        let token = issue_token(&cfg, "admin");
        assert_eq!(verify_token(&cfg, &token).as_deref(), Some("admin"));
        assert!(verify_token(&cfg, "bad").is_none());
    }
}
