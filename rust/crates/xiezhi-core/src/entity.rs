//! 审查实体（对齐 Python `biz.entity.review_entity`）。

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub message: String,
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeRequestReviewEntity {
    pub project_name: String,
    pub author: String,
    pub source_branch: String,
    pub target_branch: String,
    pub updated_at: i64,
    pub commits: Vec<CommitInfo>,
    pub score: f64,
    pub url: String,
    pub review_result: String,
    pub url_slug: String,
    pub webhook_data: Value,
    pub additions: i32,
    pub deletions: i32,
    pub last_commit_id: String,
}

impl MergeRequestReviewEntity {
    pub fn commit_messages(&self) -> String {
        self.commits
            .iter()
            .map(|c| c.message.trim())
            .collect::<Vec<_>>()
            .join("; ")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushReviewEntity {
    pub project_name: String,
    pub author: String,
    pub branch: String,
    pub updated_at: i64,
    pub commits: Vec<CommitInfo>,
    pub score: f64,
    pub review_result: String,
    pub url_slug: String,
    pub webhook_data: Value,
    pub additions: i32,
    pub deletions: i32,
}

impl PushReviewEntity {
    pub fn commit_messages(&self) -> String {
        self.commits
            .iter()
            .map(|c| c.message.trim())
            .collect::<Vec<_>>()
            .join("; ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn commit_messages_join() {
        let e = PushReviewEntity {
            project_name: "p".into(),
            author: "a".into(),
            branch: "main".into(),
            updated_at: 0,
            commits: vec![
                CommitInfo {
                    message: " fix a ".into(),
                    id: None,
                },
                CommitInfo {
                    message: "add b".into(),
                    id: None,
                },
            ],
            score: 90.0,
            review_result: String::new(),
            url_slug: String::new(),
            webhook_data: json!({}),
            additions: 1,
            deletions: 0,
        };
        assert_eq!(e.commit_messages(), "fix a; add b");
    }
}
