# EvoReview（獬豸）

<p align="center">
  <img src="assets/xiezhi-logo.png" alt="EvoReview" width="160" />
</p>

<p align="center">
  <b>Self-evolving AI code review</b> — 自进化审查 · Webhook + 本地 CLI<br/>
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

## 简介

**EvoReview**（獬豸 / Xiezhi）在 GitLab / GitHub / Gitea 的 Push / MR·PR 上自动审查代码变更，写回平台评论，并可推送到钉钉 / 企业微信 / 飞书；本地 CLI 支持提交前自审。核心差异：**Harness² 自进化**——模型权重不变，执行 harness（提示词叠加、精度偏置、课程库）随对比指标越跑越准。

品牌说明见 [BRANDING.md](./BRANDING.md)；自进化细节见 [rust/HARNESS2.md](./rust/HARNESS2.md)。

## 功能

| 能力 | 说明 |
|------|------|
| **自进化 Harness²** | Propose–Probe–Compose；锚定率 / spam 对比采纳；Improvement library |
| 多模型 | DeepSeek、智谱、OpenAI、Anthropic、通义千问、Ollama |
| 多平台 Webhook | GitLab / GitHub / Gitea |
| IM 推送 | 钉钉、企业微信、飞书 |
| Dashboard | Next.js 审查日志与统计（默认 `:5002`） |
| 审查风格 | professional / sarcastic / gentle / humorous |
| 融合审查 | SCR 结构化代码审查（逐文件、行级锚定、事实校验）+ 可选叙述总览 |
| Agentic（可选） | 工具探索全仓库；失败自动降级 |
| 本地 CLI | `xiezhi review` / `xiezhi menu` |

## 快速开始

### Docker

```bash
git clone <your-repo-url> evoreview
cd evoreview
cp conf/.env.dist conf/.env
# 编辑 conf/.env：LLM、Git Token、SUPPORTED_EXTENSIONS 等
docker compose up -d
```

| 服务 | 地址 |
|------|------|
| API | `http://<host>:5001` |
| Dashboard | `http://<host>:5002` |

### 本地运行

```bash
python -m venv .venv
# Windows: .venv\Scripts\activate
# Linux/macOS: source .venv/bin/activate
pip install -r requirements.txt
cp conf/.env.dist conf/.env

python api.py                          # :5001
cd web && npm install && npm run dev   # :5002
```

Dashboard 账号见 `DASHBOARD_USER` / `DASHBOARD_PASSWORD`（`conf/.env.dist` 默认 `admin` / `admin`）。

### 最小配置

```bash
LLM_PROVIDER=deepseek
DEEPSEEK_API_KEY=your_key
SUPPORTED_EXTENSIONS=.java,.py,.go,.js,.ts,.vue,.yml
GITLAB_ACCESS_TOKEN=your_token
```

完整项见 `conf/.env.dist`。

## GitLab Webhook

1. 创建 Personal / Project Access Token。
2. 项目 → Settings → Webhooks：
   - URL：`http://<host>:5001/review/webhook`
   - Trigger：Push Events、Merge Request Events

优先使用 `.env` 中的 `GITLAB_ACCESS_TOKEN`。  
GitHub / Gitea / IM 见 [docs/zh-CN/faq.md](docs/zh-CN/faq.md)。

## 本地 CLI

```bash
pip install -e .
xiezhi config init
xiezhi review --preview
xiezhi review
```

详见 [docs/zh-CN/cli.md](docs/zh-CN/cli.md)。

## Agentic Review（可选）

```bash
REVIEW_STRATEGY=agentic
REPO_CACHE_DIR=data/repo_cache
AGENT_MAX_ITERATIONS=20
```

失败自动降级为 `diff_only`。默认仅允许读类 shell 命令。

## 文档

| 文档 | 内容 |
|------|------|
| [docs/zh-CN/faq.md](docs/zh-CN/faq.md) | 常见问题 |
| [docs/zh-CN/cli.md](docs/zh-CN/cli.md) | 本地 Diff CLI |
| [web/README.md](web/README.md) | Dashboard |
| [CONTRIBUTING.zh-CN.md](CONTRIBUTING.zh-CN.md) | 贡献指南 |
| [SECURITY.zh-CN.md](SECURITY.zh-CN.md) | 安全政策 |

多语种索引：[docs/README.md](docs/README.md)。

## 贡献与许可

欢迎 Issue / PR，见 [CONTRIBUTING.zh-CN.md](CONTRIBUTING.zh-CN.md)。  
采用 [Apache License 2.0](LICENSE)。
