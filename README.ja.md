# EvoReview

<p align="center">
  <img src="assets/xiezhi-logo.png" alt="EvoReview" width="160" />
</p>

<p align="center">
  <b>自己進化する AI コードレビュー</b><br/>
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
</p>

## 概要

**EvoReview** は GitLab / GitHub / Gitea 向けの**自己進化型** AI コードレビュー OSS です。Webhook で変更をレビューし、コメントを書き戻し、IM 通知・Dashboard・ローカル CLI を提供します。

差別化ポイントは **Harness²**：モデル重みは固定のまま、実行 harness（プロンプト重ね・精度バイアス・レッスン庫）が指標から進化し、実行するほど精度が上がります。

詳細: [INTRODUCTION.md](./INTRODUCTION.md) · [BRANDING.md](./BRANDING.md) · [rust/HARNESS2.md](./rust/HARNESS2.md)

## 機能

| 機能 | 説明 |
|------|------|
| **Harness² 自己進化** | Propose → Probe → Compose |
| マルチ LLM | DeepSeek、ZhipuAI、OpenAI、Anthropic、Qwen、Ollama |
| マルチプラットフォーム | GitLab / GitHub / Gitea |
| IM 通知 | DingTalk、WeCom、Feishu |
| Dashboard | ログと統計（`:5002`） |
| 融合レビュー | SCR + 任意の叙述サマリー |
| Agentic（任意） | リポジトリ探索；失敗時フォールバック |
| ローカル CLI | コミット前の自己レビュー |

## クイックスタート

```bash
git clone https://github.com/linuslin-cto/evoreview.git
cd evoreview
cp conf/.env.dist conf/.env
docker compose up -d
```

| サービス | URL |
|---------|-----|
| API | `http://<host>:5001` |
| Dashboard | `http://<host>:5002` |

英語 README: [README.en.md](./README.en.md) · FAQ: [docs/ja/faq.md](docs/ja/faq.md)

## ライセンス

[Apache License 2.0](LICENSE)
