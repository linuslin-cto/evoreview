# EvoReview Dashboard

Next.js frontend; proxies `/api/*` to the EvoReview API.

> Docs: [English](../README.md) · [简体中文](../README.zh-CN.md) · [日本語](../README.ja.md) · [한국어](../README.ko.md)

## Local development

```bash
# Terminal 1 — API
python api.py

# Terminal 2 — frontend
cd web
cp .env.local.example .env.local   # if needed
npm install
npm run dev
```

Open http://localhost:5002.  
Credentials: `DASHBOARD_USER` / `DASHBOARD_PASSWORD` in `conf/.env`.

## Main routes

| Route | Purpose |
|-------|---------|
| `/login` | Auth |
| `/reviews` | Review logs, KPI, charts |
| `/members` | Member analytics |
| `/deep` | Deep review body |
| `/reports` | Reports |
| `/system` | System info |
