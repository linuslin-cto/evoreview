//! HTTP API routes（对齐 Flask `biz.api`）。

mod daily_report;
mod dashboard;
mod home;
mod im;
mod queue;
mod worker;

pub use daily_report::{
    daily_report_task, generate_daily_report, spawn_daily_report_scheduler, CronExpr,
};
pub use im::{
    notify_mr_reviewed, notify_push_reviewed, send_notification, sign_dingtalk_url,
    CompositeNotifier, DingTalkNotifier, ExtraWebhookNotifier, FeishuNotifier, LoggingNotifier,
    MemoryNotifier, MrNotify, Notification, Notifier, PushNotify, WeComNotifier,
};

use axum::{
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::env;
use tracing::{info, warn};
use xiezhi_core::VERSION;
use xiezhi_platforms::{
    normalize, slugify_url, HttpCommentPoster, LoggingCommentPoster, PlatformCredentials,
    PlatformKind,
};

pub use dashboard::{shared_review_store, DashboardState, MemoryReviewStore};
pub use worker::{handle_event, handle_event_with, WorkerContext};

#[derive(Serialize)]
struct Health {
    status: &'static str,
    version: &'static str,
}

pub fn app() -> Router {
    app_with_dashboard(DashboardState::from_env())
}

pub fn app_with_dashboard(dashboard: DashboardState) -> Router {
    Router::new()
        .route("/", get(home::home_handler))
        .route("/healthz", get(healthz))
        .route("/review/webhook", post(webhook))
        .route("/review/daily_report", get(daily_report_http))
        .merge(home::assets_router())
        .merge(dashboard::router(dashboard))
}

async fn daily_report_http() -> impl IntoResponse {
    let store = shared_review_store();
    match generate_daily_report(store.as_ref()).await {
        Ok(None) => (
            StatusCode::OK,
            Json(json!({"message": "No data to process."})),
        ),
        Ok(Some(report)) => {
            send_notification(Notification::markdown(report.clone(), "代码提交日报")).await;
            (StatusCode::OK, Json(json!({"report": report})))
        }
        Err(e) => {
            warn!("daily_report HTTP failed: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "message": format!("Failed to generate daily report: {e}")
                })),
            )
        }
    }
}

async fn healthz() -> Json<Health> {
    Json(Health {
        status: "ok",
        version: VERSION,
    })
}

async fn webhook(headers: HeaderMap, Json(payload): Json<Value>) -> impl IntoResponse {
    if let Some(event_type) = header_str(&headers, "x-gitea-event") {
        return handle_gitea_webhook(event_type, payload, &headers);
    }
    if let Some(event_type) = header_str(&headers, "x-github-event") {
        return handle_github_webhook(event_type, payload, &headers);
    }
    handle_gitlab_webhook(payload, &headers)
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

fn handle_github_webhook(
    event_type: &str,
    data: Value,
    headers: &HeaderMap,
) -> (StatusCode, Json<Value>) {
    let token = env::var("GITHUB_ACCESS_TOKEN")
        .ok()
        .or_else(|| header_str(headers, "x-github-token").map(str::to_string));
    let Some(token) = token.filter(|t| !t.is_empty()) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"message": "Missing GitHub access token"})),
        );
    };

    if event_type != "pull_request" && event_type != "push" {
        let msg = format!(
            "Only pull_request and push events are supported for GitHub webhook, but received: {event_type}."
        );
        return (StatusCode::BAD_REQUEST, Json(json!(msg)));
    }

    let github_url = env::var("GITHUB_URL").unwrap_or_else(|_| "https://github.com".into());
    let url_slug = slugify_url(&github_url);
    enqueue_stub_job(data, PlatformKind::Github, token, github_url, url_slug);
    (
        StatusCode::OK,
        Json(json!({
            "message": format!(
                "GitHub request received(event_type={event_type}), will process asynchronously."
            )
        })),
    )
}

fn handle_gitea_webhook(
    event_type: &str,
    data: Value,
    headers: &HeaderMap,
) -> (StatusCode, Json<Value>) {
    let token = env::var("GITEA_ACCESS_TOKEN")
        .ok()
        .or_else(|| header_str(headers, "x-gitea-token").map(str::to_string));
    let Some(token) = token.filter(|t| !t.is_empty()) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"message": "Missing Gitea access token"})),
        );
    };

    if event_type != "pull_request" && event_type != "push" {
        let msg = format!(
            "Only pull_request and push events are supported for Gitea webhook, but received: {event_type}."
        );
        return (StatusCode::BAD_REQUEST, Json(json!(msg)));
    }

    let gitea_url = env::var("GITEA_URL").unwrap_or_else(|_| "https://gitea.com".into());
    let url_slug = slugify_url(&gitea_url);
    enqueue_stub_job(data, PlatformKind::Gitea, token, gitea_url, url_slug);
    (
        StatusCode::OK,
        Json(json!({
            "message": format!(
                "Gitea request received(event_type={event_type}), will process asynchronously."
            )
        })),
    )
}

