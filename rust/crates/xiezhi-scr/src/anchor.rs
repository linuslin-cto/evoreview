//! Unified diff 解析与行级锚定（对齐 Python `biz.scr.anchor`）。

use regex::Regex;
use std::sync::LazyLock;
use xiezhi_core::ScrComment;

static HUNK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@").unwrap());

/// 新文件侧 (line_no, text, is_added)。
pub type LineMapEntry = (i32, String, bool);

pub fn parse_new_line_map(diff_text: &str) -> Vec<LineMapEntry> {
    let mut result = Vec::new();
    let mut new_line = 0i32;
    let mut in_hunk = false;
    for raw in diff_text.lines() {
        if let Some(caps) = HUNK_RE.captures(raw) {
            new_line = caps[1].parse().unwrap_or(0);
            in_hunk = true;
            continue;
        }
        if !in_hunk {
            continue;
        }
        if raw.starts_with("+++") || raw.starts_with("---") {
            continue;
        }
        if let Some(rest) = raw.strip_prefix('+') {
            result.push((new_line, rest.to_string(), true));
            new_line += 1;
        } else if raw.starts_with('-') || raw.starts_with('\\') {
            continue;
        } else {
            let text = raw.strip_prefix(' ').unwrap_or(raw);
            result.push((new_line, text.to_string(), false));
            new_line += 1;
        }
    }
    result
}

fn split_snippet(snippet: &str) -> Vec<String> {
    snippet
        .lines()
        .map(str::trim)
        .filter(|ln| !ln.is_empty())
        .map(str::to_string)
        .collect()
}

/// 用 existing_code 匹配行号。
pub fn anchor_comment(mut comment: ScrComment, line_map: &[LineMapEntry]) -> ScrComment {
    let snippet = split_snippet(&comment.existing_code);
    if snippet.is_empty() || line_map.is_empty() {
        return comment;
    }

    let stripped_map: Vec<(i32, String, bool)> = line_map
        .iter()
        .map(|(ln, text, added)| (*ln, text.trim().to_string(), *added))
        .collect();
    let first = &snippet[0];

    for prefer_added in [true, false] {
        for (idx, (ln_no, text, added)) in stripped_map.iter().enumerate() {
            if *added != prefer_added || text != first {
                continue;
            }
            let mut matched = 1usize;
            let mut j = idx + 1;
            let mut k = 1usize;
            while k < snippet.len() && j < stripped_map.len() {
                if stripped_map[j].1 == snippet[k] {
                    matched += 1;
                    k += 1;
                    j += 1;
                } else {
                    break;
                }
            }
            comment.start_line = *ln_no;
            let end_idx = (idx + matched - 1).min(stripped_map.len() - 1);
            comment.end_line = stripped_map[end_idx].0;
            return comment;
        }
    }

    for (ln_no, text, _) in &stripped_map {
        if !first.is_empty() && text.contains(first) {
            comment.start_line = *ln_no;
            comment.end_line = *ln_no;
            return comment;
        }
    }
    comment
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hunk_added_and_context() {
        let diff = "\
@@ -1,3 +1,4 @@
 context
-old
+new
 more
";
        let map = parse_new_line_map(diff);
        assert_eq!(map.len(), 3);
        assert_eq!(map[0], (1, "context".into(), false));
        assert_eq!(map[1], (2, "new".into(), true));
        assert_eq!(map[2], (3, "more".into(), false));
    }

    #[test]
    fn anchor_prefers_added_lines() {
        let diff = "\
@@ -1,2 +1,3 @@
 keep
+fn bad() {}
 keep2
";
        let map = parse_new_line_map(diff);
        let c = ScrComment {
            path: "a.rs".into(),
            content: "issue".into(),
            existing_code: "fn bad() {}".into(),
            ..Default::default()
        };
        let anchored = anchor_comment(c, &map);
        assert_eq!(anchored.start_line, 2);
        assert_eq!(anchored.end_line, 2);
    }
}
