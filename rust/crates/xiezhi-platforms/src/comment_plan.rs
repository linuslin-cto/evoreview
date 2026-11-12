//! 平台评论 API 路径规划（真实 HTTP 回写前的 stub）。

use crate::{CommentError, EventKind, NormalizedEvent, PlatformKind};
use serde_json::Value;

/// 一次计划中的评论回写请求（不发真实 HTTP）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedComment {
    pub method: &'static str,
    pub url: String,
    pub body: String,
}

/// 从归一化事件解析评论目标 URL。
pub fn plan_review_note_url(
    event: &NormalizedEvent,
    creds: &crate::PlatformCredentials,
) -> Result<String, CommentError> {
    match (event.platform, event.event) {
        (PlatformKind::Gitlab, EventKind::MergeRequest) => {
            let project_id = gitlab_project_id(&event.raw)
                .ok_or(CommentError::Request("missing gitlab project id".into()))?;
            let iid = event
                .raw
                .pointer("/object_attributes/iid")
                .and_then(|v| v.as_u64().or_else(|| v.as_i64().map(|i| i as u64)))
                .ok_or(CommentError::Request("missing merge_request iid".into()))?;
            Ok(format!(
                "{}api/v4/projects/{}/merge_requests/{}/notes",
                ensure_slash(&creds.base_url),
                project_id,
                iid
            ))
        }
        (PlatformKind::Gitlab, EventKind::Push) => {
            let project_id = gitlab_project_id(&event.raw)
                .ok_or(CommentError::Request("missing gitlab project id".into()))?;
            let commit = event
                .raw
                .get("after")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty() && *s != "0000000000000000000000000000000000000000")
                .ok_or(CommentError::Request("missing push commit id".into()))?;
            Ok(format!(
                "{}api/v4/projects/{}/repository/commits/{}/comments",
                ensure_slash(&creds.base_url),
                project_id,
                commit
            ))
        }
        (PlatformKind::Github, EventKind::PullRequest) => {
            let repo = github_repo_full_name(&event.raw)
                .ok_or(CommentError::Request("missing github repo".into()))?;
            let number = pr_number(&event.raw)
                .ok_or(CommentError::Request("missing pull_request number".into()))?;
            Ok(format!(
                "https://api.github.com/repos/{repo}/issues/{number}/comments"
            ))
        }
        (PlatformKind::Github, EventKind::Push) => {
            let repo = github_repo_full_name(&event.raw)
                .ok_or(CommentError::Request("missing github repo".into()))?;
            let commit = event
                .raw
                .get("after")
                .and_then(|v| v.as_str())
                .ok_or(CommentError::Request("missing push commit id".into()))?;
            Ok(format!(
                "https://api.github.com/repos/{repo}/commits/{commit}/comments"
            ))
        }
        (PlatformKind::Gitea, EventKind::PullRequest) => {
            let repo = gitea_repo_full_name(&event.raw)
                .ok_or(CommentError::Request("missing gitea repo".into()))?;
            let number = pr_number(&event.raw)
                .ok_or(CommentError::Request("missing pull_request number".into()))?;
            Ok(format!(
                "{}api/v1/repos/{repo}/issues/{number}/comments",
                ensure_slash(&creds.base_url)
            ))
        }
        (PlatformKind::Gitea, EventKind::Push) => {
            let repo = gitea_repo_full_name(&event.raw)
                .ok_or(CommentError::Request("missing gitea repo".into()))?;
            let commit = event
                .raw
                .get("after")
                .and_then(|v| v.as_str())
                .ok_or(CommentError::Request("missing push commit id".into()))?;
            Ok(format!(
                "{}api/v1/repos/{repo}/commits/{commit}/comments",
                ensure_slash(&creds.base_url)
            ))
        }
        (_, event_kind) => Err(CommentError::UnsupportedEvent(event_kind)),
    }
}

