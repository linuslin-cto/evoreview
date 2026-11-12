//! SCR：diff 解析、行级锚定、规则解析、本地采集、结构化审查解析。

pub mod anchor;
pub mod collect;
pub mod git_diff;
pub mod parse;
pub mod prompts;
pub mod reviewer;
pub mod rules;

pub use anchor::{anchor_comment, parse_new_line_map, LineMapEntry};
pub use collect::{collect_local_diffs, CollectError};
pub use git_diff::{
    count_diff_lines, decide_path, is_noise_path, split_unified_diffs, DiffPreviewItem, FileChange,
    LocalDiffBundle,
};
pub use parse::{extract_json, extract_json_array};
pub use prompts::{PromptError, ScrPrompts};
pub use reviewer::{
    active_comments, apply_filter_ids, comments_from_llm_payload, file_result_from_llm,
    format_filter_comment_block, parse_change_item, render_findings, resolve_line_numbers,
    score_from_results, severity_counts,
};
pub use rules::RuleResolver;
