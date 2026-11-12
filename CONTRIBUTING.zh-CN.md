# 贡献指南

> [English](CONTRIBUTING.md) · [简体中文](CONTRIBUTING.zh-CN.md)

感谢关注 **獬豸（Xiezhi）**。

## 环境

- Python 3.10+
- Node.js 20+（仅改 Dashboard）
- Git

```bash
git clone <your-repo-url> xiezhi
cd xiezhi
python -m venv .venv
# Windows: .venv\Scripts\activate
source .venv/bin/activate
pip install -r requirements.txt
pip install -e .
cp conf/.env.dist conf/.env
```

Dashboard：`cd web && npm install && npm run dev`（同时运行 `python api.py`）。

## 目录

| 路径 | 说明 |
|------|------|
| `api.py` | Flask 入口 |
| `biz/` | 审查、平台、CLI、评分 |
| `conf/` | 环境变量模板、SCR 规则与 Prompt |
| `web/` | Next.js Dashboard |
| `docs/` | 多语种用户文档 |

## 提交前

1. 改动保持聚焦。
2. 勿提交密钥（`conf/.env`、`web/.env.local`）。
3. 冒烟：`xiezhi version`、`xiezhi review --preview`。

## Pull Request

请说明动机、改动与自测方式。标题建议：`fix:` / `feat:` / `docs:`。

安全问题请按 [SECURITY.zh-CN.md](SECURITY.zh-CN.md) 私下披露。

贡献内容默认按 [Apache License 2.0](LICENSE) 授权。
