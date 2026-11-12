//! 工具注册与分发。

use crate::agent::tool::{ToolCall, ToolResult, ToolSchema};
use crate::agent::tools::{ReadFileTool, RunCommandTool};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait AgentTool: Send + Sync {
    fn schema(&self) -> ToolSchema;
    async fn execute(&self, args: &serde_json::Value) -> ToolResult;
}

#[derive(Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn AgentTool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_defaults(repo_root: impl AsRef<Path>) -> Self {
        let mut reg = Self::new();
        reg.register(Arc::new(ReadFileTool::new(repo_root.as_ref())));
        reg.register(Arc::new(RunCommandTool::new(repo_root.as_ref())));
        reg
    }

    pub fn register(&mut self, tool: Arc<dyn AgentTool>) {
        let name = tool.schema().name;
        self.tools.insert(name, tool);
    }

    pub fn list_schemas(&self) -> Vec<ToolSchema> {
        self.tools.values().map(|t| t.schema()).collect()
    }

    pub async fn dispatch(&self, call: &ToolCall) -> ToolResult {
        match self.tools.get(&call.name) {
            Some(tool) => tool.execute(&call.arguments).await,
            None => ToolResult::err(format!("unknown tool: {}", call.name)),
        }
    }
}
