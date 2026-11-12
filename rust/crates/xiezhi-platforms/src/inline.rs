//! 行级 inline 评论（对齐 `biz.review.inline` + 平台 discussions/PR comments）。

use crate::http_poster::apply_auth;
use crate::{CommentError, EventKind, NormalizedEvent, PlatformCredentials, PlatformKind};
use reqwest::Client;
use serde_json::{json, Value};
use tracing::{info, warn};
use xiezhi_core::ScrComment;

/// 对齐 Python `format_inline_body`。
pub fn format_inline_body(comment: &ScrComment) -> String {
    let emoji = comment.severity_emoji();
    let mut lines = vec![format!(
        "{emoji} **[{}/{}]** {}",
        comment.category, comment.severity, comment.content
    )];
    let suggestion = comment.suggestion_code.trim();
    if !suggestion.is_empty() {
        lines.push(String::new());
        lines.push("**建议：**".into());
        lines.push("```suggestion".into());
        for line in suggestion.lines() {
            lines.push(line.to_string());
        }
        lines.push("```".into());
    }
    lines.push(String::new());
    lines.push("<sub>🤖 由獬豸生成</sub>".into());
    lines.join("\n")
}

fn inline_enabled() -> bool {
    std::env::var("SCR_INLINE_COMMENTS_ENABLED").unwrap_or_else(|_| "1".into()) != "0"
}

fn github_api_root(creds: &PlatformCredentials) -> String {
    let base = creds.base_url.trim_end_matches('/');
    if base.contains("api.github.com") {
        base.to_string()
    } else if base.contains("github.com") {
        "https://api.github.com".into()
    } else {
        base.to_string()
    }
}

fn repo_full_name(raw: &Value) -> Option<String> {
    raw.pointer("/repository/full_name")
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

fn pr_number(raw: &Value) -> Option<u64> {
    raw.pointer("/pull_request/number")
        .or_else(|| raw.pointer("/number"))
        .and_then(|v| v.as_u64().or_else(|| v.as_i64().map(|i| i as u64)))
}

fn gitlab_project_id(raw: &Value) -> Option<i64> {
    raw.pointer("/project/id")
        .and_then(|v| v.as_i64())
        .or_else(|| {
            raw.pointer("/project/id")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok())
        })
}

fn mr_iid(raw: &Value) -> Option<u64> {
    raw.pointer("/object_attributes/iid")
        .and_then(|v| v.as_u64().or_else(|| v.as_i64().map(|i| i as u64)))
}

/// 向平台发表行级评论；失败的单条跳过，返回成功条数。
pub async fn post_inline_comments(
    http: &Client,
    event: &NormalizedEvent,
    creds: &PlatformCredentials,
    comments: &[ScrComment],
) -> Result<usize, CommentError> {
    if comments.is_empty() || !inline_enabled() {
        return Ok(0);
    }
    match (event.platform, event.event) {
        (PlatformKind::Gitlab, EventKind::MergeRequest) => {
            post_gitlab_mr_inline(http, event, creds, comments).await
        }
        (PlatformKind::Github, EventKind::PullRequest) => {
            post_github_pr_inline(http, event, creds, comments).await
        }
        (PlatformKind::Gitea, EventKind::PullRequest) => {
            post_gitea_pr_inline(http, event, creds, comments).await
        }
        _ => Ok(0),
    }
}

