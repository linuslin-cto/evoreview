# Contributing

> [English](CONTRIBUTING.md) · [简体中文](CONTRIBUTING.zh-CN.md)

Thanks for your interest in **Xiezhi**.

## Setup

- Python 3.10+
- Node.js 20+ (Dashboard only)
- Git

```bash
git clone <your-repo-url> xiezhi
cd xiezhi
python -m venv .venv
# Windows: .venv\Scripts\activate
source .venv/bin/activate
pip install -r requirements.txt
pip install -e .
cp conf/.env.dist conf/.env
```

Dashboard: `cd web && npm install && npm run dev` (run `python api.py` as well).

## Layout

| Path | Role |
|------|------|
| `api.py` | Flask entry |
| `biz/` | Review, platforms, CLI, scoring |
| `conf/` | Env template, SCR rules & prompts |
| `web/` | Next.js Dashboard |
| `docs/` | Multilingual user docs |

## Before you push

1. Keep changes focused; avoid unrelated reformatting.
2. Never commit secrets (`conf/.env`, `web/.env.local`).
3. Smoke: `xiezhi version`, `xiezhi review --preview`.

## Pull requests

Describe motivation, changes, and how you tested. Prefer titles like `fix:`, `feat:`, `docs:`.

Report security issues privately via [SECURITY.md](SECURITY.md).

Contributions are licensed under [Apache License 2.0](LICENSE).
