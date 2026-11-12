# Harness² 自进化审查

模型权重不变；进化对象是**执行 harness**（system overlay、精度偏置、课程库）。  
流程：`Propose → Probe → Compose`，无真值时用对比指标决定是否采纳。

## 开关

| 环境变量 | 默认 | 含义 |
|---|---|---|
| `HARNESS2_ENABLED` | `0` | `1` 时在 `review_fusion` 后跑 PPC；structured 注入 library hints |
| `HARNESS2_LIBRARY_PATH` | `data/harness2_library.json` | Improvement library 路径 |
| `HARNESS2_PROPOSE_LLM` | `0` | `1` 时用 LLM 精炼提案（否则纯启发式） |
| `HARNESS2_DUAL_PROBE` | `0` | `1` 时对候选 harness 再跑一轮 SCR（失败回退 simulate） |

`HARNESS2_PROPOSE_LLM=1` 时：模型输出经 `parse_edit_json`（去 fence、字段别名、severity 归一）再与启发式 `merge_edit`，半残 JSON 不会冲掉有效约束。

## 指标（`contrast_metrics` / `precision_score`）

- 锚定率、spam/重复率、严重度分布、发现密度
- Compose：全局候选分更高则全量采纳；否则按文件 `prefer_file_result`（精度 / 降 spam / 密度 / 严重度校准）部分换入，写入 library lesson

## 探针策略

默认 `simulate_probe_results`：对已有 SCR 评论做后处理过滤（不二次打 LLM）。  
`HARNESS2_DUAL_PROBE=1`：把候选 `ExecutionHarness` 写入 `StructuredOptions.harness` 再跑一轮 SCR（探针轮关闭 filter）；失败则回退 simulate。

## 代码位置

- `xiezhi-review/src/harness2/` — 域模型与 PPC
- `fusion.rs` — 启用后 post-SCR 编排
- `structured.rs` — system prompt 叠加 library

## 回归夹具

`rust/tests/fixtures/harness2_precision_golden.json` + `xiezhi-review/tests/golden_harness2_precision.rs`：  
低锚定 / 高 spam / simulate 后指标 / compose 采纳阈值。

## Loop

状态：`.loop/harness2-state.json`。本地动态 wake：`AGENT_LOOP_WAKE_harness2`。