async fn post_gitlab_mr_inline(
    http: &Client,
    event: &NormalizedEvent,
    creds: &PlatformCredentials,
    comments: &[ScrComment],
) -> Result<usize, CommentError> {
    let project_id = gitlab_project_id(&event.raw)
        .ok_or_else(|| CommentError::Request("missing gitlab project id".into()))?;
    let iid = mr_iid(&event.raw)
        .ok_or_else(|| CommentError::Request("missing merge_request iid".into()))?;
    let base = creds.base_url.trim_end_matches('/');

    let info_url = format!("{base}/api/v4/projects/{project_id}/merge_requests/{iid}");
    let info_req = apply_auth(http.get(&info_url), PlatformKind::Gitlab, &creds.token);
    let info_resp = info_req
        .send()
        .await
        .map_err(|e| CommentError::Request(e.to_string()))?;
    if !info_resp.status().is_success() {
        warn!(status = %info_resp.status(), "gitlab MR info fetch failed; skip inline");
        return Ok(0);
    }
    let info: Value = info_resp
        .json()
        .await
        .map_err(|e| CommentError::Request(e.to_string()))?;
    let refs = info.get("diff_refs").cloned().unwrap_or(Value::Null);
    let base_sha = refs.get("base_sha").and_then(|v| v.as_str()).unwrap_or("");
    let start_sha = refs.get("start_sha").and_then(|v| v.as_str()).unwrap_or("");
    let head_sha = refs.get("head_sha").and_then(|v| v.as_str()).unwrap_or("");
    if base_sha.is_empty() || start_sha.is_empty() || head_sha.is_empty() {
        warn!("MR diff_refs missing; skip inline");
        return Ok(0);
    }

    let url = format!("{base}/api/v4/projects/{project_id}/merge_requests/{iid}/discussions");
    let mut posted = 0usize;
    for comment in comments {
        if comment.start_line <= 0 || comment.path.is_empty() || comment.filtered {
            continue;
        }
        let payload = json!({
            "body": format_inline_body(comment),
            "position": {
                "base_sha": base_sha,
                "start_sha": start_sha,
                "head_sha": head_sha,
                "position_type": "text",
                "new_path": comment.path,
                "old_path": comment.path,
                "new_line": comment.start_line,
            }
        });
        let resp = apply_auth(http.post(&url), PlatformKind::Gitlab, &creds.token)
            .json(&payload)
            .send()
            .await
            .map_err(|e| CommentError::Request(e.to_string()))?;
        let status = resp.status();
        if status.as_u16() == 200 || status.as_u16() == 201 {
            posted += 1;
        } else {
            let text = resp.text().await.unwrap_or_default();
            info!(
                path = %comment.path,
                line = comment.start_line,
                status = %status,
                body = %text,
                "gitlab inline skipped"
            );
        }
    }
    info!(posted, total = comments.len(), "gitlab MR inline posted");
    Ok(posted)
}

async fn post_github_pr_inline(
    http: &Client,
    event: &NormalizedEvent,
    creds: &PlatformCredentials,
    comments: &[ScrComment],
) -> Result<usize, CommentError> {
    let repo = repo_full_name(&event.raw)
        .ok_or_else(|| CommentError::Request("missing github repo".into()))?;
    let number = pr_number(&event.raw)
        .ok_or_else(|| CommentError::Request("missing pull_request number".into()))?;
    let commit_id = event
        .raw
        .pointer("/pull_request/head/sha")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if commit_id.is_empty() {
        warn!("github PR missing head.sha; skip inline");
        return Ok(0);
    }
    let root = github_api_root(creds);
    let url = format!("{root}/repos/{repo}/pulls/{number}/comments");
    let mut posted = 0usize;
    for comment in comments {
        if comment.start_line <= 0 || comment.path.is_empty() || comment.filtered {
            continue;
        }
        let payload = json!({
            "body": format_inline_body(comment),
            "commit_id": commit_id,
            "path": comment.path,
            "line": comment.start_line,
            "side": "RIGHT",
        });
        let resp = apply_auth(http.post(&url), PlatformKind::Github, &creds.token)
            .json(&payload)
            .send()
            .await
            .map_err(|e| CommentError::Request(e.to_string()))?;
        let status = resp.status();
        if status.as_u16() == 200 || status.as_u16() == 201 {
            posted += 1;
        } else {
            let text = resp.text().await.unwrap_or_default();
            info!(
                path = %comment.path,
                line = comment.start_line,
                status = %status,
                body = %text,
                "github inline skipped"
            );
        }
    }
    info!(posted, total = comments.len(), "github PR inline posted");
    Ok(posted)
}

