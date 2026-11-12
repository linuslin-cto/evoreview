//! 报告合成（对齐 Python `biz.review.pipeline` 中的 compose / clean）。

use regex::Regex;
use std::sync::LazyLock;
use xiezhi_core::{score_grade, FileReviewResult, ScrComment, VERSION};

static SCORE_LINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^.*总分[:：]\s*\d+\s*分?.*$").unwrap());

static SCORE_CUT_RES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r"(?m)^\s*#{0,6}\s*\**\s*(?:\d+[.\.、)]\s*)?评分明细.*$").unwrap(),
        Regex::new(r"(?m)^\s*#{0,6}\s*\**\s*(?:\d+[.\.、)]\s*)?总分\s*\**\s*$").unwrap(),
        Regex::new(r"(?m)^\s*评分项\s*$").unwrap(),
        Regex::new(r"(?m)^\s*\|?\s*评分项\s*\|").unwrap(),
    ]
});

static NARRATIVE_NOISE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(好的|好，|作为一名|作为资深|我现在|现在，?我|以下是).{0,80}(审查|报告).{0,20}$|^\s*#{0,6}\s*\**\s*代码审查报告\s*\**\s*$|^\s*问题描述和优化建议\s*$",
    )
    .unwrap()
});

pub fn strip_score_lines(text: &str) -> String {
    SCORE_LINE_RE.replace_all(text, "").trim().to_string()
}

pub fn clean_narrative(text: &str) -> String {
    let mut text = text.to_string();
    let mut cut_at: Option<usize> = None;
    for pat in SCORE_CUT_RES.iter() {
        if let Some(m) = pat.find(&text) {
            cut_at = Some(match cut_at {
                Some(c) => c.min(m.start()),
                None => m.start(),
            });
        }
    }
    if let Some(cut) = cut_at {
        text.truncate(cut);
    }
    text = strip_score_lines(&text);
    let lines: Vec<&str> = text.lines().collect();
    let mut cleaned_head = 0usize;
    for line in lines.iter().take(6) {
        if line.trim().is_empty() || NARRATIVE_NOISE_RE.is_match(line.trim()) {
            cleaned_head += 1;
        } else {
            break;
        }
    }
    lines[cleaned_head..].join("\n").trim().to_string()
}

fn active_comments(results: &[FileReviewResult]) -> Vec<&ScrComment> {
    results
        .iter()
        .flat_map(|r| r.comments.iter())
        .filter(|c| !c.filtered)
        .collect()
}

fn severity_counts(results: &[FileReviewResult]) -> [usize; 4] {
    let mut counts = [0usize; 4];
    for c in active_comments(results) {
        match c.severity.as_str() {
            "critical" => counts[0] += 1,
            "high" => counts[1] += 1,
            "medium" => counts[2] += 1,
            "low" => counts[3] += 1,
            _ => counts[2] += 1,
        }
    }
    counts
}

fn render_findings(results: &[FileReviewResult], heading: &str) -> String {
    let mut lines = Vec::new();
    for r in results {
        let active: Vec<&ScrComment> = r.comments.iter().filter(|c| !c.filtered).collect();
        if active.is_empty() {
            continue;
        }
        lines.push(format!("{heading} `{path}`", path = r.path));
        lines.push(String::new());
        for c in active {
            let loc = if c.start_line > 0 {
                if c.end_line > c.start_line {
                    format!("L{}-L{}", c.start_line, c.end_line)
                } else {
                    format!("L{}", c.start_line)
                }
            } else {
                "未锚定".into()
            };
            lines.push(format!(
                "- {} **{}** / {} · {} — {}",
                c.severity_emoji(),
                c.severity,
                c.category,
                loc,
                c.content.trim()
            ));
        }
        lines.push(String::new());
    }
    lines.join("\n").trim_end().to_string()
}

/// 合成报告：总分 + 总览 + 问题明细。
pub fn compose_report(
    results: &[FileReviewResult],
    score: i32,
    narrative: &str,
    degraded: bool,
) -> String {
    let active = active_comments(results);
    let filtered_count = results
        .iter()
        .flat_map(|r| r.comments.iter())
        .filter(|c| c.filtered)
        .count();
    let counts = severity_counts(results);

    let mut lines: Vec<String> = Vec::new();
    lines.push("## 🛡️ 獬豸 AI 代码审查".into());
    lines.push(String::new());
    lines.push(format!(
        "**总分:{score}分（{grade}）**",
        grade = score_grade(score)
    ));
    lines.push(String::new());

    if degraded {
        lines.push("> ⚠️ 结构化审查层不可用，本次为叙述回退审查（总分解析自叙述层）".into());
        lines.push(String::new());
    } else {
        lines.push(format!(
            "**问题分布**：🔴 critical {c} · 🟠 high {h} · 🟡 medium {m} · 🔵 low {l}",
            c = counts[0],
            h = counts[1],
            m = counts[2],
            l = counts[3]
        ));
        lines.push(String::new());
        let mut summary = format!("> 审查文件 {} · 有效问题 {}", results.len(), active.len());
        if filtered_count > 0 {
            summary.push_str(&format!(" · 已过滤误报 {filtered_count}"));
        }
        lines.push(summary);
        lines.push(String::new());
    }

    let narrative = clean_narrative(narrative);
    if !narrative.is_empty() {
        lines.push("### 🧭 整体总览".into());
        lines.push(String::new());
        lines.push(narrative);
        lines.push(String::new());
    }

    if !degraded {
        if !active.is_empty() {
            lines.push("### 📋 问题明细".into());
            lines.push(String::new());
            lines.push(render_findings(results, "####"));
        } else {
            lines.push("✅ 未发现明显问题，代码质量良好。".into());
        }
    }

    lines.push(String::new());
    lines.push(format!("> xiezhi {VERSION}"));
    lines.join("\n").trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use xiezhi_core::ScrComment;

    #[test]
    fn clean_strips_noise_and_score_block() {
        let raw = "\
好的，以下是审查报告
实际内容在这里
## 评分明细
总分: 88分
";
        let cleaned = clean_narrative(raw);
        assert!(cleaned.contains("实际内容在这里"));
        assert!(!cleaned.contains("评分明细"));
        assert!(!cleaned.contains("总分"));
    }

    #[test]
    fn compose_includes_score_and_findings() {
        let results = vec![FileReviewResult {
            path: "a.py".into(),
            additions: 10,
            comments: vec![ScrComment {
                path: "a.py".into(),
                content: "可能空指针".into(),
                severity: "high".into(),
                category: "bug".into(),
                start_line: 3,
                end_line: 3,
                ..Default::default()
            }],
            ..Default::default()
        }];
        let report = compose_report(&results, 72, "", false);
        assert!(report.contains("总分:72分"));
        assert!(report.contains("可能空指针"));
        assert!(report.contains("xiezhi"));
    }
}
