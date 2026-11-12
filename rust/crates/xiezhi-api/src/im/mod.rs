//! IM 通知（对齐 `biz.utils.im.notifier` 骨架）。

mod events;

pub use events::{
    commits_from_webhook, notify_mr_reviewed, notify_push_reviewed, MrNotify, PushNotify,
};

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde_json::{json, Value};
use sha2::Sha256;
use std::env;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};
use url::Url;

#[derive(Debug, Clone)]
pub struct Notification {
    pub content: String,
    pub msg_type: String,
    pub title: String,
    pub project_name: Option<String>,
    pub url_slug: Option<String>,
    pub webhook_data: Value,
}

impl Notification {
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            msg_type: "text".into(),
            title: "通知".into(),
            project_name: None,
            url_slug: None,
            webhook_data: json!({}),
        }
    }

    pub fn markdown(content: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            msg_type: "markdown".into(),
            title: title.into(),
            project_name: None,
            url_slug: None,
            webhook_data: json!({}),
        }
    }
}

#[async_trait]
pub trait Notifier: Send + Sync {
    async fn send(&self, note: &Notification) -> Result<(), String>;
}

/// 仅打日志（未配置 IM 时的默认行为）。
pub struct LoggingNotifier;

#[async_trait]
impl Notifier for LoggingNotifier {
    async fn send(&self, note: &Notification) -> Result<(), String> {
        info!(
            msg_type = %note.msg_type,
            title = %note.title,
            project = ?note.project_name,
            chars = note.content.chars().count(),
            "im: logging notifier"
        );
        Ok(())
    }
}

/// 内存记录，便于单测。
#[derive(Clone, Default)]
pub struct MemoryNotifier {
    pub notes: Arc<Mutex<Vec<Notification>>>,
}

impl MemoryNotifier {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> Vec<Notification> {
        self.notes.lock().expect("lock").clone()
    }
}

#[async_trait]
impl Notifier for MemoryNotifier {
    async fn send(&self, note: &Notification) -> Result<(), String> {
        self.notes
            .lock()
            .map_err(|e| e.to_string())?
            .push(note.clone());
        Ok(())
    }
}

fn env_enabled(key: &str) -> bool {
    env::var(key).unwrap_or_else(|_| "0".into()) == "1"
}

