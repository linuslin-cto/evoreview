# 本地 Diff 审查 CLI

> [English](../en/cli.md) · [简体中文](cli.md) · [日本語](../ja/cli.md) · [한국어](../ko/cli.md)

命令：`xiezhi`（别名 `xz`）  
用途：提交 / 推送前对本机 Git 变更做 AI 审查，与线上 Webhook 共用 SCR 融合能力。

## 环境

- Python ≥ 3.10，已安装 Git
- 位于某个 Git 仓库内
- 已配置 LLM（`~/.xiezhi/.env` 或 `conf/.env`）
- 规则资产：`conf/scr/`、`conf/prompt_templates.yml`

## 安装

```bash
cd <xiezhi 仓库根>
python -m venv .venv
# Windows: .venv\Scripts\activate
# Linux/macOS: source .venv/bin/activate
pip install -e .
xiezhi config init
xiezhi version
```

不安装：

```bash
python -m biz.cli review --preview
```

业务仓库使用獬豸工程配置时：

```bash
export XIEZHI_HOME=/path/to/xiezhi
# 或
export XIEZHI_CONF_DIR=/path/to/xiezhi/conf
```

## 常用命令

```bash
xiezhi review --preview
xiezhi review
xiezhi review --no-narrative -o review.md
xiezhi review --commit HEAD
xiezhi review --from origin/main --to HEAD
xiezhi review --format json --audience agent -o out.json
xiezhi menu
```

## 主要参数

| 参数 | 说明 |
|------|------|
| `--repo` | Git 仓库路径，默认 `.` |
| `--commit` | 审查指定 commit |
| `--from` / `--to` | 范围 Diff |
| `--preview` | 仅预览，不调 LLM |
| `--format text\|json` | 输出格式 |
| `--audience human\|agent` | agent 时日志不污染 stdout |
| `--no-narrative` | 跳过叙述总览 |
| `-o` / `--output` | 写入文件 |

## 配置

```bash
xiezhi config init
xiezhi config init --force
xiezhi config path
```

| 变量 | 含义 |
|------|------|
| `LLM_PROVIDER` | 模型供应商 |
| `*_API_KEY` | 对应密钥 |
| `SCR_CONCURRENCY` | 并发，默认 6 |
| `SCR_MAX_FILES` | 单次最多文件，默认 500 |
| `SCR_REVIEW_FILTER_ENABLED` | 事实校验，默认 1 |

## 退出码

| 码 | 含义 |
|----|------|
| `0` | 成功（含无变更） |
| `2` | 参数错误 / 非 Git 仓库 / 配置缺失 / Diff 失败 |

相关：[faq.md](faq.md)。
