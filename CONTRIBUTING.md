# Contributing to EvoReview

> [English](CONTRIBUTING.md) · [简体中文](CONTRIBUTING.zh-CN.md)

Thanks for your interest in **EvoReview**.

## Setup

- Python 3.10+
- Node.js 20+ (Dashboard only)
- Git
- Optional: Rust toolchain (Rust runtime)

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

Dashboard: `cd web && npm install && npm run dev` (also run `python api.py`).

## Layout

| Path | Role |
|------|------|
| `api.py` | API entry |
| `biz/` | Review, platforms, CLI, scoring |
| `conf/` | Env template, SCR rules & prompts |
| `web/` | Next.js Dashboard |
| `rust/` | Optional Rust runtime + Harness² |
| `docs/` | Multilingual user docs |

## Before you push

1. Keep changes focused; avoid unrelated reformatting.
2. Never commit secrets (`conf/.env`, `web/.env.local`).
3. Prefer `pwsh scripts/rust-verify.ps1` when touching Rust.

## Pull requests

Describe *why*, link issues when relevant, and note how you tested.

## License

By contributing, you agree your work is licensed under [Apache License 2.0](LICENSE).
