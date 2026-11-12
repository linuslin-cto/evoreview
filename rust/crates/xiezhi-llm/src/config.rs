//! Provider 环境配置（对齐 `conf/.env.dist` + `biz.llm.factory`）。

use crate::LlmError;
use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    OpenAi,
    DeepSeek,
    Qwen,
    Anthropic,
    Zhipu,
    Ollama,
}

impl ProviderKind {
    pub fn parse(name: &str) -> Result<Self, LlmError> {
        match name.trim().to_ascii_lowercase().as_str() {
            "openai" => Ok(Self::OpenAi),
            "deepseek" => Ok(Self::DeepSeek),
            "qwen" => Ok(Self::Qwen),
            "anthropic" => Ok(Self::Anthropic),
            "zhipuai" | "zhipu" => Ok(Self::Zhipu),
            "ollama" => Ok(Self::Ollama),
            other => Err(LlmError::UnknownProvider(other.to_string())),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::DeepSeek => "deepseek",
            Self::Qwen => "qwen",
            Self::Anthropic => "anthropic",
            Self::Zhipu => "zhipuai",
            Self::Ollama => "ollama",
        }
    }

    pub fn is_openai_compatible(self) -> bool {
        matches!(
            self,
            Self::OpenAi | Self::DeepSeek | Self::Qwen | Self::Zhipu
        )
    }
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

impl ProviderConfig {
    pub fn from_env(kind: ProviderKind) -> Result<Self, LlmError> {
        let (key_var, url_var, model_var, default_url, default_model) = match kind {
            ProviderKind::OpenAi => (
                "OPENAI_API_KEY",
                "OPENAI_API_BASE_URL",
                "OPENAI_API_MODEL",
                "https://api.openai.com/v1",
                "gpt-4o-mini",
            ),
            ProviderKind::DeepSeek => (
                "DEEPSEEK_API_KEY",
                "DEEPSEEK_API_BASE_URL",
                "DEEPSEEK_API_MODEL",
                "https://api.deepseek.com",
                "deepseek-chat",
            ),
            ProviderKind::Qwen => (
                "QWEN_API_KEY",
                "QWEN_API_BASE_URL",
                "QWEN_API_MODEL",
                "https://dashscope.aliyuncs.com/compatible-mode/v1",
                "qwen-coder-plus",
            ),
            ProviderKind::Zhipu => (
                "ZHIPUAI_API_KEY",
                "ZHIPUAI_API_BASE_URL",
                "ZHIPUAI_API_MODEL",
                "https://open.bigmodel.cn/api/paas/v4",
                "GLM-4-Flash",
            ),
            ProviderKind::Anthropic => (
                "ANTHROPIC_API_KEY",
                "ANTHROPIC_API_BASE_URL",
                "ANTHROPIC_API_MODEL",
                "https://api.anthropic.com",
                "claude-sonnet-4-5-20250929",
            ),
            ProviderKind::Ollama => (
                "OLLAMA_API_KEY",
                "OLLAMA_API_BASE_URL",
                "OLLAMA_API_MODEL",
                "http://127.0.0.1:11434",
                "deepseek-r1:latest",
            ),
        };

        let api_key = env::var(key_var).unwrap_or_default();
        // Ollama 本地部署通常无需 Key；其它 provider 拒绝占位符。
        if kind != ProviderKind::Ollama
            && (api_key.trim().is_empty() || api_key == "xxxx" || api_key == "sk-xxx")
        {
            return Err(LlmError::NotConfigured(kind.as_str().into()));
        }
        let mut base_url = env::var(url_var).unwrap_or_else(|_| default_url.into());
        if kind == ProviderKind::Anthropic && (base_url.trim().is_empty() || base_url == "xxxx") {
            base_url = default_url.to_string();
        }
        let model = env::var(model_var).unwrap_or_else(|_| default_model.into());
        Ok(Self {
            kind,
            api_key,
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
        })
    }

    pub fn chat_completions_url(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_providers() {
        assert_eq!(
            ProviderKind::parse("DeepSeek").unwrap(),
            ProviderKind::DeepSeek
        );
        assert!(ProviderKind::parse("nope").is_err());
        assert!(ProviderKind::OpenAi.is_openai_compatible());
        assert!(ProviderKind::Zhipu.is_openai_compatible());
        assert!(!ProviderKind::Anthropic.is_openai_compatible());
        assert!(!ProviderKind::Ollama.is_openai_compatible());
    }

    #[test]
    fn chat_url_joins() {
        let cfg = ProviderConfig {
            kind: ProviderKind::DeepSeek,
            api_key: "k".into(),
            base_url: "https://api.deepseek.com/".into(),
            model: "deepseek-chat".into(),
        };
        assert_eq!(
            cfg.chat_completions_url(),
            "https://api.deepseek.com/chat/completions"
        );
    }
}
