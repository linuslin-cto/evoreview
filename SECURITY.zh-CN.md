# 安全政策

> [English](SECURITY.md) · [简体中文](SECURITY.zh-CN.md)

## 报告漏洞

若发现可被利用的问题（任意命令执行、路径逃逸、密钥泄露、Webhook 鉴权绕过等），请**不要**公开 Issue 贴出利用细节。

请通过仓库 Security Advisory（若已开启）或维护者私下渠道联系，并说明影响范围与复现步骤。

## 使用建议

- 勿将含真实密钥的 `conf/.env`、`~/.xiezhi/.env` 提交到 Git。
- 生产环境限制 Webhook 入口网络访问，妥善保管平台 Access Token。
- `REVIEW_STRATEGY=agentic` 时 shell 默认仅读类命令；扩大白名单前请评估风险。
- 部署后立即修改 `DASHBOARD_USER` / `DASHBOARD_PASSWORD`。
