# Xiezhi（獬豸）

<p align="center">
  <img src="assets/xiezhi-logo.png" alt="Xiezhi" width="160" />
</p>

<p align="center">
  <b>LLM による自動コードレビュー</b> — Webhook サービス + ローカル CLI
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

## 概要

**Xiezhi** は GitLab / GitHub / Gitea の Push または Merge Request（Pull Request）Webhook で変更を自動レビューし、結果をプラットフォームのコメントに書き戻します。DingTalk / WeCom / Feishu への通知にも対応し、コミット前の自己レビュー用 CLI も提供します。

## 機能

| 機能 | 説明 |
|------|------|
| マルチ LLM | DeepSeek、ZhipuAI、OpenAI、Anthropic、Qwen、Ollama |
| マルチプラットフォーム | GitLab / GitHub / Gitea |
| IM 通知 | DingTalk、WeCom、Feishu |
| Dashboard | Next.js のログと統計（`:5002`） |
| レビュースタイル | professional / sarcastic / gentle / humorous |
| 融合レビュー | SCR（Structured Code Review）+ 任意の叙述サマリー |
| Agentic（任意） | リポジトリ探索ツール；失敗時は自動フォールバック |
| ローカル CLI | `xiezhi review` / `xiezhi menu` |

## クイックスタート

### Docker

```bash
git clone <your-repo-url> xiezhi
cd xiezhi
cp conf/.env.dist conf/.env
# conf/.env を編集
docker compose up -d
```

| サービス | URL |
|----------|-----|
| API | `http://<host>:5001` |
| Dashboard | `http://<host>:5002` |

### ローカル

```bash
python -m venv .venv
source .venv/bin/activate   # Windows: .venv\Scripts\activate
pip install -r requirements.txt
cp conf/.env.dist conf/.env

python api.py
cd web && npm install && npm run dev
```

### 最小設定

```bash
LLM_PROVIDER=deepseek
DEEPSEEK_API_KEY=your_key
SUPPORTED_EXTENSIONS=.java,.py,.go,.js,.ts,.vue,.yml
GITLAB_ACCESS_TOKEN=your_token
```

詳細は `conf/.env.dist` を参照。

## GitLab Webhook

1. Access Token を作成。
2. Settings → Webhooks：
   - URL: `http://<host>:5001/review/webhook`
   - Push / Merge Request Events

FAQ: [docs/ja/faq.md](docs/ja/faq.md)

## ローカル CLI

```bash
pip install -e .
xiezhi config init
xiezhi review --preview
xiezhi review
```

詳細: [docs/ja/cli.md](docs/ja/cli.md)

## ドキュメント

| ドキュメント | 内容 |
|--------------|------|
| [docs/ja/faq.md](docs/ja/faq.md) | FAQ |
| [docs/ja/cli.md](docs/ja/cli.md) | CLI |
| [CONTRIBUTING.md](CONTRIBUTING.md) | コントリビュート（英語） |
| [SECURITY.md](SECURITY.md) | セキュリティ（英語） |

索引: [docs/README.md](docs/README.md)

## ライセンス

[Apache License 2.0](LICENSE)
