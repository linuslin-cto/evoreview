//! Harness² 启发的自进化审查脚手架（Propose–Probe–Compose）。
//!
//! 模型权重不变；进化对象是执行 harness（提示词叠加、精度偏置、课程库）。
//! 无真值标注时用对比指标（锚定率 / 垃圾率 / 严重度校准）决定是否采纳编辑。

mod contrast;
mod library;
mod recurse;

#[cfg(test)]
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub use contrast::{contrast_metrics, precision_score, ContrastMetrics};
pub use library::{ImprovementLibrary, Lesson};
pub use recurse::{
    apply_harness_to_system, compose_harness, dual_probe_from_env, enabled_from_env, merge_edit,
    merge_partial_results, parse_edit_json, prefer_file_result, probe_and_compose, propose_edits,
    run_harness2_step, simulate_probe_results, ExecutionHarness, Harness2Outcome, HarnessEdit,
    PartialMerge, ProposeProbeCompose,
};
