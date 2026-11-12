//! Dashboard REST API（对齐 `biz.api.routes.dashboard`）。

mod auth;
pub(crate) mod reporter;
mod stats;
mod store;

use axum::{
    extract::{Query, Request, State},
    http::StatusCode,
    middleware::{from_fn_with_state, Next},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::env;
use std::sync::Arc;
use tracing::warn;

pub use auth::{issue_token, verify_token, AuthConfig};
pub use store::{MemoryReviewStore, ReviewKind, ReviewLog, ReviewStore, SqliteReviewStore};

use std::sync::OnceLock;

static REVIEW_STORE: OnceLock<Arc<dyn ReviewStore>> = OnceLock::new();

/// 进程级共享审查日志存储（Dashboard / Worker 共用可写 SQLite）。
pub fn shared_review_store() -> Arc<dyn ReviewStore> {
    REVIEW_STORE
        .get_or_init(|| {
            let db = env::var("XIEZHI_DB_FILE").unwrap_or_else(|_| "data/data.db".into());
            match SqliteReviewStore::open_or_create(&db) {
                Ok(s) => Arc::new(s) as Arc<dyn ReviewStore>,
                Err(e) => {
                    warn!("review store: sqlite unavailable ({e}), using memory");
                    Arc::new(MemoryReviewStore::default()) as Arc<dyn ReviewStore>
                }
            }
        })
        .clone()
}

#[derive(Clone)]
pub struct DashboardState {
    pub auth: AuthConfig,
    pub store: Arc<dyn ReviewStore>,
    pub score_threshold: i64,
}

impl DashboardState {
    pub fn from_env() -> Self {
        let auth = AuthConfig::from_env();
        let store = shared_review_store();
        let score_threshold = env::var("DASHBOARD_SCORE_THRESHOLD")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60);
        Self {
            auth,
            store,
            score_threshold,
        }
    }

    pub fn with_memory(store: MemoryReviewStore) -> Self {
        Self {
            auth: AuthConfig::from_env(),
            store: Arc::new(store),
            score_threshold: 60,
        }
    }
}

pub fn router(state: DashboardState) -> Router {
    let authed = Router::new()
        .route("/api/auth/me", get(me))
        .route("/api/reviews", get(reviews))
        .route("/api/filters", get(filters))
        .route("/api/members", get(members))
        .route("/api/system", get(system_info))
        .route("/api/reports/summary", post(report_summary))
        .route_layer(from_fn_with_state(state.clone(), require_auth));

    Router::new()
        .route("/api/auth/login", post(login))
        .merge(authed)
        .with_state(state)
}

async fn require_auth(
    State(state): State<DashboardState>,
    mut req: Request,
    next: Next,
) -> Result<impl IntoResponse, (StatusCode, Json<Value>)> {
    let headers = req.headers();
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer ").map(str::to_string))
        .or_else(|| {
            headers
                .get("x-auth-token")
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        });
    match token.as_deref().and_then(|t| verify_token(&state.auth, t)) {
        Some(user) => {
            req.extensions_mut().insert(AuthUser(user));
            Ok(next.run(req).await)
        }
        None => Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"message": "unauthorized"})),
        )),
    }
}

#[derive(Clone)]
struct AuthUser(String);

#[derive(Debug, Deserialize)]
struct LoginBody {
    username: Option<String>,
    password: Option<String>,
}

async fn login(
    State(state): State<DashboardState>,
    Json(body): Json<LoginBody>,
) -> impl IntoResponse {
    let username = body.username.unwrap_or_default().trim().to_string();
    let password = body.password.unwrap_or_default();
    if username == state.auth.user && password == state.auth.password {
        let token = issue_token(&state.auth, &username);
        let must_change = username == "admin" && password == "admin";
        (
            StatusCode::OK,
            Json(json!({
                "token": token,
                "username": username,
                "must_change_password": must_change,
            })),
        )
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"message": "用户名或密码错误"})),
        )
    }
}