pub fn plan_review_note(
    event: &NormalizedEvent,
    creds: &crate::PlatformCredentials,
    body: &str,
) -> Result<PlannedComment, CommentError> {
    if creds.token.trim().is_empty() {
        return Err(CommentError::MissingCredentials(event.platform));
    }
    let url = plan_review_note_url(event, creds)?;
    Ok(PlannedComment {
        method: "POST",
        url,
        body: body.to_string(),
    })
}

fn ensure_slash(base: &str) -> String {
    if base.ends_with('/') {
        base.to_string()
    } else {
        format!("{base}/")
    }
}

fn gitlab_project_id(raw: &Value) -> Option<String> {
    raw.pointer("/project/id")
        .and_then(|v| {
            v.as_u64()
                .map(|n| n.to_string())
                .or_else(|| v.as_i64().map(|n| n.to_string()))
                .or_else(|| v.as_str().map(str::to_string))
        })
        .or_else(|| {
            raw.pointer("/project/path_with_namespace")
                .and_then(|v| v.as_str())
                .map(urlencoding_lite)
        })
}

fn github_repo_full_name(raw: &Value) -> Option<String> {
    raw.pointer("/repository/full_name")
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

fn gitea_repo_full_name(raw: &Value) -> Option<String> {
    github_repo_full_name(raw).or_else(|| {
        let owner = raw
            .pointer("/repository/owner/login")
            .or_else(|| raw.pointer("/repository/owner/username"))
            .and_then(|v| v.as_str())?;
        let name = raw.pointer("/repository/name").and_then(|v| v.as_str())?;
        Some(format!("{owner}/{name}"))
    })
}

fn pr_number(raw: &Value) -> Option<u64> {
    raw.pointer("/pull_request/number")
        .or_else(|| raw.pointer("/number"))
        .and_then(|v| v.as_u64().or_else(|| v.as_i64().map(|i| i as u64)))
}

fn urlencoding_lite(s: &str) -> String {
    s.replace('/', "%2F")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EventKind, NormalizedEvent, PlatformCredentials, PlatformKind};
    use serde_json::json;

    #[test]
    fn plans_gitlab_mr_note() {
        let event = NormalizedEvent {
            platform: PlatformKind::Gitlab,
            event: EventKind::MergeRequest,
            project_name: "g/p".into(),
            author: "a".into(),
            url_slug: "x".into(),
            raw: json!({"project":{"id":42},"object_attributes":{"iid":7}}),
        };
        let creds = PlatformCredentials {
            token: "t".into(),
            base_url: "https://gitlab.example/".into(),
            url_slug: "gitlab_example".into(),
        };
        let plan = plan_review_note(&event, &creds, "hi").unwrap();
        assert_eq!(plan.method, "POST");
        assert_eq!(
            plan.url,
            "https://gitlab.example/api/v4/projects/42/merge_requests/7/notes"
        );
    }

    #[test]
    fn plans_github_pr_comment() {
        let event = NormalizedEvent {
            platform: PlatformKind::Github,
            event: EventKind::PullRequest,
            project_name: "o/r".into(),
            author: "a".into(),
            url_slug: "x".into(),
            raw: json!({"repository":{"full_name":"o/r"},"pull_request":{"number":9}}),
        };
        let creds = PlatformCredentials {
            token: "t".into(),
            base_url: "https://github.com".into(),
            url_slug: "github_com".into(),
        };
        let plan = plan_review_note(&event, &creds, "hi").unwrap();
        assert_eq!(
            plan.url,
            "https://api.github.com/repos/o/r/issues/9/comments"
        );
    }

    #[test]
    fn empty_token_errors() {
        let event = NormalizedEvent {
            platform: PlatformKind::Gitea,
            event: EventKind::PullRequest,
            project_name: "o/r".into(),
            author: "a".into(),
            url_slug: "x".into(),
            raw: json!({"repository":{"full_name":"o/r"},"pull_request":{"number":1}}),
        };
        let creds = PlatformCredentials {
            token: " ".into(),
            base_url: "https://gitea.example/".into(),
            url_slug: "gitea_example".into(),
        };
        assert!(matches!(
            plan_review_note(&event, &creds, "x"),
            Err(CommentError::MissingCredentials(_))
        ));
    }
}
