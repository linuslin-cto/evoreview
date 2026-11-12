//! 本地 Git Diff 采集辅助（对齐 Python `biz.scr.git_diff` 纯逻辑部分）。

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

pub const PROVIDER_NOISE_DIRS: &[&str] = &[
    "vendor/",
    "node_modules/",
    "target/",
    "dist/",
    "build/",
    ".git/",
    "__pycache__/",
    ".venv/",
    "venv/",
];

static DIFF_HEADER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^diff --git a/(.+?) b/(.+)$").unwrap());

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiffPreviewItem {
    pub path: String,
    pub additions: i32,
    pub deletions: i32,
    pub included: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileChange {
    pub new_path: String,
    pub diff: String,
    pub additions: i32,
    pub deletions: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalDiffBundle {
    pub mode: String,
    pub repo_dir: String,
    pub changes: Vec<FileChange>,
    pub preview: Vec<DiffPreviewItem>,
    pub warnings: Vec<String>,
}

/// 决定路径是否纳入审查（对齐 Python `_decide`）。
pub fn decide_path(path: &str, diff: &str, rules: &crate::rules::RuleResolver) -> DiffPreviewItem {
    let (additions, deletions) = if diff.is_empty() {
        (0, 0)
    } else {
        count_diff_lines(diff)
    };
    let mut item = DiffPreviewItem {
        path: path.to_string(),
        additions,
        deletions,
        included: true,
        reason: String::new(),
    };
    if is_noise_path(path) {
        item.included = false;
        item.reason = "noise_dir".into();
        return item;
    }
    if diff.is_empty() {
        item.included = false;
        item.reason = "binary_or_empty".into();
        return item;
    }
    if rules.is_excluded_by_default(path) {
        item.included = false;
        item.reason = "default_path".into();
        return item;
    }
    if !rules.is_supported_path(path) {
        item.included = false;
        item.reason = "unsupported_ext".into();
        return item;
    }
    item
}

pub fn to_changes(
    preview: &[DiffPreviewItem],
    diff_map: &std::collections::BTreeMap<String, String>,
) -> Vec<FileChange> {
    preview
        .iter()
        .filter(|i| i.included)
        .map(|i| FileChange {
            new_path: i.path.clone(),
            diff: diff_map.get(&i.path).cloned().unwrap_or_default(),
            additions: i.additions,
            deletions: i.deletions,
        })
        .collect()
}

pub fn is_noise_path(path: &str) -> bool {
    let norm = path.replace('\\', "/");
    if PROVIDER_NOISE_DIRS.iter().any(|p| norm.contains(p)) {
        return true;
    }
    let padded = format!("/{norm}");
    [
        "vendor",
        "node_modules",
        "target",
        "dist",
        "build",
        "__pycache__",
    ]
    .iter()
    .any(|d| padded.contains(&format!("/{d}/")) || padded.ends_with(&format!("/{d}")))
}

/// Count added/deleted lines; ignores `+++` / `---` headers (no lookaround regex).
pub fn count_diff_lines(diff: &str) -> (i32, i32) {
    let mut additions = 0i32;
    let mut deletions = 0i32;
    for line in diff.lines() {
        if line.starts_with('+') && !line.starts_with("+++") {
            additions += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            deletions += 1;
        }
    }
    (additions, deletions)
}

/// Split multi-file `git diff` output into (path, diff_text) pairs.
pub fn split_unified_diffs(raw: &str) -> Vec<(String, String)> {
    if raw.trim().is_empty() {
        return Vec::new();
    }

    let mut starts: Vec<usize> = Vec::new();
    let bytes = raw.as_bytes();
    let needle = b"diff --git ";
    let mut i = 0usize;
    while i + needle.len() <= bytes.len() {
        if (i == 0 || bytes[i - 1] == b'\n') && bytes[i..].starts_with(needle) {
            starts.push(i);
            i += needle.len();
        } else {
            i += 1;
        }
    }

    let mut results = Vec::new();
    for (idx, start) in starts.iter().enumerate() {
        let end = starts.get(idx + 1).copied().unwrap_or(raw.len());
        let chunk = raw[*start..end].trim_matches('\n');
        if !chunk.starts_with("diff --git ") {
            continue;
        }
        let first = chunk.lines().next().unwrap_or("");
        let Some(caps) = DIFF_HEADER_RE.captures(first) else {
            continue;
        };
        let path = caps[2].to_string();
        let Some(hunk_rel) = chunk.find("\n@@") else {
            if chunk.contains("Binary files") || chunk.contains("GIT binary patch") {
                results.push((path, String::new()));
            }
            continue;
        };
        let meta = chunk.find("\n--- ");
        let diff_body = if let Some(meta) = meta {
            if meta < hunk_rel {
                chunk[meta + 1..].to_string()
            } else {
                chunk[hunk_rel + 1..].to_string()
            }
        } else {
            chunk[hunk_rel + 1..].to_string()
        };
        results.push((path, diff_body));
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_paths_detected() {
        assert!(is_noise_path("web/node_modules/pkg/index.js"));
        assert!(is_noise_path("rust/target/debug/xiezhi"));
        assert!(!is_noise_path("biz/scr/anchor.py"));
    }

    #[test]
    fn count_lines_ignores_headers() {
        let diff = "\
--- a/f
+++ b/f
@@ -1 +1,2 @@
-old
+new
+more
";
        assert_eq!(count_diff_lines(diff), (2, 1));
    }

    #[test]
    fn split_two_files() {
        let raw = "\
diff --git a/a.py b/a.py
--- a/a.py
+++ b/a.py
@@ -1 +1 @@
-old
+new
diff --git a/b.rs b/b.rs
--- a/b.rs
+++ b/b.rs
@@ -1 +1 @@
-x
+y
";
        let parts = split_unified_diffs(raw);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].0, "a.py");
        assert_eq!(parts[1].0, "b.rs");
        assert!(parts[0].1.contains("+new"));
    }
}
