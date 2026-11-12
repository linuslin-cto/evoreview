//! 多轮 Agent 循环（对齐 `biz.agent.runner.AgentRunner`）。

use crate::agent::adapter::{AdapterResponse, ToolCallingAdapter};
use crate::agent::registry::ToolRegistry;
use thiserror::Error;
use tracing::info;
use xiezhi_llm::ChatMessage;

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("token budget exceeded: {0}")]
    TokenBudget(String),
    #[error("invalid tool call streak")]
    EmptyStreak,
    #[error(transparent)]
    Llm(#[from] xiezhi_llm::LlmError),
}

pub struct AgentRunner<'a> {
    pub adapter: ToolCallingAdapter<'a>,
    pub registry: &'a ToolRegistry,
    pub max_iterations: usize,
    pub total_token_cap: usize,
    pub tool_output_max_chars: usize,
}

impl<'a> AgentRunner<'a> {
    pub fn new(adapter: ToolCallingAdapter<'a>, registry: &'a ToolRegistry) -> Self {
        let max_iterations = std::env::var("AGENT_MAX_ITERATIONS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(20);
        let total_token_cap = std::env::var("AGENT_TOTAL_TOKEN_CAP")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(80_000);
        let tool_output_max_tokens = std::env::var("AGENT_TOOL_OUTPUT_MAX_TOKENS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10_000usize);
        Self {
            adapter,
            registry,
            max_iterations,
            total_token_cap,
            tool_output_max_chars: tool_output_max_tokens * 4,
        }
    }

    pub async fn run(&self, initial: Vec<ChatMessage>) -> Result<(String, usize), RunnerError> {
        let mut messages = initial;
        let mut last_assistant = String::new();
        let mut empty_streak = 0usize;
        let mut iterations = 0usize;

        for i in 0..self.max_iterations {
            iterations = i + 1;
            let schemas = self.registry.list_schemas();
            let resp: AdapterResponse = self
                .adapter
                .completions_with_tools(&messages, &schemas)
                .await?;
            info!(
                round = i,
                tool_calls = resp.tool_calls.len(),
                content_len = resp.content.len(),
                "agent round"
            );

            if !resp.content.trim().is_empty() {
                last_assistant = resp.content.clone();
            }

            if resp.tool_calls.is_empty() {
                if resp.content.trim().is_empty() {
                    empty_streak += 1;
                    if empty_streak >= 3 {
                        return Err(RunnerError::EmptyStreak);
                    }
                    continue;
                }
                return Ok((resp.content, iterations));
            }

            empty_streak = 0;
            messages.push(self.adapter.build_assistant_message(&resp));
            for call in &resp.tool_calls {
                let mut result = self.registry.dispatch(call).await;
                if result.success && result.output.chars().count() > self.tool_output_max_chars {
                    let truncated: String = result
                        .output
                        .chars()
                        .take(self.tool_output_max_chars)
                        .collect();
                    result.output = format!("{truncated}\n[output truncated]");
                }
                messages.push(self.adapter.build_tool_message(call, &result));
                let usage = estimate_tokens(&messages);
                if usage > self.total_token_cap {
                    return Err(RunnerError::TokenBudget(format!(
                        "token estimate {usage} exceeds cap {}",
                        self.total_token_cap
                    )));
                }
            }
        }

        Ok((
            if last_assistant.is_empty() {
                format!(
                    "max iterations ({}) reached without final response",
                    self.max_iterations
                )
            } else {
                last_assistant
            },
            iterations,
        ))
    }
}

fn estimate_tokens(messages: &[ChatMessage]) -> usize {
    messages
        .iter()
        .map(|m| m.content.chars().count().div_ceil(4))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::adapter::ToolCallingAdapter;
    use crate::agent::registry::ToolRegistry;
    use xiezhi_llm::ScriptedClient;

    #[tokio::test]
    async fn finishes_without_tools() {
        let client = ScriptedClient::new("s", ["最终审查意见\n总分:88分".into()]);
        let registry = ToolRegistry::new();
        let adapter = ToolCallingAdapter::new(&client);
        let runner = AgentRunner {
            adapter,
            registry: &registry,
            max_iterations: 5,
            total_token_cap: 80_000,
            tool_output_max_chars: 40_000,
        };
        let (text, iters) = runner
            .run(vec![
                ChatMessage::system("sys"),
                ChatMessage::user("review"),
            ])
            .await
            .unwrap();
        assert!(text.contains("总分:88分"));
        assert_eq!(iters, 1);
    }

    #[tokio::test]
    async fn executes_tool_then_finishes() {
        let dir = std::env::temp_dir().join(format!(
            "xiezhi-agent-run-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("x.py"), "print(1)\n").unwrap();

        let client = ScriptedClient::new(
            "s",
            [
                "{\"tool\":\"read_file\",\"args\":{\"path\":\"x.py\"},\"id\":\"1\"}".into(),
                "看过文件了\n总分:90分".into(),
            ],
        );
        let registry = ToolRegistry::with_defaults(&dir);
        let adapter = ToolCallingAdapter::new(&client);
        let runner = AgentRunner {
            adapter,
            registry: &registry,
            max_iterations: 5,
            total_token_cap: 80_000,
            tool_output_max_chars: 40_000,
        };
        let (text, iters) = runner.run(vec![ChatMessage::user("go")]).await.unwrap();
        assert!(text.contains("总分:90分"));
        assert!(iters >= 2);
        let _ = std::fs::remove_dir_all(dir);
    }
}
