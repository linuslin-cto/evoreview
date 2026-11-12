//! Agentic 叙述审查（对齐 `biz.agent`）：工具循环 + 失败降级 diff_only。

mod adapter;
mod agentic;
mod prompts;
mod registry;
mod repo_syncer;
mod runner;
mod safety;
mod tool;
mod tools;

pub use adapter::{parse_json_tool_calls, ToolCallingAdapter};
pub use agentic::{generate_agentic_narrative, AgenticOptions};
pub use prompts::AgenticPrompts;
pub use registry::ToolRegistry;
pub use repo_syncer::{auth_url, sanitize_key, LocalRepoSyncer, RepoSyncSpec, SyncError};
pub use runner::{AgentRunner, RunnerError};
pub use safety::{is_path_safe, is_sensitive_path};
pub use tool::{ToolCall, ToolResult, ToolSchema};
