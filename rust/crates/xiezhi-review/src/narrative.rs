//! 叙述总览层（对齐 `XiezhiReviewer` diff_only 路径）。

use regex::Regex;
use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;
use thiserror::Error;
use xiezhi_llm::{ChatMessage, ChatRequest, LlmClient};
use xiezhi_scr::FileChange;

static SCORE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"总分[:：]\s*(\d+)分?").unwrap());

#[derive(Debug, Error)]
pub enum NarrativeError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("missing xiezhi_review_prompt in {0}")]
    MissingBlock(String),
    #[error(transparent)]
    Llm(#[from] xiezhi_llm::LlmError),
}

#[derive(Debug, Deserialize)]
struct PromptFile {
    xiezhi_review_prompt: Option<PromptBlock>,
}

#[derive(Debug, Deserialize)]
struct PromptBlock {
    system_prompt: String,
    user_prompt: String,
}

#[derive(Debug, Clone)]
pub struct NarrativePrompts {
    pub system: String,
    pub user_template: String,
}

impl NarrativePrompts {
    pub fn load(prompt_templates: impl AsRef<Path>, style: &str) -> Result<Self, NarrativeError> {
        let path = prompt_templates.as_ref();
        let raw = fs::read_to_string(path)?;
        let parsed: PromptFile = serde_yaml::from_str(&raw)?;
        let block = parsed
            .xiezhi_review_prompt
            .ok_or_else(|| NarrativeError::MissingBlock(path.display().to_string()))?;
        Ok(Self {
            system: render_style_template(&block.system_prompt, style),
            user_template: render_style_template(&block.user_prompt, style),
        })
    }

    pub fn render_user(&self, diffs_text: &str, commits_text: &str) -> String {
        self.user_template
            .replace("{diffs_text}", diffs_text)
            .replace("{commits_text}", commits_text)
    }
}

/// 最小 Jinja 子集：`{{ style }}` + `{% if style == 'x' %}...{% elif ... %}{% endif %}`。
pub fn render_style_template(tmpl: &str, style: &str) -> String {
    let mut out = String::new();
    let mut rest = tmpl;
    while let Some(start) = rest.find("{% if style ==") {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        let Some(end_rel) = after.find("{% endif %}") else {
            out.push_str(after);
            break;
        };
        let block = &after[..end_rel];
        out.push_str(&eval_style_if_block(block, style));
        rest = &after[end_rel + "{% endif %}".len()..];
    }
    out.push_str(rest);
    out.replace("{{ style }}", style)
        .replace("{{style}}", style)
}

fn eval_style_if_block(block: &str, style: &str) -> String {
    // block starts with `{% if style == ...` and excludes `{% endif %}`
    let mut branches: Vec<(String, String)> = Vec::new();
    let mut s = block;

    while let Some(tag_end) = s.find("%}") {
        let tag = &s[..=tag_end + 1];
        let cond = extract_style_cond(tag);
        let after = &s[tag_end + 2..];
        if let Some(next) = after.find("{% elif style ==") {
            branches.push((cond, after[..next].to_string()));
            s = &after[next..];
        } else {
            branches.push((cond, after.to_string()));
            break;
        }
    }

    for (cond, body) in branches {
        if cond == style {
            return body.trim_matches('\n').to_string();
        }
    }
    String::new()
}

fn extract_style_cond(tag: &str) -> String {
    if let Some(start) = tag.find('\'') {
        if let Some(end) = tag[start + 1..].find('\'') {
            return tag[start + 1..start + 1 + end].to_string();
        }
    }
    String::new()
}

pub fn parse_review_score(review_text: &str) -> i32 {
    if review_text.is_empty() {
        return 0;
    }
    SCORE_RE
        .captures(review_text)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse().ok())
        .unwrap_or(0)
}

pub fn strip_markdown_fence(text: &str) -> String {
    let t = text.trim();
    if t.starts_with("```markdown") && t.ends_with("```") {
        t.trim_start_matches("```markdown")
            .trim_end_matches("```")
            .trim()
            .to_string()
    } else {
        t.to_string()
    }
}

pub fn changes_to_diffs_text(changes: &[FileChange]) -> String {
    changes
        .iter()
        .map(|c| format!("### {}\n{}", c.new_path, c.diff))
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub async fn generate_narrative(
    client: &dyn LlmClient,
    prompts: &NarrativePrompts,
    changes: &[FileChange],
    commits_text: &str,
) -> Result<String, NarrativeError> {
    if changes.is_empty() {
        return Ok("代码为空".into());
    }
    let diffs_text = changes_to_diffs_text(changes);
    let user = prompts.render_user(&diffs_text, commits_text);
    let raw = client
        .chat(ChatRequest {
            messages: vec![
                ChatMessage::system(prompts.system.clone()),
                ChatMessage::user(user),
            ],
            ..Default::default()
        })
        .await?;
    Ok(strip_markdown_fence(&raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_score_variants() {
        assert_eq!(parse_review_score("总分:80分"), 80);
        assert_eq!(parse_review_score("总分：90"), 90);
        assert_eq!(parse_review_score("no score"), 0);
    }

    #[test]
    fn style_if_selects_branch() {
        let tmpl = "head\n{% if style == 'professional' %}\nPRO\n{% elif style == 'gentle' %}\nGEN\n{% endif %}\ntail {{ style }}";
        let out = render_style_template(tmpl, "gentle");
        assert!(out.contains("GEN"), "{out}");
        assert!(!out.contains("PRO"), "{out}");
        assert!(out.contains("tail gentle"), "{out}");
    }

    #[test]
    fn load_repo_prompt_if_present() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from("../.."));
        let yml = root.join("conf/prompt_templates.yml");
        if !yml.exists() {
            return;
        }
        let p = NarrativePrompts::load(&yml, "professional").unwrap();
        assert!(p.system.contains("总分") || p.system.contains("工程"));
        assert!(p.user_template.contains("{diffs_text}"));
        assert!(
            p.system.contains("专业")
                || p.system.contains("professional")
                || !p.system.contains("{%")
        );
    }
}
