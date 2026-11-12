//! 与 Python `biz.scr.anchor` 黄金样例对照。

use serde::Deserialize;
use xiezhi_core::ScrComment;
use xiezhi_scr::{anchor_comment, parse_new_line_map};

#[derive(Debug, Deserialize)]
struct Golden {
    anchor: AnchorCase,
}

#[derive(Debug, Deserialize)]
struct AnchorCase {
    diff: String,
    existing_code: String,
    start_line: i32,
    end_line: i32,
    line_map: Vec<(i32, String, bool)>,
}

#[test]
fn parity_with_python_anchor() {
    let raw = include_str!("../../../tests/fixtures/python_golden.json");
    let golden: Golden = serde_json::from_str(raw).unwrap();
    let a = &golden.anchor;

    let map = parse_new_line_map(&a.diff);
    assert_eq!(map, a.line_map);

    let c = ScrComment {
        path: "a.rs".into(),
        content: "issue".into(),
        existing_code: a.existing_code.clone(),
        ..Default::default()
    };
    let anchored = anchor_comment(c, &map);
    assert_eq!(anchored.start_line, a.start_line);
    assert_eq!(anchored.end_line, a.end_line);
}
