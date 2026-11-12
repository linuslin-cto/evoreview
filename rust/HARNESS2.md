# EvoReview · Harness² self-evolution

EvoReview keeps **model weights fixed**. What evolves is the **execution harness**: system overlays, precision bias, and an improvement (lesson) library.

Pipeline: **Propose → Probe → Compose**. Without ground-truth labels, contrast metrics decide whether to adopt an edit.

## Enable

| Env | Default | Meaning |
|---|---|---|
| `HARNESS2_ENABLED` | `0` | `1` runs PPC after fusion SCR; injects library hints into structured prompts |
| `HARNESS2_LIBRARY_PATH` | `data/harness2_library.json` | Lesson library path |
| `HARNESS2_PROPOSE_LLM` | `0` | `1` refines proposals with an LLM (else heuristics) |
| `HARNESS2_DUAL_PROBE` | `0` | `1` re-runs SCR under the candidate harness (fallback to simulate on failure) |

When `HARNESS2_PROPOSE_LLM=1`, model output is parsed via `parse_edit_json` (fence strip, field aliases, severity normalize) then merged with heuristics via `merge_edit`.

## Metrics

- Anchor rate, spam/duplicate rate, severity mix, finding density (`contrast_metrics` / `precision_score`)
- Compose: full adopt if the candidate score wins; otherwise **per-file** `prefer_file_result` / `merge_partial_results`

## Probe strategy

- Default: `simulate_probe_results` (post-filter existing SCR comments, no second LLM)
- Dual probe: set candidate harness on `StructuredOptions.harness` and run SCR again (filter off on the probe pass)

## Code

- `rust/crates/xiezhi-review/src/harness2/` — domain + PPC
- `fusion.rs` — post-SCR orchestration
- `structured.rs` — system prompt overlays

## Regression fixtures

`rust/tests/fixtures/harness2_precision_golden.json` + `xiezhi-review/tests/golden_harness2_precision.rs`
