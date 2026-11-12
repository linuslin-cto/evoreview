//! LLM provider clients（对齐 `biz.llm`）。

mod anthropic;
mod config;
mod factory;
mod ollama;
mod openai_compat;

pub use anthropic::AnthropicClient;
pub use config::{ProviderConfig, ProviderKind};
pub use factory::{factory, factory_from_env};
pub use ollama::OllamaClient;
pub use openai_compat::OpenAiCompatClient;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("provider not configured: {0}")]
    NotConfigured(String),
    #[error("unknown provider: {0}")]
    UnknownProvider(String),
    #[error("request failed: {0}")]
    Request(String),
    #[error("empty response from provider")]
    EmptyResponse,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    /// 覆盖客户端默认模型
    pub model: Option<String>,
}

#[async_trait]
pub trait LlmClient: Send + Sync {
    fn provider_name(&self) -> &str;
    async fn chat(&self, req: ChatRequest) -> Result<String, LlmError>;
}

/// 占位客户端：未配置真实 Key 时返回明确错误，便于测试门禁。
pub struct NullClient {
    pub provider: String,
}

#[async_trait]
impl LlmClient for NullClient {
    fn provider_name(&self) -> &str {
        &self.provider
    }

    async fn chat(&self, _req: ChatRequest) -> Result<String, LlmError> {
        Err(LlmError::NotConfigured(self.provider.clone()))
    }
}

/// 按顺序吐出预设回复（测试 / dry-run 编排用）。
pub struct ScriptedClient {
    pub provider: String,
    responses: std::sync::Mutex<std::collections::VecDeque<String>>,
}

impl ScriptedClient {
    pub fn new(provider: impl Into<String>, responses: impl IntoIterator<Item = String>) -> Self {
        Self {
            provider: provider.into(),
            responses: std::sync::Mutex::new(responses.into_iter().collect()),
        }
    }
}

#[async_trait]
impl LlmClient for ScriptedClient {
    fn provider_name(&self) -> &str {
        &self.provider
    }

    async fn chat(&self, _req: ChatRequest) -> Result<String, LlmError> {
        let mut q = self
            .responses
            .lock()
            .map_err(|e| LlmError::Request(e.to_string()))?;
        q.pop_front()
            .ok_or_else(|| LlmError::Request("scripted client exhausted".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn null_client_errors() {
        let c = NullClient {
            provider: "deepseek".into(),
        };
        let err = c.chat(ChatRequest::default()).await.unwrap_err();
        assert!(matches!(err, LlmError::NotConfigured(_)));
    }
}
