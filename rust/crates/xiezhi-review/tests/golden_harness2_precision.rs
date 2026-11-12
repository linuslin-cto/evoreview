//! Harness² precision 黄金样例：低锚定 / 高 spam 回归夹具。
//! 数据：`rust/tests/fixtures/harness2_precision_golden.json`

use serde::Deserialize;
use std::collections::HashMap;
use xiezhi_core::{FileReviewResult, ScrComment};
use xiezhi_review::{
    contrast_metrics, precision_score, probe_and_compose, simulate_probe_results, ExecutionHarness,
    HarnessEdit,
};

#[derive(Debug, Deserialize)]
struct Golden {
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    name: String,
    #[serde(default)]
    comments: Vec<CommentFix>,
    #[serde(default)]
    expect: Option<ExpectMetrics>,
    #[serde(default)]
    harness: Option<HarnessFix>,
    #[serde(default)]
    expect_after_simulate: Option<ExpectMetrics>,
    #[serde(default)]
    base: Option<String>,
    #[serde(default)]
    candidate: Option<String>,
    #[serde(default)]
    expect_adopted: Option<bool>,
    #[serde(default)]
    expect_candidate_beats_base_by: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct CommentFix {
    path: String,
    severity: String,
    content: String,
    start_line: i32,
    #[serde(default)]
    filtered: bool,
}

#[derive(Debug, Deserialize)]
struct ExpectMetrics {
    #[serde(default)]
    findings: Option<usize>,
    #[serde(default)]
    anchored: Option<usize>,
    #[serde(default)]
    duplicates: Option<usize>,
    #[serde(default)]
    anchored_rate: Option<f64>,
    #[serde(default)]
    spam_rate: Option<f64>,
    #[serde(default)]
    precision_score: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct HarnessFix {
    #[serde(default)]
    precision_mode: bool,
    #[serde(default)]
    min_severity: String,
    #[serde(default)]
    focus_categories: Vec<String>,
}

fn load() -> Golden {
    let raw = include_str!("../../../tests/fixtures/harness2_precision_golden.json");
    serde_json::from_str(raw).expect("parse harness2_precision_golden.json")
}

fn to_results(comments: &[CommentFix]) -> Vec<FileReviewResult> {
    let mut by_path: HashMap<String, FileReviewResult> = HashMap::new();
    for c in comments {
        let entry = by_path
            .entry(c.path.clone())
            .or_insert_with(|| FileReviewResult {
                path: c.path.clone(),
                ..Default::default()
            });
        entry.comments.push(ScrComment {
            path: c.path.clone(),
            content: c.content.clone(),
            severity: c.severity.clone(),
            start_line: c.start_line,
            end_line: c.start_line,
            filtered: c.filtered,
            ..Default::default()
        });
    }
    let mut out: Vec<_> = by_path.into_values().collect();
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

fn assert_metrics(label: &str, results: &[FileReviewResult], expect: &ExpectMetrics) {
    let m = contrast_metrics(results);
    let score = precision_score(&m);
    if let Some(v) = expect.findings {
        assert_eq!(m.findings, v, "{label} findings");
    }
    if let Some(v) = expect.anchored {
        assert_eq!(m.anchored, v, "{label} anchored");
    }
    if let Some(v) = expect.duplicates {
        assert_eq!(m.duplicates, v, "{label} duplicates");
    }
    if let Some(v) = expect.anchored_rate {
        assert!(
            (m.anchored_rate - v).abs() < 1e-9,
            "{label} anchored_rate got={} want={v}",
            m.anchored_rate
        );
    }
    if let Some(v) = expect.spam_rate {
        assert!(
            (m.spam_rate - v).abs() < 1e-9,
            "{label} spam_rate got={} want={v}",
            m.spam_rate
        );
    }
    if let Some(v) = expect.precision_score {
        assert!(
            (score - v).abs() < 1e-9,
            "{label} precision_score got={score} want={v}"
        );
    }
}

#[test]
fn harness2_precision_golden_cases() {
    let golden = load();
    let mut named: HashMap<String, Vec<FileReviewResult>> = HashMap::new();

    for case in &golden.cases {
        if !case.comments.is_empty() {
            let results = to_results(&case.comments);
            if let Some(exp) = &case.expect {
                assert_metrics(&case.name, &results, exp);
            }
            if let (Some(h), Some(exp)) = (&case.harness, &case.expect_after_simulate) {
                let harness = ExecutionHarness {
                    precision_mode: h.precision_mode,
                    min_severity: if h.min_severity.is_empty() {
                        "low".into()
                    } else {
                        h.min_severity.clone()
                    },
                    focus_categories: h.focus_categories.clone(),
                    system_overlay: String::new(),
                };
                let probed = simulate_probe_results(&results, &harness);
                assert_metrics(&format!("{}@simulate", case.name), &probed, exp);
            }
            named.insert(case.name.clone(), results);
        }
    }

    for case in &golden.cases {
        let (Some(base_name), Some(cand_name)) = (&case.base, &case.candidate) else {
            continue;
        };
        let base = named
            .get(base_name)
            .unwrap_or_else(|| panic!("missing base case {base_name}"));
        let cand = named
            .get(cand_name)
            .unwrap_or_else(|| panic!("missing candidate case {cand_name}"));
        let base_s = precision_score(&contrast_metrics(base));
        let cand_s = precision_score(&contrast_metrics(cand));
        if let Some(delta) = case.expect_candidate_beats_base_by {
            assert!(
                cand_s >= base_s + delta,
                "{}: cand={cand_s} base={base_s} need +{delta}",
                case.name
            );
        }
        let edit = HarnessEdit {
            rationale: "golden".into(),
            system_overlay_append: "precision".into(),
            precision_mode: Some(true),
            min_severity: Some("medium".into()),
            focus_categories: None,
        };
        let ppc = probe_and_compose(&ExecutionHarness::base(), &edit, base, cand, None);
        if let Some(want) = case.expect_adopted {
            assert_eq!(ppc.adopted, want, "{} adopted", case.name);
        }
    }
}
