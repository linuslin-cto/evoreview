# 贡献指南 · EvoReview

> [English](CONTRIBUTING.md) · [简体中文](CONTRIBUTING.zh-CN.md)

感谢关注 **EvoReview**。

## 环境

- Python 3.10+
- Node.js 20+（仅 Dashboard）
- Git
- 可选：Rust 工具链

```bash
git clone https://github.com/linuslin-cto/evoreview.git
cd evoreview
python -m venv .venv
# Windows: .venv\Scripts\activate
source .venv/bin/activate
pip install -r requirements.txt
pip install -e .
cp conf/.env.dist conf/.env
```

Dashboard：`cd web && npm install && npm run dev`（同时运行 `python api.py`）。

## 目录

| 路径 | 作用 |
|------|------|
| `api.py` | API 入口 |
| `biz/` | 审查、平台、CLI、评分 |
| `conf/` | 环境模板、SCR 规则与提示词 |
| `web/` | Next.js Dashboard |
| `rust/` | 可选 Rust 运行时 + Harness² |
| `docs/` | 多语种文档 |

## 提交前

1. 变更聚焦，避免无关格式化。
2. 勿提交密钥（`conf/.env`、`web/.env.local`）。
3. 改动 Rust 时优先跑 `pwsh scripts/rust-verify.ps1`。

## Pull Request

说明动机，关联 Issue，并写明如何验证。

## 许可

贡献内容按 [Apache License 2.0](LICENSE) 授权。
