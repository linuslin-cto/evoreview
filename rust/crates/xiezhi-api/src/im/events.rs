//! 审查完成 IM 文案（对齐 `biz.event.event_manager`）。

use super::{send_notification, Notification};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct CommitBrief {
    pub message: String,
    pub url: Option<String>,
}

pub struct MrNotify<'a> {
    pub project_name: &'a str,
    pub author: &'a str,
    pub source_branch: &'a str,
    pub target_branch: &'a str,
    pub commits: &'a [CommitBrief],
    pub url: &'a str,
    pub review_result: &'a str,
    pub score: i32,
    pub url_slug: &'a str,
    pub webhook_data: &'a Value,
}

pub struct PushNotify<'a> {
    pub project_name: &'a str,
    pub author: &'a str,
    pub branch: &'a str,
    pub commits: &'a [CommitBrief],
    pub review_result: &'a str,
    pub score: i32,
    pub url_slug: &'a str,
    pub webhook_data: &'a Value,
}

fn brief_commits(commits: &[CommitBrief], max_items: usize, max_chars: usize) -> String {
    let mut firsts = Vec::new();
    for c in commits.iter().take(max_items) {
        let first_line = c
            .message
            .trim()
            .lines()
            .next()
            .unwrap_or("")
            .chars()
            .take(max_chars)
            .collect::<String>();
        if !first_line.is_empty() {
            firsts.push(first_line);
        }
    }
    let more = commits.len().saturating_sub(max_items);
    let mut text = if firsts.is_empty() {
        "（无提交信息）".to_string()
    } else {
        firsts.join("；")
    };
    if more > 0 {
        text.push_str(&format!(" …（共 {} 个提交）", commits.len()));
    }
    text
}

fn has_report(review_result: &str) -> bool {
    !review_result.is_empty() && review_result.contains("总分")
}

pub fn format_mr_reviewed_markdown(n: &MrNotify<'_>) -> String {
    format!(
        "### 🔀 {}: Merge Request\n\n\
         - **提交者:** {}\n\
         - **分支**: `{}` → `{}`\n\
         - **提交:** {}\n\
         - [查看合并详情]({})\n\n\
         ---\n\n\
         {}\n",
        n.project_name,
        n.author,
        n.source_branch,
        n.target_branch,
        brief_commits(n.commits, 3, 160),
        n.url,
        n.review_result
    )
}

pub fn format_push_reviewed_markdown(n: &PushNotify<'_>) -> String {
    let mut msg = format!(
        "### 🚀 {}: Push\n\n\
         - **提交者:** {}\n\
         - **分支**: `{}`\n\
         - **提交:** {}\n",
        n.project_name,
        n.author,
        n.branch,
        brief_commits(n.commits, 3, 160)
    );
    if let Some(u) = n.commits.iter().find_map(|c| c.url.as_deref()) {
        msg.push_str(&format!("- [查看提交详情]({u})\n"));
    }
    if !n.review_result.is_empty() {
        msg.push_str(&format!("\n---\n\n{}\n", n.review_result));
    }
    msg
}

pub async fn notify_mr_reviewed(n: MrNotify<'_>) {
    let content = format_mr_reviewed_markdown(&n);
    let mut title = format!("MR 审查 · {}", n.project_name);
    if has_report(n.review_result) {
        title.push_str(&format!(" · {}分", n.score));
    }
    let mut note = Notification::markdown(content, title);
    note.project_name = Some(n.project_name.to_string());
    note.url_slug = Some(n.url_slug.to_string());
    note.webhook_data = n.webhook_data.clone();
    send_notification(note).await;
}

pub async fn notify_push_reviewed(n: PushNotify<'_>) {
    let content = format_push_reviewed_markdown(&n);
    let mut title = format!("Push 审查 · {}", n.project_name);
    if has_report(n.review_result) {
        title.push_str(&format!(" · {}分", n.score));
    }
    let mut note = Notification::markdown(content, title);
    note.project_name = Some(n.project_name.to_string());
    note.url_slug = Some(n.url_slug.to_string());
    note.webhook_data = n.webhook_data.clone();
    send_notification(note).await;
}

pub fn commits_from_webhook(raw: &Value) -> Vec<CommitBrief> {
    let mut out = Vec::new();
    if let Some(arr) = raw.get("commits").and_then(|v| v.as_array()) {
        for c in arr {
            out.push(CommitBrief {
                message: c
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                url: c.get("url").and_then(|v| v.as_str()).map(str::to_string),
            });
        }
    }
    if out.is_empty() {
        if let Some(msg) = raw
            .pointer("/object_attributes/last_commit/message")
            .and_then(|v| v.as_str())
        {
            out.push(CommitBrief {
                message: msg.to_string(),
                url: raw
                    .pointer("/object_attributes/url")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn brief_and_titles() {
        let commits = vec![
            CommitBrief {
                message: "fix a\nmore".into(),
                url: Some("https://ex/1".into()),
            },
            CommitBrief {
                message: "fix b".into(),
                url: None,
            },
        ];
        let mr = MrNotify {
            project_name: "g/p",
            author: "alice",
            source_branch: "feat",
            target_branch: "main",
            commits: &commits,
            url: "https://ex/mr/1",
            review_result: "总分:88分\nok",
            score: 88,
            url_slug: "g-p",
            webhook_data: &json!({}),
        };
        let md = format_mr_reviewed_markdown(&mr);
        assert!(md.contains("alice"));
        assert!(md.contains("fix a"));
        assert!(md.contains("总分:88分"));
        assert!(has_report("总分:88分"));
        assert!(!has_report("关注的文件没有修改"));
        let push = PushNotify {
            project_name: "g/p",
            author: "bob",
            branch: "main",
            commits: &commits,
            review_result: "hi",
            score: 0,
            url_slug: "g-p",
            webhook_data: &json!({}),
        };
        let push_md = format_push_reviewed_markdown(&push);
        assert!(push_md.contains("查看提交详情"));
    }
}
