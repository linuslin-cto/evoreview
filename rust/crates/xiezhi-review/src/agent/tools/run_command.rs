//! run_command 沙箱工具（只读 allowlist）。

use crate::agent::registry::AgentTool;
use crate::agent::safety::{is_path_safe, is_sensitive_path};
use crate::agent::tool::{ToolResult, ToolSchema};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const DEFAULT_ALLOWLIST: &[&str] = &[
    "ls", "cat", "head", "tail", "find", "rg", "grep", "wc", "tree", "file", "stat", "diff", "git",
    "echo", "pwd", "true", "false", "dir", "where", "type",
];

pub const DEFAULT_BLOCKLIST: &[&str] = &[
    "rm", "mv", "cp", "chmod", "chown", "sudo", "curl", "wget", "ssh", "scp", "rsync", "dd",
    "mkfs", "mount", "umount", "kubectl", "docker", "podman", "npm", "yarn", "pnpm", "pip", "pip3",
    "bash", "sh", "zsh", "fish", "python", "python3", "ruby", "perl", "node",
];

const SAFE_GIT_SUBCOMMANDS: &[&str] = &[
    "log",
    "show",
    "diff",
    "blame",
    "ls-files",
    "ls-tree",
    "status",
    "--version",
];

pub struct RunCommandTool {
    repo_root: PathBuf,
    allowlist: Vec<String>,
    blocklist: Vec<String>,
    default_timeout: u64,
}

impl RunCommandTool {
    pub fn new(repo_root: impl AsRef<Path>) -> Self {
        let allow = std::env::var("AGENT_SHELL_ALLOWLIST")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(|s| {
                s.split(',')
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty())
                    .collect()
            })
            .unwrap_or_else(|| DEFAULT_ALLOWLIST.iter().map(|s| (*s).to_string()).collect());
        let block = std::env::var("AGENT_SHELL_BLOCKLIST")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(|s| {
                s.split(',')
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty())
                    .collect()
            })
            .unwrap_or_else(|| DEFAULT_BLOCKLIST.iter().map(|s| (*s).to_string()).collect());
        Self {
            repo_root: repo_root.as_ref().to_path_buf(),
            allowlist: allow,
            blocklist: block,
            default_timeout: 30,
        }
    }
}

fn tokenize(cmd: &str) -> Result<Vec<String>, String> {
    // Minimal shell-ish split: whitespace, supports "double quotes".
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    for ch in cmd.chars() {
        match ch {
            '"' => in_quote = !in_quote,
            c if c.is_whitespace() && !in_quote => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if in_quote {
        return Err("unclosed quote".into());
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    if out.is_empty() {
        return Err("no tokens parsed".into());
    }
    Ok(out)
}

fn truncate(text: &str, limit: usize) -> String {
    let t = text.trim();
    if t.len() <= limit {
        t.to_string()
    } else {
        format!("{}...", &t[..limit])
    }
}

#[async_trait::async_trait]
impl AgentTool for RunCommandTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "run_command".into(),
            description: "Run a sandboxed read-only command in the repository root.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "cmd": {"type": "string"},
                    "timeout": {"type": "integer", "default": 30}
                },
                "required": ["cmd"]
            }),
        }
    }

    async fn execute(&self, args: &serde_json::Value) -> ToolResult {
        let cmd = args
            .get("cmd")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if cmd.is_empty() {
            return ToolResult::err("empty command");
        }
        if cmd.contains("../") || cmd.contains("..\\") {
            return ToolResult::err("path outside repo: command rejected");
        }

        let tokens = match tokenize(cmd) {
            Ok(t) => t,
            Err(e) => return ToolResult::err(format!("parse error: {e}")),
        };

        for token in &tokens {
            if token.contains('/') || token.contains('\\') || token.starts_with('.') {
                let p = if Path::new(token).is_absolute() {
                    PathBuf::from(token)
                } else {
                    self.repo_root.join(token)
                };
                if is_sensitive_path(&p) {
                    return ToolResult::err(format!("sensitive path: {token}"));
                }
                if Path::new(token).is_absolute() && !is_path_safe(&p, &self.repo_root) {
                    return ToolResult::err(format!("path outside repo: {token}"));
                }
            }
        }

        let head = tokens[0].as_str();
        if self.blocklist.iter().any(|b| b == head) {
            return ToolResult::err(format!("command blocked by sandbox: {head}"));
        }
        if !self.allowlist.iter().any(|a| a == head) {
            return ToolResult::err(format!("command not allowed: {head}"));
        }
        if head == "git" {
            let sub = tokens.get(1).map(String::as_str).unwrap_or("");
            if !SAFE_GIT_SUBCOMMANDS.contains(&sub) {
                return ToolResult::err(format!("git subcommand not allowed: {sub}"));
            }
        }

        let timeout = args
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.default_timeout)
            .clamp(1, 300);

        let mut command = tokio::process::Command::new(&tokens[0]);
        command
            .args(&tokens[1..])
            .current_dir(&self.repo_root)
            .kill_on_drop(true)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let child = match command.spawn() {
            Ok(c) => c,
            Err(e) => return ToolResult::err(format!("execution error: {e}")),
        };
        let output = match tokio::time::timeout(
            Duration::from_secs(timeout),
            child.wait_with_output(),
        )
        .await
        {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => return ToolResult::err(format!("execution error: {e}")),
            Err(_) => return ToolResult::err(format!("command timed out after {timeout}s")),
        };
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            let msg = truncate(&err, 2000);
            return ToolResult::err(if msg.is_empty() {
                format!("exit code {}", output.status.code().unwrap_or(-1))
            } else {
                msg
            });
        }
        ToolResult::ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn blocks_rm() {
        let dir = std::env::temp_dir();
        let tool = RunCommandTool::new(&dir);
        let out = tool.execute(&json!({"cmd": "rm -rf /"})).await;
        assert!(!out.success);
        assert!(out.error.contains("blocked") || out.error.contains("not allowed"));
    }

    #[tokio::test]
    async fn allows_git_version() {
        let dir = std::env::temp_dir();
        let tool = RunCommandTool::new(&dir);
        let out = tool.execute(&json!({"cmd": "git --version"})).await;
        // git may or may not be installed in CI; allow either success or spawn error.
        if out.success {
            assert!(out.output.to_lowercase().contains("git"));
        }
    }
}
