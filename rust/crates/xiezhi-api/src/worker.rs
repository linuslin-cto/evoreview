//! Webhook background worker (review_fusion + DB + comments).
use crate::dashboard::{shared_review_store, ReviewLog, ReviewStore};
#[cfg(test)]
use crate::dashboard::{MemoryReviewStore, SqliteReviewStore};
use chrono::Utc;
use serde_json::Value;
use std::env;
use std::sync::Arc;
use tracing::{info, warn};
use xiezhi_llm::{factory_from_env, LlmClient, ScriptedClient};
use xiezhi_platforms::{
    CommentPoster, EventKind, NormalizedEvent, PlatformCredentials, PlatformKind,
};
use xiezhi_review::{review_fusion, FusionOptions, StructuredOptions};
use xiezhi_scr::{count_diff_lines, FileChange, RuleResolver};

const MR_ACTIONS: &[&str] = &["open", "update", "reopen", "merge"];
const PR_ACTIONS: &[&str] = &["opened", "synchronize", "reopened", "closed"];

/// Injectable worker deps (tests / orchestration).
pub struct WorkerContext {
    pub store: Arc<dyn ReviewStore>,
    pub changes: Option<Vec<FileChange>>,
    pub llm: Option<Arc<dyn LlmClient>>,
    /// Override protected-branch check (tests); None uses real API.
    pub protected_override: Option<bool>,
}

impl Default for WorkerContext {
    fn default() -> Self {
        Self {
            store: shared_review_store(),
            changes: None,
            llm: None,
            protected_override: None,
        }
    }
}

/// Dispatch a normalized event to the appropriate handler.
pub async fn handle_event<P: CommentPoster>(
    event: &NormalizedEvent,
    creds: &PlatformCredentials,
    poster: &P,
) -> Result<(), String> {
    handle_event_with(event, creds, poster, WorkerContext::default()).await
}

pub async fn handle_event_with<P: CommentPoster>(
    event: &NormalizedEvent,
    creds: &PlatformCredentials,
    poster: &P,
    ctx: WorkerContext,
) -> Result<(), String> {
    match (event.platform, event.event) {
        (PlatformKind::Gitlab, EventKind::MergeRequest) => {
            handle_merge_request(event, creds, poster, ctx).await
        }
        (PlatformKind::Gitlab, EventKind::Push) => handle_push(event, creds, poster, ctx).await,
        (PlatformKind::Github | PlatformKind::Gitea, EventKind::PullRequest) => {
            handle_pull_request(event, creds, poster, ctx).await
        }
        (PlatformKind::Github | PlatformKind::Gitea, EventKind::Push) => {
            handle_push(event, creds, poster, ctx).await
        }
        (platform, event_kind) => Err(format!(
            "unsupported dispatch: {platform:?} / {event_kind:?}"
        )),
    }
}

