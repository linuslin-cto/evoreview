//! Agentic 提示词加载。

use crate::narrative::render_style_template;
use serde::Deserialize;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgenticPromptError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("missing agentic_xiezhi_review_prompt in {0}")]
    Missing(String),
}

#[derive(Debug, Deserialize)]
struct PromptFile {
    agentic_xiezhi_review_prompt: Option<PromptBlock>,
}

#[derive(Debug, Deserialize)]
struct PromptBlock {
    system_prompt: String,
    user_prompt: String,
}

#[derive(Debug, Clone)]
pub struct AgenticPrompts {
    pub system: String,
    pub user_template: String,
}

impl AgenticPrompts {
    pub fn load(
        prompt_templates: impl AsRef<Path>,
        style: &str,
    ) -> Result<Self, AgenticPromptError> {
        let path = prompt_templates.as_ref();
        let raw = fs::read_to_string(path)?;
        let parsed: PromptFile = serde_yaml::from_str(&raw)?;
        let block = parsed
            .agentic_xiezhi_review_prompt
            .ok_or_else(|| AgenticPromptError::Missing(path.display().to_string()))?;
        Ok(Self {
            system: render_style_template(&block.system_prompt, style),
            user_template: render_style_template(&block.user_prompt, style),
        })
    }

    pub fn render(
        &self,
        repo_root: &str,
        diffs_text: &str,
        commits_text: &str,
    ) -> (String, String) {
        let system = self.system.replace("{repo_root}", repo_root);
        let user = self
            .user_template
            .replace("{repo_root}", repo_root)
            .replace("{diffs_text}", diffs_text)
            .replace("{commits_text}", commits_text);
        (system, user)
    }
}
