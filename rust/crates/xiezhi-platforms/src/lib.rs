//! GitLab / GitHub / Gitea webhook 适配。

mod comment_plan;
mod comments;
mod http_poster;
mod inline;
mod slug;

pub use comment_plan::{plan_review_note, plan_review_note_url, PlannedComment};
pub use comments::{
    CommentError, CommentPoster, LoggingCommentPoster, MemoryCommentPoster, PlanningCommentPoster,
    PlatformCredentials, PostedNote,
};
pub use http_poster::{review_note_json, HttpCommentPoster};
pub use inline::{format_inline_body, post_inline_comments};
pub use slug::slugify_url;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PlatformError {
    #[error("unsupported webhook payload")]
    Unsupported,
    #[error("missing field: {0}")]
    MissingField(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformKind {
    Gitlab,
    Github,
    Gitea,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Push,
    MergeRequest,
    PullRequest,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedEvent {
    pub platform: PlatformKind,
    pub event: EventKind,
    pub project_name: String,
    pub author: String,
    pub url_slug: String,
    pub raw: Value,
}

pub fn detect_platform(payload: &Value) -> PlatformKind {
    if payload.get("object_kind").is_some() {
        if payload
            .pointer("/repository/html_url")
            .and_then(|v| v.as_str())
            .map(|u| u.contains("gitea") || u.contains("/git/"))
            .unwrap_or(false)
        {
            return PlatformKind::Gitea;
        }
        return PlatformKind::Gitlab;
    }
    if payload.get("pull_request").is_some() || payload.get("pusher").is_some() {
        return PlatformKind::Github;
    }
    PlatformKind::Unknown
}

pub fn detect_event(payload: &Value) -> EventKind {
    match payload.get("object_kind").and_then(|v| v.as_str()) {
        Some("push") => return EventKind::Push,
        Some("merge_request") => return EventKind::MergeRequest,
        _ => {}
    }
    if payload.get("pull_request").is_some() {
        return EventKind::PullRequest;
    }
    if payload.get("ref").is_some() && payload.get("commits").is_some() {
        return EventKind::Push;
    }
    EventKind::Unknown
}

pub fn normalize(payload: Value) -> Result<NormalizedEvent, PlatformError> {
    let platform = detect_platform(&payload);
    let event = detect_event(&payload);
    if platform == PlatformKind::Unknown || event == EventKind::Unknown {
        return Err(PlatformError::Unsupported);
    }

    let project_name = payload
        .pointer("/project/path_with_namespace")
        .or_else(|| payload.pointer("/project/name"))
        .or_else(|| payload.pointer("/repository/full_name"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let author = payload
        .pointer("/user_username")
        .or_else(|| payload.pointer("/user_name"))
        .or_else(|| payload.pointer("/pusher/name"))
        .or_else(|| payload.pointer("/sender/login"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let url_slug = project_name.replace('/', "-");

    Ok(NormalizedEvent {
        platform,
        event,
        project_name,
        author,
        url_slug,
        raw: payload,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detect_gitlab_push() {
        let p = json!({
            "object_kind": "push",
            "project": {"path_with_namespace": "g/p"},
            "user_username": "alice"
        });
        let n = normalize(p).unwrap();
        assert_eq!(n.platform, PlatformKind::Gitlab);
        assert_eq!(n.event, EventKind::Push);
        assert_eq!(n.author, "alice");
    }

    #[test]
    fn reject_unknown() {
        let err = normalize(json!({"foo": 1})).unwrap_err();
        assert_eq!(err, PlatformError::Unsupported);
    }
}
