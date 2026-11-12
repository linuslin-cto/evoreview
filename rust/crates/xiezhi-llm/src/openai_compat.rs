//! OpenAI 兼容 Chat Completions 客户端（OpenAI / DeepSeek / Qwen）。

use crate::config::ProviderConfig;
use crate::{ChatRequest, LlmClient, LlmError};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
pub struct OpenAiCompatClient {
    pub config: ProviderConfig,
    http: Client,
}

impl OpenAiCompatClient {
    pub fn new(config: ProviderConfig) -> Self {
        Self {
            config,
            http: Client::new(),
        }
    }

    pub fn with_http(config: ProviderConfig, http: Client) -> Self {
        Self { config, http }
    }
}

#[derive(Debug, Serialize)]
struct ApiMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Option<Vec<Choice>>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Option<ChoiceMessage>,
}

#[derive(Debug, Deserialize)]
struct ChoiceMessage {
    content: Option<String>,
}

pub fn build_chat_body(model: &str, req: &ChatRequest, qwen_extra: bool) -> Value {
    let messages: Vec<ApiMessage<'_>> = req
        .messages
        .iter()
        .map(|m| ApiMessage {
            role: &m.role,
            content: &m.content,
        })
        .collect();
    let mut body = json!({
        "model": model,
        "messages": messages,
    });
    if let Some(t) = req.temperature {
        body["temperature"] = json!(t);
    }
    if let Some(m) = req.max_tokens {
        body["max_tokens"] = json!(m);
    }
    if qwen_extra {
        body["enable_thinking"] = json!(false);
    }
    body
}

pub fn extract_assistant_text(raw: &str) -> Result<String, LlmError> {
    let parsed: ChatCompletionResponse =
        serde_json::from_str(raw).map_err(|e| LlmError::Request(format!("invalid json: {e}")))?;
    let content = parsed
        .choices
        .as_ref()
        .and_then(|c| c.first())
        .and_then(|c| c.message.as_ref())
        .and_then(|m| m.content.clone())
        .filter(|s| !s.is_empty());
    content.ok_or(LlmError::EmptyResponse)
}

#[async_trait]
impl LlmClient for OpenAiCompatClient {
    fn provider_name(&self) -> &str {
        self.config.kind.as_str()
    }

    async fn chat(&self, req: ChatRequest) -> Result<String, LlmError> {
        let model = req
            .model
            .clone()
            .unwrap_or_else(|| self.config.model.clone());
        let qwen_extra = self.config.kind == crate::config::ProviderKind::Qwen;
        let body = build_chat_body(&model, &req, qwen_extra);
        let url = self.config.chat_completions_url();

        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.config.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::Request(e.to_string()))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| LlmError::Request(e.to_string()))?;
        if !status.is_success() {
            return Err(LlmError::Request(format!("HTTP {status}: {text}")));
        }
        extract_assistant_text(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProviderKind;
    use crate::ChatMessage;
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn build_and_extract() {
        let body = build_chat_body(
            "deepseek-chat",
            &ChatRequest {
                messages: vec![ChatMessage::user("hi")],
                temperature: Some(0.2),
                max_tokens: Some(128),
                model: None,
            },
            false,
        );
        assert_eq!(body["model"], "deepseek-chat");
        assert!((body["temperature"].as_f64().unwrap() - 0.2).abs() < 1e-5);

        let raw = r#"{"choices":[{"message":{"content":"hello"}}]}"#;
        assert_eq!(extract_assistant_text(raw).unwrap(), "hello");
        assert!(matches!(
            extract_assistant_text(r#"{"choices":[]}"#).unwrap_err(),
            LlmError::EmptyResponse
        ));
    }

    #[tokio::test]
    async fn chat_hits_compat_endpoint() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let n = sock.read(&mut buf).await.unwrap();
            let req = String::from_utf8_lossy(&buf[..n]);
            assert!(req.contains("POST /v1/chat/completions"));
            assert!(req.contains("Bearer test-key"));
            let body = r#"{"choices":[{"message":{"content":"ok-from-mock"}}]}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            sock.write_all(resp.as_bytes()).await.unwrap();
        });

        let cfg = ProviderConfig {
            kind: ProviderKind::OpenAi,
            api_key: "test-key".into(),
            base_url: format!("http://{addr}/v1"),
            model: "gpt-test".into(),
        };
        let client = OpenAiCompatClient::new(cfg);
        let out = client
            .chat(ChatRequest {
                messages: vec![ChatMessage::user("ping")],
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(out, "ok-from-mock");
        server.await.unwrap();
    }
}
