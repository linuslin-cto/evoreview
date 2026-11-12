# 로컬 Diff 리뷰 CLI

> [English](../en/cli.md) · [简体中文](../zh-CN/cli.md) · [日本語](../ja/cli.md) · [한국어](cli.md)

명령: `xiezhi` (별칭 `xz`)  
용도: 커밋/푸시 전 로컬 Git 변경을 AI 리뷰 (Webhook 과 동일한 SCR 융합 파이프라인).

## 요구 사항

- Python ≥ 3.10, Git 설치
- Git 저장소 안에서 실행
- LLM 설정 (`~/.xiezhi/.env` 또는 `conf/.env`)
- `conf/scr/`, `conf/prompt_templates.yml`

## 설치

```bash
cd <xiezhi-repo-root>
python -m venv .venv
source .venv/bin/activate
pip install -e .
xiezhi config init
xiezhi version
```

설치 없이:

```bash
python -m biz.cli review --preview
```

## 자주 쓰는 명령

```bash
xiezhi review --preview
xiezhi review
xiezhi review --no-narrative -o review.md
xiezhi review --commit HEAD
xiezhi review --from origin/main --to HEAD
xiezhi review --format json --audience agent -o out.json
xiezhi menu
```

## 주요 옵션

| 옵션 | 설명 |
|------|------|
| `--repo` | 저장소 경로 (기본 `.`) |
| `--commit` | 특정 commit |
| `--from` / `--to` | 범위 Diff |
| `--preview` | 미리보기만 (LLM 없음) |
| `--format text\|json` | 출력 형식 |
| `--no-narrative` | 서술 요약 생략 |
| `-o` / `--output` | 파일 출력 |

## 종료 코드

| 코드 | 의미 |
|------|------|
| `0` | 성공 (변경 없음 포함) |
| `2` | 인자 오류 / Git 아님 / 설정 부족 / Diff 실패 |

관련: [faq.md](faq.md)
