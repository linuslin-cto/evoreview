//! Task-level Propose–Probe–Compose。

use super::contrast::{contrast_metrics, precision_score, ContrastMetrics};
use super::library::{ImprovementLibrary, Lesson};
use serde::{Deserialize, Serialize};
use tracing::info;
use xiezhi_core::FileReviewResult;
use xiezhi_llm::{ChatMessage, ChatRequest, LlmClient};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ExecutionHarness {
    /// 追加到 SCR system prompt 的精度/风格约束
    pub system_overlay: String,
    /// 要求更少但更准的发现
    pub precision_mode: bool,
    /// 最低 severity（低于此的倾向过滤提示）
    pub min_severity: String,
    /// 聚焦类别提示，如 security, correctness
    pub focus_categories: Vec<String>,
}

impl ExecutionHarness {
    pub fn base() -> Self {
        Self {
            system_overlay: String::new(),
            precision_mode: false,
            min_severity: "low".into(),
            focus_categories: Vec::new(),
        }
    }

    pub fn with_library_hints(mut self, library: &ImprovementLibrary) -> Self {
        let hints = library.render_hints(5);
        if !hints.is_empty() {
            if !self.system_overlay.is_empty() {
                self.system_overlay.push('\n');
            }
            self.system_overlay.push_str(&hints);
        }
        self
    }

