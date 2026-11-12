# Local Diff review CLI

> [English](cli.md) · [简体中文](../zh-CN/cli.md) · [日本語](../ja/cli.md) · [한국어](../ko/cli.md)

Command: `xiezhi` (alias `xz`)  
Purpose: AI review of local Git changes before commit/push, sharing the same fusion SCR pipeline as webhooks.

## Requirements

- Python ≥ 3.10, Git installed
- Working directory inside a Git repo
- LLM configured (`~/.xiezhi/.env` or `conf/.env`)
- Assets: `conf/scr/`, `conf/prompt_templates.yml`

## Install

```bash
cd <xiezhi-repo-root>
python -m venv .venv
# Windows: .venv\Scripts\activate
# Linux/macOS: source .venv/bin/activate
pip install -e .
xiezhi config init
xiezhi version
```

Without install:

```bash
python -m biz.cli review --preview
```

When reviewing another repo while config lives in the Xiezhi tree:

```bash
export XIEZHI_HOME=/path/to/xiezhi
# or
export XIEZHI_CONF_DIR=/path/to/xiezhi/conf
```

## Common commands

```bash
xiezhi review --preview
xiezhi review
xiezhi review --no-narrative -o review.md
xiezhi review --commit HEAD
xiezhi review --from origin/main --to HEAD
xiezhi review --format json --audience agent -o out.json
xiezhi menu
```

## Main options

| Option | Description |
|--------|-------------|
| `--repo` | Git repo path (default `.`) |
| `--commit` | Review a commit |
| `--from` / `--to` | Range diff (merge-base) |
| `--preview` | Filter preview only (no LLM) |
| `--format text\|json` | Output format |
| `--audience human\|agent` | Agent mode keeps stdout clean |
| `--no-narrative` | Skip narrative summary |
| `-o` / `--output` | Write to file |

## Config helpers

```bash
xiezhi config init
xiezhi config init --force
xiezhi config path
```

| Variable | Meaning |
|----------|---------|
| `LLM_PROVIDER` | Provider id |
| `*_API_KEY` | Provider key |
| `SCR_CONCURRENCY` | Per-file concurrency (default 6) |
| `SCR_MAX_FILES` | Max files per run (default 500) |
| `SCR_REVIEW_FILTER_ENABLED` | Fact filter (default 1) |

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | Success (including no changes) |
| `2` | Bad args, not a Git repo, missing config, or diff failure |

See also: [faq.md](faq.md).
