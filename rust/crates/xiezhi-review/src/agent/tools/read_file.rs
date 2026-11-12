//! read_file 工具。

use crate::agent::registry::AgentTool;
use crate::agent::safety::{is_path_safe, is_sensitive_path};
use crate::agent::tool::{ToolResult, ToolSchema};
use serde_json::json;
use std::path::{Path, PathBuf};

const BINARY_SAMPLE: usize = 8192;
const BINARY_NUL_THRESHOLD: f64 = 0.10;

pub struct ReadFileTool {
    repo_root: PathBuf,
}

impl ReadFileTool {
    pub fn new(repo_root: impl AsRef<Path>) -> Self {
        Self {
            repo_root: repo_root.as_ref().to_path_buf(),
        }
    }
}

#[async_trait::async_trait]
impl AgentTool for ReadFileTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "read_file".into(),
            description: "Read a file from the repository with optional line paging.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "offset": {"type": "integer", "default": 0},
                    "limit": {"type": "integer", "default": 500}
                },
                "required": ["path"]
            }),
        }
    }

    async fn execute(&self, args: &serde_json::Value) -> ToolResult {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if path.is_empty() {
            return ToolResult::err("missing path");
        }
        let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(500)
            .clamp(1, 5000) as usize;

        let candidate = self.repo_root.join(path);
        if !is_path_safe(&candidate, &self.repo_root) {
            return ToolResult::err("path outside repo root");
        }
        if is_sensitive_path(&candidate) {
            return ToolResult::err("path is sensitive and cannot be read");
        }
        if !candidate.is_file() {
            return ToolResult::err(format!("file not found: {path}"));
        }

        let sample = match std::fs::read(&candidate) {
            Ok(bytes) => {
                let take = bytes.len().min(BINARY_SAMPLE);
                bytes[..take].to_vec()
            }
            Err(e) => return ToolResult::err(format!("read error: {e}")),
        };
        if !sample.is_empty() {
            let nul = sample.iter().filter(|b| **b == 0).count() as f64 / sample.len() as f64;
            if nul > BINARY_NUL_THRESHOLD {
                return ToolResult::err("binary file, not readable");
            }
        }

        let text = match std::fs::read_to_string(&candidate) {
            Ok(t) => t,
            Err(e) => return ToolResult::err(format!("read error: {e}")),
        };
        let lines: Vec<&str> = text.lines().collect();
        let start = offset.min(lines.len());
        let end = (start + limit).min(lines.len());
        let body = lines[start..end].join("\n");
        ToolResult::ok(format!("lines {}-{}:\n{body}", start + 1, end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reads_relative_file() {
        let dir = std::env::temp_dir().join(format!(
            "xiezhi-read-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.py"), "one\ntwo\nthree\n").unwrap();
        let tool = ReadFileTool::new(&dir);
        let out = tool
            .execute(&json!({"path": "a.py", "offset": 1, "limit": 1}))
            .await;
        assert!(out.success, "{}", out.error);
        assert!(out.output.contains("two"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
