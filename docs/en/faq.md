# FAQ

> [English](faq.md) · [简体中文](../zh-CN/faq.md) · [日本語](../ja/faq.md) · [한국어](../ko/faq.md)

## `.env` changes not applied in Docker

Containers do not auto-sync later edits to the host `.env`. Recreate:

```bash
docker compose down
docker compose up -d
```

## GitLab webhook: Invalid url given

GitLab may block webhooks to local networks. Enable local requests under **Admin Area → Settings → Network → Outbound requests**.

## Different projects → different IM groups

DingTalk example (suffix is an uppercase project key):

```bash
DINGTALK_ENABLED=1
DINGTALK_WEBHOOK_URL_PROJECT_A=https://oapi.dingtalk.com/robot/send?access_token=...
DINGTALK_WEBHOOK_URL_PROJECT_B=https://oapi.dingtalk.com/robot/send?access_token=...
DINGTALK_WEBHOOK_URL=https://oapi.dingtalk.com/robot/send?access_token=...
```

Same pattern for WeCom / Feishu. Match order: repo name → Git host → default URL.

## Different GitLab hosts → different groups

```bash
DINGTALK_ENABLED=1
DINGTALK_WEBHOOK_gitlab_internal_example_com=https://oapi.dingtalk.com/robot/send?access_token=...
DINGTALK_WEBHOOK_example_gitlab_com=https://oapi.dingtalk.com/robot/send?access_token=...
```

Replace `.` / `-` in the hostname with `_`.

## WeCom / Feishu

```bash
WECOM_ENABLED=1
WECOM_WEBHOOK_URL=https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=xxx

FEISHU_ENABLED=1
FEISHU_WEBHOOK_URL=https://open.feishu.cn/open-apis/bot/v2/hook/xxx
```

## Ollama unreachable from Docker

`127.0.0.1` inside the container is not the host. Use the host IP or `http://host.docker.internal:11434`, and bind Ollama to `0.0.0.0`.

## GitHub support

1. Repo → Settings → Webhooks → URL `http://<host>:5001/review/webhook`, JSON, push / pull_request.
2. Create a PAT with Contents, Pull requests, Issues, Metadata (as needed).
3. Set `GITHUB_ACCESS_TOKEN` in `.env`.

For Gitea use `GITEA_ACCESS_TOKEN` and the matching webhook.

## Dashboard login fails

Check `DASHBOARD_USER` / `DASHBOARD_PASSWORD` and that the frontend proxies to the API (`API_PROXY_TARGET`).

## No review / no comments

1. Correct webhook events and reachable URL.
2. Valid token; platform can reach this service.
3. Changed files match `SUPPORTED_EXTENSIONS`.
4. Inspect logs under `log/`.
