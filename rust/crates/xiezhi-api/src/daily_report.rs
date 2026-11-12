//! 每日报告（对齐 `biz.api.routes.daily_report` + `biz.api.scheduler`）。

use crate::dashboard::reporter::{fallback_report, generate_report};
use crate::dashboard::{ReviewKind, ReviewLog, ReviewStore};
use crate::im::{send_notification, Notification};
use chrono::{Datelike, Local, Timelike};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::env;
use std::sync::Arc;
use tracing::{info, warn};

/// 当天本地时区 `[00:00:00, 23:59:59]` 的 unix 秒边界。
pub fn today_bounds_local() -> (i64, i64) {
    let now = Local::now();
    let start = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("midnight")
        .and_local_timezone(Local)
        .single()
        .expect("local midnight")
        .timestamp();
    let end = now
        .date_naive()
        .and_hms_opt(23, 59, 59)
        .expect("eod")
        .and_local_timezone(Local)
        .single()
        .expect("local eod")
        .timestamp();
    (start, end)
}

fn push_review_enabled() -> bool {
    env::var("PUSH_REVIEW_ENABLED").unwrap_or_else(|_| "0".into()) == "1"
}

/// 去重 author+commit_messages，按 author 排序（对齐 Python drop_duplicates + sort）。
pub fn prepare_commits(mut records: Vec<ReviewLog>) -> Vec<Value> {
    let mut seen = HashSet::new();
    records.retain(|r| {
        let key = (
            r.author.clone().unwrap_or_default(),
            r.commit_messages.clone().unwrap_or_default(),
        );
        seen.insert(key)
    });
    records.sort_by(|a, b| {
        a.author
            .as_deref()
            .unwrap_or("")
            .cmp(b.author.as_deref().unwrap_or(""))
    });
    records
        .into_iter()
        .map(|r| {
            json!({
                "author": r.author,
                "project_name": r.project_name,
                "commit_messages": r.commit_messages,
                "score": r.score,
            })
        })
        .collect()
}

pub fn query_today_logs(store: &dyn ReviewStore) -> Vec<ReviewLog> {
    let (gte, lte) = today_bounds_local();
    let kind = if push_review_enabled() {
        ReviewKind::Push
    } else {
        ReviewKind::Mr
    };
    store.query(kind, None, None, Some(gte), Some(lte))
}

/// 生成日报正文；无数据时返回 `None`。
pub async fn generate_daily_report(store: &dyn ReviewStore) -> Result<Option<String>, String> {
    let records = query_today_logs(store);
    info!(n = records.len(), "daily_report: fetched records");
    if records.is_empty() {
        info!("daily_report: No data to process.");
        return Ok(None);
    }
    let commits = prepare_commits(records);
    let data = serde_json::to_string(&commits).map_err(|e| e.to_string())?;
    let n = commits.len();
    let text = match xiezhi_llm::factory_from_env() {
        Ok(client) => match generate_report(client.as_ref(), &data).await {
            Ok(t) => t,
            Err(e) => {
                warn!("daily_report LLM failed: {e}; using fallback");
                fallback_report(&data, n)
            }
        },
        Err(e) => {
            warn!("daily_report factory unavailable ({e}); using fallback");
            fallback_report(&data, n)
        }
    };
    info!(chars = text.chars().count(), "daily_report: generated");
    Ok(Some(text))
}

/// 定时 / 手动触发：生成并 IM 推送。
pub async fn daily_report_task(store: Arc<dyn ReviewStore>) {
    info!("daily_report: start");
    match generate_daily_report(store.as_ref()).await {
        Ok(None) => {}
        Ok(Some(text)) => {
            send_notification(Notification::markdown(text, "代码提交日报")).await;
            info!("daily_report: sent");
        }
        Err(e) => warn!("daily_report failed: {e}"),
    }
}

/// 解析 5 段 cron：`minute hour day month weekday`（weekday：0=Sun … 6=Sat，与常见 crontab 一致）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronExpr {
    minutes: Field,
    hours: Field,
    days: Field,
    months: Field,
    weekdays: Field,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Field {
    Any,
    List(Vec<u32>),
}

impl Field {
    fn parse(raw: &str, min: u32, max: u32) -> Result<Self, String> {
        let raw = raw.trim();
        if raw == "*" {
            return Ok(Field::Any);
        }
        let mut vals = Vec::new();
        for part in raw.split(',') {
            let part = part.trim();
            if let Some((a, b)) = part.split_once('-') {
                let start: u32 = a
                    .trim()
                    .parse()
                    .map_err(|_| format!("bad cron range start: {a}"))?;
                let end: u32 = b
                    .trim()
                    .parse()
                    .map_err(|_| format!("bad cron range end: {b}"))?;
                if start > end || start < min || end > max {
                    return Err(format!("cron range out of bounds: {part}"));
                }
                vals.extend(start..=end);
            } else if let Some((base, step)) = part.split_once('/') {
                let step: u32 = step
                    .trim()
                    .parse()
                    .map_err(|_| format!("bad cron step: {step}"))?;
                if step == 0 {
                    return Err("cron step must be > 0".into());
                }
                let start = if base.trim() == "*" {
                    min
                } else {
                    base.trim()
                        .parse()
                        .map_err(|_| format!("bad cron step base: {base}"))?
                };
                let mut v = start;
                while v <= max {
                    vals.push(v);
                    v = v.saturating_add(step);
                }
            } else {
                let v: u32 = part
                    .parse()
                    .map_err(|_| format!("bad cron value: {part}"))?;
                if v < min || v > max {
                    return Err(format!("cron value out of bounds: {v}"));
                }
                vals.push(v);
            }
        }
        vals.sort_unstable();
        vals.dedup();
        Ok(Field::List(vals))
    }