fn resolve_webhook(prefix: &str, default_key: &str, note: &Notification) -> Option<String> {
    if let Some(slug) = note.url_slug.as_deref() {
        let key = format!("{prefix}_{}", slug.to_ascii_uppercase());
        if let Ok(v) = env::var(&key) {
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    if let Some(proj) = note.project_name.as_deref() {
        let key = format!("{prefix}_{}", proj.to_ascii_uppercase().replace('/', "_"));
        if let Ok(v) = env::var(&key) {
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    env::var(default_key).ok().filter(|s| !s.is_empty())
}

fn http_client() -> Result<Client, String> {
    Client::builder()
        .no_proxy()
        .build()
        .map_err(|e| e.to_string())
}

async fn post_json(url: &str, body: &Value) -> Result<(), String> {
    let client = http_client()?;
    let resp = client
        .post(url)
        .json(body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("IM HTTP {status}: {text}"));
    }
    Ok(())
}

pub struct FeishuNotifier {
    pub enabled: bool,
    pub webhook_url: Option<String>,
}

impl Default for FeishuNotifier {
    fn default() -> Self {
        Self {
            enabled: env_enabled("FEISHU_ENABLED"),
            webhook_url: None,
        }
    }
}

#[async_trait]
impl Notifier for FeishuNotifier {
    async fn send(&self, note: &Notification) -> Result<(), String> {
        if !self.enabled {
            info!("im: feishu disabled");
            return Ok(());
        }
        let url = self
            .webhook_url
            .clone()
            .or_else(|| resolve_webhook("FEISHU_WEBHOOK_URL", "FEISHU_WEBHOOK_URL", note))
            .ok_or_else(|| "feishu webhook url missing".to_string())?;
        let body = if note.msg_type == "markdown" {
            json!({
                "msg_type": "interactive",
                "card": {
                    "header": {"title": {"tag": "plain_text", "content": note.title}},
                    "elements": [{"tag": "markdown", "content": note.content}]
                }
            })
        } else {
            json!({"msg_type": "text", "content": {"text": note.content}})
        };
        post_json(&url, &body).await
    }
}

pub struct WeComNotifier {
    pub enabled: bool,
    pub webhook_url: Option<String>,
}

impl Default for WeComNotifier {
    fn default() -> Self {
        Self {
            enabled: env_enabled("WECOM_ENABLED"),
            webhook_url: None,
        }
    }
}

#[async_trait]
impl Notifier for WeComNotifier {
    async fn send(&self, note: &Notification) -> Result<(), String> {
        if !self.enabled {
            info!("im: wecom disabled");
            return Ok(());
        }
        let url = self
            .webhook_url
            .clone()
            .or_else(|| resolve_webhook("WECOM_WEBHOOK_URL", "WECOM_WEBHOOK_URL", note))
            .ok_or_else(|| "wecom webhook url missing".to_string())?;
        let body = if note.msg_type == "markdown" {
            json!({"msgtype": "markdown", "markdown": {"content": format!("## {}\n{}", note.title, note.content)}})
        } else {
            json!({"msgtype": "text", "text": {"content": note.content}})
        };
        post_json(&url, &body).await
    }
}

pub struct DingTalkNotifier {
    pub enabled: bool,
    pub webhook_url: Option<String>,
    pub secret_enabled: bool,
    pub secret: Option<String>,
}

impl Default for DingTalkNotifier {
    fn default() -> Self {
        Self {
            enabled: env_enabled("DINGTALK_ENABLED"),
            webhook_url: None,
            secret_enabled: env_enabled("DINGTALK_SECRET_ENABLED"),
            secret: env::var("DINGTALK_SECRET").ok().filter(|s| !s.is_empty()),
        }
    }
}

/// 钉钉加签：`timestamp` + HMAC-SHA256(secret, `{ts}\n{secret}`) → base64，再写入 query（对齐 Python `_sign_url`）。
pub fn sign_dingtalk_url(
    base_url: &str,
    secret: &str,
    timestamp_ms: i64,
) -> Result<String, String> {
    let string_to_sign = format!("{timestamp_ms}\n{secret}");
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|e| format!("dingtalk hmac key: {e}"))?;
    mac.update(string_to_sign.as_bytes());
    let sign_b64 = B64.encode(mac.finalize().into_bytes());
    let mut u = Url::parse(base_url).map_err(|e| format!("dingtalk url: {e}"))?;
    u.query_pairs_mut()
        .append_pair("timestamp", &timestamp_ms.to_string())
        .append_pair("sign", &sign_b64);
    Ok(u.to_string())
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[async_trait]
impl Notifier for DingTalkNotifier {
    async fn send(&self, note: &Notification) -> Result<(), String> {
        if !self.enabled {
            info!("im: dingtalk disabled");
            return Ok(());
        }
        let mut url = self
            .webhook_url
            .clone()
            .or_else(|| resolve_webhook("DINGTALK_WEBHOOK_URL", "DINGTALK_WEBHOOK_URL", note))
            .ok_or_else(|| "dingtalk webhook url missing".to_string())?;
        if self.secret_enabled {
            let secret = self
                .secret
                .as_deref()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "钉钉加签已启用但未配置 DINGTALK_SECRET".to_string())?;
            url = sign_dingtalk_url(&url, secret, now_millis())?;
        }
        let body = if note.msg_type == "markdown" {
            json!({
                "msgtype": "markdown",
                "markdown": {"title": note.title, "text": note.content},
                "at": {"isAtAll": false}
            })
        } else {
            json!({
                "msgtype": "text",
                "text": {"content": note.content},
                "at": {"isAtAll": false}
            })
        };
        post_json(&url, &body).await
    }
}

pub struct ExtraWebhookNotifier {
    pub enabled: bool,
    pub webhook_url: Option<String>,
}

impl Default for ExtraWebhookNotifier {
    fn default() -> Self {
        Self {
            enabled: env_enabled("EXTRA_WEBHOOK_ENABLED"),
            webhook_url: env::var("EXTRA_WEBHOOK_URL").ok().filter(|s| !s.is_empty()),
        }
    }
}

#[async_trait]
impl Notifier for ExtraWebhookNotifier {
    async fn send(&self, note: &Notification) -> Result<(), String> {
        if !self.enabled {
            info!("im: extra webhook disabled");
            return Ok(());
        }
        let url = self
            .webhook_url
            .clone()
            .ok_or_else(|| "extra webhook url missing".to_string())?;
        let body = json!({
            "xiezhi_data": {
                "content": note.content,
                "msg_type": note.msg_type,
                "title": note.title,
                "project_name": note.project_name,
                "url_slug": note.url_slug,
            },
            "webhook_data": note.webhook_data,
        });
        post_json(&url, &body).await
    }
}

/// 向所有已启用渠道广播（对齐 Python `send_notification`）。
pub struct CompositeNotifier {
    channels: Vec<Box<dyn Notifier>>,
}

impl CompositeNotifier {
    pub fn from_env() -> Self {
        Self {
            channels: vec![
                Box::new(DingTalkNotifier::default()),
                Box::new(WeComNotifier::default()),
                Box::new(FeishuNotifier::default()),
                Box::new(ExtraWebhookNotifier::default()),
            ],
        }
    }
}

#[async_trait]
impl Notifier for CompositeNotifier {
    async fn send(&self, note: &Notification) -> Result<(), String> {
        let mut last_err = None;
        for ch in &self.channels {
            if let Err(e) = ch.send(note).await {
                warn!(error = %e, "im channel send failed");
                last_err = Some(e);
            }
        }
        // 个别渠道失败不阻断；全部失败时仍 Ok（与 Python 各 notifier 独立吞错一致）
        let _ = last_err;
        Ok(())
    }
}

/// 便捷入口：按环境配置广播。
pub async fn send_notification(note: Notification) {
    let n = CompositeNotifier::from_env();
    if let Err(e) = n.send(&note).await {
        warn!(error = %e, "im send_notification failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn memory_records() {
        let n = MemoryNotifier::new();
        n.send(&Notification::text("hello")).await.unwrap();
        assert_eq!(n.snapshot().len(), 1);
        assert_eq!(n.snapshot()[0].content, "hello");
    }

    #[tokio::test]
    async fn feishu_posts_when_enabled() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 2048];
            let n = sock.read(&mut buf).await.unwrap();
            let req = String::from_utf8_lossy(&buf[..n]);
            assert!(req.contains("POST"), "{req}");
            assert!(req.contains("msg_type"), "{req}");
            let body = r#"{"StatusCode":0}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            sock.write_all(resp.as_bytes()).await.unwrap();
        });

        let n = FeishuNotifier {
            enabled: true,
            webhook_url: Some(format!("http://{addr}/hook")),
        };
        n.send(&Notification::text("ping")).await.unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn disabled_channels_noop() {
        let n = FeishuNotifier {
            enabled: false,
            webhook_url: None,
        };
        n.send(&Notification::text("x")).await.unwrap();
    }

    #[test]
    fn dingtalk_sign_matches_python() {
        let signed = sign_dingtalk_url(
            "https://oapi.dingtalk.com/robot/send?access_token=t",
            "SEC123",
            1_700_000_000_000,
        )
        .unwrap();
        assert!(signed.contains("timestamp=1700000000000"), "{signed}");
        // quote_plus(base64) → `=` becomes %3D in query; raw b64 fixture from Python
        assert!(
            signed.contains("lkcPI1uoxBY1gUnCnnPH1Kkru0Hqjo7rFpA3haIVhEQ%3D")
                || signed.contains("sign=lkcPI1uoxBY1gUnCnnPH1Kkru0Hqjo7rFpA3haIVhEQ%3D"),
            "{signed}"
        );
    }

    #[tokio::test]
    async fn dingtalk_secret_appends_query() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let n = sock.read(&mut buf).await.unwrap();
            let req = String::from_utf8_lossy(&buf[..n]);
            assert!(req.contains("timestamp="), "{req}");
            assert!(req.contains("sign="), "{req}");
            let body = r#"{"errcode":0,"errmsg":"ok"}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            sock.write_all(resp.as_bytes()).await.unwrap();
        });

        let n = DingTalkNotifier {
            enabled: true,
            webhook_url: Some(format!("http://{addr}/robot/send?access_token=t")),
            secret_enabled: true,
            secret: Some("SEC123".into()),
        };
        n.send(&Notification::text("ping")).await.unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn dingtalk_secret_enabled_without_secret_errors() {
        let n = DingTalkNotifier {
            enabled: true,
            webhook_url: Some("http://127.0.0.1:9/hook".into()),
            secret_enabled: true,
            secret: None,
        };
        let err = n.send(&Notification::text("x")).await.unwrap_err();
        assert!(err.contains("DINGTALK_SECRET"), "{err}");
    }
}
