# Xiezhi Rust Workspace

Python 系统的渐进式 Rust 重构。当前阶段：**核心域模型 / 评分 / SCR 锚定 / 报告合成 / API·CLI 骨架**，附带自动化测试门禁。

## Crates

| Crate | 职责 |
|-------|------|
| `xiezhi-core` | 模型、实体、统一评分 |
| `xiezhi-scr` | Diff 拆分、行锚定、规则映射 |
| `xiezhi-review` | 融合报告合成、webhook 仓库解析 |
| `xiezhi-llm` | LLM 客户端（骨架） |
| `xiezhi-platforms` | GitLab/GitHub/Gitea webhook 归一化 |
| `xiezhi-api` | Axum 路由 |
| `xiezhi-cli` | `xiezhi` CLI 二进制 |
| `xiezhi-server` | Webhook API 服务二进制 |

## 校验

在 `rust/` 目录：

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

或运行仓库根目录脚本：`./scripts/rust-verify.sh`（Windows: `pwsh scripts/rust-verify.ps1`）。

## 迁移状态

见 [`RUST_MIGRATION.md`](./RUST_MIGRATION.md)。