fn handle_gitlab_webhook(data: Value, headers: &HeaderMap) -> (StatusCode, Json<Value>) {
    let object_kind = data
        .get("object_kind")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let mut gitlab_url = env::var("GITLAB_URL")
        .ok()
        .or_else(|| header_str(headers, "x-gitlab-instance").map(str::to_string));

    if gitlab_url.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
        let homepage = data
            .pointer("/repository/homepage")
            .or_else(|| data.pointer("/project/web_url"))
            .and_then(|v| v.as_str());
        let Some(homepage) = homepage else {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "message": "Missing GitLab URL (set GITLAB_URL or ensure webhook payload has repository.homepage)"
                })),
            );
        };
        match url::Url::parse(homepage) {
            Ok(parsed) if parsed.host_str().is_some() => {
                gitlab_url = Some(format!(
                    "{}://{}/",
                    parsed.scheme(),
                    parsed.host_str().unwrap()
                ));
            }
            _ => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"message": "Invalid GitLab homepage URL in payload"})),
                );
            }
        }
    }

    let mut gitlab_url = gitlab_url.unwrap();
    if !gitlab_url.ends_with('/') {
        gitlab_url = format!("{}/", gitlab_url.trim_end_matches('/'));
    }

    let token = env::var("GITLAB_ACCESS_TOKEN")
        .ok()
        .or_else(|| header_str(headers, "x-gitlab-token").map(str::to_string));
    let Some(token) = token.filter(|t| !t.is_empty()) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"message": "Missing GitLab access token"})),
        );
    };

    if object_kind != "merge_request" && object_kind != "push" {
        let msg = format!(
            "Only merge_request and push events are supported (both Webhook and System Hook), but received: {object_kind}."
        );
        return (StatusCode::BAD_REQUEST, Json(json!(msg)));
    }

    let url_slug = slugify_url(&gitlab_url);
    info!(object_kind = %object_kind, "Received GitLab event");
    enqueue_stub_job(data, PlatformKind::Gitlab, token, gitlab_url, url_slug);
    (
        StatusCode::OK,
        Json(json!({
            "message": format!(
                "Request received(object_kind={object_kind}), will process asynchronously."
            )
        })),
    )
}

fn enqueue_stub_job(
    data: Value,
    expected_platform: PlatformKind,
    token: String,
    base_url: String,
    url_slug: String,
) {
    queue::spawn_job("webhook_review", async move {
        let event = normalize(data).map_err(|e| e.to_string())?;
        info!(
            expected = ?expected_platform,
            platform = ?event.platform,
            event = ?event.event,
            project = %event.project_name,
            "worker: accepted event"
        );
        let creds = PlatformCredentials {
            token,
            base_url,
            url_slug,
        };
        // COMMENT_POSTER=logging 保留纯日志 stub，默认真实 HTTP 回写。
        let mode = env::var("COMMENT_POSTER").unwrap_or_else(|_| "http".into());
        if mode.eq_ignore_ascii_case("logging") {
            worker::handle_event(&event, &creds, &LoggingCommentPoster).await
        } else {
            worker::handle_event(&event, &creds, &HttpCommentPoster::new()).await
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn healthz_ok() {
        let app = app();
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["status"], "ok");
    }

    #[tokio::test]
    async fn home_html_ok() {
        let res = app()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let body = String::from_utf8_lossy(&bytes);
        assert!(body.contains("獬豸"), "{body}");
        assert!(
            body.contains("text/html") || body.contains("<!DOCTYPE html>"),
            "{body}"
        );
    }

    #[tokio::test]
    async fn assets_logo_or_missing() {
        // 若仓库 assets/ 可见则 200；否则 404（CI 工作目录差异时仍可通过）。
        let res = app()
            .oneshot(
                Request::builder()
                    .uri("/assets/xiezhi-logo.png")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            res.status() == StatusCode::OK || res.status() == StatusCode::NOT_FOUND,
            "unexpected {}",
            res.status()
        );
    }

    #[tokio::test]
    async fn daily_report_empty_ok() {
        let res = app()
            .oneshot(
                Request::builder()
                    .uri("/review/daily_report")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        // 共享 store 可能已有数据；有报告或无数据均视为成功。
        assert!(v.get("message").is_some() || v.get("report").is_some());
    }

    #[tokio::test]
    async fn gitlab_webhook_token_and_accept() {
        // Combined to avoid flaky env var races across concurrent tests.
        std::env::remove_var("GITLAB_ACCESS_TOKEN");
        std::env::remove_var("GITLAB_URL");

        let res = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/review/webhook")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"object_kind":"push","project":{"name":"p"},"user_username":"a"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        std::env::set_var("GITLAB_ACCESS_TOKEN", "test-token");
        std::env::set_var("GITLAB_URL", "https://gitlab.example.com/");
        let res = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/review/webhook")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"object_kind":"push","project":{"path_with_namespace":"g/p","name":"p"},"user_username":"alice","ref":"refs/heads/main","after":"abc"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        assert!(v["message"]
            .as_str()
            .unwrap_or("")
            .contains("will process asynchronously"));
        std::env::remove_var("GITLAB_ACCESS_TOKEN");
        std::env::remove_var("GITLAB_URL");
    }

    #[tokio::test]
    async fn github_webhook_accepts_pr() {
        std::env::set_var("GITHUB_ACCESS_TOKEN", "gh-token");
        let res = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/review/webhook")
                    .header("content-type", "application/json")
                    .header("x-github-event", "pull_request")
                    .body(Body::from(
                        r#"{"action":"opened","repository":{"full_name":"o/r","name":"r"},"pull_request":{"number":1,"user":{"login":"alice"},"draft":false},"sender":{"login":"alice"}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        std::env::remove_var("GITHUB_ACCESS_TOKEN");
    }
}
