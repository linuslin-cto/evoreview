//! 审查流水线入口与 webhook 仓库解析。

use serde_json::Value;
use xiezhi_core::ScrComment;

#[derive(Debug, Clone, Default)]
pub struct ReviewOutcome {
    pub report_text: String,
    pub score: i32,
    pub inline_comments: Vec<ScrComment>,
}

/// 从 webhook 推断 (repo_url, repo_key, ref)；无法推断则全为 None。
pub fn resolve_repo_for_event(
    webhook_data: &Value,
    gitlab_url: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    let object_kind = webhook_data
        .get("object_kind")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if object_kind == "merge_request" {
        let repo = webhook_data.get("project").cloned().unwrap_or(Value::Null);
        let path = repo
            .get("path_with_namespace")
            .or_else(|| repo.get("name"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let url = repo
            .get("git_http_url")
            .or_else(|| repo.get("url"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| {
                if let (Some(path), true) = (path.as_ref(), !gitlab_url.is_empty()) {
                    Some(format!("{}/{}", gitlab_url.trim_end_matches('/'), path))
                } else {
                    None
                }
            });
        let attrs = webhook_data
            .get("object_attributes")
            .cloned()
            .unwrap_or(Value::Null);
        let ref_branch = attrs
            .get("source_branch")
            .or_else(|| attrs.get("ref"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let sha = attrs
            .get("last_commit")
            .and_then(|c| c.get("id"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        if path.is_some() && url.is_some() && (ref_branch.is_some() || sha.is_some()) {
            return (url, path, sha.or(ref_branch));
        }
        return (None, None, None);
    }

    if object_kind == "push" {
        let repo = webhook_data.get("project").cloned().unwrap_or(Value::Null);
        let path = repo
            .get("path_with_namespace")
            .or_else(|| repo.get("name"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let url = repo
            .get("git_http_url")
            .or_else(|| repo.get("url"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| {
                if let (Some(path), true) = (path.as_ref(), !gitlab_url.is_empty()) {
                    Some(format!("{}/{}", gitlab_url.trim_end_matches('/'), path))
                } else {
                    None
                }
            });
        let rref = webhook_data
            .get("after")
            .or_else(|| webhook_data.get("ref"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        if path.is_some() && url.is_some() && rref.is_some() {
            return (url, path, rref);
        }
        return (None, None, None);
    }

    if webhook_data.get("repository").is_some() && webhook_data.get("pull_request").is_some() {
        let repo = &webhook_data["repository"];
        let url = repo
            .get("clone_url")
            .or_else(|| repo.get("html_url"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let path = repo
            .get("full_name")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let pr = &webhook_data["pull_request"];
        let rref = pr
            .pointer("/head/sha")
            .or_else(|| pr.pointer("/head/ref"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        if path.is_some() && url.is_some() && rref.is_some() {
            return (url, path, rref);
        }
        return (None, None, None);
    }

    if webhook_data.get("repository").is_some() && webhook_data.get("ref").is_some() {
        let repo = &webhook_data["repository"];
        let url = repo
            .get("clone_url")
            .or_else(|| repo.get("html_url"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let path = repo
            .get("full_name")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let rref = webhook_data
            .get("after")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| {
                webhook_data
                    .pointer("/head_commit/id")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            });
        if path.is_some() && url.is_some() && rref.is_some() {
            return (url, path, rref);
        }
        return (None, None, None);
    }

    (None, None, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolve_gitlab_mr() {
        let data = json!({
            "object_kind": "merge_request",
            "project": {
                "path_with_namespace": "g/p",
                "git_http_url": "https://git.example/g/p.git"
            },
            "object_attributes": {
                "source_branch": "feat",
                "last_commit": {"id": "abc123"}
            }
        });
        let (url, key, rref) = resolve_repo_for_event(&data, "");
        assert_eq!(url.unwrap(), "https://git.example/g/p.git");
        assert_eq!(key.unwrap(), "g/p");
        assert_eq!(rref.unwrap(), "abc123");
    }

    #[test]
    fn resolve_github_pr() {
        let data = json!({
            "repository": {
                "full_name": "o/r",
                "clone_url": "https://github.com/o/r.git"
            },
            "pull_request": {
                "head": {"sha": "deadbeef", "ref": "feat"}
            }
        });
        let (url, key, rref) = resolve_repo_for_event(&data, "");
        assert_eq!(url.unwrap(), "https://github.com/o/r.git");
        assert_eq!(key.unwrap(), "o/r");
        assert_eq!(rref.unwrap(), "deadbeef");
    }
}
