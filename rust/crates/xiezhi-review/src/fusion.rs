//! 融合审查：SCR 结构化 + 可选叙述总览（对齐 `biz.review.pipeline.review_changes`）。

use crate::agent::{generate_agentic_narrative, AgenticOptions, RepoSyncSpec};
use crate::compose::compose_report;
use crate::harness2::{
    dual_probe_from_env, enabled_from_env, probe_and_compose, propose_edits,
    simulate_probe_results, ExecutionHarness, ImprovementLibrary,
};
use crate::narrative::{generate_narrative, NarrativePrompts};
use crate::pipeline::ReviewOutcome;
use crate::structured::{review_changes_structured, StructuredError, StructuredOptions};
use std::path::PathBuf;
use tracing::{info, warn};
use xiezhi_llm::LlmClient;
use xiezhi_scr::{score_from_results, FileChange, RuleResolver};

#[derive(Debug, Clone)]
pub struct FusionOptions {
    pub structured: StructuredOptions,
    pub narrative_enabled: bool,
    pub strategy: String,
    pub commits_text: String,
    /// Agentic 策略使用的本地仓库根；缺省时 agentic 降级 diff_only。
    pub repo_root: Option<PathBuf>,
    /// 可选：先 sync 远程仓库到缓存再跑 agentic。
    pub repo_sync: Option<RepoSyncSpec>,
}

impl FusionOptions {
    pub fn from_structured(structured: StructuredOptions) -> Self {
        Self {
            narrative_enabled: std::env::var("REVIEW_NARRATIVE_ENABLED")
                .unwrap_or_else(|_| "1".into())
                != "0",
            strategy: std::env::var("REVIEW_STRATEGY").unwrap_or_else(|_| "diff_only".into()),
            commits_text: String::new(),
            structured,
            repo_root: None,
            repo_sync: None,
        }
    }
}

pub async fn review_fusion(
    client: &dyn LlmClient,
    changes: &[FileChange],
    rules: &RuleResolver,
    opts: &FusionOptions,
) -> Result<ReviewOutcome, StructuredError> {
    if changes.is_empty() {
        return Ok(ReviewOutcome {
            report_text: "关注的文件没有修改".into(),
            score: 0,
            inline_comments: Vec::new(),
        });
    }

    let mut results = review_changes_structured(client, changes, rules, &opts.structured).await?;

    // Harness²：Propose–Probe–Compose（默认 simulate；HARNESS2_DUAL_PROBE=1 时二次 SCR）
    if enabled_from_env() {
        let mut library = ImprovementLibrary::load_from_env();
        let base_harness = ExecutionHarness::base().with_library_hints(&library);
        let base_metrics = crate::harness2::contrast_metrics(&results);
        let edit = propose_edits(Some(client), &base_metrics).await;
        let candidate_harness = edit.apply(&base_harness);
        let candidate_results = if dual_probe_from_env() {
            let mut probe_opts = opts.structured.clone();
            probe_opts.harness = Some(candidate_harness.clone());
            // 探针轮关闭 filter，避免成本再翻倍
            probe_opts.filter_enabled = false;
            match review_changes_structured(client, changes, rules, &probe_opts).await {
                Ok(r) => {
                    info!("fusion: harness2 dual-probe SCR completed");
                    r
                }
                Err(e) => {
                    warn!("fusion: harness2 dual-probe failed, fallback simulate: {e}");
                    simulate_probe_results(&results, &candidate_harness)
                }
            }
        } else {
            simulate_probe_results(&results, &candidate_harness)
        };
        let ppc = probe_and_compose(
            &base_harness,
            &edit,
            &results,
            &candidate_results,
            Some(&mut library),
        );
        if ppc.adopted {
            info!(
                base = ppc.base_score,
                cand = ppc.candidate_score,
                files = ppc.files_adopted,
                "fusion: harness2 adopted (partial merge)"
            );
            results = ppc.merged_results;
        } else {
            info!(
                base = ppc.base_score,
                cand = ppc.candidate_score,
                "fusion: harness2 kept base harness"
            );
        }
    }

    let score = score_from_results(&results);
    let inline_comments = results
        .iter()
        .flat_map(|r| r.comments.iter())
        .filter(|c| !c.filtered && c.start_line > 0)
        .cloned()
        .collect();

    let mut narrative = String::new();
    let want_narrative =
        opts.narrative_enabled && (opts.strategy == "diff_only" || opts.strategy == "agentic");
    if want_narrative {
        narrative = if opts.strategy == "agentic" {
            let fallback_root = opts.repo_root.clone().unwrap_or_else(|| PathBuf::from("."));
            if opts.repo_root.is_none() && opts.repo_sync.is_none() {
                warn!("fusion: agentic 缺少 repo_root/repo_sync，叙述层降级 diff_only");
                generate_diff_only(client, changes, opts).await
            } else {
                let mut agentic = AgenticOptions::new(
                    &opts.structured.prompt_templates,
                    &opts.structured.style,
                    fallback_root,
                );
                if let Some(sync) = &opts.repo_sync {
                    agentic = agentic.with_sync(sync.clone());
                }
                generate_agentic_narrative(client, changes, &opts.commits_text, &agentic).await
            }
        } else {
            generate_diff_only(client, changes, opts).await
        };
    }

    let report_text = compose_report(&results, score, &narrative, false);
    Ok(ReviewOutcome {
        report_text,
        score,
        inline_comments,
    })
}

