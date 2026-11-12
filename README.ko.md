# Xiezhi (獬豸)

<p align="center">
  <img src="assets/xiezhi-logo.png" alt="Xiezhi" width="160" />
</p>

<p align="center">
  <b>LLM 기반 자동 코드 리뷰</b> — Webhook 서비스 + 로컬 CLI
</p>

<p align="center">
  <a href="README.zh-CN.md">简体中文</a> ·
  <a href="README.en.md">English</a> ·
  <a href="README.ja.md">日本語</a> ·
  <a href="README.ko.md">한국어</a>
</p>

<p align="center">
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" /></a>
  <a href="https://www.python.org/downloads/"><img alt="Python" src="https://img.shields.io/badge/Python-3.10%2B-green.svg" /></a>
</p>

## 소개

**Xiezhi**는 GitLab / GitHub / Gitea 에서 Push 또는 Merge Request(Pull Request) Webhook 이 발생할 때 변경을 자동 리뷰하고, 결과를 플랫폼 댓글로 남기며 DingTalk / WeCom / Feishu 로 알릴 수 있습니다. 커밋 전 자체 리뷰용 로컬 CLI 도 제공합니다.

## 기능

| 기능 | 설명 |
|------|------|
| 다중 LLM | DeepSeek, ZhipuAI, OpenAI, Anthropic, Qwen, Ollama |
| 다중 플랫폼 | GitLab / GitHub / Gitea |
| IM 알림 | DingTalk, WeCom, Feishu |
| Dashboard | Next.js 로그·통계 (`:5002`) |
| 리뷰 스타일 | professional / sarcastic / gentle / humorous |
| 융합 리뷰 | SCR(Structured Code Review) + 선택적 서술 요약 |
| Agentic (선택) | 저장소 탐색 도구; 실패 시 자동 폴백 |
| 로컬 CLI | `xiezhi review` / `xiezhi menu` |

## 빠른 시작

### Docker

```bash
git clone <your-repo-url> xiezhi
cd xiezhi
cp conf/.env.dist conf/.env
# conf/.env 편집
docker compose up -d
```

| 서비스 | URL |
|--------|-----|
| API | `http://<host>:5001` |
| Dashboard | `http://<host>:5002` |

### 로컬

```bash
python -m venv .venv
source .venv/bin/activate   # Windows: .venv\Scripts\activate
pip install -r requirements.txt
cp conf/.env.dist conf/.env

python api.py
cd web && npm install && npm run dev
```

### 최소 설정

```bash
LLM_PROVIDER=deepseek
DEEPSEEK_API_KEY=your_key
SUPPORTED_EXTENSIONS=.java,.py,.go,.js,.ts,.vue,.yml
GITLAB_ACCESS_TOKEN=your_token
```

전체 항목은 `conf/.env.dist` 참고.

## GitLab Webhook

1. Access Token 생성.
2. Settings → Webhooks:
   - URL: `http://<host>:5001/review/webhook`
   - Push / Merge Request Events

FAQ: [docs/ko/faq.md](docs/ko/faq.md)

## 로컬 CLI

```bash
pip install -e .
xiezhi config init
xiezhi review --preview
xiezhi review
```

상세: [docs/ko/cli.md](docs/ko/cli.md)

## 문서

| 문서 | 내용 |
|------|------|
| [docs/ko/faq.md](docs/ko/faq.md) | FAQ |
| [docs/ko/cli.md](docs/ko/cli.md) | CLI |
| [CONTRIBUTING.md](CONTRIBUTING.md) | 기여 가이드 (영어) |
| [SECURITY.md](SECURITY.md) | 보안 (영어) |

인덱스: [docs/README.md](docs/README.md)

## 라이선스

[Apache License 2.0](LICENSE)