async fn post_gitea_pr_inline(
    http: &Client,
    event: &NormalizedEvent,
    creds: &PlatformCredentials,
    comments: &[ScrComment],
) -> Result<usize, CommentError> {
    let repo = repo_full_name(&event.raw)
        .ok_or_else(|| CommentError::Request("missing gitea repo".into()))?;
    let number = pr_number(&event.raw)
        .ok_or_else(|| CommentError::Request("missing pull_request number".into()))?;
    let commit_id = event
        .raw
        .pointer("/pull_request/head/sha")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if commit_id.is_empty() {
        warn!("gitea PR missing head.sha; skip inline");
        return Ok(0);
    }
    let base = creds.base_url.trim_end_matches('/');
    let url = format!("{base}/api/v1/repos/{repo}/pulls/{number}/reviews");
    let review_comments: Vec<Value> = comments
        .iter()
        .filter(|c| c.start_line > 0 && !c.path.is_empty() && !c.filtered)
        .map(|c| {
            json!({
                "body": format_inline_body(c),
                "path": c.path,
                "new_position": c.start_line,
            })
        })
        .collect();
    let n = review_comments.len();
    if n == 0 {
        return Ok(0);
    }
    let payload = json!({
        "body": "",
        "commit_id": commit_id,
        "comments": review_comments,
    });
    let resp = apply_auth(http.post(&url), PlatformKind::Gitea, &creds.token)
        .json(&payload)
        .send()
        .await
        .map_err(|e| CommentError::Request(e.to_string()))?;
    let status = resp.status();
    if status.as_u16() == 200 || status.as_u16() == 201 {
        info!(posted = n, "gitea PR inline review posted");
        Ok(n)
    } else {
        let text = resp.text().await.unwrap_or_default();
        warn!(status = %status, body = %text, "gitea inline review failed");
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EventKind, PlatformKind};
    use serde_json::json;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn format_includes_suggestion_and_footer() {
        let c = ScrComment {
            path: "a.py".into(),
            content: "bad".into(),
            category: "bug".into(),
            severity: "high".into(),
            suggestion_code: "x = 1".into(),
            start_line: 3,
            ..Default::default()
        };
        let body = format_inline_body(&c);
        assert!(body.contains("**[bug/high]** bad"));
        assert!(body.contains("```suggestion"));
        assert!(body.contains("x = 1"));
        assert!(body.contains("由獬豸生成"));
    }

    #[tokio::test]
    async fn posts_github_inline_comment() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            let mut tmp = [0u8; 1024];
            loop {
                let n = sock.read(&mut tmp).await.unwrap();
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
                if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let req = String::from_utf8_lossy(&buf).to_ascii_lowercase();
            assert!(req.contains("post /repos/o/r/pulls/3/comments"), "{req}");
            assert!(req.contains("\"path\":\"a.py\""), "{req}");
            assert!(req.contains("\"line\":7"), "{req}");
            let body = r#"{"id":1}"#;
            let resp = format!(
                "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            sock.write_all(resp.as_bytes()).await.unwrap();
        });

        let event = NormalizedEvent {
            platform: PlatformKind::Github,
            event: EventKind::PullRequest,
            project_name: "o/r".into(),
            author: "a".into(),
            url_slug: "x".into(),
            raw: json!({
                "repository": {"full_name": "o/r"},
                "pull_request": {"number": 3, "head": {"sha": "abc123"}}
            }),
        };
        let creds = PlatformCredentials {
            token: "gh".into(),
            base_url: format!("http://{addr}/"),
            url_slug: "local".into(),
        };
        let comments = vec![ScrComment {
            path: "a.py".into(),
            content: "issue".into(),
            start_line: 7,
            category: "bug".into(),
            severity: "medium".into(),
            ..Default::default()
        }];
        let http = Client::builder().no_proxy().build().unwrap();
        let n = post_inline_comments(&http, &event, &creds, &comments)
            .await
            .unwrap();
        assert_eq!(n, 1);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn posts_gitlab_inline_discussion() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            // 1) MR info
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 2048];
            let _ = sock.read(&mut buf).await.unwrap();
            let info = r#"{"diff_refs":{"base_sha":"b","start_sha":"s","head_sha":"h"}}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{info}",
                info.len()
            );
            sock.write_all(resp.as_bytes()).await.unwrap();

            // 2) discussion
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            let mut tmp = [0u8; 1024];
            loop {
                let n = sock.read(&mut tmp).await.unwrap();
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
                if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let req = String::from_utf8_lossy(&buf).to_ascii_lowercase();
            assert!(
                req.contains("post /api/v4/projects/9/merge_requests/2/discussions"),
                "{req}"
            );
            assert!(req.contains("\"new_line\":4"), "{req}");
            let body = r#"{"id":"1"}"#;
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
            raw: json!({"project":{"id":9},"object_attributes":{"iid":2}}),
        };
        let creds = PlatformCredentials {
            token: "tok".into(),
            base_url: format!("http://{addr}/"),
            url_slug: "local".into(),
        };
        let comments = vec![ScrComment {
            path: "m.py".into(),
            content: "x".into(),
            start_line: 4,
            ..Default::default()
        }];
        let http = Client::builder().no_proxy().build().unwrap();
        let n = post_inline_comments(&http, &event, &creds, &comments)
            .await
            .unwrap();
        assert_eq!(n, 1);
        server.await.unwrap();
    }
}
