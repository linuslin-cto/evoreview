//! 审查结果对比指标（无真值时的行为对比）。

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use xiezhi_core::{FileReviewResult, ScrComment};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ContrastMetrics {
    pub files: usize,
    pub findings: usize,
    pub filtered: usize,
    pub anchored: usize,
    pub critical: usize,
    pub high: usize,
    pub duplicates: usize,
    /// 0..=1：有行号锚定的占比
    pub anchored_rate: f64,
    /// 0..=1：重复内容占比
    pub spam_rate: f64,
}

pub fn contrast_metrics(results: &[FileReviewResult]) -> ContrastMetrics {
    let files = results.len();
    let mut findings = 0usize;
    let mut filtered = 0usize;
    let mut anchored = 0usize;
    let mut critical = 0usize;
    let mut high = 0usize;
    let mut seen = HashSet::new();
    let mut duplicates = 0usize;

    for r in results {
        for c in &r.comments {
            if c.filtered {
                filtered += 1;
                continue;
            }
            findings += 1;
            if c.start_line > 0 {
                anchored += 1;
            }
            match c.severity.as_str() {
                "critical" => critical += 1,
                "high" => high += 1,
                _ => {}
            }
            let key = fingerprint(c);
            if !seen.insert(key) {
                duplicates += 1;
            }
        }
    }

    let anchored_rate = if findings == 0 {
        1.0
    } else {
        anchored as f64 / findings as f64
    };
    let spam_rate = if findings == 0 {
        0.0
    } else {
        duplicates as f64 / findings as f64
    };

    ContrastMetrics {
        files,
        findings,
        filtered,
        anchored,
        critical,
        high,
        duplicates,
        anchored_rate,
        spam_rate,
    }
}

fn fingerprint(c: &ScrComment) -> String {
    format!(
        "{}|{}|{}",
        c.path.to_ascii_lowercase(),
        c.severity,
        c.content.trim().chars().take(80).collect::<String>()
    )
}

/// 精度启发式：锚定高、垃圾低、严重度不过度膨胀 → 更高分。
pub fn precision_score(m: &ContrastMetrics) -> f64 {
    let density_penalty = if m.files == 0 {
        0.0
    } else {
        let per_file = m.findings as f64 / m.files as f64;
        // 每文件 > 8 条开始惩罚（洪水式误报）
        ((per_file - 8.0).max(0.0) / 8.0).min(1.0)
    };
    let severity_balance = if m.findings == 0 {
        1.0
    } else {
        let hot = (m.critical + m.high) as f64 / m.findings as f64;
        // 全是 critical 或全是无严重度都不理想；0.15..=0.55 最佳
        if (0.15..=0.55).contains(&hot) {
            1.0
        } else if hot < 0.15 {
            0.7 + hot
        } else {
            (1.2 - hot).clamp(0.3, 1.0)
        }
    };

    let score = m.anchored_rate * 40.0
        + (1.0 - m.spam_rate) * 30.0
        + severity_balance * 20.0
        + (1.0 - density_penalty) * 10.0;
    (score * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use xiezhi_core::ScrComment;

    fn c(path: &str, sev: &str, line: i32, content: &str) -> ScrComment {
        ScrComment {
            path: path.into(),
            content: content.into(),
            severity: sev.into(),
            start_line: line,
            end_line: line,
            filtered: false,
            ..Default::default()
        }
    }

    #[test]
    fn prefers_anchored_over_spam() {
        let good = vec![FileReviewResult {
            path: "a.rs".into(),
            comments: vec![
                c("a.rs", "high", 10, "null deref"),
                c("a.rs", "medium", 20, "unwrap"),
            ],
            ..Default::default()
        }];
        let spam = vec![FileReviewResult {
            path: "a.rs".into(),
            comments: vec![
                c("a.rs", "critical", 0, "bad"),
                c("a.rs", "critical", 0, "bad"),
                c("a.rs", "critical", 0, "bad"),
                c("a.rs", "critical", 0, "bad"),
            ],
            ..Default::default()
        }];
        let g = precision_score(&contrast_metrics(&good));
        let s = precision_score(&contrast_metrics(&spam));
        assert!(g > s, "good={g} spam={s}");
    }
}