async fn generate_diff_only(
    client: &dyn LlmClient,
    changes: &[FileChange],
    opts: &FusionOptions,
) -> String {
    match NarrativePrompts::load(&opts.structured.prompt_templates, &opts.structured.style) {
        Ok(prompts) => {
            match generate_narrative(client, &prompts, changes, &opts.commits_text).await {
                Ok(text) => text,
                Err(e) => {
                    warn!("fusion: 叙述层生成失败，仅用结构化结果: {e}");
                    String::new()
                }
            }
        }
        Err(e) => {
            warn!("fusion: 叙述提示词加载失败: {e}");
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xiezhi_llm::ScriptedClient;

    fn clear_harness2_env() {
        for k in [
            "HARNESS2_ENABLED",
            "HARNESS2_DUAL_PROBE",
            "HARNESS2_PROPOSE_LLM",
            "HARNESS2_LIBRARY_PATH",
        ] {
            std::env::remove_var(k);
        }
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn fusion_harness2_filters_spam_when_enabled() {
        let _guard = crate::harness2::ENV_LOCK.lock().unwrap();
        clear_harness2_env();
        std::env::set_var("HARNESS2_ENABLED", "1");
        std::env::set_var("HARNESS2_DUAL_PROBE", "0");
        std::env::set_var(
            "HARNESS2_LIBRARY_PATH",
            std::env::temp_dir()
                .join(format!(
                    "h2-{}-lib.json",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ))
                .display()
                .to_string(),
        );
        let dir = std::env::temp_dir().join(format!(
            "xiezhi-h2-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("scr/prompts")).unwrap();
        let yml = r#"
scr_review_prompt:
  system_prompt: sys
  user_prompt: "{diff}"
xiezhi_review_prompt:
  system_prompt: narr
  user_prompt: "{diffs_text}"
"#;
        std::fs::write(dir.join("prompt_templates.yml"), yml).unwrap();
        // 多条无锚定的重复 critical → harness2 应变严并过滤
        let client = ScriptedClient::new(
            "scripted",
            [r#"{"comments":[
                {"content":"bad","existing_code":"","category":"other","severity":"critical"},
                {"content":"bad","existing_code":"","category":"other","severity":"critical"},
                {"content":"bad","existing_code":"","category":"other","severity":"critical"},
                {"content":"bad","existing_code":"","category":"other","severity":"critical"}
            ]}"#
            .into()],
        );
        let rules = RuleResolver::new("missing-rules", "missing-allowlist");
        let structured = StructuredOptions {
            style: "professional".into(),
            max_files: 10,
            filter_enabled: false,
            prompt_templates: dir.join("prompt_templates.yml").display().to_string(),
            conf_dir: dir.display().to_string(),
            harness: None,
        };
        let opts = FusionOptions {
            structured,
            narrative_enabled: false,
            strategy: "diff_only".into(),
            commits_text: String::new(),
            repo_root: None,
            repo_sync: None,
        };
        let changes = vec![FileChange {
            new_path: "a.py".into(),
            diff: "@@ -1 +1,2 @@\n keep\n+x=1\n".into(),
            additions: 1,
            deletions: 0,
        }];
        let out = review_fusion(&client, &changes, &rules, &opts)
            .await
            .unwrap();
        // 精度模式过滤未锚定后，有效 inline 应减少
        assert!(
            out.inline_comments.len() <= 1,
            "inline={}",
            out.inline_comments.len()
        );
        clear_harness2_env();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn fusion_harness2_dual_probe_uses_second_scr() {
        let _guard = crate::harness2::ENV_LOCK.lock().unwrap();
        clear_harness2_env();
        std::env::set_var("HARNESS2_ENABLED", "1");
        std::env::set_var("HARNESS2_DUAL_PROBE", "1");
        std::env::set_var("HARNESS2_PROPOSE_LLM", "0");
        std::env::set_var(
            "HARNESS2_LIBRARY_PATH",
            std::env::temp_dir()
                .join(format!(
                    "h2-dual-{}-lib.json",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ))
                .display()
                .to_string(),
        );
        let dir = std::env::temp_dir().join(format!(
            "xiezhi-h2-dual-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("scr/prompts")).unwrap();
        let yml = r#"
scr_review_prompt:
  system_prompt: sys
  user_prompt: "{diff}"
xiezhi_review_prompt:
  system_prompt: narr
  user_prompt: "{diffs_text}"
"#;
        std::fs::write(dir.join("prompt_templates.yml"), yml).unwrap();
        // 基线 spam → 双探针第二轮返回已锚定高质量评论
        let client = ScriptedClient::new(
            "scripted",
            [
                r#"{"comments":[
                    {"content":"bad","existing_code":"","category":"other","severity":"critical"},
                    {"content":"bad","existing_code":"","category":"other","severity":"critical"},
                    {"content":"bad","existing_code":"","category":"other","severity":"critical"}
                ]}"#
                .into(),
                r#"{"comments":[
                    {"content":"null deref risk","existing_code":"x=1","category":"correctness","severity":"high","start_line":2,"end_line":2}
                ]}"#
                .into(),
            ],
        );
        let rules = RuleResolver::new("missing-rules", "missing-allowlist");
        let structured = StructuredOptions {
            style: "professional".into(),
            max_files: 10,
            filter_enabled: false,
            prompt_templates: dir.join("prompt_templates.yml").display().to_string(),
            conf_dir: dir.display().to_string(),
            harness: None,
        };
        let opts = FusionOptions {
            structured,
            narrative_enabled: false,
            strategy: "diff_only".into(),
            commits_text: String::new(),
            repo_root: None,
            repo_sync: None,
        };
        let changes = vec![FileChange {
            new_path: "a.py".into(),
            diff: "@@ -1 +1,2 @@\n keep\n+x=1\n".into(),
            additions: 1,
            deletions: 0,
        }];
        let out = review_fusion(&client, &changes, &rules, &opts)
            .await
            .unwrap();
        assert!(
            out.inline_comments
                .iter()
                .any(|c| c.content.contains("null")),
            "dual-probe should adopt second SCR: {:?}",
            out.inline_comments
                .iter()
                .map(|c| &c.content)
                .collect::<Vec<_>>()
        );
        clear_harness2_env();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn fusion_includes_overview_section() {
        let _guard = crate::harness2::ENV_LOCK.lock().unwrap();
        clear_harness2_env();
        let dir = std::env::temp_dir().join(format!(
            "xiezhi-fusion-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("scr/prompts")).unwrap();
        let yml = r#"
scr_review_prompt:
  system_prompt: |
    sys
  user_prompt: |
    {diff}
xiezhi_review_prompt:
  system_prompt: |
    narr-sys {{ style }}
  user_prompt: |
    diffs:{diffs_text}
    commits:{commits_text}
"#;
        std::fs::write(dir.join("prompt_templates.yml"), yml).unwrap();

        let client = ScriptedClient::new(
            "scripted",
            [
                // SCR file review
                r#"{"comments":[{"content":"bad","existing_code":"x=1","category":"bug","severity":"high"}]}"#.into(),
                // narrative
                "整体看起来有风险\n总分:70分".into(),
            ],
        );
        let rules = RuleResolver::new("missing-rules", "missing-allowlist");
        let structured = StructuredOptions {
            style: "professional".into(),
            max_files: 10,
            filter_enabled: false,
            prompt_templates: dir.join("prompt_templates.yml").display().to_string(),
            conf_dir: dir.display().to_string(),
            harness: None,
        };
        let opts = FusionOptions {
            structured,
            narrative_enabled: true,
            strategy: "diff_only".into(),
            commits_text: "init".into(),
            repo_root: None,
            repo_sync: None,
        };
        let changes = vec![FileChange {
            new_path: "a.py".into(),
            diff: "@@ -1 +1,2 @@\n keep\n+x=1\n".into(),
            additions: 1,
            deletions: 0,
        }];
        let out = review_fusion(&client, &changes, &rules, &opts)
            .await
            .unwrap();
        assert!(out.report_text.contains("整体总览"), "{}", out.report_text);
        assert!(out.report_text.contains("整体看起来有风险"));
        assert!(out.score < 100);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn fusion_agentic_path() {
        let _guard = crate::harness2::ENV_LOCK.lock().unwrap();
        clear_harness2_env();
        let dir = std::env::temp_dir().join(format!(
            "xiezhi-fusion-agentic-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.py"), "x=1\n").unwrap();
        let yml = r#"
scr_review_prompt:
  system_prompt: sys
  user_prompt: "{diff}"
agentic_xiezhi_review_prompt:
  system_prompt: "agent {repo_root}"
  user_prompt: "{diffs_text}"
xiezhi_review_prompt:
  system_prompt: narr
  user_prompt: "{diffs_text}"
"#;
        std::fs::write(dir.join("prompt_templates.yml"), yml).unwrap();
        let client = ScriptedClient::new(
            "scripted",
            [
                r#"{"comments":[]}"#.into(),
                "agentic overview\n总分:85分".into(),
            ],
        );
        let rules = RuleResolver::new("missing-rules", "missing-allowlist");
        let structured = StructuredOptions {
            style: "professional".into(),
            max_files: 10,
            filter_enabled: false,
            prompt_templates: dir.join("prompt_templates.yml").display().to_string(),
            conf_dir: dir.display().to_string(),
            harness: None,
        };
        let opts = FusionOptions {
            structured,
            narrative_enabled: true,
            strategy: "agentic".into(),
            commits_text: "c".into(),
            repo_root: Some(dir.clone()),
            repo_sync: None,
        };
        let changes = vec![FileChange {
            new_path: "a.py".into(),
            diff: "@@ -0,0 +1 @@\n+x=1\n".into(),
            additions: 1,
            deletions: 0,
        }];
        let out = review_fusion(&client, &changes, &rules, &opts)
            .await
            .unwrap();
        assert!(out.report_text.contains("整体总览"));
        assert!(out.report_text.contains("agentic overview"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
