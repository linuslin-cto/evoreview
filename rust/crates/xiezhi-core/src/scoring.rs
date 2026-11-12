//! 统一评分（严重级别定档 + 问题密度定位），对齐 Python `biz.review.scoring`。

use crate::models::{FileReviewResult, ScrComment, SEVERITY_PENALTY};
use std::collections::HashMap;

const DENSITY_FACTOR: f64 = 3.0;
const SPAM_DECAY: f64 = 0.7;
const ANCHOR_DISCOUNT: f64 = 0.8;

const UNIT_LINES: f64 = 40.0;
const UNIT_MIN: f64 = 0.5;
const UNIT_MAX: f64 = 3.0;

const BAND_EVIDENCE_MIN: f64 = 1.0;
const ANCHOR_BAND_WEIGHT: f64 = 0.5;

fn category_factor(cat: &str) -> f64 {
    match cat {
        "security" => 1.3,
        "bug" => 1.15,
        "performance" | "maintainability" | "other" => 1.0,
        "test" => 0.9,
        "style" | "documentation" => 0.7,
        _ => 1.0,
    }
}

fn severity_band(sev: &str) -> (i32, i32) {
    match sev {
        "critical" => (20, 59),
        "high" => (55, 79),
        "medium" => (70, 92),
        "low" => (85, 98),
        _ => (70, 92),
    }
}

const SEVERITY_ORDER: &[&str] = &["critical", "high", "medium", "low"];

fn norm_severity(sev: &str) -> &str {
    if matches!(sev, "critical" | "high" | "medium" | "low") {
        sev
    } else {
        "medium"
    }
}

fn band_severity(active: &[&ScrComment]) -> String {
    let mut evidence: HashMap<&str, f64> = HashMap::new();
    for c in active {
        let weight = if c.start_line > 0 {
            1.0
        } else {
            ANCHOR_BAND_WEIGHT
        };
        *evidence.entry(norm_severity(&c.severity)).or_default() += weight;
    }
    for (i, sev) in SEVERITY_ORDER.iter().enumerate() {
        let ev = evidence.get(sev).copied().unwrap_or(0.0);
        if ev <= 0.0 {
            continue;
        }
        if ev >= BAND_EVIDENCE_MIN || i == SEVERITY_ORDER.len() - 1 {
            return (*sev).to_string();
        }
        return SEVERITY_ORDER[i + 1].to_string();
    }
    "medium".into()
}

fn effective_penalty(active: &[&ScrComment]) -> f64 {
    let mut seen: HashMap<(String, String), i32> = HashMap::new();
    let mut total = 0.0;
    for c in active {
        let sev = norm_severity(&c.severity).to_string();
        let key = (c.path.clone(), sev.clone());
        let k = *seen.get(&key).unwrap_or(&0);
        *seen.entry(key).or_default() += 1;
        let mut weight = SPAM_DECAY.powi(k);
        weight *= category_factor(&c.category);
        if c.start_line <= 0 {
            weight *= ANCHOR_DISCOUNT;
        }
        let base = SEVERITY_PENALTY
            .get(c.severity.as_str())
            .copied()
            .unwrap_or(5) as f64;
        total += base * weight;
    }
    total
}

fn effective_units(results: &[FileReviewResult]) -> f64 {
    let mut units = 0.0;
    for r in results {
        let additions = r.additions as f64;
        units += (additions / UNIT_LINES).clamp(UNIT_MIN, UNIT_MAX);
    }
    units.max(1.0)
}

fn banded_score(active: &[&ScrComment], units: f64) -> i32 {
    if active.is_empty() {
        return 100;
    }
    let density = effective_penalty(active) / units.max(1.0);
    let density_score = 100.0 * (-density * DENSITY_FACTOR / 100.0).exp();
    let (band_low, band_high) = severity_band(&band_severity(active));
    let score =
        (band_low as f64 + (band_high - band_low) as f64 * density_score / 100.0).round() as i32;
    score.clamp(0, 100)
}

/// 由 FileReviewResult 列表计算总分（0-100）。
pub fn score_results(results: &[FileReviewResult]) -> i32 {
    let active: Vec<&ScrComment> = results
        .iter()
        .flat_map(|r| r.comments.iter())
        .filter(|c| !c.filtered)
        .collect();
    banded_score(&active, effective_units(results))
}

/// 仅有评论与文件数时计算总分（1 文件 = 1 单元）。
pub fn unified_score(comments: &[ScrComment], files_reviewed: usize) -> i32 {
    let active: Vec<&ScrComment> = comments.iter().filter(|c| !c.filtered).collect();
    banded_score(&active, (files_reviewed.max(1)) as f64)
}

pub fn score_grade(score: i32) -> &'static str {
    if score >= 90 {
        "优秀"
    } else if score >= 80 {
        "良好"
    } else if score >= 60 {
        "及格"
    } else {
        "需改进"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{FileReviewResult, ScrComment};

    fn comment(path: &str, sev: &str, cat: &str, start: i32, filtered: bool) -> ScrComment {
        ScrComment {
            path: path.into(),
            content: "issue".into(),
            severity: sev.into(),
            category: cat.into(),
            start_line: start,
            end_line: start,
            filtered,
            ..Default::default()
        }
    }

    #[test]
    fn empty_results_score_100() {
        assert_eq!(score_results(&[]), 100);
        assert_eq!(score_grade(100), "优秀");
    }

    #[test]
    fn filtered_comments_ignored() {
        let results = vec![FileReviewResult {
            path: "a.py".into(),
            additions: 40,
            comments: vec![comment("a.py", "high", "bug", 1, true)],
            ..Default::default()
        }];
        assert_eq!(score_results(&results), 100);
    }

    #[test]
    fn critical_anchors_into_critical_band() {
        let results = vec![FileReviewResult {
            path: "a.py".into(),
            additions: 40,
            comments: vec![comment("a.py", "critical", "security", 10, false)],
            ..Default::default()
        }];
        let score = score_results(&results);
        assert!((20..=59).contains(&score), "score={score}");
        assert_eq!(score_grade(score), "需改进");
    }

    #[test]
    fn low_severity_stays_high_band() {
        let results = vec![FileReviewResult {
            path: "a.py".into(),
            additions: 80,
            comments: vec![comment("a.py", "low", "style", 3, false)],
            ..Default::default()
        }];
        let score = score_results(&results);
        assert!((85..=98).contains(&score), "score={score}");
    }

    #[test]
    fn unified_score_matches_single_unit() {
        let comments = vec![comment("a.py", "medium", "bug", 1, false)];
        let s = unified_score(&comments, 1);
        assert!((70..=92).contains(&s), "score={s}");
    }

    #[test]
    fn spam_decay_reduces_repeated_same_file_severity() {
        let one = vec![FileReviewResult {
            path: "a.py".into(),
            additions: 40,
            comments: vec![comment("a.py", "high", "bug", 1, false)],
            ..Default::default()
        }];
        let many = vec![FileReviewResult {
            path: "a.py".into(),
            additions: 40,
            comments: (0..5)
                .map(|i| comment("a.py", "high", "bug", i + 1, false))
                .collect(),
            ..Default::default()
        }];
        let s1 = score_results(&one);
        let s5 = score_results(&many);
        // more issues should not score higher
        assert!(s5 <= s1, "s1={s1} s5={s5}");
    }
}
