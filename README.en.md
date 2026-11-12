# EvoReview

<p align="center">
  <img src="assets/xiezhi-logo.png" alt="EvoReview" width="160" />
</p>

<p align="center">
  <b>Self-evolving AI code review</b><br/>
  <i>Reviews that get sharper with every run</i>
</p>

<p align="center">
  <a href="README.zh-CN.md">简体中文</a> ·
  <a href="README.en.md">English</a> ·
  <a href="README.ja.md">日本語</a> ·
  <a href="README.ko.md">한국어</a>
</p>

<p align="center">
  <a href="https://github.com/linuslin-cto/evoreview"><img alt="GitHub" src="https://img.shields.io/badge/GitHub-evoreview-181717?logo=github" /></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" /></a>
  <a href="https://www.python.org/downloads/"><img alt="Python" src="https://img.shields.io/badge/Python-3.10%2B-green.svg" /></a>
  <a href="rust/README.md"><img alt="Rust" src="https://img.shields.io/badge/Rust-optional%20runtime-orange.svg" /></a>
</p>

## Overview

**EvoReview** is an open-source **self-evolving** AI code review system for GitLab, GitHub, and Gitea. It reviews Push / MR·PR changes via webhooks, posts comments back to the platform, notifies IM channels, and ships a dashboard plus local CLI.

The differentiator is **Harness²**: model weights stay fixed; the *execution harness* (prompt overlays, precision bias, lesson library) improves from contrast metrics — anchor rate, spam rate, severity balance — so reviews get sharper over time.

Start with [INTRODUCTION.md](./INTRODUCTION.md), [BRANDING.md](./BRANDING.md), and [rust/HARNESS2.md](./rust/HARNESS2.md).

## Features

| Feature | Description |
|---------|-------------|
| **Harness² self-evolution** | Propose → Probe → Compose; per-file partial adopt; improvement library |
| Multi-LLM | DeepSeek, ZhipuAI, OpenAI, Anthropic, Qwen, Ollama |
| Multi-platform webhooks | GitLab / GitHub / Gitea |
| IM notifications | DingTalk, WeCom, Feishu |
| Dashboard | Review logs & stats (default `:5002`) |
| Review styles | professional / sarcastic / gentle / humorous |
| Fusion review | SCR (per-file, line anchors, fact filter) + optional narrative |
| Agentic (optional) | Tool-based repo exploration with auto-fallback |
| Local CLI | Pre-push self-review |

## Quick start

### Docker

```bash
git clone https://github.com/linuslin-cto/evoreview.git
cd evoreview
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
HARNESS2_ENABLED=1
```

See `conf/.env.dist` for the full list.

### Enable self-evolution

```bash
HARNESS2_ENABLED=1
# optional: HARNESS2_DUAL_PROBE=1   # second SCR for candidate harness
# optional: HARNESS2_PROPOSE_LLM=1  # LLM-refined harness proposals
```

## Webhooks

1. Create a platform access token.
2. Point the webhook to `http://<host>:5001/review/webhook`.
3. Enable Push and Merge/Pull Request events.

GitHub / Gitea / IM setup: [docs/en/faq.md](docs/en/faq.md).

## Local CLI

```bash
pip install -e .
# See docs/en/cli.md for commands
```

## Documentation

| Doc | Description |
|-----|-------------|
| [INTRODUCTION.md](./INTRODUCTION.md) | Product intro |
| [BRANDING.md](./BRANDING.md) | Brand & copy |
| [rust/HARNESS2.md](./rust/HARNESS2.md) | Self-evolution |
| [docs/en/faq.md](docs/en/faq.md) | FAQ |
| [docs/en/cli.md](docs/en/cli.md) | Local CLI |
| [web/README.md](web/README.md) | Dashboard |

Other languages: [docs/README.md](docs/README.md).

## Contributing & license

Issues and PRs welcome. Licensed under [Apache License 2.0](LICENSE).
