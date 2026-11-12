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

## 一句话

**EvoReview** 是面向 GitLab / GitHub / Gitea 的**自进化** AI 代码审查系统：Webhook 自动审查 + 评论回写 + IM 通知 + Dashboard + 本地 CLI。  
与「每次裸调 LLM」不同——**模型权重不变，审查 harness 会进化**（提示叠加、精度偏置、课程库），按锚定率 / 噪音率等对比指标越跑越准。

产品介绍：[INTRODUCTION.md](./INTRODUCTION.md) · 品牌：[BRANDING.md](./BRANDING.md) · 自进化：[rust/HARNESS2.md](./rust/HARNESS2.md)

## 核心能力

| 能力 | 说明 |
|------|------|
| **Harness² 自进化** | Propose → Probe → Compose；可按文件部分采纳；Improvement library |
| 多模型 | DeepSeek、智谱、OpenAI、Anthropic、通义千问、Ollama |
| 多平台 Webhook | GitLab / GitHub / Gitea |
| IM 推送 | 钉钉、企业微信、飞书 |
| Dashboard | 审查日志与统计（默认 `:5002`） |
| 审查风格 | professional / sarcastic / gentle / humorous |
| 融合审查 | SCR 结构化审查（逐文件、行级锚定、事实过滤）+ 可选叙述总览 |
| Agentic（可选） | 工具探索仓库；失败自动降级 |
| 本地 CLI | 提交前自审 |

## 快速开始

### Docker

```bash
git clone https://github.com/linuslin-cto/evoreview.git
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

Dashboard 账号：`DASHBOARD_USER` / `DASHBOARD_PASSWORD`（`conf/.env.dist` 默认 `admin` / `admin`）。

### 最小配置

```bash
LLM_PROVIDER=deepseek
DEEPSEEK_API_KEY=your_key
SUPPORTED_EXTENSIONS=.java,.py,.go,.js,.ts,.vue,.yml
GITLAB_ACCESS_TOKEN=your_token
HARNESS2_ENABLED=1
```

完整项见 `conf/.env.dist`。

### 启用自进化

```bash
HARNESS2_ENABLED=1
# 可选：HARNESS2_DUAL_PROBE=1   # 候选 harness 二次 SCR（成本更高）
# 可选：HARNESS2_PROPOSE_LLM=1  # 用 LLM 精炼 harness 提案
```

## 接入 Webhook

1. 创建平台 Access Token。
2. 配置 Webhook：`http://<host>:5001/review/webhook`
3. 勾选 Push / Merge Request（或 Pull Request）事件。

GitHub / Gitea / IM 细节见 [docs/zh-CN/faq.md](docs/zh-CN/faq.md)。

## 本地 CLI

```bash
pip install -e .
# 安装后使用项目 CLI 入口（见 docs/zh-CN/cli.md）
```

## 文档

| 文档 | 内容 |
|------|------|
| [INTRODUCTION.md](./INTRODUCTION.md) | 产品介绍 |
| [BRANDING.md](./BRANDING.md) | 品牌与文案 |
| [rust/HARNESS2.md](./rust/HARNESS2.md) | 自进化机制 |
| [docs/zh-CN/faq.md](docs/zh-CN/faq.md) | 常见问题 |
| [docs/zh-CN/cli.md](docs/zh-CN/cli.md) | 本地 CLI |
| [web/README.md](web/README.md) | Dashboard |

多语种索引：[docs/README.md](docs/README.md)。

## 贡献与许可

欢迎 Issue / PR。采用 [Apache License 2.0](LICENSE)。
