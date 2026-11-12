//! 与 Python `biz.review.scoring` 黄金样例对照。
//! 样例由 `rust/tests/fixtures/gen_golden.py` 生成。

use serde::Deserialize;
use xiezhi_core::{score_grade, score_results, unified_score, FileReviewResult, ScrComment};

#[derive(Debug, Deserialize)]
struct Golden {
    scoring_cases: Vec<ScoringCase>,
}

#[derive(Debug, Deserialize)]
struct ScoringCase {
    name: String,
    #[serde(default)]
    score: Option<i32>,
    #[serde(default)]
    grade: Option<String>,
    #[serde(default)]
    score_one: Option<i32>,
    #[serde(default)]
    score_five: Option<i32>,
}

fn comment(
    path: &str,
    severity: &str,
    category: &str,
    start_line: i32,
    filtered: bool,
) -> ScrComment {
    ScrComment {
        path: path.into(),
        content: "x".into(),
        severity: severity.into(),
        category: category.into(),
        start_line,
        end_line: start_line,
        filtered,
        ..Default::default()
    }
}

fn load_golden() -> Golden {
    let raw = include_str!("../../../tests/fixtures/python_golden.json");
    serde_json::from_str(raw).expect("parse python_golden.json")
}

#[test]
fn parity_with_python_scoring_cases() {
    let golden = load_golden();
    for case in &golden.scoring_cases {
        match case.name.as_str() {
            "empty" => {
                assert_eq!(score_results(&[]), case.score.unwrap());
                assert_eq!(score_grade(100), case.grade.as_deref().unwrap());
            }
            "critical_security" => {
                let results = vec![FileReviewResult {
                    path: "a.py".into(),
                    additions: 40,
                    comments: vec![comment("a.py", "critical", "security", 10, false)],
                    ..Default::default()
                }];
                let score = score_results(&results);
                assert_eq!(score, case.score.unwrap(), "critical_security");
                assert_eq!(score_grade(score), case.grade.as_deref().unwrap());
            }
            "low_style" => {
                let results = vec![FileReviewResult {
                    path: "a.py".into(),
                    additions: 80,
                    comments: vec![comment("a.py", "low", "style", 3, false)],
                    ..Default::default()
                }];
                let score = score_results(&results);
                assert_eq!(score, case.score.unwrap(), "low_style");
                assert_eq!(score_grade(score), case.grade.as_deref().unwrap());
            }
            "filtered" => {
                let results = vec![FileReviewResult {
                    path: "a.py".into(),
                    additions: 40,
                    comments: vec![comment("a.py", "high", "bug", 1, true)],
                    ..Default::default()
                }];
                assert_eq!(score_results(&results), case.score.unwrap());
            }
            "spam" => {
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
                assert_eq!(score_results(&one), case.score_one.unwrap(), "spam one");
                assert_eq!(score_results(&many), case.score_five.unwrap(), "spam five");
            }
            "unified" => {
                let comments = vec![comment("a.py", "medium", "bug", 1, false)];
                assert_eq!(unified_score(&comments, 1), case.score.unwrap());
            }
            "multi_file" => {
                let results = vec![
                    FileReviewResult {
                        path: "a.py".into(),
                        additions: 10,
                        comments: vec![comment("a.py", "medium", "bug", 1, false)],
                        ..Default::default()
                    },
                    FileReviewResult {
                        path: "b.py".into(),
                        additions: 100,
                        comments: vec![comment("b.py", "high", "security", 2, false)],
                        ..Default::default()
                    },
                ];
                let score = score_results(&results);
                assert_eq!(score, case.score.unwrap(), "multi_file");
                assert_eq!(score_grade(score), case.grade.as_deref().unwrap());
            }
            "unanchored" => {
                let results = vec![FileReviewResult {
                    path: "a.py".into(),
                    additions: 40,
                    comments: vec![comment("a.py", "high", "bug", 0, false)],
                    ..Default::default()
                }];
                assert_eq!(score_results(&results), case.score.unwrap());
            }
            other => panic!("unexpected golden case: {other}"),
        }
    }
}