    pub fn summary(&self) -> String {
        format!(
            "precision={} min_sev={} focus={:?} overlay_chars={}",
            self.precision_mode,
            self.min_severity,
            self.focus_categories,
            self.system_overlay.chars().count()
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HarnessEdit {
    pub rationale: String,
    pub system_overlay_append: String,
    pub precision_mode: Option<bool>,
    pub min_severity: Option<String>,
    pub focus_categories: Option<Vec<String>>,
}

impl HarnessEdit {
    pub fn apply(&self, base: &ExecutionHarness) -> ExecutionHarness {
        let mut h = base.clone();
        if !self.system_overlay_append.trim().is_empty() {
            if !h.system_overlay.is_empty() {
                h.system_overlay.push('\n');
            }
            h.system_overlay.push_str(self.system_overlay_append.trim());
        }
        if let Some(p) = self.precision_mode {
            h.precision_mode = p;
        }
        if let Some(s) = &self.min_severity {
            h.min_severity = s.clone();
        }
        if let Some(cats) = &self.focus_categories {
            h.focus_categories = cats.clone();
        }
        h
    }
}

pub fn apply_harness_to_system(base_system: &str, harness: &ExecutionHarness) -> String {
    let mut out = base_system.to_string();
    if harness.precision_mode {
        out.push_str(
            "\n\n## Precision mode\n优先报告高置信、可定位的问题；避免空泛建议与重复吐槽；每条必须引用具体 diff 行。\n",
        );
    }
    if harness.min_severity != "low" {
        out.push_str(&format!(
            "\n## Severity floor\n忽略低于 `{}` 的琐碎问题。\n",
            harness.min_severity
        ));
    }
    if !harness.focus_categories.is_empty() {
        out.push_str(&format!(
            "\n## Focus\n优先关注类别：{}。\n",
            harness.focus_categories.join(", ")
        ));
    }
    if !harness.system_overlay.trim().is_empty() {
        out.push('\n');
        out.push_str(harness.system_overlay.trim());
        out.push('\n');
    }
    out
}

pub fn enabled_from_env() -> bool {
    std::env::var("HARNESS2_ENABLED").unwrap_or_else(|_| "0".into()) == "1"
}

/// 成本门控：为候选 harness 再跑一轮真实 SCR（默认关闭，用 simulate_probe）。
pub fn dual_probe_from_env() -> bool {
    std::env::var("HARNESS2_DUAL_PROBE").unwrap_or_else(|_| "0".into()) == "1"
}

/// 规则启发的提案（无 LLM 也可运行）；可选 LLM 精炼。
pub fn propose_edits_heuristic(base_metrics: &ContrastMetrics) -> HarnessEdit {
    let mut append = String::new();
    let mut precision = None;
    let mut min_sev = None;
    let mut focus = None;
    let mut rationale = String::new();

    if base_metrics.anchored_rate < 0.5 && base_metrics.findings > 0 {
        append.push_str("每条评论必须包含可锚定的 existing_code，并给出准确 start_line。\n");
        rationale.push_str("锚定率偏低；");
    }
    if base_metrics.spam_rate > 0.25 {
        append.push_str("禁止重复相同内容；合并同类问题。\n");
        precision = Some(true);
        rationale.push_str("重复率偏高；");
    }
    if base_metrics.findings > base_metrics.files.saturating_mul(8).max(8) {
        append.push_str("控制发现数量：优先 critical/high，砍掉样式类噪音。\n");
        precision = Some(true);
        min_sev = Some("medium".into());
        rationale.push_str("发现过密；");
    }
    if base_metrics.critical == 0 && base_metrics.high == 0 && base_metrics.findings > 3 {
        focus = Some(vec!["security".into(), "correctness".into()]);
        rationale.push_str("缺少高严重度信号；");
    }
    if rationale.is_empty() {
        rationale = "基线已较好，尝试轻量精度约束".into();
        precision = Some(true);
        append.push_str("保持简洁：只报告真实缺陷与安全风险。\n");
    }

    HarnessEdit {
        rationale,
        system_overlay_append: append,
        precision_mode: precision,
        min_severity: min_sev,
        focus_categories: focus,
    }
}

pub async fn propose_edits(
    client: Option<&dyn LlmClient>,
    base_metrics: &ContrastMetrics,
) -> HarnessEdit {
    let heuristic = propose_edits_heuristic(base_metrics);
    let Some(client) = client else {
        return heuristic;
    };
    if std::env::var("HARNESS2_PROPOSE_LLM").unwrap_or_else(|_| "0".into()) != "1" {
        return heuristic;
    }

    let user = format!(
        "你是代码审查 harness 优化器。当前审查指标 JSON：\n{}\n\n\
         请提出一小段 system overlay（中文），目标是提高精度、减少误报。\n\
         只返回 JSON：{{\"rationale\":\"...\",\"system_overlay_append\":\"...\",\"precision_mode\":true,\"min_severity\":\"medium\",\"focus_categories\":[\"security\"]}}",
        serde_json::to_string(base_metrics).unwrap_or_default()
    );
    match client
        .chat(ChatRequest {
            messages: vec![
                ChatMessage::system("Return only valid JSON for harness edit."),
                ChatMessage::user(user),
            ],
            ..Default::default()
        })
        .await
    {
        Ok(raw) => parse_edit_json(&raw)
            .map(|llm| merge_edit(&heuristic, llm))
            .unwrap_or(heuristic),
        Err(_) => heuristic,
    }
}

const MAX_OVERLAY_CHARS: usize = 800;

/// 从模型原始输出中稳健提取 HarnessEdit（fence / 噪声 / 别名 / 类型宽松）。
pub fn parse_edit_json(raw: &str) -> Option<HarnessEdit> {
    let cleaned = strip_code_fence(raw.trim());
    let json = extract_json_object(&cleaned)?;
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let obj = value.as_object()?;

    let rationale = string_field(obj, &["rationale", "reason", "why"]).unwrap_or_default();
    let append = string_field(
        obj,
        &[
            "system_overlay_append",
            "systemOverlayAppend",
            "overlay",
            "system_overlay",
        ],
    )
    .unwrap_or_default();
    let precision = bool_field(obj, &["precision_mode", "precisionMode", "precision"]);
    let min_sev = string_field(obj, &["min_severity", "minSeverity", "severity_floor"])
        .map(|s| normalize_severity(&s));
    let focus = string_list_field(
        obj,
        &["focus_categories", "focusCategories", "focus", "categories"],
    );

    // 至少要有可执行信号，否则视为解析失败
    if rationale.trim().is_empty()
        && append.trim().is_empty()
        && precision.is_none()
        && min_sev.is_none()
        && focus.as_ref().map(|v| v.is_empty()).unwrap_or(true)
    {
        return None;
    }

    Some(HarnessEdit {
        rationale: rationale.chars().take(240).collect(),
        system_overlay_append: append.chars().take(MAX_OVERLAY_CHARS).collect(),
        precision_mode: precision,
        min_severity: min_sev,
        focus_categories: focus,
    })
}

fn strip_code_fence(s: &str) -> String {
    let t = s.trim();
    if !t.starts_with("```") {
        return t.to_string();
    }
    let mut lines = t.lines();
    let _ = lines.next(); // opening ```
    let mut body: Vec<&str> = lines.collect();
    if body
        .last()
        .map(|l| l.trim().starts_with("```"))
        .unwrap_or(false)
    {
        body.pop();
    }
    body.join("\n")
}

fn extract_json_object(s: &str) -> Option<&str> {
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(&s[start..=end])
}

fn string_field(obj: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some(v) = obj.get(*k) {
            match v {
                serde_json::Value::String(s) if !s.trim().is_empty() => return Some(s.clone()),
                serde_json::Value::Number(n) => return Some(n.to_string()),
                serde_json::Value::Bool(b) => return Some(b.to_string()),
                _ => {}
            }
        }
    }
    None
}

fn bool_field(obj: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> Option<bool> {
    for k in keys {
        if let Some(v) = obj.get(*k) {
            match v {
                serde_json::Value::Bool(b) => return Some(*b),
                serde_json::Value::String(s) => {
                    let l = s.trim().to_ascii_lowercase();
                    if matches!(l.as_str(), "1" | "true" | "yes" | "on") {
                        return Some(true);
                    }
                    if matches!(l.as_str(), "0" | "false" | "no" | "off") {
                        return Some(false);
                    }
                }
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        return Some(i != 0);
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn string_list_field(
    obj: &serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<Vec<String>> {
    for k in keys {
        let Some(v) = obj.get(*k) else { continue };
        match v {
            serde_json::Value::Array(arr) => {
                let list: Vec<String> = arr
                    .iter()
                    .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
                    .filter(|s| !s.is_empty())
                    .take(8)
                    .collect();
                return Some(list);
            }
            serde_json::Value::String(s) if !s.trim().is_empty() => {
                let list: Vec<String> = s
                    .split([',', ';', '|'])
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .take(8)
                    .collect();
                return Some(list);
            }
            _ => {}
        }
    }
    None
}

fn normalize_severity(s: &str) -> String {
    match s.trim().to_ascii_lowercase().as_str() {
        "critical" | "crit" | "blocker" => "critical".into(),
        "high" | "error" | "major" => "high".into(),
        "medium" | "med" | "warning" | "warn" => "medium".into(),
        "low" | "info" | "minor" | "nit" => "low".into(),
        other if !other.is_empty() => "medium".into(),
        _ => "low".into(),
    }
}

/// LLM 提案与启发式合并：空字段回退启发式，避免半残 JSON 冲掉有效约束。
pub fn merge_edit(heuristic: &HarnessEdit, llm: HarnessEdit) -> HarnessEdit {
    HarnessEdit {
        rationale: if llm.rationale.trim().is_empty() {
            heuristic.rationale.clone()
        } else {
            llm.rationale
        },
        system_overlay_append: if llm.system_overlay_append.trim().is_empty() {
            heuristic.system_overlay_append.clone()
        } else {
            // 启发式约束保留，LLM overlay 追加其后
            let mut combined = heuristic.system_overlay_append.clone();
            if !combined.is_empty() && !combined.ends_with('\n') {
                combined.push('\n');
            }
            combined.push_str(llm.system_overlay_append.trim());
            combined.chars().take(MAX_OVERLAY_CHARS).collect()
        },
        precision_mode: llm.precision_mode.or(heuristic.precision_mode),
        min_severity: llm.min_severity.or_else(|| heuristic.min_severity.clone()),
        focus_categories: match llm.focus_categories {
            Some(v) if !v.is_empty() => Some(v),
            _ => heuristic.focus_categories.clone(),
        },
    }
}

#[derive(Debug, Clone)]
pub struct ProposeProbeCompose {
    pub base: ExecutionHarness,
    pub candidate: ExecutionHarness,
    pub edit: HarnessEdit,
    pub base_metrics: ContrastMetrics,
    pub candidate_metrics: ContrastMetrics,
    pub base_score: f64,
    pub candidate_score: f64,
    /// 是否至少采纳了部分文件（或全局全量采纳）
    pub adopted: bool,
    /// 从候选侧换入的文件数
    pub files_adopted: usize,
    /// Compose 后的合并结果（按文件部分采纳）
    pub merged_results: Vec<FileReviewResult>,
}

#[derive(Debug, Clone)]
pub struct PartialMerge {
    pub results: Vec<FileReviewResult>,
    pub files_adopted: usize,
    pub files_kept: usize,
}

pub fn compose_harness(
    base: &ExecutionHarness,
    edit: &HarnessEdit,
    base_metrics: &ContrastMetrics,
    candidate_metrics: &ContrastMetrics,
) -> (ExecutionHarness, bool, f64, f64) {
    let candidate = edit.apply(base);
    let base_score = precision_score(base_metrics);
    let candidate_score = precision_score(candidate_metrics);
    // 需有实质增益才采纳（避免抖动）
    let adopted = candidate_score >= base_score + 0.5;
    let chosen = if adopted { candidate } else { base.clone() };
    (chosen, adopted, base_score, candidate_score)
}

/// 单文件：候选更准 / 降 spam 且锚定不掉 / 同密度下类别更聚焦 → 换候选。
pub fn prefer_file_result(
    base: &FileReviewResult,
    cand: &FileReviewResult,
) -> (FileReviewResult, bool) {
    let bm = contrast_metrics(std::slice::from_ref(base));
    let cm = contrast_metrics(std::slice::from_ref(cand));
    let bs = precision_score(&bm);
    let cs = precision_score(&cm);
    if cs >= bs + 0.5 {
        return (cand.clone(), true);
    }
    // 降 spam，锚定不明显变差
    if cm.spam_rate + 0.1 < bm.spam_rate && cm.anchored_rate + 0.05 >= bm.anchored_rate {
        return (cand.clone(), true);
    }
    // 密度：候选明显更稀且仍有锚定发现
    if bm.findings > 0
        && cm.findings > 0
        && cm.findings + 2 <= bm.findings
        && cm.anchored_rate >= bm.anchored_rate
        && cm.anchored > 0
    {
        return (cand.clone(), true);
    }
    // 类别：候选 critical/high 占比更健康（不过度膨胀）
    if bm.findings >= 3 && cm.findings > 0 {
        let base_hot = (bm.critical + bm.high) as f64 / bm.findings as f64;
        let cand_hot = (cm.critical + cm.high) as f64 / cm.findings as f64;
        if base_hot > 0.85 && cand_hot <= 0.7 && cm.anchored_rate >= bm.anchored_rate {
            return (cand.clone(), true);
        }
    }
    (base.clone(), false)
}

/// 按路径对齐，逐文件决定保留基线或换入候选（非全有全无）。
pub fn merge_partial_results(
    base_results: &[FileReviewResult],
    candidate_results: &[FileReviewResult],
) -> PartialMerge {
    use std::collections::HashMap;
    let mut cand_map: HashMap<&str, &FileReviewResult> = HashMap::new();
    for r in candidate_results {
        cand_map.insert(r.path.as_str(), r);
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(base_results.len().max(candidate_results.len()));
    let mut files_adopted = 0usize;
    let mut files_kept = 0usize;

    for base in base_results {
        seen.insert(base.path.as_str());
        if let Some(cand) = cand_map.get(base.path.as_str()) {
            let (picked, adopted) = prefer_file_result(base, cand);
            if adopted {
                files_adopted += 1;
            } else {
                files_kept += 1;
            }
            out.push(picked);
        } else {
            files_kept += 1;
            out.push(base.clone());
        }
    }
    for cand in candidate_results {
        if seen.insert(cand.path.as_str()) {
            // 仅候选有的文件：若有有效发现则纳入
            let m = contrast_metrics(std::slice::from_ref(cand));
            if m.findings > 0 {
                files_adopted += 1;
                out.push(cand.clone());
            }
        }
    }

    PartialMerge {
        results: out,
        files_adopted,
        files_kept,
    }
}

/// 给定基线与候选审查结果，完成 Probe–Compose，并可选写入 library。
pub fn probe_and_compose(
    base_harness: &ExecutionHarness,
    edit: &HarnessEdit,
    base_results: &[FileReviewResult],
    candidate_results: &[FileReviewResult],
    library: Option<&mut ImprovementLibrary>,
) -> ProposeProbeCompose {
    let base_metrics = contrast_metrics(base_results);
    let candidate_metrics = contrast_metrics(candidate_results);
    let (chosen, global_adopted, base_score, candidate_score) =
        compose_harness(base_harness, edit, &base_metrics, &candidate_metrics);
    let candidate = edit.apply(base_harness);

    let partial = if global_adopted {
        PartialMerge {
            results: candidate_results.to_vec(),
            files_adopted: candidate_results.len(),
            files_kept: 0,
        }
    } else {
        merge_partial_results(base_results, candidate_results)
    };
    let adopted = global_adopted || partial.files_adopted > 0;
    let files_adopted = partial.files_adopted;
    let merged_results = partial.results;
    let merged_metrics = contrast_metrics(&merged_results);
    let merged_score = precision_score(&merged_metrics);

    if adopted {
        if let Some(lib) = library {
            let lesson = Lesson {
                id: format!(
                    "p{}-a{}-s{}",
                    (base_metrics.spam_rate * 100.0) as u32,
                    (base_metrics.anchored_rate * 100.0) as u32,
                    edit.min_severity.clone().unwrap_or_default()
                ),
                when: edit.rationale.clone(),
                change: edit.system_overlay_append.chars().take(120).collect(),
                why: format!(
                    "precision {base_score} → {candidate_score} (merged {merged_score}; files={files_adopted})"
                ),
                evidence: format!(
                    "anchor {:.2}→{:.2} spam {:.2}→{:.2} partial={}",
                    base_metrics.anchored_rate,
                    candidate_metrics.anchored_rate,
                    base_metrics.spam_rate,
                    candidate_metrics.spam_rate,
                    !global_adopted
                ),
                adopted_count: 1,
            };
            lib.record_adoption(chosen.summary(), lesson);
        }
    }

    info!(
        adopted,
        global_adopted,
        files_adopted,
        base_score,
        candidate_score,
        merged_score,
        "harness2: compose done"
    );

    ProposeProbeCompose {
        base: base_harness.clone(),
        candidate,
        edit: edit.clone(),
        base_metrics,
        candidate_metrics,
        base_score,
        candidate_score,
        adopted,
        files_adopted,
        merged_results,
    }
}

#[derive(Debug, Clone)]
pub struct Harness2Outcome {
    pub harness: ExecutionHarness,
    pub ppc: ProposeProbeCompose,
}

/// 用 harness 规则对审查结果做「探针」后处理（无需二次 LLM）。
pub fn simulate_probe_results(
    results: &[FileReviewResult],
    harness: &ExecutionHarness,
) -> Vec<FileReviewResult> {
    let sev_rank = |s: &str| match s {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    };
    let floor = sev_rank(&harness.min_severity);
    let mut out = Vec::with_capacity(results.len());
    for r in results {
        let mut nr = r.clone();
        let mut seen = std::collections::HashSet::new();
        for c in &mut nr.comments {
            if c.filtered {
                continue;
            }
            if sev_rank(&c.severity) < floor {
                c.filtered = true;
                continue;
            }
            if harness.precision_mode && c.start_line <= 0 {
                c.filtered = true;
                continue;
            }
            if harness.precision_mode && !harness.focus_categories.is_empty() {
                let content_l = c.content.to_ascii_lowercase();
                let focused = harness.focus_categories.iter().any(|f| {
                    c.category.eq_ignore_ascii_case(f)
                        || content_l.contains(&f.to_ascii_lowercase())
                });
                if !focused {
                    c.filtered = true;
                    continue;
                }
            }
            let snippet: String = c.content.trim().chars().take(80).collect();
            let fp = format!("{}|{snippet}", c.severity);
            if !seen.insert(fp) {
                c.filtered = true;
            }
        }
        out.push(nr);
    }
    out
}

/// 单步 Harness²：启发式 Propose + 外部提供的 probe 结果 Compose。
pub async fn run_harness2_step(
    client: Option<&dyn LlmClient>,
    base_harness: ExecutionHarness,
    base_results: &[FileReviewResult],
    candidate_results: &[FileReviewResult],
    mut library: ImprovementLibrary,
) -> Harness2Outcome {
    let base_metrics = contrast_metrics(base_results);
    let edit = propose_edits(client, &base_metrics).await;
    let ppc = probe_and_compose(
        &base_harness,
        &edit,
        base_results,
        candidate_results,
        Some(&mut library),
    );
    let harness = if ppc.adopted {
        ppc.candidate.clone()
    } else {
        base_harness
    };
    Harness2Outcome { harness, ppc }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xiezhi_core::{FileReviewResult, ScrComment};

    fn rc(line: i32, sev: &str, content: &str) -> ScrComment {
        ScrComment {
            path: "f.rs".into(),
            content: content.into(),
            severity: sev.into(),
            start_line: line,
            end_line: line,
            filtered: false,
            ..Default::default()
        }
    }

    #[test]
    fn compose_adopts_better_candidate() {
        let base_h = ExecutionHarness::base();
        let edit = HarnessEdit {
            rationale: "improve anchor".into(),
            system_overlay_append: "锚定行号".into(),
            precision_mode: Some(true),
            min_severity: Some("medium".into()),
            focus_categories: None,
        };
        let base_results = vec![FileReviewResult {
            path: "f.rs".into(),
            comments: vec![
                rc(0, "critical", "x"),
                rc(0, "critical", "x"),
                rc(0, "critical", "x"),
            ],
            ..Default::default()
        }];
        let cand_results = vec![FileReviewResult {
            path: "f.rs".into(),
            comments: vec![
                rc(12, "high", "null check"),
                rc(20, "medium", "error handle"),
            ],
            ..Default::default()
        }];
        let ppc = probe_and_compose(&base_h, &edit, &base_results, &cand_results, None);
        assert!(
            ppc.adopted,
            "base={} cand={}",
            ppc.base_score, ppc.candidate_score
        );
        assert!(ppc.candidate.precision_mode);
        assert!(!ppc.merged_results.is_empty());
    }

    #[test]
    fn partial_merge_swaps_only_noisy_file() {
        let base = vec![
            FileReviewResult {
                path: "good.rs".into(),
                comments: vec![rc(10, "high", "null check")],
                ..Default::default()
            },
            FileReviewResult {
                path: "spam.rs".into(),
                comments: vec![
                    rc(0, "critical", "bad"),
                    rc(0, "critical", "bad"),
                    rc(0, "critical", "bad"),
                    rc(0, "critical", "bad"),
                ],
                ..Default::default()
            },
        ];
        let cand = vec![
            FileReviewResult {
                path: "good.rs".into(),
                // 候选反而更差（无锚定）——应保留基线 good.rs
                comments: vec![rc(0, "low", "nit")],
                ..Default::default()
            },
            FileReviewResult {
                path: "spam.rs".into(),
                comments: vec![rc(12, "high", "real bug")],
                ..Default::default()
            },
        ];
        let m = merge_partial_results(&base, &cand);
        assert_eq!(m.files_adopted, 1, "only spam.rs should swap");
        assert_eq!(m.files_kept, 1);
        let good = m.results.iter().find(|r| r.path == "good.rs").unwrap();
        assert_eq!(good.comments[0].content, "null check");
        let spam = m.results.iter().find(|r| r.path == "spam.rs").unwrap();
        assert_eq!(spam.comments[0].content, "real bug");
    }

    #[test]
    fn overlay_appended_to_system() {
        let h = ExecutionHarness {
            system_overlay: "自定义课".into(),
            precision_mode: true,
            min_severity: "medium".into(),
            focus_categories: vec!["security".into()],
        };
        let s = apply_harness_to_system("BASE", &h);
        assert!(s.contains("BASE"));
        assert!(s.contains("Precision mode"));
        assert!(s.contains("security"));
        assert!(s.contains("自定义课"));
    }

    #[test]
    fn dual_probe_env_flag() {
        let _g = crate::harness2::ENV_LOCK.lock().unwrap();
        let prev = std::env::var("HARNESS2_DUAL_PROBE").ok();
        std::env::remove_var("HARNESS2_DUAL_PROBE");
        assert!(!dual_probe_from_env());
        std::env::set_var("HARNESS2_DUAL_PROBE", "1");
        assert!(dual_probe_from_env());
        match prev {
            Some(v) => std::env::set_var("HARNESS2_DUAL_PROBE", v),
            None => std::env::remove_var("HARNESS2_DUAL_PROBE"),
        }
    }

    #[tokio::test]
    async fn heuristic_propose_on_spam() {
        let m = ContrastMetrics {
            files: 1,
            findings: 4,
            spam_rate: 0.5,
            anchored_rate: 0.25,
            ..Default::default()
        };
        let e = propose_edits(None, &m).await;
        assert!(e.precision_mode.unwrap_or(false) || !e.system_overlay_append.is_empty());
    }

    #[test]
    fn parse_edit_json_from_fenced_aliases() {
        let raw = r#"这里是说明：
```json
{
  "reason": "spam",
  "overlay": "合并重复评论",
  "precisionMode": "true",
  "minSeverity": "WARN",
  "focus": "security, correctness"
}
```
"#;
        let e = parse_edit_json(raw).expect("parse");
        assert_eq!(e.rationale, "spam");
        assert!(e.system_overlay_append.contains("合并"));
        assert_eq!(e.precision_mode, Some(true));
        assert_eq!(e.min_severity.as_deref(), Some("medium"));
        assert_eq!(e.focus_categories.as_ref().map(|v| v.len()), Some(2));
    }

    #[test]
    fn parse_edit_json_rejects_empty() {
        assert!(parse_edit_json("{}").is_none());
        assert!(parse_edit_json("not json").is_none());
    }

    #[test]
    fn merge_edit_keeps_heuristic_when_llm_partial() {
        let h = HarnessEdit {
            rationale: "启发式".into(),
            system_overlay_append: "锚定要求\n".into(),
            precision_mode: Some(true),
            min_severity: Some("medium".into()),
            focus_categories: Some(vec!["security".into()]),
        };
        let llm = HarnessEdit {
            rationale: String::new(),
            system_overlay_append: "LLM 补充".into(),
            precision_mode: None,
            min_severity: None,
            focus_categories: Some(vec![]),
        };
        let m = merge_edit(&h, llm);
        assert_eq!(m.rationale, "启发式");
        assert!(m.system_overlay_append.contains("锚定要求"));
        assert!(m.system_overlay_append.contains("LLM 补充"));
        assert_eq!(m.precision_mode, Some(true));
        assert_eq!(m.min_severity.as_deref(), Some("medium"));
        assert_eq!(m.focus_categories.as_ref().unwrap()[0], "security");
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn propose_edits_uses_llm_when_enabled() {
        let _g = crate::harness2::ENV_LOCK.lock().unwrap();
        std::env::set_var("HARNESS2_PROPOSE_LLM", "1");
        let client = xiezhi_llm::ScriptedClient::new(
            "scripted",
            [r#"```json
{"rationale":"llm-ok","system_overlay_append":"只报安全问题","precision_mode":true,"min_severity":"high","focus_categories":["security"]}
```"#
            .into()],
        );
        let m = ContrastMetrics {
            files: 1,
            findings: 4,
            spam_rate: 0.5,
            anchored_rate: 0.2,
            ..Default::default()
        };
        let e = propose_edits(Some(&client), &m).await;
        assert!(e.rationale.contains("llm-ok") || e.system_overlay_append.contains("安全"));
        assert_eq!(e.precision_mode, Some(true));
        assert_eq!(e.min_severity.as_deref(), Some("high"));
        std::env::remove_var("HARNESS2_PROPOSE_LLM");
    }
}
