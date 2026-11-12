//! SCR 提示词加载（对齐 Python `ScrReviewer._load_prompts` / filter prompts）。

use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PromptError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("missing scr_review_prompt in {0}")]
    MissingBlock(String),
}

#[derive(Debug, Clone)]
pub struct ScrPrompts {
    pub system: String,
    pub user_template: String,
    pub filter_system: String,
    pub filter_user_template: String,
}

#[derive(Debug, Deserialize)]
struct PromptFile {
    scr_review_prompt: Option<ScrReviewPromptBlock>,
}

#[derive(Debug, Deserialize)]
struct ScrReviewPromptBlock {
    system_prompt: String,
    user_prompt: String,
}

impl ScrPrompts {
    pub fn load(
        prompt_templates: impl AsRef<Path>,
        conf_dir: impl AsRef<Path>,
        style: &str,
    ) -> Result<Self, PromptError> {
        let path = prompt_templates.as_ref();
        let raw = fs::read_to_string(path)?;
        let parsed: PromptFile = serde_yaml::from_str(&raw)?;
        let block = parsed
            .scr_review_prompt
            .ok_or_else(|| PromptError::MissingBlock(path.display().to_string()))?;

        let system = render_style(&block.system_prompt, style);
        let user_template = render_style(&block.user_prompt, style);

        let prompts_dir = conf_dir.as_ref().join("scr").join("prompts");
        let filter_system =
            read_optional(prompts_dir.join("review_filter_task_system.md")).unwrap_or_default();
        let filter_user_template =
            read_optional(prompts_dir.join("review_filter_task_user.md")).unwrap_or_default();

        Ok(Self {
            system,
            user_template,
            filter_system,
            filter_user_template,
        })
    }

    pub fn load_default(style: &str) -> Result<Self, PromptError> {
        Self::load("conf/prompt_templates.yml", "conf", style)
    }

    pub fn render_user(
        &self,
        change_files: &str,
        current_file_path: &str,
        diff: &str,
        system_rule: &str,
    ) -> String {
        self.user_template
            .replace("{change_files}", change_files)
            .replace("{current_file_path}", current_file_path)
            .replace("{diff}", diff)
            .replace("{system_rule}", system_rule)
    }

    pub fn render_filter_user(&self, path: &str, diff: &str, comments_block: &str) -> String {
        self.filter_user_template
            .replace("{{path}}", path)
            .replace("{{diff}}", diff)
            .replace("{{comments}}", comments_block)
    }

    pub fn filter_enabled_content(&self) -> bool {
        !self.filter_system.trim().is_empty() && !self.filter_user_template.trim().is_empty()
    }
}

fn render_style(tmpl: &str, style: &str) -> String {
    // 对齐 Jinja `{{ style }}` 的最小子集；复杂 {% if %} 块留给叙述层。
    tmpl.replace("{{ style }}", style)
        .replace("{{style}}", style)
}

fn read_optional(path: PathBuf) -> Option<String> {
    fs::read_to_string(path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn load_from_repo_conf_if_present() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from("../.."));
        let yml = root.join("conf/prompt_templates.yml");
        if !yml.exists() {
            return;
        }
        let p = ScrPrompts::load(&yml, root.join("conf"), "professional").unwrap();
        assert!(p.system.contains("JSON"));
        assert!(p.user_template.contains("{diff}"));
        let user = p.render_user("- a.py", "a.py", "+x", "rule");
        assert!(user.contains("+x"));
        assert!(user.contains("a.py"));
    }

    #[test]
    fn style_substitution() {
        let dir = std::env::temp_dir().join(format!(
            "xiezhi-prompts-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(dir.join("scr/prompts")).unwrap();
        let yaml = "scr_review_prompt:\n  system_prompt: |\n    style={{ style }}\n  user_prompt: |\n    {diff}\n";
        fs::write(dir.join("prompt_templates.yml"), yaml).unwrap();
        let p = ScrPrompts::load(dir.join("prompt_templates.yml"), &dir, "gentle").unwrap();
        assert!(p.system.contains("style=gentle"));
    }
}
