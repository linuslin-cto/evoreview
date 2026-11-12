# 常见问题

> [English](../en/faq.md) · [简体中文](faq.md) · [日本語](../ja/faq.md) · [한국어](../ko/faq.md)

## Docker 更新 `.env` 后不生效

容器不会自动同步宿主机后续修改。重建：

```bash
docker compose down
docker compose up -d
```

## GitLab Webhook 提示 Invalid url given

GitLab 默认可能禁止访问本地网段。在 **Admin Area → Settings → Network → Outbound requests** 勾选允许本地网络请求。

## 不同项目推送到不同群

钉钉示例（后缀为大写项目标识）：

```bash
DINGTALK_ENABLED=1
DINGTALK_WEBHOOK_URL_PROJECT_A=https://oapi.dingtalk.com/robot/send?access_token=...
DINGTALK_WEBHOOK_URL_PROJECT_B=https://oapi.dingtalk.com/robot/send?access_token=...
DINGTALK_WEBHOOK_URL=https://oapi.dingtalk.com/robot/send?access_token=...
```

企微、飞书同理。匹配优先级：仓库名 → Git 服务器 → 默认 URL。

## 不同 GitLab 实例推送到不同群

```bash
DINGTALK_ENABLED=1
DINGTALK_WEBHOOK_gitlab_internal_example_com=https://oapi.dingtalk.com/robot/send?access_token=...
DINGTALK_WEBHOOK_example_gitlab_com=https://oapi.dingtalk.com/robot/send?access_token=...
```

主机名中的 `.` / `-` 转为 `_`。

## 企业微信 / 飞书

```bash
WECOM_ENABLED=1
WECOM_WEBHOOK_URL=https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=xxx

FEISHU_ENABLED=1
FEISHU_WEBHOOK_URL=https://open.feishu.cn/open-apis/bot/v2/hook/xxx
```

## Docker 内连接 Ollama 失败

容器内 `127.0.0.1` 不是宿主机。请使用宿主机 IP 或 `http://host.docker.internal:11434`，并让 Ollama 监听 `0.0.0.0`。

## 支持 GitHub

1. Webhooks → URL：`http://<host>:5001/review/webhook`，JSON，勾选 push / pull_request。
2. 创建 PAT（Contents、Pull requests、Issues、Metadata 等）。
3. `.env` 设置 `GITHUB_ACCESS_TOKEN`。

Gitea 使用 `GITEA_ACCESS_TOKEN`。

## Dashboard 无法登录

检查 `DASHBOARD_USER` / `DASHBOARD_PASSWORD`，确认前端代理指向 API。

## 审查未触发或无评论

1. Webhook 事件与 URL 是否正确可达。
2. Token 是否有效。
3. 文件后缀是否在 `SUPPORTED_EXTENSIONS` 内。
4. 查看 `log/` 日志。
