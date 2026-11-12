//! SCR 结构化审查编排：提示词 + LLM + 解析/过滤 + 报告。

use crate::compose::compose_report;
use crate::pipeline::ReviewOutcome;
use thiserror::Error;
use tracing::warn;
use xiezhi_core::FileReviewResult;
use xiezhi_llm::{ChatMessage, ChatRequest, LlmClient, LlmError};
use xiezhi_scr::{
    apply_filter_ids, comments_from_llm_payload, format_filter_comment_block, resolve_line_numbers,
    score_from_results, FileChange, RuleResolver, ScrPrompts,
};

#[derive(Debug, Error)]
pub enum StructuredError {
    #[error(transparent)]
    Llm(#[from] LlmError),
    #[error(transparent)]
    Prompt(#[from] xiezhi_scr::PromptError),
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Clone)]
pub struct StructuredOptions {
    pub style: String,
    pub max_files: usize,
    pub filter_enabled: bool,
    pub prompt_templates: String,
    pub conf_dir: String,
    /// 可选：显式 harness（双探针候选轮）；缺省时若启用 Harness² 则注入 library hints。
    pub harness: Option<crate::harness2::ExecutionHarness>,
}

impl Default for StructuredOptions {
    fn default() -> Self {
        Self::from_search_roots(&[])
    }
}

impl StructuredOptions {
    /// 从候选根目录向上查找 `conf/`，并填充 prompt 路径。
    pub fn from_search_roots(roots: &[std::path::PathBuf]) -> Self {
        let conf_dir = std::env::var("XIEZHI_CONF_DIR")
            .ok()
            .filter(|s| !s.is_empty())
            .map(std::path::PathBuf::from)
            .or_else(|| find_conf_dir(roots))
            .unwrap_or_else(|| std::path::PathBuf::from("conf"));

        let prompt_templates = std::env::var("XIEZHI_PROMPT_TEMPLATES")
            .unwrap_or_else(|_| conf_dir.join("prompt_templates.yml").display().to_string());

        Self {
            style: std::env::var("REVIEW_STYLE").unwrap_or_else(|_| "professional".into()),
            max_files: std::env::var("SCR_MAX_FILES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(50),
            filter_enabled: std::env::var("SCR_REVIEW_FILTER_ENABLED")
                .unwrap_or_else(|_| "1".into())
                != "0",
            prompt_templates,
            conf_dir: conf_dir.display().to_string(),
            harness: None,
        }
    }
}

fn find_conf_dir(roots: &[std::path::PathBuf]) -> Option<std::path::PathBuf> {
    let mut candidates = Vec::new();
    for root in roots {
        candidates.push(root.clone());
        if let Ok(c) = root.canonicalize() {
            candidates.push(c);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd);
    }
    for start in candidates {
        let mut p = start;
        loop {
            let conf = p.join("conf");
            if conf.join("prompt_templates.yml").is_file() {
                return Some(conf);
            }
            if !p.pop() {
                break;
            }
        }
    }
    None
}

pub async fn review_file_change(
    client: &dyn LlmClient,
    prompts: &ScrPrompts,
    rules: &RuleResolver,
    change: &FileChange,
    change_files: &str,
    filter_enabled: bool,
) -> FileReviewResult {
    let path = &change.new_path;
    let mut result = FileReviewResult {
        path: path.clone(),
        additions: change.additions,
        deletions: change.deletions,
        ..Default::default()
    };
    if change.diff.trim().is_empty() {
        return result;
    }

    let system_rule = rules.resolve(path);
    let user = prompts.render_user(change_files, path, &change.diff, &system_rule);
    let req = ChatRequest {
        messages: vec![
            ChatMessage::system(prompts.system.clone()),
            ChatMessage::user(user),
        ],
        ..Default::default()
    };

    let raw = match client.chat(req).await {
        Ok(t) => t,
        Err(e) => {
            result.warning = format!("审查调用失败: {e}");
            return result;
        }
    };

    let mut comments = comments_from_llm_payload(path, &raw);
    resolve_line_numbers(&mut comments, &change.diff);

    if filter_enabled && prompts.filter_enabled_content() && !comments.is_empty() {
        let block = format_filter_comment_block(&comments);
        let filter_user = prompts.render_filter_user(path, &change.diff, &block);
        let filter_req = ChatRequest {
            messages: vec![
                ChatMessage::system(prompts.filter_system.clone()),
                ChatMessage::user(filter_user),
            ],
            ..Default::default()
        };
        match client.chat(filter_req).await {
            Ok(filter_raw) => apply_filter_ids(&mut comments, &filter_raw),
            Err(e) => warn!("SCR review filter 调用失败({path})，跳过过滤: {e}"),
        }
    }

    result.comments = comments;
    result
}

pub async fn review_changes_structured(
    client: &dyn LlmClient,
    changes: &[FileChange],
    rules: &RuleResolver,
    opts: &StructuredOptions,
) -> Result<Vec<FileReviewResult>, StructuredError> {
    if changes.is_empty() {
        return Ok(Vec::new());
    }
    let mut prompts = ScrPrompts::load(&opts.prompt_templates, &opts.conf_dir, &opts.style)?;
    if let Some(harness) = &opts.harness {
        prompts.system = crate::harness2::apply_harness_to_system(&prompts.system, harness);
    } else if crate::harness2::enabled_from_env() {
        let lib = crate::harness2::ImprovementLibrary::load_from_env();
        let harness = crate::harness2::ExecutionHarness::base().with_library_hints(&lib);
        prompts.system = crate::harness2::apply_harness_to_system(&prompts.system, &harness);
    }
    let sliced: Vec<&FileChange> = changes.iter().take(opts.max_files).collect();
    let change_files = sliced
        .iter()
        .map(|c| format!("- {}", c.new_path))
        .collect::<Vec<_>>()
        .join("\n");

    let mut results = Vec::with_capacity(sliced.len());
    for change in sliced {
        let r = review_file_change(
            client,
            &prompts,
            rules,
            change,
            &change_files,
            opts.filter_enabled,
        )
        .await;
        results.push(r);
    }
    Ok(results)
}

pub async fn review_changes_to_outcome(
    client: &dyn LlmClient,
    changes: &[FileChange],
    rules: &RuleResolver,
    opts: &StructuredOptions,
) -> Result<ReviewOutcome, StructuredError> {
    if changes.is_empty() {
        return Ok(ReviewOutcome {
            report_text: "关注的文件没有修改".into(),
            score: 0,
            inline_comments: Vec::new(),
        });
    }
    let results = review_changes_structured(client, changes, rules, opts).await?;
    let score = score_from_results(&results);
    let inline_comments = results
        .iter()
        .flat_map(|r| r.comments.iter())
        .filter(|c| !c.filtered && c.start_line > 0)
        .cloned()
        .collect();
    let report_text = compose_report(&results, score, "", false);
    Ok(ReviewOutcome {
        report_text,
        score,
        inline_comments,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use xiezhi_llm::ScriptedClient;

    #[tokio::test]
    async fn scripted_review_produces_report() {
        let dir = std::env::temp_dir().join(format!(
            "xiezhi-struct-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("scr/prompts")).unwrap();
        std::fs::write(
            dir.join("prompt_templates.yml"),
            "scr_review_prompt:\n  system_prompt: |\n    sys\n  user_prompt: |\n    {diff}\n",
        )
        .unwrap();

        let client = ScriptedClient::new(
            "scripted",
            [r#"{"comments":[{"content":"bad","existing_code":"x=1","category":"bug","severity":"high"}]}"#.into()],
        );
        let rules = RuleResolver::new("missing-rules", "missing-allowlist");
        let opts = StructuredOptions {
            style: "professional".into(),
            max_files: 10,
            filter_enabled: false,
            prompt_templates: dir.join("prompt_templates.yml").display().to_string(),
            conf_dir: dir.display().to_string(),
            harness: None,
        };
        let changes = vec![FileChange {
            new_path: "a.py".into(),
            diff: "@@ -1 +1,2 @@\n keep\n+x=1\n".into(),
            additions: 1,
            deletions: 0,
        }];
        let out = review_changes_to_outcome(&client, &changes, &rules, &opts)
            .await
            .unwrap();
        assert!(out.report_text.contains("总分:"));
        assert!(out.score < 100);
        assert_eq!(out.inline_comments.len(), 1);
    }
}