    fn matches(&self, v: u32) -> bool {
        match self {
            Field::Any => true,
            Field::List(xs) => xs.contains(&v),
        }
    }
}

impl CronExpr {
    pub fn parse(expr: &str) -> Result<Self, String> {
        let parts: Vec<_> = expr.split_whitespace().collect();
        if parts.len() != 5 {
            return Err(format!(
                "cron must have 5 fields, got {}: {expr}",
                parts.len()
            ));
        }
        Ok(Self {
            minutes: Field::parse(parts[0], 0, 59)?,
            hours: Field::parse(parts[1], 0, 23)?,
            days: Field::parse(parts[2], 1, 31)?,
            months: Field::parse(parts[3], 1, 12)?,
            weekdays: Field::parse(parts[4], 0, 6)?,
        })
    }

    pub fn matches_local(&self, dt: chrono::DateTime<Local>) -> bool {
        self.minutes.matches(dt.minute())
            && self.hours.matches(dt.hour())
            && self.days.matches(dt.day())
            && self.months.matches(dt.month())
            && self.weekdays.matches(dt.weekday().num_days_from_sunday())
    }
}

pub fn crontab_from_env() -> CronExpr {
    let raw = env::var("REPORT_CRONTAB_EXPRESSION").unwrap_or_else(|_| "0 18 * * 1-5".into());
    match CronExpr::parse(&raw) {
        Ok(c) => c,
        Err(e) => {
            warn!("invalid REPORT_CRONTAB_EXPRESSION ({raw}): {e}; using 0 18 * * 1-5");
            CronExpr::parse("0 18 * * 1-5").expect("default cron")
        }
    }
}

/// 后台每分钟检查 crontab；命中则跑日报。进程级只应调用一次。
pub fn spawn_daily_report_scheduler(store: Arc<dyn ReviewStore>) {
    let cron = crontab_from_env();
    info!(?cron, "daily_report scheduler armed");
    tokio::spawn(async move {
        let mut last_fire_minute: Option<i64> = None;
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(30));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let now = Local::now();
            let minute_key = now.timestamp() / 60;
            if !cron.matches_local(now) {
                continue;
            }
            if last_fire_minute == Some(minute_key) {
                continue;
            }
            last_fire_minute = Some(minute_key);
            daily_report_task(store.clone()).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::MemoryReviewStore;
    use chrono::TimeZone;

    #[test]
    fn cron_weekday_range() {
        let c = CronExpr::parse("0 18 * * 1-5").unwrap();
        // 2026-11-11 is Wednesday (weekday 3)
        let wed = Local.with_ymd_and_hms(2026, 11, 11, 18, 0, 0).unwrap();
        assert!(c.matches_local(wed));
        let wed_wrong_min = Local.with_ymd_and_hms(2026, 11, 11, 18, 1, 0).unwrap();
        assert!(!c.matches_local(wed_wrong_min));
        // Sunday
        let sun = Local.with_ymd_and_hms(2026, 11, 8, 18, 0, 0).unwrap();
        assert!(!c.matches_local(sun));
    }

    #[test]
    fn prepare_dedupes_and_sorts() {
        let rows = vec![
            ReviewLog {
                author: Some("bob".into()),
                commit_messages: Some("b".into()),
                ..Default::default()
            },
            ReviewLog {
                author: Some("alice".into()),
                commit_messages: Some("a".into()),
                ..Default::default()
            },
            ReviewLog {
                author: Some("alice".into()),
                commit_messages: Some("a".into()),
                ..Default::default()
            },
        ];
        let out = prepare_commits(rows);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0]["author"], "alice");
        assert_eq!(out[1]["author"], "bob");
    }

    #[tokio::test]
    async fn generate_none_when_empty() {
        let store = MemoryReviewStore::default();
        let r = generate_daily_report(&store).await.unwrap();
        assert!(r.is_none());
    }

    #[tokio::test]
    async fn generate_fallback_with_rows() {
        let mut store = MemoryReviewStore::default();
        let (gte, _) = today_bounds_local();
        store.insert_mr_mut(ReviewLog {
            author: Some("alice".into()),
            commit_messages: Some("feat x".into()),
            updated_at: Some(gte + 10),
            score: Some(90),
            ..Default::default()
        });
        // PUSH_REVIEW_ENABLED default 0 → MR
        let r = generate_daily_report(&store).await.unwrap().unwrap();
        assert!(r.contains("alice") || r.contains("审查摘要") || r.contains("feat"));
    }
}
