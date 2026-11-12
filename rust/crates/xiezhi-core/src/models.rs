//! SCR 审查评论数据模型（对齐 Python `biz.scr.models`）。

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::LazyLock;

pub static VALID_CATEGORIES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "bug",
        "security",
        "performance",
        "maintainability",
        "test",
        "style",
        "documentation",
        "other",
    ])
});

pub static VALID_SEVERITIES: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| HashSet::from(["critical", "high", "medium", "low"]));

pub static SEVERITY_PENALTY: LazyLock<std::collections::HashMap<&'static str, i32>> =
    LazyLock::new(|| {
        [("critical", 25), ("high", 12), ("medium", 5), ("low", 2)]
            .into_iter()
            .collect()
    });

pub static SEVERITY_EMOJI: LazyLock<std::collections::HashMap<&'static str, &'static str>> =
    LazyLock::new(|| {
        [
            ("critical", "🔴"),
            ("high", "🟠"),
            ("medium", "🟡"),
            ("low", "🔵"),
        ]
        .into_iter()
        .collect()
    });

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScrComment {
    pub path: String,
    pub content: String,
    #[serde(default)]
    pub existing_code: String,
    #[serde(default)]
    pub suggestion_code: String,
    #[serde(default)]
    pub start_line: i32,
    #[serde(default)]
    pub end_line: i32,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default = "default_severity")]
    pub severity: String,
    #[serde(default)]
    pub filtered: bool,
}

fn default_category() -> String {
    "other".into()
}

fn default_severity() -> String {
    "medium".into()
}

impl Default for ScrComment {
    fn default() -> Self {
        Self {
            path: String::new(),
            content: String::new(),
            existing_code: String::new(),
            suggestion_code: String::new(),
            start_line: 0,
            end_line: 0,
            category: default_category(),
            severity: default_severity(),
            filtered: false,
        }
    }
}

impl ScrComment {
    pub fn new(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            content: content.into(),
            ..Default::default()
        }
    }

    /// 规范化 category / severity / 行号（对齐 Python `normalized`）。
    pub fn normalized(mut self) -> Self {
        let cat = self.category.trim().to_ascii_lowercase();
        let sev = self.severity.trim().to_ascii_lowercase();
        self.category = if VALID_CATEGORIES.contains(cat.as_str()) {
            cat
        } else {
            "other".into()
        };
        self.severity = if VALID_SEVERITIES.contains(sev.as_str()) {
            sev
        } else {
            "medium".into()
        };
        if self.end_line < self.start_line {
            self.end_line = self.start_line;
        }
        self
    }

    pub fn severity_emoji(&self) -> &'static str {
        SEVERITY_EMOJI
            .get(self.severity.as_str())
            .copied()
            .unwrap_or("🔵")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FileReviewResult {
    pub path: String,
    #[serde(default)]
    pub comments: Vec<ScrComment>,
    #[serde(default)]
    pub warning: String,
    #[serde(default)]
    pub additions: i32,
    #[serde(default)]
    pub deletions: i32,
}

impl FileReviewResult {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_fixes_category_severity_and_lines() {
        let c = ScrComment {
            path: "a.py".into(),
            content: "x".into(),
            category: "UNKNOWN".into(),
            severity: "ultra".into(),
            start_line: 10,
            end_line: 5,
            ..Default::default()
        }
        .normalized();
        assert_eq!(c.category, "other");
        assert_eq!(c.severity, "medium");
        assert_eq!(c.end_line, 10);
    }

    #[test]
    fn severity_emoji_maps() {
        let c = ScrComment {
            severity: "critical".into(),
            ..Default::default()
        };
        assert_eq!(c.severity_emoji(), "🔴");
    }
}
