//! SCR 结构化审查解析与报告辅助（对齐 `biz.scr.reviewer` 纯逻辑部分）。

use crate::anchor::{anchor_comment, parse_new_line_map};
use crate::parse::{extract_json, extract_json_array};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use xiezhi_core::{score_results, FileReviewResult, ScrComment};

/// 把 LLM 原始文本解析为某文件的评论列表（含规范化）。
pub fn comments_from_llm_payload(path: &str, raw: &str) -> Vec<ScrComment> {
    let payload = extract_json(raw);
    let Some(arr) = payload.get("comments").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for raw_comment in arr {
        let Some(obj) = raw_comment.as_object() else {
            continue;
        };
        let content = obj
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if content.is_empty() {
            continue;
        }
        let comment = ScrComment {
            path: path.to_string(),
            content,
            existing_code: obj
                .get("existing_code")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string(),
            suggestion_code: obj
                .get("suggestion_code")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string(),
            start_line: obj.get("start_line").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            end_line: obj.get("end_line").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            category: obj
                .get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("other")
                .to_string(),
            severity: obj
                .get("severity")
                .and_then(|v| v.as_str())
                .unwrap_or("medium")
                .to_string(),
            filtered: false,
        }
        .normalized();
        out.push(comment);
    }
    out
}

/// 用 diff 锚定评论行号（对齐 `resolve_line_numbers`）。
pub fn resolve_line_numbers(comments: &mut [ScrComment], diff_text: &str) {
    let line_map = parse_new_line_map(diff_text);
    for comment in comments.iter_mut() {
        if !comment.existing_code.is_empty() {
            *comment = anchor_comment(comment.clone(), &line_map);
        }
    }
}

/// 根据过滤模型返回的 id 列表标记误报（`c-0` / `c-1` ...）。
pub fn apply_filter_ids(comments: &mut [ScrComment], filter_raw: &str) {
    let bad: HashSet<String> = extract_json_array(filter_raw)
        .into_iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    for (idx, c) in comments.iter_mut().enumerate() {
        if bad.contains(&format!("c-{idx}")) {
            c.filtered = true;
        }
    }
}

pub fn active_comments(results: &[FileReviewResult]) -> Vec<&ScrComment> {
    results
        .iter()
        .flat_map(|r| r.comments.iter())
        .filter(|c| !c.filtered)
        .collect()
}

pub fn severity_counts(results: &[FileReviewResult]) -> HashMap<&'static str, usize> {
    let mut counts = HashMap::from([("critical", 0usize), ("high", 0), ("medium", 0), ("low", 0)]);
    for c in active_comments(results) {
        let key = match c.severity.as_str() {
            "critical" => "critical",
            "high" => "high",
            "medium" => "medium",
            "low" => "low",
            _ => "medium",
        };
        *counts.entry(key).or_default() += 1;
    }
    counts
}

pub fn score_from_results(results: &[FileReviewResult]) -> i32 {
    score_results(results)
}

pub fn render_findings(results: &[FileReviewResult], heading: &str) -> String {
    let severity_order = |s: &str| match s {
        "critical" => 0,
        "high" => 1,
        "medium" => 2,
        "low" => 3,
        _ => 9,
    };
    let mut lines = Vec::new();
    for result in results {
        let mut file_comments: Vec<&ScrComment> =
            result.comments.iter().filter(|c| !c.filtered).collect();
        if file_comments.is_empty() && result.warning.is_empty() {
            continue;
        }
        lines.push(format!("{heading} 📄 `{path}`", path = result.path));
        if !result.warning.is_empty() {
            lines.push(format!("> ⚠️ {warning}", warning = result.warning));
        }
        file_comments.sort_by_key(|c| severity_order(&c.severity));
        for c in file_comments {
            let mut loc = if c.start_line > 0 {
                format!("L{}", c.start_line)
            } else {
                "未锚定".into()
            };
            if c.end_line > c.start_line && c.start_line > 0 {
                loc = format!("L{}-L{}", c.start_line, c.end_line);
            }
            lines.push(format!(
                "- {} **[{}/{}]** ({loc}) {}",
                c.severity_emoji(),
                c.category,
                c.severity,
                c.content
            ));
            if !c.suggestion_code.is_empty() {
                lines.push(String::new());
                lines.push("  建议：".into());
                lines.push(String::new());
                lines.push("  ```".into());
                for sline in c.suggestion_code.lines() {
                    lines.push(format!("  {sline}"));
                }
                lines.push("  ```".into());
            }
        }
        lines.push(String::new());
    }
    lines.join("\n").trim_end().to_string()
}

