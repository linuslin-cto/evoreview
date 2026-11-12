//! Agentic 叙述入口（对齐 `AgenticReviewer.review`）。

use crate::agent::adapter::ToolCallingAdapter;
use crate::agent::prompts::AgenticPrompts;
use crate::agent::registry::ToolRegistry;
use crate::agent::repo_syncer::{LocalRepoSyncer, RepoSyncSpec};
use crate::agent::runner::AgentRunner;
use crate::narrative::{
    changes_to_diffs_text, generate_narrative, parse_review_score, strip_markdown_fence,
    NarrativePrompts,
};
use std::path::PathBuf;
use tracing::{info, warn};
use xiezhi_llm::{ChatMessage, LlmClient};
use xiezhi_scr::FileChange;

#[derive(Debug, Clone)]
pub struct AgenticOptions {
    pub prompt_templates: PathBuf,
    pub style: String,
    pub repo_root: PathBuf,
    pub max_iterations: usize,
    /// 若设置，先 LocalRepoSyncer 再在缓存目录上跑 agent。
    pub sync: Option<RepoSyncSpec>,
}

impl AgenticOptions {
    pub fn new(
        prompt_templates: impl Into<PathBuf>,
        style: impl Into<String>,
        repo_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            prompt_templates: prompt_templates.into(),
            style: style.into(),
            repo_root: repo_root.into(),
            max_iterations: std::env::var("AGENT_MAX_ITERATIONS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(20),
            sync: None,
        }
    }

    pub fn with_sync(mut self, sync: RepoSyncSpec) -> Self {
        self.sync = Some(sync);
        self
    }
}

fn looks_like_review(text: &str) -> bool {
    parse_review_score(text) > 0 || text.contains("总分")
}

/// 运行 agentic 叙述；失败时降级为 diff_only `generate_narrative`。
pub async fn generate_agentic_narrative(
    client: &dyn LlmClient,
    changes: &[FileChange],
    commits_text: &str,
    opts: &AgenticOptions,
) -> String {
    let diffs_text = changes_to_diffs_text(changes);
    match run_agentic(client, &diffs_text, commits_text, opts).await {
        Ok(text) if looks_like_review(&text) => {
            info!(len = text.len(), "agentic narrative ok");
            strip_markdown_fence(&text)
        }
        Ok(text) => {
            warn!(
                len = text.len(),
                "agentic output missing 总分 marker; falling back to diff_only"
            );
            fallback_diff_only(client, changes, commits_text, opts).await
        }
        Err(e) => {
            warn!("agentic run failed, degrading to diff_only: {e}");
            fallback_diff_only(client, changes, commits_text, opts).await
        }
    }
}

async fn run_agentic(
    client: &dyn LlmClient,
    diffs_text: &str,
    commits_text: &str,
    opts: &AgenticOptions,
) -> Result<String, String> {
    let root = resolve_repo_root(opts).await?;
    if !root.exists() {
        return Err(format!("repo_root missing: {}", root.display()));
    }

    let prompts =
        AgenticPrompts::load(&opts.prompt_templates, &opts.style).map_err(|e| e.to_string())?;
    let repo_root = root
        .canonicalize()
        .unwrap_or_else(|_| root.clone())
        .display()
        .to_string();
    let (system, user) = prompts.render(&repo_root, diffs_text, commits_text);

    let registry = ToolRegistry::with_defaults(&root);
    let adapter = ToolCallingAdapter::new(client);
    let mut runner = AgentRunner::new(adapter, &registry);
    runner.max_iterations = opts.max_iterations;

    let (text, iterations) = runner
        .run(vec![ChatMessage::system(system), ChatMessage::user(user)])
        .await
        .map_err(|e| e.to_string())?;
    info!(iterations, "agentic finished");
    Ok(text)
}

async fn resolve_repo_root(opts: &AgenticOptions) -> Result<PathBuf, String> {
    if let Some(spec) = &opts.sync {
        let url = spec.url.clone();
        let key = spec.key.clone();
        let git_ref = spec.git_ref.clone();
        let cache = spec.cache_root.clone();
        tokio::task::spawn_blocking(move || {
            LocalRepoSyncer::new(cache).sync_to(&url, &key, &git_ref)
        })
        .await
        .map_err(|e| format!("sync join: {e}"))?
        .map_err(|e| {
            warn!(error = %e, "agentic repo sync failed");
            e.to_string()
        })
    } else {
        Ok(opts.repo_root.clone())
    }
}

async fn fallback_diff_only(
    client: &dyn LlmClient,
    changes: &[FileChange],
    commits_text: &str,
    opts: &AgenticOptions,
) -> String {
    match NarrativePrompts::load(&opts.prompt_templates, &opts.style) {
        Ok(prompts) => match generate_narrative(client, &prompts, changes, commits_text).await {
            Ok(t) => t,
            Err(e) => {
                warn!("diff_only fallback also failed: {e}");
                String::new()
            }
        },
        Err(e) => {
            warn!("diff_only prompt load failed: {e}");
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xiezhi_llm::ScriptedClient;

    #[tokio::test]
    async fn agentic_uses_tools_then_scores() {
        let dir = std::env::temp_dir().join(format!(
            "xiezhi-agentic-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("scr/prompts")).unwrap();
        std::fs::write(dir.join("main.py"), "x=1\n").unwrap();
        let yml = r#"
agentic_xiezhi_review_prompt:
  system_prompt: |
    sys {repo_root} {{ style }}
  user_prompt: |
    {repo_root}
    {diffs_text}
    {commits_text}
xiezhi_review_prompt:
  system_prompt: |
    narr
  user_prompt: |
    {diffs_text}
"#;
        std::fs::write(dir.join("prompt_templates.yml"), yml).unwrap();

        let client = ScriptedClient::new(
            "s",
            [
                "{\"tool\":\"read_file\",\"args\":{\"path\":\"main.py\"},\"id\":\"1\"}".into(),
                "问题不多\n总分:92分".into(),
            ],
        );
        let opts = AgenticOptions::new(
            dir.join("prompt_templates.yml"),
            "professional",
            dir.clone(),
        );
        let changes = vec![FileChange {
            new_path: "main.py".into(),
            diff: "@@ -0,0 +1 @@\n+x=1\n".into(),
            additions: 1,
            deletions: 0,
        }];
        let text = generate_agentic_narrative(&client, &changes, "init", &opts).await;
        assert!(
            text.contains("总分:92分") || text.contains("问题不多"),
            "{text}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
