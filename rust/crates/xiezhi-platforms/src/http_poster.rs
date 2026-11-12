//! 真实 HTTP 评论回写（GitLab / GitHub / Gitea）。

use crate::comment_plan::plan_review_note;
use crate::{
    CommentError, CommentPoster, EventKind, NormalizedEvent, PlatformCredentials, PlatformKind,
};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use tracing::{info, warn};

#[derive(Clone)]
pub struct HttpCommentPoster {
    http: Client,
}

impl Default for HttpCommentPoster {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpCommentPoster {
    pub fn new() -> Self {
        Self {
            http: Client::new(),
        }
    }

    pub fn with_http(http: Client) -> Self {
        Self { http }
    }
}

/// 平台请求体字段：GitLab push commit comments 用 `note`，其余用 `body`。
pub fn review_note_json(platform: PlatformKind, event: EventKind, body: &str) -> Value {
    match (platform, event) {
        (PlatformKind::Gitlab, EventKind::Push) => json!({ "note": body }),
        _ => json!({ "body": body }),
    }
}

pub(crate) fn apply_auth(
    builder: reqwest::RequestBuilder,
    platform: PlatformKind,
    token: &str,
) -> reqwest::RequestBuilder {
    match platform {
        PlatformKind::Gitlab => builder
            .header("Private-Token", token)
            .header("Content-Type", "application/json"),
        PlatformKind::Github => builder
            .header("Authorization", format!("token {token}"))
            .header("Accept", "application/vnd.github.v3+json")
            .header("Content-Type", "application/json"),
        PlatformKind::Gitea => builder
            .header("Authorization", format!("token {token}"))
            .header("Content-Type", "application/json"),
        PlatformKind::Unknown => builder.header("Content-Type", "application/json"),
    }
}

#[async_trait]
impl CommentPoster for HttpCommentPoster {
    async fn post_review_note(
        &self,
        event: &NormalizedEvent,
        creds: &PlatformCredentials,
        body: &str,
    ) -> Result<(), CommentError> {
        let plan = plan_review_note(event, creds, body)?;
        let payload = review_note_json(event.platform, event.event, body);
        let req =
            apply_auth(self.http.post(&plan.url), event.platform, &creds.token).json(&payload);
        let resp = req
            .send()
            .await
            .map_err(|e| CommentError::Request(e.to_string()))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| CommentError::Request(e.to_string()))?;
        // GitLab/GitHub/Gitea 创建评论通常 201；部分代理可能回 200。
        if status.as_u16() == 200 || status.as_u16() == 201 {
            info!(
                platform = ?event.platform,
                url = %plan.url,
                status = %status,
                "posted review note"
            );
            Ok(())
        } else {
            warn!(
                platform = ?event.platform,
                url = %plan.url,
                status = %status,
                body = %text,
                "failed to post review note"
            );
            Err(CommentError::Request(format!("HTTP {status}: {text}")))
        }
    }

    async fn post_inline_comments(
        &self,
        event: &NormalizedEvent,
        creds: &PlatformCredentials,
        comments: &[xiezhi_core::ScrComment],
    ) -> Result<usize, CommentError> {
        crate::inline::post_inline_comments(&self.http, event, creds, comments).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EventKind, NormalizedEvent, PlatformKind};
    use serde_json::json;
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn gitlab_push_uses_note_field() {
        let v = review_note_json(PlatformKind::Gitlab, EventKind::Push, "hi");
        assert_eq!(v["note"], "hi");
        assert!(v.get("body").is_none());
        let v2 = review_note_json(PlatformKind::Github, EventKind::PullRequest, "hi");
        assert_eq!(v2["body"], "hi");
    }

    async fn read_http_request(sock: &mut tokio::net::TcpStream) -> String {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 1024];
        loop {
            let n = sock.read(&mut tmp).await.unwrap();
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
            if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                // If Content-Length present, keep reading until body arrives.
                let header = String::from_utf8_lossy(&buf);
                if let Some(cl) = header
                    .lines()
                    .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
                    .and_then(|l| l.split(':').nth(1))
                    .and_then(|v| v.trim().parse::<usize>().ok())
                {
                    if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let body_len = buf.len() - (pos + 4);
                        if body_len >= cl {
                            break;
                        }
                    }
                } else {
                    break;
                }
            }
        }
        String::from_utf8_lossy(&buf).into_owned()
    }

    #[tokio::test]
    async fn posts_gitlab_mr_note() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let req = read_http_request(&mut sock).await;
            let lower = req.to_ascii_lowercase();
            assert!(
                lower.contains("post /api/v4/projects/42/merge_requests/7/notes"),
                "{req}"
            );
            assert!(lower.contains("private-token: tok"), "{req}");
            assert!(req.contains("\"body\":\"review-ok\""), "{req}");
            let body = r#"{"id":1}"#;
            let resp = format!(
                "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            sock.write_all(resp.as_bytes()).await.unwrap();
        });

        let event = NormalizedEvent {
            platform: PlatformKind::Gitlab,
            event: EventKind::MergeRequest,
            project_name: "g/p".into(),
            author: "a".into(),
            url_slug: "x".into(),
            raw: json!({"project":{"id":42},"object_attributes":{"iid":7,"action":"open","draft":false}}),
        };
        let creds = PlatformCredentials {
            token: "tok".into(),
            base_url: format!("http://{addr}/"),
            url_slug: "local".into(),
        };
        let http = Client::builder().no_proxy().build().unwrap();
        HttpCommentPoster::with_http(http)
            .post_review_note(&event, &creds, "review-ok")
            .await
            .unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn posts_gitea_pr_comment() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let req = read_http_request(&mut sock).await;
            let lower = req.to_ascii_lowercase();
            assert!(
                lower.contains("post /api/v1/repos/o/r/issues/3/comments"),
                "{req}"
            );
            assert!(lower.contains("authorization: token gt"), "{req}");
            let body = r#"{"id":1}"#;
            let resp = format!(
                "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            sock.write_all(resp.as_bytes()).await.unwrap();
        });
        let event = NormalizedEvent {
            platform: PlatformKind::Gitea,
            event: EventKind::PullRequest,
            project_name: "o/r".into(),
            author: "b".into(),
            url_slug: "x".into(),
            raw: json!({
                "action": "opened",
                "repository": {"full_name": "o/r"},
                "pull_request": {"number": 3, "draft": false}
            }),
        };
        let creds = PlatformCredentials {
            token: "gt".into(),
            base_url: format!("http://{addr}/"),
            url_slug: "local".into(),
        };
        let http = Client::builder().no_proxy().build().unwrap();
        HttpCommentPoster::with_http(http)
            .post_review_note(&event, &creds, "gitea-ok")
            .await
            .unwrap();
        server.await.unwrap();
    }
}
