# EvoReview

<p align="center">
  <img src="assets/xiezhi-logo.png" alt="EvoReview" width="160" />
</p>

<p align="center">
  <b>자기 진화형 AI 코드 리뷰</b><br/>
  <i>Reviews that get sharper with every run</i>
</p>

<p align="center">
  <a href="README.zh-CN.md">简体中文</a> ·
  <a href="README.en.md">English</a> ·
  <a href="README.ja.md">日本語</a> ·
  <a href="README.ko.md">한국어</a>
</p>

<p align="center">
  <a href="https://github.com/linuslin-cto/evoreview"><img alt="GitHub" src="https://img.shields.io/badge/GitHub-evoreview-181717?logo=github" /></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" /></a>
</p>

## 소개

**EvoReview**는 GitLab / GitHub / Gitea를 위한 **자기 진화형** AI 코드 리뷰 오픈소스입니다. Webhook으로 변경을 리뷰하고, 댓글을 되돌리며, IM 알림·대시보드·로컬 CLI를 제공합니다.

핵심은 **Harness²**: 모델 가중치는 고정하고, 실행 harness(프롬프트 오버레이·정밀도 편향·레슨 라이브러리)가 지표로부터 진화해 **실행할수록 더 정확**해집니다.

자세한 내용: [INTRODUCTION.md](./INTRODUCTION.md) · [BRANDING.md](./BRANDING.md) · [rust/HARNESS2.md](./rust/HARNESS2.md)

## 기능

| 기능 | 설명 |
|------|------|
| **Harness² 자기 진화** | Propose → Probe → Compose |
| 멀티 LLM | DeepSeek, ZhipuAI, OpenAI, Anthropic, Qwen, Ollama |
| 멀티 플랫폼 | GitLab / GitHub / Gitea |
| IM 알림 | DingTalk, WeCom, Feishu |
| Dashboard | 로그·통계 (`:5002`) |
| 융합 리뷰 | SCR + 선택적 서술 요약 |
| Agentic (선택) | 저장소 탐색; 실패 시 폴백 |
| 로컬 CLI | 푸시 전 셀프 리뷰 |

## 빠른 시작

```bash
git clone https://github.com/linuslin-cto/evoreview.git
cd evoreview
cp conf/.env.dist conf/.env
docker compose up -d
```

| 서비스 | URL |
|---------|-----|
| API | `http://<host>:5001` |
| Dashboard | `http://<host>:5002` |

영문 README: [README.en.md](./README.en.md) · FAQ: [docs/ko/faq.md](docs/ko/faq.md)

## 라이선스

[Apache License 2.0](LICENSE)
