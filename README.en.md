# EvoReview (獬豸 / Xiezhi)

<p align="center">
  <img src="assets/xiezhi-logo.png" alt="EvoReview" width="160" />
</p>

<p align="center">
  <b>Self-evolving AI code review</b> — Webhook service + local CLI<br/>
  <i>Reviews that get sharper with every run</i>
</p>

<p align="center">
  <a href="README.zh-CN.md">简体中文</a> ·
  <a href="README.en.md">English</a> ·
  <a href="README.ja.md">日本語</a> ·
  <a href="README.ko.md">한국어</a>
</p>

<p align="center">
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" /></a>
  <a href="https://www.python.org/downloads/"><img alt="Python" src="https://img.shields.io/badge/Python-3.10%2B-green.svg" /></a>
</p>

## Overview

**EvoReview** (heritage brand: 獬豸 / Xiezhi) reviews code changes when GitLab / GitHub / Gitea fire Push or Merge Request (Pull Request) webhooks. Results are posted back as platform comments and can be pushed to DingTalk / WeCom / Feishu. A local CLI supports pre-push self-review.

What stands out: **Harness² self-evolution** — model weights stay fixed; the *execution harness* (prompt overlays, precision bias, lesson library) improves from contrast metrics (anchor rate, spam, severity calibration). See [BRANDING.md](./BRANDING.md) and [rust/HARNESS2.md](./rust/HARNESS2.md).

## Features

| Feature | Description |
|---------|-------------|
| **Self-evolving Harness²** | Propose–Probe–Compose; adopt edits by precision metrics; improvement library |
| Multi-LLM | DeepSeek, ZhipuAI, OpenAI, Anthropic, Qwen, Ollama |
| Multi-platform webhooks | GitLab / GitHub / Gitea |
| IM notifications | DingTalk, WeCom, Feishu |
| Dashboard | Next.js review logs & stats (port `:5002`) |
| Review styles | professional / sarcastic / gentle / humorous |
| Fusion review | SCR (Structured Code Review: per-file, line anchors, fact filter) + optional narrative |
| Agentic (optional) | Tool-based repo exploration; auto-fallback on failure |
| Local CLI | `xiezhi review` / `xiezhi menu` |

## Quick start

### Docker

```bash
git clone <your-repo-url> evoreview
cd xiezhi
cp conf/.env.dist conf/.env
# Edit conf/.env: LLM keys, Git token, SUPPORTED_EXTENSIONS, ...
docker compose up -d
```

| Service | URL |
|---------|-----|
| API | `http://<host>:5001` |
| Dashboard | `http://<host>:5002` |

### Local

```bash
python -m venv .venv
# Windows: .venv\Scripts\activate
# Linux/macOS: source .venv/bin/activate
pip install -r requirements.txt
cp conf/.env.dist conf/.env

python api.py                          # :5001
cd web && npm install && npm run dev   # :5002
```

Dashboard credentials: `DASHBOARD_USER` / `DASHBOARD_PASSWORD` (defaults in `conf/.env.dist`: `admin` / `admin`).

### Minimal config

```bash
LLM_PROVIDER=deepseek
DEEPSEEK_API_KEY=your_key
SUPPORTED_EXTENSIONS=.java,.py,.go,.js,.ts,.vue,.yml
GITLAB_ACCESS_TOKEN=your_token
```

See `conf/.env.dist` for the full list.

## GitLab webhook

1. Create a Personal or Project Access Token.
2. Project → Settings → Webhooks:
   - URL: `http://<host>:5001/review/webhook`
   - Triggers: Push Events, Merge Request Events

`.env` `GITLAB_ACCESS_TOKEN` takes priority; otherwise the webhook Secret Token may be used.  
For GitHub / Gitea / IM setup, see [docs/en/faq.md](docs/en/faq.md).

## Local CLI

```bash
pip install -e .
xiezhi config init
xiezhi review --preview
xiezhi review
```

Details: [docs/en/cli.md](docs/en/cli.md).

## Agentic review (optional)

```bash
REVIEW_STRATEGY=agentic
REPO_CACHE_DIR=data/repo_cache
AGENT_MAX_ITERATIONS=20
```

Failures fall back to `diff_only`. Shell tools are read-only by default (allowlist + path sandbox + timeout).

## Documentation

| Doc | Description |
|-----|-------------|
| [docs/en/faq.md](docs/en/faq.md) | FAQ |
| [docs/en/cli.md](docs/en/cli.md) | Local Diff CLI |
| [web/README.md](web/README.md) | Dashboard |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contributing |
| [SECURITY.md](SECURITY.md) | Security policy |

Other languages: [docs/README.md](docs/README.md).

## Contributing & license

Issues and PRs are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md).  
Licensed under [Apache License 2.0](LICENSE).
