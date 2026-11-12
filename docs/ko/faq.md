# FAQ

> [English](../en/faq.md) · [简体中文](../zh-CN/faq.md) · [日本語](../ja/faq.md) · [한국어](faq.md)

## Docker 에서 `.env` 수정이 반영되지 않음

컨테이너는 이후 호스트 변경을 자동 동기화하지 않습니다. 재생성하세요:

```bash
docker compose down
docker compose up -d
```

## GitLab Webhook: Invalid url given

로컬 네트워크 Webhook 이 차단될 수 있습니다. **Admin Area → Settings → Network → Outbound requests** 에서 허용하세요.

## 프로젝트별 다른 IM 그룹

DingTalk 예시:

```bash
DINGTALK_ENABLED=1
DINGTALK_WEBHOOK_URL_PROJECT_A=https://oapi.dingtalk.com/robot/send?access_token=...
DINGTALK_WEBHOOK_URL=https://oapi.dingtalk.com/robot/send?access_token=...
```

WeCom / Feishu 동일. 매칭 순서: 저장소명 → Git 호스트 → 기본 URL.

## 서로 다른 GitLab 호스트

```bash
DINGTALK_ENABLED=1
DINGTALK_WEBHOOK_example_gitlab_com=https://oapi.dingtalk.com/robot/send?access_token=...
```

호스트명의 `.` / `-` 는 `_` 로 바꿉니다.

## WeCom / Feishu

```bash
WECOM_ENABLED=1
WECOM_WEBHOOK_URL=https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=xxx
FEISHU_ENABLED=1
FEISHU_WEBHOOK_URL=https://open.feishu.cn/open-apis/bot/v2/hook/xxx
```

## Docker 에서 Ollama 연결 실패

컨테이너의 `127.0.0.1` 은 호스트가 아닙니다. 호스트 IP 또는 `http://host.docker.internal:11434` 를 쓰고 Ollama 를 `0.0.0.0` 에 바인딩하세요.

## GitHub 지원

1. Webhooks → `http://<host>:5001/review/webhook` (JSON, push / pull_request).
2. PAT 생성.
3. `.env` 에 `GITHUB_ACCESS_TOKEN`.

Gitea 는 `GITEA_ACCESS_TOKEN`.

## Dashboard 로그인 실패

`DASHBOARD_USER` / `DASHBOARD_PASSWORD` 와 API 프록시를 확인하세요.

## 리뷰가 안 되거나 댓글이 없음

Webhook, Token, `SUPPORTED_EXTENSIONS`, `log/` 를 확인하세요.
