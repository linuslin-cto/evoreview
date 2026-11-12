# ローカル Diff レビュー CLI

> [English](../en/cli.md) · [简体中文](../zh-CN/cli.md) · [日本語](cli.md) · [한국어](../ko/cli.md)

コマンド: `xiezhi`（別名 `xz`）  
用途: コミット / プッシュ前にローカル Git 変更を AI レビュー（Webhook と同じ SCR 融合パイプライン）。

## 要件

- Python ≥ 3.10、Git インストール済み
- Git リポジトリ内で実行
- LLM 設定（`~/.xiezhi/.env` または `conf/.env`）
- `conf/scr/`、`conf/prompt_templates.yml`

## インストール

```bash
cd <xiezhi-repo-root>
python -m venv .venv
source .venv/bin/activate
pip install -e .
xiezhi config init
xiezhi version
```

インストールなし:

```bash
python -m biz.cli review --preview
```

## よく使うコマンド

```bash
xiezhi review --preview
xiezhi review
xiezhi review --no-narrative -o review.md
xiezhi review --commit HEAD
xiezhi review --from origin/main --to HEAD
xiezhi review --format json --audience agent -o out.json
xiezhi menu
```

## 主なオプション

| オプション | 説明 |
|------------|------|
| `--repo` | リポジトリパス（デフォルト `.`） |
| `--commit` | 指定 commit |
| `--from` / `--to` | 範囲 Diff |
| `--preview` | プレビューのみ（LLM なし） |
| `--format text\|json` | 出力形式 |
| `--no-narrative` | 叙述サマリーを省略 |
| `-o` / `--output` | ファイル出力 |

## 終了コード

| コード | 意味 |
|--------|------|
| `0` | 成功（変更なし含む） |
| `2` | 引数エラー / Git 外 / 設定不足 / Diff 失敗 |

関連: [faq.md](faq.md)
