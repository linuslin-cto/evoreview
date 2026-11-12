//! Anthropic Messages API 客户端（对齐 `biz.llm.client.anthropic`）。

use crate::config::ProviderConfig;
use crate::{ChatMessage, ChatRequest, LlmClient, LlmError};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::env;

#[derive(Debug, Clone)]
pub struct AnthropicClient {
    pub config: ProviderConfig,
    pub max_tokens: u32,
    http: Client,
}

impl AnthropicClient {
    pub fn new(config: ProviderConfig) -> Self {
        let max_tokens = env::var("ANTHROPIC_MAX_TOKENS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4096);
        Self {
            config,
            max_tokens,
            http: Client::new(),
        }
    }

    pub fn with_http(config: ProviderConfig, http: Client) -> Self {
        Self {
            config,
            max_tokens: 4096,
            http,
        }
    }

    pub fn messages_url(&self) -> String {
        format!("{}/v1/messages", self.config.base_url.trim_end_matches('/'))
    }
}

pub fn split_system(messages: &[ChatMessage]) -> (Option<String>, Vec<Value>) {
    let mut system = None;
    let mut rest = Vec::new();
    for m in messages {
        if m.role == "system" {
            system = Some(m.content.clone());
        } else {
            rest.push(json!({
                "role": m.role,
                "content": m.content,
            }));
        }
    }
    (system, rest)
}

pub fn build_messages_body(model: &str, req: &ChatRequest, max_tokens: u32) -> Value {
    let (system, messages) = split_system(&req.messages);
    let mut body = json!({
        "model": model,
        "messages": messages,
        "max_tokens": req.max_tokens.unwrap_or(max_tokens),
    });
    if let Some(sys) = system {
        body["system"] = json!(sys);
    }
    if let Some(t) = req.temperature {
        body["temperature"] = json!(t);
    }
    body
}

#[derive(Debug, Deserialize)]
struct MessagesResponse {
    content: Option<Vec<ContentBlock>>,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    kind: Option<String>,
    text: Option<String>,
}

pub fn extract_text(raw: &str) -> Result<String, LlmError> {
    let parsed: MessagesResponse =
        serde_json::from_str(raw).map_err(|e| LlmError::Request(format!("invalid json: {e}")))?;
    let text = parsed
        .content
        .as_ref()
        .into_iter()
        .flatten()
        .find(|b| b.kind.as_deref() == Some("text") || b.text.is_some())
        .and_then(|b| b.text.clone())
        .filter(|s| !s.is_empty());
    text.ok_or(LlmError::EmptyResponse)
}

#[async_trait]
impl LlmClient for AnthropicClient {
    fn provider_name(&self) -> &str {
        self.config.kind.as_str()
    }

    async fn chat(&self, req: ChatRequest) -> Result<String, LlmError> {
        let model = req
            .model
            .clone()
            .unwrap_or_else(|| self.config.model.clone());
        let body = build_messages_body(&model, &req, self.max_tokens);
        let resp = self
            .http
            .post(self.messages_url())
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
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
        extract_text(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChatMessage;
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn split_keeps_system_out_of_messages() {
        let msgs = vec![
            ChatMessage::system("sys"),
            ChatMessage::user("hi"),
            ChatMessage::assistant("yo"),
        ];
        let (sys, rest) = split_system(&msgs);
        assert_eq!(sys.as_deref(), Some("sys"));
        assert_eq!(rest.len(), 2);
        assert_eq!(rest[0]["role"], "user");
    }

    #[test]
    fn extract_text_block() {
        let raw = r#"{"content":[{"type":"text","text":"hello"}]}"#;
        assert_eq!(extract_text(raw).unwrap(), "hello");
    }

    #[tokio::test]
    async fn chat_hits_messages_endpoint() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 8192];
            let n = sock.read(&mut buf).await.unwrap();
            let req = String::from_utf8_lossy(&buf[..n]);
            assert!(req.contains("POST /v1/messages"));
            assert!(req.contains("x-api-key: ak"));
            assert!(req.contains("anthropic-version: 2023-06-01"));
            let body = r#"{"content":[{"type":"text","text":"ok-anthropic"}]}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            sock.write_all(resp.as_bytes()).await.unwrap();
        });

        let cfg = ProviderConfig {
            kind: crate::ProviderKind::Anthropic,
            api_key: "ak".into(),
            base_url: format!("http://{addr}"),
            model: "claude-test".into(),
        };
        let client = AnthropicClient::with_http(cfg, Client::new());
        let out = client
            .chat(ChatRequest {
                messages: vec![ChatMessage::system("s"), ChatMessage::user("u")],
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(out, "ok-anthropic");
        server.await.unwrap();
    }
}