async fn me(req: Request) -> impl IntoResponse {
    let user = req
        .extensions()
        .get::<AuthUser>()
        .map(|u| u.0.clone())
        .unwrap_or_default();
    Json(json!({"username": user}))
}

#[derive(Debug, Deserialize)]
struct ReviewQuery {
    kind: Option<String>,
    authors: Option<String>,
    projects: Option<String>,
    start: Option<String>,
    end: Option<String>,
    q: Option<String>,
    min_score: Option<i64>,
    max_score: Option<i64>,
}

async fn reviews(
    State(state): State<DashboardState>,
    Query(q): Query<ReviewQuery>,
) -> impl IntoResponse {
    let kind = q.kind.as_deref().unwrap_or("mr");
    if kind != "mr" && kind != "push" {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"message": "invalid kind"})),
        )
            .into_response();
    }
    let mut records = query_logs(&state, kind, &q);
    records = filter_keyword(records, q.q.as_deref());
    records = filter_score(records, q.min_score, q.max_score);
    let stats = stats::compute_stats(&records, state.score_threshold);
    (
        StatusCode::OK,
        Json(json!({"items": records_to_json(&records), "stats": stats})),
    )
        .into_response()
}

async fn filters(
    State(state): State<DashboardState>,
    Query(q): Query<ReviewQuery>,
) -> impl IntoResponse {
    let kind = q.kind.as_deref().unwrap_or("mr");
    let records = query_logs(&state, kind, &q);
    let mut authors: Vec<_> = records
        .iter()
        .filter_map(|r| r.author.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    authors.sort();
    let mut projects: Vec<_> = records
        .iter()
        .filter_map(|r| r.project_name.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    projects.sort();
    Json(json!({"authors": authors, "projects": projects}))
}

#[derive(Debug, Deserialize)]
struct MembersQuery {
    source: Option<String>,
    q: Option<String>,
    authors: Option<String>,
    projects: Option<String>,
    start: Option<String>,
    end: Option<String>,
}

async fn members(
    State(state): State<DashboardState>,
    Query(q): Query<MembersQuery>,
) -> impl IntoResponse {
    let source = q.source.as_deref().unwrap_or("mr");
    let rq = ReviewQuery {
        kind: None,
        authors: q.authors.clone(),
        projects: q.projects.clone(),
        start: q.start.clone(),
        end: q.end.clone(),
        q: None,
        min_score: None,
        max_score: None,
    };
    let mut records = Vec::new();
    if source == "mr" || source == "all" {
        records.extend(query_logs(&state, "mr", &rq));
    }
    if (source == "push" || source == "all")
        && env::var("PUSH_REVIEW_ENABLED").unwrap_or_else(|_| "0".into()) == "1"
    {
        records.extend(query_logs(&state, "push", &rq));
    }
    records = filter_keyword(records, q.q.as_deref());

    let mut summary_map: HashMap<String, MemberAgg> = HashMap::new();
    for r in &records {
        let author = r.author.clone().unwrap_or_else(|| "unknown".into());
        let item = summary_map
            .entry(author.clone())
            .or_insert_with(|| MemberAgg::new(author));
        let score = r.score.unwrap_or(0) as f64;
        item.review_count += 1;
        item.score_sum += score;
        item.min_score = item.min_score.min(score);
        item.max_score = item.max_score.max(score);
        if score < state.score_threshold as f64 {
            item.low_count += 1;
        }
        item.additions += r.additions.unwrap_or(0);
        item.deletions += r.deletions.unwrap_or(0);
        if let Some(p) = &r.project_name {
            item.projects.insert(p.clone());
        }
    }

    let mut summary: Vec<Value> = summary_map
        .into_values()
        .map(|item| {
            let count = item.review_count.max(1) as f64;
            json!({
                "author": item.author,
                "review_count": item.review_count,
                "avg_score": (item.score_sum / count * 10.0).round() / 10.0,
                "min_score": item.min_score,
                "max_score": item.max_score,
                "low_count": item.low_count,
                "additions": item.additions,
                "deletions": item.deletions,
                "project_count": item.projects.len(),
            })
        })
        .collect();
    summary.sort_by(|a, b| {
        let la = a["low_count"].as_i64().unwrap_or(0);
        let lb = b["low_count"].as_i64().unwrap_or(0);
        lb.cmp(&la).then_with(|| {
            let aa = a["avg_score"].as_f64().unwrap_or(0.0);
            let ba = b["avg_score"].as_f64().unwrap_or(0.0);
            aa.partial_cmp(&ba).unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    let stats = stats::compute_stats(&records, state.score_threshold);
    Json(json!({
        "summary": summary,
        "items": records_to_json(&records),
        "stats": stats,
        "score_threshold": state.score_threshold,
    }))
}

struct MemberAgg {
    author: String,
    review_count: i64,
    score_sum: f64,
    min_score: f64,
    max_score: f64,
    low_count: i64,
    additions: i64,
    deletions: i64,
    projects: HashSet<String>,
}

impl MemberAgg {
    fn new(author: String) -> Self {
        Self {
            author,
            review_count: 0,
            score_sum: 0.0,
            min_score: 100.0,
            max_score: 0.0,
            low_count: 0,
            additions: 0,
            deletions: 0,
            projects: HashSet::new(),
        }
    }
}

async fn system_info(State(state): State<DashboardState>) -> impl IntoResponse {
    let log_file = env::var("LOG_FILE").unwrap_or_else(|_| "log/app.log".into());
    let log_tail = read_log_tail(&log_file, 120);
    Json(json!({
        "llm_provider": env::var("LLM_PROVIDER").unwrap_or_else(|_| "deepseek".into()),
        "review_strategy": env::var("REVIEW_STRATEGY").unwrap_or_else(|_| "diff_only".into()),
        "review_style": env::var("REVIEW_STYLE").unwrap_or_else(|_| "professional".into()),
        "push_review_enabled": env::var("PUSH_REVIEW_ENABLED").unwrap_or_else(|_| "0".into()) == "1",
        "protected_branches_only": env::var("MERGE_REVIEW_ONLY_PROTECTED_BRANCHES_ENABLED")
            .unwrap_or_else(|_| "0".into())
            == "1",
        "supported_extensions": env::var("SUPPORTED_EXTENSIONS").unwrap_or_default(),
        "review_max_tokens": env::var("REVIEW_MAX_TOKENS").unwrap_or_else(|_| "10000".into()),
        "repo_cache_dir": env::var("REPO_CACHE_DIR").unwrap_or_else(|_| "data/repo_cache".into()),
        "score_threshold": state.score_threshold,
        "report_crontab": env::var("REPORT_CRONTAB_EXPRESSION").unwrap_or_else(|_| "0 18 * * 1-5".into()),
        "tokens": {
            "gitlab": mask_secret(env::var("GITLAB_ACCESS_TOKEN").ok().as_deref()),
            "github": mask_secret(env::var("GITHUB_ACCESS_TOKEN").ok().as_deref()),
            "gitea": mask_secret(env::var("GITEA_ACCESS_TOKEN").ok().as_deref()),
        },
        "notify": {
            "dingtalk": env::var("DINGTALK_ENABLED").unwrap_or_default() == "1",
            "wecom": env::var("WECOM_ENABLED").unwrap_or_default() == "1",
            "feishu": env::var("FEISHU_ENABLED").unwrap_or_default() == "1",
            "extra_webhook": env::var("EXTRA_WEBHOOK_ENABLED").unwrap_or_default() == "1",
        },
        "log_tail": log_tail,
    }))
}

#[derive(Debug, Deserialize)]
struct ReportBody {
    kind: Option<String>,
    authors: Option<Vec<String>>,
    projects: Option<Vec<String>>,
    start: Option<String>,
    end: Option<String>,
}

async fn report_summary(
    State(state): State<DashboardState>,
    Json(body): Json<ReportBody>,
) -> impl IntoResponse {
    let kind = body.kind.as_deref().unwrap_or("mr");
    let rq = ReviewQuery {
        kind: Some(kind.into()),
        authors: body.authors.map(|v| v.join(",")),
        projects: body.projects.map(|v| v.join(",")),
        start: body.start,
        end: body.end,
        q: None,
        min_score: None,
        max_score: None,
    };
    let records = query_logs(&state, kind, &rq);
    if records.is_empty() {
        return (
            StatusCode::OK,
            Json(json!({"message": "No data to process.", "content": Value::Null})),
        );
    }
    let payload: Vec<Value> = records
        .iter()
        .map(|r| {
            json!({
                "author": r.author,
                "project_name": r.project_name,
                "commit_messages": r.commit_messages,
                "score": r.score,
                "updated_at": r.updated_at.and_then(|ts| {
                    chrono::DateTime::from_timestamp(ts, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                }),
            })
        })
        .collect();
    let data = serde_json::to_string(&payload).unwrap_or_else(|_| "[]".into());

    match xiezhi_llm::factory_from_env() {
        Ok(client) => match reporter::generate_report(client.as_ref(), &data).await {
            Ok(content) => (StatusCode::OK, Json(json!({"content": content}))),
            Err(e) => {
                warn!("reporter LLM failed: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"message": format!("生成失败: {e}")})),
                )
            }
        },
        Err(e) => {
            // 无 Key / 未配置时回退确定性摘要，避免 Dashboard 完全不可用。
            warn!("reporter factory unavailable ({e}), using fallback summary");
            let content = reporter::fallback_report(&data, records.len());
            (StatusCode::OK, Json(json!({"content": content})))
        }
    }
}

fn query_logs(state: &DashboardState, kind: &str, q: &ReviewQuery) -> Vec<ReviewLog> {
    let authors = split_csv(q.authors.as_deref());
    let projects = split_csv(q.projects.as_deref());
    let gte = parse_ts(q.start.as_deref(), false);
    let lte = parse_ts(q.end.as_deref(), true);
    let k = if kind == "push" {
        ReviewKind::Push
    } else {
        ReviewKind::Mr
    };
    state
        .store
        .query(k, authors.as_deref(), projects.as_deref(), gte, lte)
}

fn split_csv(value: Option<&str>) -> Option<Vec<String>> {
    let raw = value?.trim();
    if raw.is_empty() {
        return None;
    }
    let items: Vec<_> = raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}

fn parse_ts(date_str: Option<&str>, end_of_day: bool) -> Option<i64> {
    let s = date_str?;
    if s.len() < 10 {
        return None;
    }
    let date = &s[..10];
    let naive = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    let dt = if end_of_day {
        naive.and_hms_opt(23, 59, 59)?
    } else {
        naive.and_hms_opt(0, 0, 0)?
    };
    Some(dt.and_utc().timestamp())
}

fn filter_keyword(records: Vec<ReviewLog>, keyword: Option<&str>) -> Vec<ReviewLog> {
    let Some(kw) = keyword.map(str::trim).filter(|s| !s.is_empty()) else {
        return records;
    };
    let kw = kw.to_lowercase();
    records
        .into_iter()
        .filter(|r| {
            [
                r.project_name.as_deref(),
                r.author.as_deref(),
                r.commit_messages.as_deref(),
                r.review_result.as_deref(),
                r.source_branch.as_deref(),
                r.target_branch.as_deref(),
                r.branch.as_deref(),
            ]
            .into_iter()
            .flatten()
            .any(|v| v.to_lowercase().contains(&kw))
        })
        .collect()
}

fn filter_score(records: Vec<ReviewLog>, min: Option<i64>, max: Option<i64>) -> Vec<ReviewLog> {
    records
        .into_iter()
        .filter(|r| {
            let s = r.score.unwrap_or(0);
            min.map(|m| s >= m).unwrap_or(true) && max.map(|m| s <= m).unwrap_or(true)
        })
        .collect()
}

fn records_to_json(records: &[ReviewLog]) -> Vec<Value> {
    records
        .iter()
        .map(|r| {
            let updated = r.updated_at.and_then(|ts| {
                chrono::DateTime::from_timestamp(ts, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            });
            let delta = match (r.additions, r.deletions) {
                (Some(a), Some(d)) => format!("+{a}  -{d}"),
                _ => String::new(),
            };
            json!({
                "id": r.id,
                "project_name": r.project_name,
                "author": r.author,
                "source_branch": r.source_branch,
                "target_branch": r.target_branch,
                "branch": r.branch,
                "updated_at": updated,
                "commit_messages": r.commit_messages,
                "score": r.score,
                "url": r.url,
                "review_result": r.review_result,
                "additions": r.additions,
                "deletions": r.deletions,
                "last_commit_id": r.last_commit_id,
                "delta": delta,
            })
        })
        .collect()
}

fn mask_secret(value: Option<&str>) -> String {
    match value {
        None | Some("") => "（未配置）".into(),
        Some(v) if v.len() <= 8 => "****".into(),
        Some(v) => format!("{}****{}", &v[..3], &v[v.len() - 2..]),
    }
}

fn read_log_tail(path: &str, lines: usize) -> String {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let all: Vec<_> = text.lines().collect();
            let start = all.len().saturating_sub(lines);
            all[start..].join("\n")
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => format!("读取日志失败: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_app() -> Router {
        let mut mem = MemoryReviewStore::default();
        mem.insert_mr_mut(ReviewLog {
            id: Some(1),
            project_name: Some("g/p".into()),
            author: Some("alice".into()),
            source_branch: Some("feat".into()),
            target_branch: Some("main".into()),
            branch: None,
            updated_at: Some(1_700_000_000),
            commit_messages: Some("init".into()),
            score: Some(55),
            url: Some("https://example/mr/1".into()),
            review_result: Some("needs work".into()),
            additions: Some(10),
            deletions: Some(2),
            last_commit_id: Some("abc".into()),
        });
        let mut state = DashboardState::with_memory(mem);
        state.auth = AuthConfig {
            user: "admin".into(),
            password: "admin".into(),
            secret: "test-secret".into(),
            ttl_secs: 86_400,
        };
        router(state)
    }

    async fn json_body(res: axum::response::Response) -> Value {
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn login_and_reviews() {
        let app = test_app();
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"username":"admin","password":"admin"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = json_body(res).await;
        let token = body["token"].as_str().unwrap();

        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/reviews?kind=mr")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = json_body(res).await;
        assert_eq!(body["items"].as_array().unwrap().len(), 1);
        assert_eq!(body["stats"]["total"], 1);
        assert_eq!(body["stats"]["low_score_count"], 1);
    }

    #[tokio::test]
    async fn report_summary_fallback_without_llm_key() {
        let app = test_app();
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"username":"admin","password":"admin"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let token = json_body(res).await["token"].as_str().unwrap().to_string();

        // Ensure factory has no usable deepseek key for this process snapshot.
        let old = std::env::var("DEEPSEEK_API_KEY").ok();
        std::env::set_var("DEEPSEEK_API_KEY", "xxxx");
        std::env::set_var("LLM_PROVIDER", "deepseek");

        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/reports/summary")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"kind":"mr"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = json_body(res).await;
        assert!(
            body["content"].as_str().unwrap_or("").contains("审查摘要"),
            "{body}"
        );

        match old {
            Some(v) => std::env::set_var("DEEPSEEK_API_KEY", v),
            None => std::env::remove_var("DEEPSEEK_API_KEY"),
        }
    }

    #[tokio::test]
    async fn unauthorized_without_token() {
        let app = test_app();
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/system")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }
}
