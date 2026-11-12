# Security policy

> [English](SECURITY.md) · [简体中文](SECURITY.zh-CN.md)

## Reporting a vulnerability

If you find a security issue (RCE, path escape, secret leakage, webhook auth bypass, etc.), **do not** open a public issue with exploit details.

Contact maintainers privately (GitHub Security Advisory if enabled, or another private channel). Include impact scope and reproduction steps.

## Hardening tips

- Never commit real secrets in `conf/.env` or `~/.xiezhi/.env`.
- Restrict network access to the webhook endpoint; protect Git platform tokens.
- With `REVIEW_STRATEGY=agentic`, shell tools are read-only by default — review any allowlist expansion.
- Change `DASHBOARD_USER` / `DASHBOARD_PASSWORD` immediately after deploy.