/// 从单文件 change + LLM 原文组装 `FileReviewResult`（不含真实 LLM 调用）。
pub fn file_result_from_llm(
    path: &str,
    diff: &str,
    additions: i32,
    deletions: i32,
    llm_raw: &str,
    filter_raw: Option<&str>,
) -> FileReviewResult {
    let mut result = FileReviewResult {
        path: path.to_string(),
        additions,
        deletions,
        ..Default::default()
    };
    if diff.trim().is_empty() {
        return result;
    }
    let mut comments = comments_from_llm_payload(path, llm_raw);
    resolve_line_numbers(&mut comments, diff);
    if let Some(filter_raw) = filter_raw {
        apply_filter_ids(&mut comments, filter_raw);
    }
    result.comments = comments;
    result
}

/// 构建过滤 prompt 用的 comments 块（便于后续接 LLM）。
pub fn format_filter_comment_block(comments: &[ScrComment]) -> String {
    comments
        .iter()
        .enumerate()
        .map(|(idx, c)| {
            format!(
                "- id: c-{idx}\n  comment: {}\n  code: {}",
                c.content, c.existing_code
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn parse_change_item(item: &Value) -> (String, String, i32, i32) {
    let path = item
        .get("new_path")
        .or_else(|| item.get("path"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let diff = item
        .get("diff")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let mut additions = item.get("additions").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let mut deletions = item.get("deletions").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    if !diff.is_empty() && additions == 0 && deletions == 0 {
        let (a, d) = crate::git_diff::count_diff_lines(&diff);
        additions = a;
        deletions = d;
    }
    (path, diff, additions, deletions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_comments_normalize_and_anchor() {
        let raw = r#"
```json
{
  "comments": [
    {
      "content": "可能空指针",
      "existing_code": "fn bad() {}",
      "category": "BUG",
      "severity": "HIGH",
      "suggestion_code": "fn bad() -> Option<()> {}"
    },
    { "content": "  " }
  ]
}
```"#;
        let diff = "\
@@ -1,2 +1,3 @@
 keep
+fn bad() {}
 keep2
";
        let mut comments = comments_from_llm_payload("a.rs", raw);
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].category, "bug");
        assert_eq!(comments[0].severity, "high");
        resolve_line_numbers(&mut comments, diff);
        assert_eq!(comments[0].start_line, 2);
    }

    #[test]
    fn filter_marks_ids() {
        let mut comments = vec![
            ScrComment::new("a.py", "one"),
            ScrComment::new("a.py", "two"),
            ScrComment::new("a.py", "three"),
        ];
        apply_filter_ids(&mut comments, r#"["c-1"]"#);
        assert!(!comments[0].filtered);
        assert!(comments[1].filtered);
        assert!(!comments[2].filtered);
    }

    #[test]
    fn file_result_end_to_end_scoring() {
        let llm = r#"{"comments":[{"content":"sec","existing_code":"x = 1","category":"security","severity":"critical"}]}"#;
        let diff = "@@ -1 +1,2 @@\n keep\n+x = 1\n";
        let result = file_result_from_llm("a.py", diff, 1, 0, llm, Some(r#"[]"#));
        assert_eq!(result.comments.len(), 1);
        assert!(result.comments[0].start_line > 0);
        let score = score_from_results(&[result]);
        assert!((20..=59).contains(&score), "score={score}");
    }

    #[test]
    fn render_includes_suggestion() {
        let results = vec![FileReviewResult {
            path: "a.py".into(),
            comments: vec![ScrComment {
                path: "a.py".into(),
                content: "issue".into(),
                suggestion_code: "fixed".into(),
                severity: "medium".into(),
                category: "bug".into(),
                start_line: 1,
                end_line: 1,
                ..Default::default()
            }],
            ..Default::default()
        }];
        let text = render_findings(&results, "##");
        assert!(text.contains("issue"));
        assert!(text.contains("建议"));
        assert!(text.contains("fixed"));
    }
}
