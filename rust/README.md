# EvoReview — Rust runtime

Optional high-performance Rust workspace for the EvoReview API server and CLI.

## Crates

| Crate | Role |
|-------|------|
| `xiezhi-core` | Models & scoring |
| `xiezhi-scr` | Diff split, anchors, rules |
| `xiezhi-review` | Fusion review + **Harness²** |
| `xiezhi-llm` | LLM clients |
| `xiezhi-platforms` | GitLab / GitHub / Gitea |
| `xiezhi-api` | Axum HTTP API |
| `xiezhi-cli` / `xiezhi-server` | Binaries |

## Verify

```bash
# from rust/
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
# or
pwsh ../scripts/rust-verify.ps1
```

## Docs

- Self-evolution: [HARNESS2.md](./HARNESS2.md)
- Product: [../INTRODUCTION.md](../INTRODUCTION.md) · [../README.en.md](../README.en.md)
