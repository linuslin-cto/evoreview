//! 平台评论回写接口（骨架；真实 HTTP 回写后续迭代）。

use crate::comment_plan::{plan_review_note, PlannedComment};
use crate::{EventKind, NormalizedEvent, PlatformKind};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub enum CommentError {
    #[error("missing credentials for {0:?}")]
    MissingCredentials(PlatformKind),
    #[error("unsupported event for comments: {0:?}")]
    UnsupportedEvent(EventKind),
    #[error("request failed: {0}")]
    Request(String),
}

#[derive(Debug, Clone)]
pub struct PlatformCredentials {
    pub token: String,
    pub base_url: String,
    pub url_slug: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostedNote {
    pub platform: PlatformKind,
    pub event: EventKind,
    pub project_name: String,
    pub url_slug: String,
    pub body: String,
}

#[async_trait]
pub trait CommentPoster: Send + Sync {
    async fn post_review_note(
        &self,
        event: &NormalizedEvent,
        creds: &PlatformCredentials,
        body: &str,
    ) -> Result<(), CommentError>;

    /// 行级 inline 评论；默认 no-op。
    async fn post_inline_comments(
        &self,
        _event: &NormalizedEvent,
        _creds: &PlatformCredentials,
        _comments: &[xiezhi_core::ScrComment],
    ) -> Result<usize, CommentError> {
        Ok(0)
    }
}

/// 仅记录日志的 stub poster（会规划 URL，不发 HTTP）。
pub struct LoggingCommentPoster;

#[async_trait]
impl CommentPoster for LoggingCommentPoster {
    async fn post_review_note(
        &self,
        event: &NormalizedEvent,
        creds: &PlatformCredentials,
        body: &str,
    ) -> Result<(), CommentError> {
        match plan_review_note(event, creds, body) {
            Ok(plan) => {
                info!(
                    platform = ?event.platform,
                    event = ?event.event,
                    project = %event.project_name,
                    url_slug = %creds.url_slug,
                    method = plan.method,
                    url = %plan.url,
                    body_chars = body.chars().count(),
                    "stub: would post review note to platform"
                );
            }
            Err(e) => {
                info!(
                    platform = ?event.platform,
                    event = ?event.event,
                    project = %event.project_name,
                    url_slug = %creds.url_slug,
                    error = %e,
                    body_chars = body.chars().count(),
                    "stub: would post review note (url plan unavailable)"
                );
            }
        }
        Ok(())
    }
}

/// 记录规划中的 HTTP 请求，便于断言回写目标。
#[derive(Clone, Default)]
pub struct PlanningCommentPoster {
    pub plans: Arc<Mutex<Vec<PlannedComment>>>,
}

impl PlanningCommentPoster {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> Vec<PlannedComment> {
        self.plans.lock().expect("lock").clone()
    }
}

#[async_trait]
impl CommentPoster for PlanningCommentPoster {
    async fn post_review_note(
        &self,
        event: &NormalizedEvent,
        creds: &PlatformCredentials,
        body: &str,
    ) -> Result<(), CommentError> {
        let plan = plan_review_note(event, creds, body)?;
        self.plans.lock().expect("lock").push(plan);
        Ok(())
    }
}

/// 内存记录 poster，便于单测断言入队后的回写行为。
#[derive(Clone, Default)]
pub struct MemoryCommentPoster {
    pub notes: Arc<Mutex<Vec<PostedNote>>>,
}

impl MemoryCommentPoster {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> Vec<PostedNote> {
        self.notes.lock().expect("lock").clone()
    }
}

#[async_trait]
impl CommentPoster for MemoryCommentPoster {
    async fn post_review_note(
        &self,
        event: &NormalizedEvent,
        creds: &PlatformCredentials,
        body: &str,
    ) -> Result<(), CommentError> {
        let note = PostedNote {
            platform: event.platform,
            event: event.event,
            project_name: event.project_name.clone(),
            url_slug: creds.url_slug.clone(),
            body: body.to_string(),
        };
        self.notes.lock().expect("lock").push(note);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EventKind, PlatformKind};
    use serde_json::json;

    fn sample_event() -> crate::NormalizedEvent {
        crate::NormalizedEvent {
            platform: PlatformKind::Gitlab,
            event: EventKind::MergeRequest,
            project_name: "g/p".into(),
            author: "alice".into(),
            url_slug: "g-p".into(),
            raw: json!({}),
        }
    }

    #[tokio::test]
    async fn logging_poster_ok() {
        let creds = PlatformCredentials {
            token: "t".into(),
            base_url: "https://gitlab.example/".into(),
            url_slug: "gitlab_example".into(),
        };
        LoggingCommentPoster
            .post_review_note(&sample_event(), &creds, "hello review")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn memory_poster_records() {
        let poster = MemoryCommentPoster::new();
        let creds = PlatformCredentials {
            token: "t".into(),
            base_url: "https://gitlab.example/".into(),
            url_slug: "gitlab_example".into(),
        };
        poster
            .post_review_note(&sample_event(), &creds, "note-1")
            .await
            .unwrap();
        let snap = poster.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].body, "note-1");
        assert_eq!(snap[0].platform, PlatformKind::Gitlab);
    }
}
