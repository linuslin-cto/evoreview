//! 融合审查流水线：SCR 结构化层 + 叙述总览合成。

pub mod agent;
pub mod compose;
pub mod fusion;
pub mod harness2;
pub mod narrative;
pub mod pipeline;
pub mod structured;

pub use agent::{
    generate_agentic_narrative, AgenticOptions, AgenticPrompts, LocalRepoSyncer, RepoSyncSpec,
};
pub use compose::{clean_narrative, compose_report, strip_score_lines};
pub use fusion::{review_fusion, FusionOptions};
pub use harness2::{
    apply_harness_to_system, contrast_metrics, enabled_from_env, precision_score,
    probe_and_compose, simulate_probe_results, ExecutionHarness, HarnessEdit, ImprovementLibrary,
    Lesson,
};
pub use narrative::{
    generate_narrative, parse_review_score, render_style_template, NarrativePrompts,
};
pub use pipeline::{resolve_repo_for_event, ReviewOutcome};
pub use structured::{
    review_changes_structured, review_changes_to_outcome, review_file_change, StructuredError,
    StructuredOptions,
};
