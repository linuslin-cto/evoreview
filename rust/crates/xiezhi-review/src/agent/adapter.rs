//! JSON tool-calling 协议适配（对齐 `LLMAdapter._json_fallback`）。

use crate::agent::tool::{ToolCall, ToolResult};
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;
use xiezhi_llm::{ChatMessage, ChatRequest, LlmClient, LlmError};

static JSON_TOOL_BLOCK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\{(?:[^{}]|\{[^{}]*\})*?\"tool\"\s*:\s*\"[^\"]+\"(?:[^{}]|\{[^{}]*\})*?\}"#)
        .expect("tool regex")
});

const JSON_PROTOCOL_INSTRUCTION: &str = "\n\nWhen you need to call a tool, emit a single JSON object on its own line, of the form:\n{\"tool\": \"<tool_name>\", \"args\": {<json_args>}, \"id\": \"<unique_id>\"}\nOtherwise respond with plain text. Do NOT wrap JSON in markdown fences.";

#[derive(Debug, Clone)]
pub struct AdapterResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
}

pub struct ToolCallingAdapter<'a> {
    pub client: &'a dyn LlmClient,
}

impl<'a> ToolCallingAdapter<'a> {
    pub fn new(client: &'a dyn LlmClient) -> Self {
        Self { client }
    }

    pub async fn completions_with_tools(
        &self,
        messages: &[ChatMessage],
        _tools: &[crate::agent::tool::ToolSchema],
    ) -> Result<AdapterResponse, LlmError> {
        let mut msgs = messages.to_vec();
        if let Some(first) = msgs.first_mut() {
            if first.role == "system" {
                first.content.push_str(JSON_PROTOCOL_INSTRUCTION);
            } else {
                msgs.insert(
                    0,
                    ChatMessage::system(JSON_PROTOCOL_INSTRUCTION.trim_start()),
                );
            }
        } else {
            msgs.push(ChatMessage::system(JSON_PROTOCOL_INSTRUCTION.trim_start()));
        }

        let text = self
            .client
            .chat(ChatRequest {
                messages: msgs,
                ..Default::default()
            })
            .await?;
        Ok(parse_json_tool_calls(&text))
    }

    pub fn build_assistant_message(&self, resp: &AdapterResponse) -> ChatMessage {
        let mut content = resp.content.clone();
        for tc in &resp.tool_calls {
            if let Some(block) = &tc.raw_block {
                content = content.replace(block, "");
            }
        }
        ChatMessage::assistant(content.trim())
    }

    pub fn build_tool_message(&self, call: &ToolCall, result: &ToolResult) -> ChatMessage {
        let body = if result.success {
            result.output.clone()
        } else {
            format!("ERROR: {}", result.error)
        };
        ChatMessage {
            role: "tool".into(),
            content: format!("tool_call_id={} name={}\n{body}", call.id, call.name),
        }
    }
}

pub fn parse_json_tool_calls(text: &str) -> AdapterResponse {
    let mut tool_calls = Vec::new();
    for m in JSON_TOOL_BLOCK.find_iter(text) {
        let block = m.as_str();
        let Ok(obj) = serde_json::from_str::<Value>(block) else {
            continue;
        };
        let Some(name) = obj.get("tool").and_then(|v| v.as_str()) else {
            continue;
        };
        let args = obj.get("args").cloned().unwrap_or_else(json_object);
        if !args.is_object() {
            continue;
        }
        let id = obj
            .get("id")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format!("call_{}", tool_calls.len()));
        tool_calls.push(ToolCall {
            id,
            name: name.to_string(),
            arguments: args,
            raw_block: Some(block.to_string()),
        });
    }
    let cleaned = JSON_TOOL_BLOCK.replace_all(text, "").trim().to_string();
    AdapterResponse {
        content: cleaned,
        tool_calls,
    }
}

fn json_object() -> Value {
    Value::Object(serde_json::Map::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tool_json_line() {
        let text = "thinking\n{\"tool\":\"read_file\",\"args\":{\"path\":\"a.py\"},\"id\":\"1\"}\n";
        let resp = parse_json_tool_calls(text);
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].name, "read_file");
        assert_eq!(resp.tool_calls[0].arguments["path"], "a.py");
        assert!(!resp.content.contains("read_file"));
    }
}
