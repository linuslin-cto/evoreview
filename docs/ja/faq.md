# FAQ

> [English](../en/faq.md) · [简体中文](../zh-CN/faq.md) · [日本語](faq.md) · [한국어](../ko/faq.md)

## Docker で `.env` を更新しても反映されない

コンテナは後からのホスト側変更を自動同期しません。再作成してください：

```bash
docker compose down
docker compose up -d
```

## GitLab Webhook: Invalid url given

ローカルネットワークへの Webhook が禁止されている場合があります。**Admin Area → Settings → Network → Outbound requests** でローカル宛を許可してください。

## プロジェクトごとに別の IM グループへ送る

DingTalk の例：

```bash
DINGTALK_ENABLED=1
DINGTALK_WEBHOOK_URL_PROJECT_A=https://oapi.dingtalk.com/robot/send?access_token=...
DINGTALK_WEBHOOK_URL=https://oapi.dingtalk.com/robot/send?access_token=...
```

WeCom / Feishu も同様。照合順：リポジトリ名 → Git ホスト → デフォルト URL。

## 異なる GitLab ホストごとに送る

```bash
DINGTALK_ENABLED=1
DINGTALK_WEBHOOK_example_gitlab_com=https://oapi.dingtalk.com/robot/send?access_token=...
```

ホスト名の `.` / `-` は `_` に置き換えます。

## WeCom / Feishu

```bash
WECOM_ENABLED=1
WECOM_WEBHOOK_URL=https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=xxx
FEISHU_ENABLED=1
FEISHU_WEBHOOK_URL=https://open.feishu.cn/open-apis/bot/v2/hook/xxx
```

## Docker から Ollama に繋がらない

コンテナ内の `127.0.0.1` はホストではありません。ホスト IP または `http://host.docker.internal:11434` を使い、Ollama を `0.0.0.0` で待受させてください。

## GitHub 対応

1. Webhooks → `http://<host>:5001/review/webhook`（JSON、push / pull_request）。
2. PAT を作成。
3. `.env` に `GITHUB_ACCESS_TOKEN`。

Gitea は `GITEA_ACCESS_TOKEN`。

## Dashboard にログインできない

`DASHBOARD_USER` / `DASHBOARD_PASSWORD` と API プロキシ設定を確認してください。

## レビューが動かない / コメントがない

Webhook・Token・`SUPPORTED_EXTENSIONS`・`log/` を確認してください。
