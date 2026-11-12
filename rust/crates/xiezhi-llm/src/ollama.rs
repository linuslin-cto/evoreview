//! Ollama `/api/chat` 客户端（对齐 `biz.llm.client.ollama_client`）。

use crate::config::ProviderConfig;
use crate::{ChatRequest, LlmClient, LlmError};
use async_trait::async_trait;
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub struct OllamaClient {
    pub config: ProviderConfig,
    http: Client,
}

impl OllamaClient {
    pub fn new(config: ProviderConfig) -> Self {
        Self {
            config,
            http: Client::new(),
        }
    }

    pub fn with_http(config: ProviderConfig, http: Client) -> Self {
        Self { config, http }
    }

    pub fn chat_url(&self) -> String {
        format!("{}/api/chat", self.config.base_url.trim_end_matches('/'))
    }
}

pub fn strip_think_blocks(content: &str) -> String {
    if content.contains("<think>") && !content.contains("</think>") {
        return "COT ABORT!".into();
    }
    if !content.contains("<think>") && content.contains("</think>") {
        return content
            .split("</think>")
            .nth(1)
            .unwrap_or("")
            .trim()
            .to_string();
    }
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?s)<think>.*?</think>").expect("regex"));
    re.replace_all(content, "").trim().to_string()
}

pub fn build_chat_body(model: &str, req: &ChatRequest) -> Value {
    let messages: Vec<Value> = req
        .messages
        .iter()
        .map(|m| {
            json!({
                "role": m.role,
                "content": m.content,
            })
        })
        .collect();
    json!({
        "model": model,
        "messages": messages,
        "stream": false,
    })
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    message: Option<OllamaMessage>,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    content: Option<String>,
}

pub fn extract_content(raw: &str) -> Result<String, LlmError> {
    let parsed: OllamaResponse =
        serde_json::from_str(raw).map_err(|e| LlmError::Request(format!("invalid json: {e}")))?;
    let content = parsed
        .message
        .and_then(|m| m.content)
        .filter(|s| !s.is_empty())
        .ok_or(LlmError::EmptyResponse)?;
    Ok(strip_think_blocks(&content))
}

#[async_trait]
impl LlmClient for OllamaClient {
    fn provider_name(&self) -> &str {
        self.config.kind.as_str()
    }

    async fn chat(&self, req: ChatRequest) -> Result<String, LlmError> {
        let model = req
            .model
            .clone()
            .unwrap_or_else(|| self.config.model.clone());
        let body = build_chat_body(&model, &req);
        let resp = self
            .http
            .post(self.chat_url())
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
        extract_content(&text)
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
    fn strips_think() {
        assert_eq!(
            strip_think_blocks("<think>secret</think>\nanswer"),
            "answer"
        );
        assert_eq!(strip_think_blocks("<think>truncated"), "COT ABORT!");
    }

    #[tokio::test]
    async fn chat_hits_ollama() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let n = sock.read(&mut buf).await.unwrap();
            let req = String::from_utf8_lossy(&buf[..n]);
            assert!(req.contains("POST /api/chat"));
            let body = r#"{"message":{"content":"<think>x</think>hello"}}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            sock.write_all(resp.as_bytes()).await.unwrap();
        });

        let cfg = ProviderConfig {
            kind: crate::ProviderKind::Ollama,
            api_key: String::new(),
            base_url: format!("http://{addr}"),
            model: "qwen".into(),
        };
        let client = OllamaClient::new(cfg);
        let out = client
            .chat(ChatRequest {
                messages: vec![ChatMessage::user("ping")],
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(out, "hello");
        server.await.unwrap();
    }
}
