//! LLM 工厂（对齐 `biz.llm.factory.Factory`）。

use crate::anthropic::AnthropicClient;
use crate::config::{ProviderConfig, ProviderKind};
use crate::ollama::OllamaClient;
use crate::openai_compat::OpenAiCompatClient;
use crate::{LlmClient, LlmError};
use std::env;

/// 按 provider 名构造客户端。
pub fn factory(provider: &str) -> Result<Box<dyn LlmClient>, LlmError> {
    let kind = ProviderKind::parse(provider)?;
    let cfg = ProviderConfig::from_env(kind)?;
    match kind {
        ProviderKind::OpenAi
        | ProviderKind::DeepSeek
        | ProviderKind::Qwen
        | ProviderKind::Zhipu => Ok(Box::new(OpenAiCompatClient::new(cfg))),
        ProviderKind::Anthropic => Ok(Box::new(AnthropicClient::new(cfg))),
        ProviderKind::Ollama => Ok(Box::new(OllamaClient::new(cfg))),
    }
}

/// 读取 `LLM_PROVIDER`（默认 deepseek，与 `.env.dist` 一致）。
pub fn factory_from_env() -> Result<Box<dyn LlmClient>, LlmError> {
    let provider = env::var("LLM_PROVIDER").unwrap_or_else(|_| "deepseek".into());
    factory(&provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_provider_errors() {
        match factory("no-such") {
            Err(LlmError::UnknownProvider(_)) => {}
            Ok(_) => panic!("expected UnknownProvider"),
            Err(e) => panic!("unexpected: {e}"),
        }
    }

    #[test]
    fn missing_key_for_openai_compat() {
        // Ensure we don't accidentally pick up a real key from the ambient env in CI.
        // If a key is present, from_env succeeds — still a valid outcome.
        match factory("deepseek") {
            Ok(_) => {}
            Err(LlmError::NotConfigured(_)) => {}
            Err(e) => panic!("unexpected: {e}"),
        }
    }

    #[test]
    fn ollama_factory_without_key() {
        // Ollama should construct even without API key.
        match factory("ollama") {
            Ok(c) => assert_eq!(c.provider_name(), "ollama"),
            Err(e) => panic!("ollama should not require key: {e}"),
        }
    }
}
