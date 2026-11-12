# EvoReview

<p align="center">
  <img src="assets/xiezhi-logo.png" alt="EvoReview" width="160" />
</p>

<p align="center">
  <b>自进化 AI 代码审查</b><br/>
  <i>Reviews that get sharper with every run</i>
</p>

<p align="center">
  <a href="README.md">English</a> ·
  <b>简体中文</b> ·
  <a href="README.ja.md">日本語</a> ·
  <a href="README.ko.md">한국어</a>
</p>

<p align="center">
  <a href="https://github.com/linuslin-cto/evoreview"><img alt="GitHub" src="https://img.shields.io/badge/GitHub-evoreview-181717?logo=github" /></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" /></a>
  <a href="https://www.python.org/downloads/"><img alt="Python" src="https://img.shields.io/badge/Python-3.10%2B-green.svg" /></a>
</p>

## 简介

**EvoReview** 是一个全新的开源项目：面向 GitLab / GitHub / Gitea 的**自进化 AI 代码审查**。  
Webhook 自动审查、评论回写、IM 通知、Dashboard、本地 CLI 一应俱全。

核心卖点不是「再包一层 LLM」，而是 **Harness²**：模型权重不动，审查 harness（提示叠加、精度偏置、课程库）根据锚定率、噪音率等指标持续进化——**越跑越准**。

详见 [INTRODUCTION.md](./INTRODUCTION.md)、[BRANDING.md](./BRANDING.md)、[rust/HARNESS2.md](./rust/HARNESS2.md)。

## 功能

| 能力 | 说明 |
|------|------|
| **Harness² 自进化** | Propose → Probe → Compose；按文件部分采纳；Improvement library |
| 多模型 | DeepSeek、智谱、OpenAI、Anthropic、通义千问、Ollama |
| 多平台 Webhook | GitLab / GitHub / Gitea |
| IM 推送 | 钉钉、企业微信、飞书 |
| Dashboard | 审查日志与统计（默认 `:5002`） |
| 审查风格 | professional / sarcastic / gentle / humorous |
| 融合审查 | SCR 结构化审查 + 可选叙述总览 |
| Agentic（可选） | 工具探索仓库；失败自动降级 |
| 本地 CLI | 提交前自审 |

## 快速开始

### Docker

```bash
git clone https://github.com/linuslin-cto/evoreview.git
cd evoreview
cp conf/.env.dist conf/.env
# 编辑 conf/.env
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

python api.py
cd web && npm install && npm run dev
```

### 最小配置

```bash
LLM_PROVIDER=deepseek
DEEPSEEK_API_KEY=your_key
SUPPORTED_EXTENSIONS=.java,.py,.go,.js,.ts,.vue,.yml
GITLAB_ACCESS_TOKEN=your_token
HARNESS2_ENABLED=1
```

## 文档

| 文档 | 内容 |
|------|------|
| [INTRODUCTION.md](./INTRODUCTION.md) | 产品介绍 |
| [BRANDING.md](./BRANDING.md) | 品牌文案 |
| [rust/HARNESS2.md](./rust/HARNESS2.md) | 自进化说明 |
| [docs/zh-CN/faq.md](docs/zh-CN/faq.md) | FAQ |
| [docs/zh-CN/cli.md](docs/zh-CN/cli.md) | CLI |

## 许可

[Apache License 2.0](LICENSE)