async fn handle_merge_request<P: CommentPoster>(
    event: &NormalizedEvent,
    creds: &PlatformCredentials,
    poster: &P,
    ctx: WorkerContext,
) -> Result<(), String> {
    let action = event
        .raw
        .pointer("/object_attributes/action")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    info!(
        project = %event.project_name,
        author = %event.author,
        action,
        "merge_request handler"
    );

    if is_draft_mr(&event.raw) {
        let src = event
            .raw
            .pointer("/object_attributes/source_branch")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let tgt = event
            .raw
            .pointer("/object_attributes/target_branch")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let url = event
            .raw
            .pointer("/object_attributes/url")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let msg = format!(
            "[ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ©ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ§ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¥] MRÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¤ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¸ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂºÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¨ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ§ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¨ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¿ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¯ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¼ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂdraftÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¯ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¼ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¯ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¼ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¦ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂªÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¨ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ§ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¦ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¥ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂAIÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¥ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ®ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¡ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¦ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¥ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ£ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ\nÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ©ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¡ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¹ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ§ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ®: {}\nÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¤ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ½ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¨ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ: {}\nÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¦ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂºÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¥ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¦ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¯: {src}\nÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ§ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ®ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¦ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¥ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¦ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¯: {tgt}\nÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ©ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¾ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¦ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¥: {url}",
            event.project_name, event.author
        );
        let mut note = crate::im::Notification::text(msg);
        note.project_name = Some(event.project_name.clone());
        note.url_slug = Some(event.url_slug.clone());
        note.webhook_data = event.raw.clone();
        crate::im::send_notification(note).await;
        info!("draft MR skipped");
        return Ok(());
    }
    if !MR_ACTIONS.contains(&action) {
        info!(action, "MR action ignored");
        return Ok(());
    }

    let source_branch = event
        .raw
        .pointer("/object_attributes/source_branch")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let target_branch = event
        .raw
        .pointer("/object_attributes/target_branch")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if !target_branch_allowed(&target_branch) {
        info!(%target_branch, "MR target branch not allowed");
        return Ok(());
    }

    if only_protected_enabled() {
        let ok = match ctx.protected_override {
            Some(v) => v,
            None => target_branch_protected(event, creds, &target_branch).await,
        };
        if !ok {
            info!(%target_branch, "MR target branch not protected, ignored");
            return Ok(());
        }
    }

    let last_commit_id = resolve_mr_review_commit_id(action, &event.raw);
    if !last_commit_id.is_empty()
        && ctx.store.mr_last_commit_exists(
            &event.project_name,
            &source_branch,
            &target_branch,
            &last_commit_id,
        )
    {
        info!(
            %last_commit_id,
            project = %event.project_name,
            "MR last_commit already reviewed, skip"
        );
        return Ok(());
    }

    let changes = resolve_changes(event, creds, ctx.changes).await?;
    let commits_text = commits_text_from_event(event);
    let outcome = run_fusion(&changes, &commits_text, ctx.llm.as_deref()).await?;

    let mut report = outcome.report_text.clone();
    if action == "merge" {
        report = format!("> Re-review after merge into `{target_branch}`\n\n{report}");
    }

    poster
        .post_review_note(event, creds, &report)
        .await
        .map_err(|e| e.to_string())?;

    let inline_n = poster
        .post_inline_comments(event, creds, &outcome.inline_comments)
        .await
        .unwrap_or_else(|e| {
            warn!(error = %e, "MR inline comments failed");
            0
        });
    if inline_n > 0 {
        info!(inline_n, "MR inline comments posted");
    }

    let (additions, deletions) = sum_lines(&changes);
    let url = event
        .raw
        .pointer("/object_attributes/url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let commits = crate::im::commits_from_webhook(&event.raw);

    ctx.store.insert_mr(ReviewLog {
        id: None,
        project_name: Some(event.project_name.clone()),
        author: Some(event.author.clone()),
        source_branch: Some(source_branch.clone()),
        target_branch: Some(target_branch.clone()),
        branch: None,
        updated_at: Some(Utc::now().timestamp()),
        commit_messages: Some(commits_text),
        score: Some(outcome.score as i64),
        url: Some(url.clone()),
        review_result: Some(report.clone()),
        additions: Some(additions as i64),
        deletions: Some(deletions as i64),
        last_commit_id: Some(last_commit_id),
    })?;

    crate::im::notify_mr_reviewed(crate::im::MrNotify {
        project_name: &event.project_name,
        author: &event.author,
        source_branch: &source_branch,
        target_branch: &target_branch,
        commits: &commits,
        url: &url,
        review_result: &report,
        score: outcome.score,
        url_slug: &event.url_slug,
        webhook_data: &event.raw,
    })
    .await;

    info!(score = outcome.score, "MR review persisted");
    Ok(())
}

async fn handle_pull_request<P: CommentPoster>(
    event: &NormalizedEvent,
    creds: &PlatformCredentials,
    poster: &P,
    ctx: WorkerContext,
) -> Result<(), String> {
    let action = event
        .raw
        .pointer("/action")
        .or_else(|| event.raw.pointer("/pull_request/state"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    info!(
        project = %event.project_name,
        author = %event.author,
        action,
        "pull_request handler"
    );

    if is_draft_pr(&event.raw) {
        info!("draft PR skipped");
        return Ok(());
    }
    if event.raw.get("action").is_some() && !PR_ACTIONS.contains(&action) {
        info!(action, "PR action ignored");
        return Ok(());
    }

    let source_branch = event
        .raw
        .pointer("/pull_request/head/ref")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let target_branch = event
        .raw
        .pointer("/pull_request/base/ref")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if only_protected_enabled() {
        let ok = match ctx.protected_override {
            Some(v) => v,
            None => target_branch_protected(event, creds, &target_branch).await,
        };
        if !ok {
            info!(%target_branch, "PR target branch not protected, ignored");
            return Ok(());
        }
    }

    let last_commit_id = event
        .raw
        .pointer("/pull_request/head/sha")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if !last_commit_id.is_empty()
        && ctx.store.mr_last_commit_exists(
            &event.project_name,
            &source_branch,
            &target_branch,
            &last_commit_id,
        )
    {
        info!(
            %last_commit_id,
            project = %event.project_name,
            "PR last_commit already reviewed, skip"
        );
        return Ok(());
    }

    let changes = resolve_changes(event, creds, ctx.changes).await?;
    let commits_text = commits_text_from_event(event);
    let outcome = run_fusion(&changes, &commits_text, ctx.llm.as_deref()).await?;

    poster
        .post_review_note(event, creds, &outcome.report_text)
        .await
        .map_err(|e| e.to_string())?;

    let inline_n = poster
        .post_inline_comments(event, creds, &outcome.inline_comments)
        .await
        .unwrap_or_else(|e| {
            warn!(error = %e, "PR inline comments failed");
            0
        });
    if inline_n > 0 {
        info!(inline_n, "PR inline comments posted");
    }

    let (additions, deletions) = sum_lines(&changes);
    let url = event
        .raw
        .pointer("/pull_request/html_url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let commits = crate::im::commits_from_webhook(&event.raw);

    ctx.store.insert_mr(ReviewLog {
        id: None,
        project_name: Some(event.project_name.clone()),
        author: Some(event.author.clone()),
        source_branch: Some(source_branch.clone()),
        target_branch: Some(target_branch.clone()),
        branch: None,
        updated_at: Some(Utc::now().timestamp()),
        commit_messages: Some(commits_text),
        score: Some(outcome.score as i64),
        url: Some(url.clone()),
        review_result: Some(outcome.report_text.clone()),
        additions: Some(additions as i64),
        deletions: Some(deletions as i64),
        last_commit_id: Some(last_commit_id),
    })?;

    crate::im::notify_mr_reviewed(crate::im::MrNotify {
        project_name: &event.project_name,
        author: &event.author,
        source_branch: &source_branch,
        target_branch: &target_branch,
        commits: &commits,
        url: &url,
        review_result: &outcome.report_text,
        score: outcome.score,
        url_slug: &event.url_slug,
        webhook_data: &event.raw,
    })
    .await;

    info!(score = outcome.score, "PR review persisted");
    Ok(())
}

async fn handle_push<P: CommentPoster>(
    event: &NormalizedEvent,
    creds: &PlatformCredentials,
    poster: &P,
    ctx: WorkerContext,
) -> Result<(), String> {
    let push_enabled = env::var("PUSH_REVIEW_ENABLED").unwrap_or_else(|_| "0".into()) == "1";
    info!(
        project = %event.project_name,
        author = %event.author,
        push_enabled,
        "push handler"
    );
    if !push_enabled {
        info!("push review disabled, skip");
        return Ok(());
    }

    let changes = resolve_changes(event, creds, ctx.changes).await?;
    let commits_text = commits_text_from_event(event);
    let outcome = run_fusion(&changes, &commits_text, ctx.llm.as_deref()).await?;

    poster
        .post_review_note(event, creds, &outcome.report_text)
        .await
        .map_err(|e| e.to_string())?;

    let (additions, deletions) = sum_lines(&changes);
    let branch = event
        .raw
        .get("ref")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim_start_matches("refs/heads/")
        .to_string();
    let commits = crate::im::commits_from_webhook(&event.raw);

    ctx.store.insert_push(ReviewLog {
        id: None,
        project_name: Some(event.project_name.clone()),
        author: Some(event.author.clone()),
        source_branch: None,
        target_branch: None,
        branch: Some(branch.clone()),
        updated_at: Some(Utc::now().timestamp()),
        commit_messages: Some(commits_text),
        score: Some(outcome.score as i64),
        url: None,
        review_result: Some(outcome.report_text.clone()),
        additions: Some(additions as i64),
        deletions: Some(deletions as i64),
        last_commit_id: None,
    })?;

    crate::im::notify_push_reviewed(crate::im::PushNotify {
        project_name: &event.project_name,
        author: &event.author,
        branch: &branch,
        commits: &commits,
        review_result: &outcome.report_text,
        score: outcome.score,
        url_slug: &event.url_slug,
        webhook_data: &event.raw,
    })
    .await;

    info!(score = outcome.score, "push review persisted");
    Ok(())
}

async fn run_fusion(
    changes: &[FileChange],
    commits_text: &str,
    llm: Option<&dyn LlmClient>,
) -> Result<xiezhi_review::ReviewOutcome, String> {
    if changes.is_empty() {
        return Ok(xiezhi_review::ReviewOutcome {
            report_text: "No reviewed files changed".into(),
            score: 0,
            inline_comments: Vec::new(),
        });
    }

    let owned: Option<Box<dyn LlmClient>> = if llm.is_none() {
        match factory_from_env() {
            Ok(c) => Some(c),
            Err(e) => {
                if env::var("WORKER_DRY_RUN").unwrap_or_else(|_| "0".into()) == "1" {
                    let n = changes.len().max(1);
                    let mut responses = Vec::with_capacity(n + 1);
                    for _ in 0..n {
                        responses.push(r#"{"comments":[]}"#.into());
                    }
                    responses.push("auto review (dry-run)\nscore:100".into());
                    Some(Box::new(ScriptedClient::new("dry-run", responses)) as Box<dyn LlmClient>)
                } else {
                    return Err(format!("LLM unavailable: {e}"));
                }
            }
        }
    } else {
        None
    };
    let client: &dyn LlmClient = match llm {
        Some(c) => c,
        None => owned.as_ref().map(|b| b.as_ref()).unwrap(),
    };

    let structured = StructuredOptions::from_search_roots(&[]);
    let mut opts = FusionOptions::from_structured(structured);
    opts.commits_text = commits_text.to_string();
    // Default: narrative off unless REVIEW_NARRATIVE_ENABLED=1
    if env::var("REVIEW_NARRATIVE_ENABLED").ok().as_deref() != Some("1") {
        opts.narrative_enabled = false;
    }

    let rules = RuleResolver::default();
    review_fusion(client, changes, &rules, &opts)
        .await
        .map_err(|e| e.to_string())
}

async fn resolve_changes(
    event: &NormalizedEvent,
    creds: &PlatformCredentials,
    injected: Option<Vec<FileChange>>,
) -> Result<Vec<FileChange>, String> {
    if let Some(ch) = injected {
        return Ok(filter_changes(ch));
    }
    if let Some(ch) = changes_from_payload(&event.raw) {
        return Ok(filter_changes(ch));
    }
    match fetch_platform_changes(event, creds).await {
        Ok(ch) => Ok(filter_changes(ch)),
        Err(e) => {
            warn!(error = %e, "fetch changes failed; treating as empty");
            Ok(Vec::new())
        }
    }
}

fn changes_from_payload(raw: &Value) -> Option<Vec<FileChange>> {
    let arr = raw.get("_xiezhi_changes")?.as_array()?;
    let mut out = Vec::new();
    for item in arr {
        let new_path = item
            .get("new_path")
            .or_else(|| item.get("filename"))
            .or_else(|| item.get("path"))
            .and_then(|v| v.as_str())?
            .to_string();
        let diff = item
            .get("diff")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let (a, d) = count_diff_lines(&diff);
        let additions = item
            .get("additions")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32)
            .unwrap_or(a);
        let deletions = item
            .get("deletions")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32)
            .unwrap_or(d);
        out.push(FileChange {
            new_path,
            diff,
            additions,
            deletions,
        });
    }
    Some(out)
}

async fn fetch_platform_changes(
    event: &NormalizedEvent,
    creds: &PlatformCredentials,
) -> Result<Vec<FileChange>, String> {
    match (event.platform, event.event) {
        (PlatformKind::Gitlab, EventKind::MergeRequest) => {
            fetch_gitlab_mr_changes(event, creds).await
        }
        (PlatformKind::Gitlab, EventKind::Push) => fetch_gitlab_push_changes(event, creds).await,
        (PlatformKind::Github, EventKind::PullRequest) => {
            fetch_github_pr_changes(event, creds).await
        }
        (PlatformKind::Github, EventKind::Push) => fetch_github_push_changes(event, creds).await,
        (PlatformKind::Gitea, EventKind::PullRequest) => fetch_gitea_pr_changes(event, creds).await,
        (PlatformKind::Gitea, EventKind::Push) => fetch_gitea_push_changes(event, creds).await,
        _ => Ok(Vec::new()),
    }
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .map_err(|e| e.to_string())
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
        .or_else(|| {
            let owner = raw
                .pointer("/repository/owner/login")
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

fn file_change_from_patch_item(item: &Value) -> Option<FileChange> {
    if item
        .get("status")
        .and_then(|v| v.as_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("removed") || s.eq_ignore_ascii_case("deleted"))
    {
        return None;
    }
    let new_path = item
        .get("filename")
        .or_else(|| item.get("new_path"))
        .or_else(|| item.get("path"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())?
        .to_string();
    let diff = item
        .get("patch")
        .or_else(|| item.get("diff"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let (a, d) = count_diff_lines(&diff);
    let additions = item
        .get("additions")
        .and_then(|v| v.as_i64())
        .map(|v| v as i32)
        .unwrap_or(a);
    let deletions = item
        .get("deletions")
        .and_then(|v| v.as_i64())
        .map(|v| v as i32)
        .unwrap_or(d);
    Some(FileChange {
        new_path,
        diff,
        additions,
        deletions,
    })
}

fn parse_files_array(arr: &[Value]) -> Vec<FileChange> {
    arr.iter().filter_map(file_change_from_patch_item).collect()
}

async fn fetch_json(url: &str, platform: PlatformKind, token: &str) -> Result<Value, String> {
    let client = http_client()?;
    let mut req = client.get(url);
    req = match platform {
        PlatformKind::Gitlab => req.header("PRIVATE-TOKEN", token),
        PlatformKind::Github | PlatformKind::Gitea => req
            .header("Authorization", format!("token {token}"))
            .header("Accept", "application/json"),
        PlatformKind::Unknown => req,
    };
    let resp = req.send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("changes HTTP {} for {url}", resp.status()));
    }
    resp.json().await.map_err(|e| e.to_string())
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

fn is_null_sha(sha: &str) -> bool {
    !sha.is_empty() && sha.chars().all(|c| c == '0')
}

fn parse_gitlab_diff_items(arr: &[Value]) -> Vec<FileChange> {
    let mut out = Vec::new();
    for item in arr {
        if item
            .get("deleted_file")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            continue;
        }
        let new_path = item
            .get("new_path")
            .or_else(|| item.get("new_file"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if new_path.is_empty() {
            continue;
        }
        let diff = item
            .get("diff")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let (a, d) = count_diff_lines(&diff);
        out.push(FileChange {
            new_path,
            diff,
            additions: a,
            deletions: d,
        });
    }
    out
}

async fn fetch_gitlab_mr_changes(
    event: &NormalizedEvent,
    creds: &PlatformCredentials,
) -> Result<Vec<FileChange>, String> {
    let Some(project_id) = gitlab_project_id(&event.raw) else {
        return Ok(Vec::new());
    };
    let iid = event
        .raw
        .pointer("/object_attributes/iid")
        .and_then(|v| v.as_i64());
    let Some(iid) = iid else {
        return Ok(Vec::new());
    };

    let base = creds.base_url.trim_end_matches('/');
    let url = format!("{base}/api/v4/projects/{project_id}/merge_requests/{iid}/changes");
    let body = fetch_json(&url, PlatformKind::Gitlab, &creds.token).await?;
    let arr = body
        .get("changes")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(parse_gitlab_diff_items(&arr))
}

async fn fetch_gitlab_push_changes(
    event: &NormalizedEvent,
    creds: &PlatformCredentials,
) -> Result<Vec<FileChange>, String> {
    let Some(project_id) = gitlab_project_id(&event.raw) else {
        return Ok(Vec::new());
    };
    let before = event
        .raw
        .get("before")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let after = event
        .raw
        .get("after")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if before.is_empty() || after.is_empty() {
        return Ok(Vec::new());
    }
    if is_null_sha(after) {
        // ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¥ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¥ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¦ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ?        return Ok(Vec::new());
    }

    let base = creds.base_url.trim_end_matches('/');
    if is_null_sha(before) {
        let url = format!("{base}/api/v4/projects/{project_id}/repository/commits/{after}/diff");
        let body = fetch_json(&url, PlatformKind::Gitlab, &creds.token).await?;
        let arr = body.as_array().cloned().unwrap_or_default();
        return Ok(parse_gitlab_diff_items(&arr));
    }

    let url =
        format!("{base}/api/v4/projects/{project_id}/repository/compare?from={before}&to={after}");
    let body = fetch_json(&url, PlatformKind::Gitlab, &creds.token).await?;
    let arr = body
        .get("diffs")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(parse_gitlab_diff_items(&arr))
}

async fn fetch_github_pr_changes(
    event: &NormalizedEvent,
    creds: &PlatformCredentials,
) -> Result<Vec<FileChange>, String> {
    let Some(repo) = repo_full_name(&event.raw) else {
        return Ok(Vec::new());
    };
    let Some(number) = pr_number(&event.raw) else {
        return Ok(Vec::new());
    };
    let root = github_api_root(creds);
    let url = format!("{root}/repos/{repo}/pulls/{number}/files");
    let body = fetch_json(&url, PlatformKind::Github, &creds.token).await?;
    let arr = body.as_array().cloned().unwrap_or_default();
    Ok(parse_files_array(&arr))
}

async fn fetch_github_push_changes(
    event: &NormalizedEvent,
    creds: &PlatformCredentials,
) -> Result<Vec<FileChange>, String> {
    if event
        .raw
        .get("deleted")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return Ok(Vec::new());
    }
    let Some(repo) = repo_full_name(&event.raw) else {
        return Ok(Vec::new());
    };
    let before = event
        .raw
        .get("before")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let after = event
        .raw
        .get("after")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if before.is_empty() || after.is_empty() || after.chars().all(|c| c == '0') {
        return Ok(Vec::new());
    }
    let root = github_api_root(creds);
    let url = format!("{root}/repos/{repo}/compare/{before}...{after}");
    let body = fetch_json(&url, PlatformKind::Github, &creds.token).await?;
    let arr = body
        .get("files")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(parse_files_array(&arr))
}

async fn fetch_gitea_pr_changes(
    event: &NormalizedEvent,
    creds: &PlatformCredentials,
) -> Result<Vec<FileChange>, String> {
    let Some(repo) = repo_full_name(&event.raw) else {
        return Ok(Vec::new());
    };
    let Some(number) = pr_number(&event.raw) else {
        return Ok(Vec::new());
    };
    let base = creds.base_url.trim_end_matches('/');
    let url = format!("{base}/api/v1/repos/{repo}/pulls/{number}/files");
    let body = fetch_json(&url, PlatformKind::Gitea, &creds.token).await?;
    let arr = body.as_array().cloned().unwrap_or_default();
    Ok(parse_files_array(&arr))
}

async fn fetch_gitea_push_changes(
    event: &NormalizedEvent,
    creds: &PlatformCredentials,
) -> Result<Vec<FileChange>, String> {
    if event
        .raw
        .get("deleted")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return Ok(Vec::new());
    }
    let Some(repo) = repo_full_name(&event.raw) else {
        return Ok(Vec::new());
    };
    let before = event
        .raw
        .get("before")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let after = event
        .raw
        .get("after")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if before.is_empty() || after.is_empty() || after.chars().all(|c| c == '0') {
        return Ok(Vec::new());
    }
    let base = creds.base_url.trim_end_matches('/');
    let url = format!("{base}/api/v1/repos/{repo}/compare/{before}...{after}");
    let body = fetch_json(&url, PlatformKind::Gitea, &creds.token).await?;
    if let Some(files) = body.get("files").and_then(|v| v.as_array()) {
        return Ok(parse_files_array(files));
    }
    Ok(Vec::new())
}

fn filter_changes(items: Vec<FileChange>) -> Vec<FileChange> {
    let raw = env::var("SUPPORTED_EXTENSIONS").unwrap_or_default();
    let exts: Vec<&str> = raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if exts.is_empty() {
        return items;
    }
    items
        .into_iter()
        .filter(|c| exts.iter().any(|e| c.new_path.ends_with(e)))
        .collect()
}

fn commits_text_from_event(event: &NormalizedEvent) -> String {
    if let Some(arr) = event.raw.get("commits").and_then(|v| v.as_array()) {
        return arr
            .iter()
            .filter_map(|c| c.get("message").and_then(|m| m.as_str()))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(";");
    }
    if let Some(msg) = event
        .raw
        .pointer("/object_attributes/last_commit/message")
        .and_then(|v| v.as_str())
    {
        return msg.trim().to_string();
    }
    if let Some(title) = event
        .raw
        .pointer("/pull_request/title")
        .and_then(|v| v.as_str())
    {
        return title.trim().to_string();
    }
    String::new()
}

fn resolve_mr_review_commit_id(action: &str, raw: &Value) -> String {
    let last = raw
        .pointer("/object_attributes/last_commit/id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if action == "merge" {
        if let Some(sha) = raw
            .pointer("/object_attributes/merge_commit_sha")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            return sha.to_string();
        }
        if !last.is_empty() {
            return format!("{last}:merge");
        }
        return "merge".into();
    }
    last
}

fn sum_lines(changes: &[FileChange]) -> (i32, i32) {
    let a = changes.iter().map(|c| c.additions).sum();
    let d = changes.iter().map(|c| c.deletions).sum();
    (a, d)
}

fn is_draft_mr(raw: &Value) -> bool {
    raw.pointer("/object_attributes/draft")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
        || raw
            .pointer("/object_attributes/work_in_progress")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
}

fn is_draft_pr(raw: &Value) -> bool {
    raw.pointer("/pull_request/draft")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
        || raw
            .pointer("/pull_request/work_in_progress")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
}

fn target_branch_allowed(branch: &str) -> bool {
    let raw = env::var("MERGE_REVIEW_TARGET_BRANCHES").unwrap_or_default();
    let list: Vec<&str> = raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if list.is_empty() {
        return true;
    }
    list.iter().any(|pat| fnmatch(pat, branch))
}

fn only_protected_enabled() -> bool {
    env::var("MERGE_REVIEW_ONLY_PROTECTED_BRANCHES_ENABLED").unwrap_or_else(|_| "0".into()) == "1"
}

/// Simple shell-style glob (fnmatch `*` / `?`).
fn fnmatch(pattern: &str, text: &str) -> bool {
    fnmatch_bytes(pattern.as_bytes(), text.as_bytes())
}

fn fnmatch_bytes(pat: &[u8], text: &[u8]) -> bool {
    let (mut i, mut j) = (0usize, 0usize);
    let mut star_i = None;
    let mut star_j = 0usize;
    while j < text.len() {
        if i < pat.len() && (pat[i] == b'?' || pat[i] == text[j]) {
            i += 1;
            j += 1;
        } else if i < pat.len() && pat[i] == b'*' {
            star_i = Some(i);
            star_j = j;
            i += 1;
        } else if let Some(si) = star_i {
            i = si + 1;
            star_j += 1;
            j = star_j;
        } else {
            return false;
        }
    }
    while i < pat.len() && pat[i] == b'*' {
        i += 1;
    }
    i == pat.len()
}

async fn target_branch_protected(
    event: &NormalizedEvent,
    creds: &PlatformCredentials,
    target_branch: &str,
) -> bool {
    match event.platform {
        PlatformKind::Gitlab => gitlab_target_protected(event, creds, target_branch).await,
        PlatformKind::Github => github_target_protected(event, creds, target_branch).await,
        PlatformKind::Gitea => gitea_target_protected(event, creds, target_branch).await,
        PlatformKind::Unknown => false,
    }
}

async fn gitlab_target_protected(
    event: &NormalizedEvent,
    creds: &PlatformCredentials,
    target_branch: &str,
) -> bool {
    let Some(project_id) = gitlab_project_id(&event.raw) else {
        return false;
    };
    let base = creds.base_url.trim_end_matches('/');
    let url = format!("{base}/api/v4/projects/{project_id}/protected_branches");
    match fetch_json(&url, PlatformKind::Gitlab, &creds.token).await {
        Ok(body) => {
            let arr = body.as_array().cloned().unwrap_or_default();
            arr.iter().any(|item| {
                item.get("name")
                    .and_then(|v| v.as_str())
                    .is_some_and(|name| fnmatch(name, target_branch))
            })
        }
        Err(e) => {
            warn!(error = %e, "gitlab protected branches fetch failed");
            false
        }
    }
}

async fn github_target_protected(
    event: &NormalizedEvent,
    creds: &PlatformCredentials,
    target_branch: &str,
) -> bool {
    let Some(repo) = repo_full_name(&event.raw) else {
        return false;
    };
    let root = github_api_root(creds);
    let url = format!("{root}/repos/{repo}/branches?protected=true");
    match fetch_json(&url, PlatformKind::Github, &creds.token).await {
        Ok(body) => {
            let arr = body.as_array().cloned().unwrap_or_default();
            arr.iter().any(|item| {
                item.get("name")
                    .and_then(|v| v.as_str())
                    .is_some_and(|name| fnmatch(name, target_branch))
            })
        }
        Err(e) => {
            warn!(error = %e, "github protected branches fetch failed");
            false
        }
    }
}

async fn gitea_target_protected(
    event: &NormalizedEvent,
    creds: &PlatformCredentials,
    target_branch: &str,
) -> bool {
    let Some(repo) = repo_full_name(&event.raw) else {
        return false;
    };
    let base = creds.base_url.trim_end_matches('/');
    let url = format!("{base}/api/v1/repos/{repo}/branches?protected=true");
    let body = match fetch_json(&url, PlatformKind::Gitea, &creds.token).await {
        Ok(b) => b,
        Err(e) => {
            warn!(error = %e, "gitea protected branches fetch failed");
            return false;
        }
    };
    let arr = body.as_array().cloned().unwrap_or_default();
    arr.iter().any(|item| {
        item.get("name")
            .and_then(|v| v.as_str())
            .is_some_and(|name| fnmatch(name, target_branch))
    })
}

#[cfg(test)]
#[allow(clippy::await_holding_lock)]
mod tests {
    use super::*;
    use crate::dashboard::ReviewKind;
    use serde_json::json;
    use std::sync::Mutex;
    use xiezhi_platforms::{
        EventKind, MemoryCommentPoster, NormalizedEvent, PlanningCommentPoster,
        PlatformCredentials, PlatformKind,
    };

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn creds() -> PlatformCredentials {
        PlatformCredentials {
            token: "t".into(),
            base_url: "https://gitlab.example/".into(),
            url_slug: "gitlab_example".into(),
        }
    }

    fn mem_ctx() -> WorkerContext {
        WorkerContext {
            store: Arc::new(MemoryReviewStore::default()),
            changes: None,
            llm: None,
            protected_override: None,
        }
    }

    #[test]
    fn fnmatch_basic() {
        assert!(fnmatch("main", "main"));
        assert!(fnmatch("release/*", "release/1.0"));
        assert!(!fnmatch("main", "master"));
        assert!(fnmatch("feat?", "feat1"));
    }

    #[tokio::test]
    async fn mr_skips_when_not_protected() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::remove_var("MERGE_REVIEW_TARGET_BRANCHES");
        env::set_var("MERGE_REVIEW_ONLY_PROTECTED_BRANCHES_ENABLED", "1");
        let poster = MemoryCommentPoster::new();
        let event = NormalizedEvent {
            platform: PlatformKind::Gitlab,
            event: EventKind::MergeRequest,
            project_name: "g/p".into(),
            author: "alice".into(),
            url_slug: "g-p".into(),
            raw: json!({
                "object_attributes": {
                    "action": "open",
                    "source_branch": "feat",
                    "target_branch": "dev",
                    "last_commit": {"id": "x"}
                }
            }),
        };
        handle_event_with(
            &event,
            &creds(),
            &poster,
            WorkerContext {
                store: Arc::new(MemoryReviewStore::default()),
                changes: Some(vec![]),
                llm: None,
                protected_override: Some(false),
            },
        )
        .await
        .unwrap();
        assert!(poster.snapshot().is_empty());
        env::remove_var("MERGE_REVIEW_ONLY_PROTECTED_BRANCHES_ENABLED");
    }

    #[tokio::test]
    async fn gitlab_protected_branches_api() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 2048];
            let n = sock.read(&mut buf).await.unwrap();
            let req = String::from_utf8_lossy(&buf[..n]).to_ascii_lowercase();
            assert!(
                req.contains("get /api/v4/projects/1/protected_branches"),
                "{req}"
            );
            let body = r#"[{"name":"main"},{"name":"release/*"}]"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
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
            raw: json!({"project":{"id":1},"object_attributes":{"target_branch":"main"}}),
        };
        let creds = PlatformCredentials {
            token: "t".into(),
            base_url: format!("http://{addr}/"),
            url_slug: "local".into(),
        };
        assert!(target_branch_protected(&event, &creds, "main").await);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn draft_mr_skips_post() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::remove_var("MERGE_REVIEW_TARGET_BRANCHES");
        let poster = MemoryCommentPoster::new();
        let event = NormalizedEvent {
            platform: PlatformKind::Gitlab,
            event: EventKind::MergeRequest,
            project_name: "g/p".into(),
            author: "alice".into(),
            url_slug: "g-p".into(),
            raw: json!({"object_attributes": {"draft": true, "action": "open"}}),
        };
        handle_event_with(&event, &creds(), &poster, mem_ctx())
            .await
            .unwrap();
        assert!(poster.snapshot().is_empty());
    }

    #[tokio::test]
    async fn mr_posts_and_persists_empty_changes() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::remove_var("MERGE_REVIEW_TARGET_BRANCHES");
        let poster = MemoryCommentPoster::new();
        let store = Arc::new(MemoryReviewStore::default());
        let event = NormalizedEvent {
            platform: PlatformKind::Gitlab,
            event: EventKind::MergeRequest,
            project_name: "g/p".into(),
            author: "alice".into(),
            url_slug: "g-p".into(),
            raw: json!({
                "object_attributes": {
                    "draft": false,
                    "action": "open",
                    "source_branch": "feat",
                    "target_branch": "main",
                    "url": "https://gitlab.example/g/p/-/merge_requests/1",
                    "last_commit": {"id": "abc", "message": "init"}
                }
            }),
        };
        handle_event_with(
            &event,
            &creds(),
            &poster,
            WorkerContext {
                store: store.clone(),
                changes: Some(vec![]),
                llm: None,
                protected_override: None,
            },
        )
        .await
        .unwrap();
        let snap = poster.snapshot();
        assert_eq!(snap.len(), 1);
        assert!(snap[0].body.contains("No reviewed files changed"));
        let logs = store.query(ReviewKind::Mr, None, None, None, None);
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].score, Some(0));
        assert_eq!(logs[0].last_commit_id.as_deref(), Some("abc"));
    }

    #[tokio::test]
    async fn mr_fusion_with_scripted_llm_writes_db() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::remove_var("MERGE_REVIEW_TARGET_BRANCHES");
        env::set_var("REVIEW_NARRATIVE_ENABLED", "0");
        env::set_var("SCR_REVIEW_FILTER_ENABLED", "0");
        let poster = MemoryCommentPoster::new();
        let store = Arc::new(MemoryReviewStore::default());
        let client = Arc::new(ScriptedClient::new(
            "scripted",
            [r#"{"comments":[]}"#.into()],
        )) as Arc<dyn LlmClient>;
        let changes = vec![FileChange {
            new_path: "a.py".into(),
            diff: "@@ -0,0 +1 @@\n+print(1)\n".into(),
            additions: 1,
            deletions: 0,
        }];
        let event = NormalizedEvent {
            platform: PlatformKind::Gitlab,
            event: EventKind::MergeRequest,
            project_name: "g/p".into(),
            author: "alice".into(),
            url_slug: "g-p".into(),
            raw: json!({
                "object_attributes": {
                    "action": "open",
                    "source_branch": "feat",
                    "target_branch": "main",
                    "last_commit": {"id": "def"}
                }
            }),
        };
        handle_event_with(
            &event,
            &creds(),
            &poster,
            WorkerContext {
                store: store.clone(),
                changes: Some(changes),
                llm: Some(client),
                protected_override: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(poster.snapshot().len(), 1);
        let logs = store.query(ReviewKind::Mr, None, None, None, None);
        assert_eq!(logs.len(), 1);
        assert!(logs[0].review_result.as_ref().unwrap().len() > 5);
        assert_eq!(logs[0].additions, Some(1));
        env::remove_var("REVIEW_NARRATIVE_ENABLED");
        env::remove_var("SCR_REVIEW_FILTER_ENABLED");
    }

    #[tokio::test]
    async fn sqlite_insert_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "xiezhi-worker-db-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("data.db");
        let store = SqliteReviewStore::open_or_create(&path).unwrap();
        store
            .insert_mr(ReviewLog {
                project_name: Some("g/p".into()),
                author: Some("a".into()),
                source_branch: Some("f".into()),
                target_branch: Some("m".into()),
                updated_at: Some(1),
                commit_messages: Some("c".into()),
                score: Some(88),
                url: Some("u".into()),
                review_result: Some("ok".into()),
                additions: Some(2),
                deletions: Some(1),
                last_commit_id: Some("sha".into()),
                ..Default::default()
            })
            .unwrap();
        let rows = store.query(ReviewKind::Mr, None, None, None, None);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].score, Some(88));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn mr_skips_duplicate_last_commit() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::remove_var("MERGE_REVIEW_TARGET_BRANCHES");
        let poster = MemoryCommentPoster::new();
        let store = Arc::new(MemoryReviewStore::default());
        store
            .insert_mr(ReviewLog {
                project_name: Some("g/p".into()),
                author: Some("alice".into()),
                source_branch: Some("feat".into()),
                target_branch: Some("main".into()),
                last_commit_id: Some("abc".into()),
                score: Some(90),
                review_result: Some("done".into()),
                updated_at: Some(1),
                ..Default::default()
            })
            .unwrap();
        let event = NormalizedEvent {
            platform: PlatformKind::Gitlab,
            event: EventKind::MergeRequest,
            project_name: "g/p".into(),
            author: "alice".into(),
            url_slug: "g-p".into(),
            raw: json!({
                "object_attributes": {
                    "action": "update",
                    "source_branch": "feat",
                    "target_branch": "main",
                    "last_commit": {"id": "abc"}
                }
            }),
        };
        handle_event_with(
            &event,
            &creds(),
            &poster,
            WorkerContext {
                store: store.clone(),
                changes: Some(vec![]),
                llm: None,
                protected_override: None,
            },
        )
        .await
        .unwrap();
        assert!(poster.snapshot().is_empty());
        assert_eq!(store.query(ReviewKind::Mr, None, None, None, None).len(), 1);
    }

    #[tokio::test]
    async fn sqlite_mr_last_commit_exists() {
        let dir = std::env::temp_dir().join(format!(
            "xiezhi-dedup-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let store = SqliteReviewStore::open_or_create(dir.join("data.db")).unwrap();
        assert!(!store.mr_last_commit_exists("g/p", "feat", "main", "sha1"));
        store
            .insert_mr(ReviewLog {
                project_name: Some("g/p".into()),
                source_branch: Some("feat".into()),
                target_branch: Some("main".into()),
                last_commit_id: Some("sha1".into()),
                ..Default::default()
            })
            .unwrap();
        assert!(store.mr_last_commit_exists("g/p", "feat", "main", "sha1"));
        assert!(!store.mr_last_commit_exists("g/p", "feat", "main", "sha2"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn mr_ignored_action_skips() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::remove_var("MERGE_REVIEW_TARGET_BRANCHES");
        let poster = MemoryCommentPoster::new();
        let event = NormalizedEvent {
            platform: PlatformKind::Gitlab,
            event: EventKind::MergeRequest,
            project_name: "g/p".into(),
            author: "alice".into(),
            url_slug: "g-p".into(),
            raw: json!({"object_attributes": {"action": "close"}}),
        };
        handle_event_with(&event, &creds(), &poster, mem_ctx())
            .await
            .unwrap();
        assert!(poster.snapshot().is_empty());
    }

    #[tokio::test]
    async fn mr_target_branch_filter() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::set_var("MERGE_REVIEW_TARGET_BRANCHES", "main,master");
        let poster = MemoryCommentPoster::new();
        let event = NormalizedEvent {
            platform: PlatformKind::Gitlab,
            event: EventKind::MergeRequest,
            project_name: "g/p".into(),
            author: "alice".into(),
            url_slug: "g-p".into(),
            raw: json!({"object_attributes": {"action": "open", "target_branch": "feature"}}),
        };
        handle_event_with(&event, &creds(), &poster, mem_ctx())
            .await
            .unwrap();
        assert!(poster.snapshot().is_empty());
        env::remove_var("MERGE_REVIEW_TARGET_BRANCHES");
    }

    #[tokio::test]
    async fn pr_plans_github_url() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::remove_var("MERGE_REVIEW_TARGET_BRANCHES");
        let poster = PlanningCommentPoster::new();
        let event = NormalizedEvent {
            platform: PlatformKind::Github,
            event: EventKind::PullRequest,
            project_name: "o/r".into(),
            author: "bob".into(),
            url_slug: "o-r".into(),
            raw: json!({
                "action": "opened",
                "repository": {"full_name": "o/r"},
                "pull_request": {"number": 3, "draft": false}
            }),
        };
        let creds = PlatformCredentials {
            token: "t".into(),
            base_url: "https://github.com".into(),
            url_slug: "github_com".into(),
        };
        handle_event_with(
            &event,
            &creds,
            &poster,
            WorkerContext {
                store: Arc::new(MemoryReviewStore::default()),
                changes: Some(vec![]),
                llm: None,
                protected_override: None,
            },
        )
        .await
        .unwrap();
        let snap = poster.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(
            snap[0].url,
            "https://api.github.com/repos/o/r/issues/3/comments"
        );
    }

    #[tokio::test]
    async fn fetch_gitlab_push_compare() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 2048];
            let n = sock.read(&mut buf).await.unwrap();
            let req = String::from_utf8_lossy(&buf[..n]).to_ascii_lowercase();
            assert!(
                req.contains("get /api/v4/projects/42/repository/compare?from=aaa&to=bbb"),
                "{req}"
            );
            assert!(req.contains("private-token: tok"), "{req}");
            let body = r#"{"diffs":[{"new_path":"a.py","diff":"@@ -0,0 +1 @@\n+x\n","deleted_file":false}]}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            sock.write_all(resp.as_bytes()).await.unwrap();
        });

        let event = NormalizedEvent {
            platform: PlatformKind::Gitlab,
            event: EventKind::Push,
            project_name: "g/p".into(),
            author: "alice".into(),
            url_slug: "g-p".into(),
            raw: json!({
                "before": "aaa",
                "after": "bbb",
                "project": {"id": 42, "path_with_namespace": "g/p"}
            }),
        };
        let creds = PlatformCredentials {
            token: "tok".into(),
            base_url: format!("http://{addr}/"),
            url_slug: "local".into(),
        };
        let changes = fetch_gitlab_push_changes(&event, &creds).await.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].new_path, "a.py");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn fetch_gitlab_push_new_branch_uses_commit_diff() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 2048];
            let n = sock.read(&mut buf).await.unwrap();
            let req = String::from_utf8_lossy(&buf[..n]).to_ascii_lowercase();
            assert!(
                req.contains("get /api/v4/projects/7/repository/commits/deadbeef/diff"),
                "{req}"
            );
            let body = r#"[{"new_path":"n.py","diff":"+n\n"}]"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            sock.write_all(resp.as_bytes()).await.unwrap();
        });

        let event = NormalizedEvent {
            platform: PlatformKind::Gitlab,
            event: EventKind::Push,
            project_name: "g/p".into(),
            author: "alice".into(),
            url_slug: "g-p".into(),
            raw: json!({
                "before": "0000000000000000000000000000000000000000",
                "after": "deadbeef",
                "project": {"id": 7}
            }),
        };
        let creds = PlatformCredentials {
            token: "tok".into(),
            base_url: format!("http://{addr}/"),
            url_slug: "local".into(),
        };
        let changes = fetch_gitlab_push_changes(&event, &creds).await.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].new_path, "n.py");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn fetch_github_pr_files_from_api() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 2048];
            let n = sock.read(&mut buf).await.unwrap();
            let req = String::from_utf8_lossy(&buf[..n]).to_ascii_lowercase();
            assert!(req.contains("get /repos/o/r/pulls/9/files"), "{req}");
            assert!(req.contains("authorization: token gh"), "{req}");
            let body = r#"[{"filename":"a.py","patch":"@@ -0,0 +1 @@\n+x\n","additions":1,"deletions":0,"status":"added"}]"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            sock.write_all(resp.as_bytes()).await.unwrap();
        });

        let event = NormalizedEvent {
            platform: PlatformKind::Github,
            event: EventKind::PullRequest,
            project_name: "o/r".into(),
            author: "bob".into(),
            url_slug: "o-r".into(),
            raw: json!({
                "action": "opened",
                "repository": {"full_name": "o/r"},
                "pull_request": {"number": 9, "draft": false, "head": {"sha": "s1", "ref": "f"}, "base": {"ref": "main"}}
            }),
        };
        let creds = PlatformCredentials {
            token: "gh".into(),
            base_url: format!("http://{addr}/"),
            url_slug: "local".into(),
        };
        let changes = fetch_github_pr_changes(&event, &creds).await.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].new_path, "a.py");
        assert_eq!(changes[0].additions, 1);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn fetch_gitea_pr_files_from_api() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 2048];
            let n = sock.read(&mut buf).await.unwrap();
            let req = String::from_utf8_lossy(&buf[..n]).to_ascii_lowercase();
            assert!(req.contains("get /api/v1/repos/o/r/pulls/2/files"), "{req}");
            let body = r#"[{"filename":"b.rs","diff":"@@ -0,0 +1 @@\n+fn main(){}\n","additions":1,"deletions":0}]"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            sock.write_all(resp.as_bytes()).await.unwrap();
        });

        let event = NormalizedEvent {
            platform: PlatformKind::Gitea,
            event: EventKind::PullRequest,
            project_name: "o/r".into(),
            author: "bob".into(),
            url_slug: "o-r".into(),
            raw: json!({
                "action": "opened",
                "repository": {"full_name": "o/r"},
                "pull_request": {"number": 2, "draft": false}
            }),
        };
        let creds = PlatformCredentials {
            token: "gt".into(),
            base_url: format!("http://{addr}/"),
            url_slug: "local".into(),
        };
        let changes = fetch_gitea_pr_changes(&event, &creds).await.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].new_path, "b.rs");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn fetch_github_push_compare() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 2048];
            let n = sock.read(&mut buf).await.unwrap();
            let req = String::from_utf8_lossy(&buf[..n]).to_ascii_lowercase();
            assert!(req.contains("get /repos/o/r/compare/aaa...bbb"), "{req}");
            let body = r#"{"files":[{"filename":"c.py","patch":"+y\n","status":"modified","additions":1,"deletions":0}]}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            sock.write_all(resp.as_bytes()).await.unwrap();
        });

        let event = NormalizedEvent {
            platform: PlatformKind::Github,
            event: EventKind::Push,
            project_name: "o/r".into(),
            author: "bob".into(),
            url_slug: "o-r".into(),
            raw: json!({
                "before": "aaa",
                "after": "bbb",
                "repository": {"full_name": "o/r"}
            }),
        };
        let creds = PlatformCredentials {
            token: "gh".into(),
            base_url: format!("http://{addr}/"),
            url_slug: "local".into(),
        };
        let changes = fetch_github_push_changes(&event, &creds).await.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].new_path, "c.py");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn push_respects_flag() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::remove_var("PUSH_REVIEW_ENABLED");
        let poster = MemoryCommentPoster::new();
        let event = NormalizedEvent {
            platform: PlatformKind::Github,
            event: EventKind::Push,
            project_name: "o/r".into(),
            author: "bob".into(),
            url_slug: "o-r".into(),
            raw: json!({}),
        };
        let creds = PlatformCredentials {
            token: "t".into(),
            base_url: "https://github.com".into(),
            url_slug: "github_com".into(),
        };
        let ctx = WorkerContext {
            store: Arc::new(MemoryReviewStore::default()),
            changes: Some(vec![]),
            llm: None,
            protected_override: None,
        };
        handle_event_with(
            &event,
            &creds,
            &poster,
            WorkerContext {
                store: ctx.store.clone(),
                changes: Some(vec![]),
                llm: None,
                protected_override: None,
            },
        )
        .await
        .unwrap();
        assert!(poster.snapshot().is_empty());

        env::set_var("PUSH_REVIEW_ENABLED", "1");
        handle_event_with(
            &event,
            &creds,
            &poster,
            WorkerContext {
                store: Arc::new(MemoryReviewStore::default()),
                changes: Some(vec![]),
                llm: None,
                protected_override: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(poster.snapshot().len(), 1);
        env::remove_var("PUSH_REVIEW_ENABLED");
    }
}
