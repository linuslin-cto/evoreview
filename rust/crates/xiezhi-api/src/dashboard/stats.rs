//! Dashboard 聚合统计（对齐 `_stats`）。

use crate::dashboard::store::ReviewLog;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

pub fn compute_stats(records: &[ReviewLog], score_threshold: i64) -> Value {
    if records.is_empty() {
        return json!({
            "total": 0,
            "avg_score": 0,
            "low_score_count": 0,
            "authors": 0,
            "projects": 0,
            "additions": 0,
            "deletions": 0,
            "score_threshold": score_threshold,
            "by_project_count": [],
            "by_project_score": [],
            "by_author_count": [],
            "by_author_score": [],
            "by_author_lines": [],
            "by_project_lines": [],
            "score_distribution": [],
            "daily_trend": [],
            "low_authors": [],
        });
    }

    let scores: Vec<f64> = records
        .iter()
        .map(|r| r.score.unwrap_or(0) as f64)
        .collect();
    let authors: HashSet<_> = records.iter().filter_map(|r| r.author.clone()).collect();
    let projects: HashSet<_> = records
        .iter()
        .filter_map(|r| r.project_name.clone())
        .collect();
    let additions: i64 = records.iter().map(|r| r.additions.unwrap_or(0)).sum();
    let deletions: i64 = records.iter().map(|r| r.deletions.unwrap_or(0)).sum();
    let low: Vec<&ReviewLog> = records
        .iter()
        .filter(|r| r.score.unwrap_or(0) < score_threshold)
        .collect();

    let mut dist = [("0-39", 0i64), ("40-59", 0), ("60-79", 0), ("80-100", 0)];
    for s in &scores {
        if *s < 40.0 {
            dist[0].1 += 1;
        } else if *s < 60.0 {
            dist[1].1 += 1;
        } else if *s < 80.0 {
            dist[2].1 += 1;
        } else {
            dist[3].1 += 1;
        }
    }

    let mut daily: HashMap<String, (i64, f64)> = HashMap::new();
    for r in records {
        let day = r
            .updated_at
            .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_default();
        if day.is_empty() {
            continue;
        }
        let e = daily.entry(day).or_insert((0, 0.0));
        e.0 += 1;
        e.1 += r.score.unwrap_or(0) as f64;
    }
    let mut daily_trend: Vec<Value> = daily
        .into_iter()
        .map(|(date, (count, sum))| {
            json!({
                "date": date,
                "count": count,
                "avg_score": ((sum / count as f64) * 10.0).round() / 10.0,
            })
        })
        .collect();
    daily_trend.sort_by(|a, b| {
        a["date"]
            .as_str()
            .unwrap_or("")
            .cmp(b["date"].as_str().unwrap_or(""))
    });

    let mut low_bucket: HashMap<String, Vec<f64>> = HashMap::new();
    for r in &low {
        let a = r.author.clone().unwrap_or_else(|| "unknown".into());
        low_bucket
            .entry(a)
            .or_default()
            .push(r.score.unwrap_or(0) as f64);
    }
    let mut low_authors: Vec<Value> = low_bucket
        .into_iter()
        .map(|(name, vals)| {
            let avg = vals.iter().sum::<f64>() / vals.len() as f64;
            json!({
                "name": name,
                "count": vals.len(),
                "avg": (avg * 10.0).round() / 10.0,
            })
        })
        .collect();
    low_authors.sort_by(|a, b| {
        let ca = a["count"].as_u64().unwrap_or(0);
        let cb = b["count"].as_u64().unwrap_or(0);
        cb.cmp(&ca).then_with(|| {
            let aa = a["avg"].as_f64().unwrap_or(0.0);
            let ba = b["avg"].as_f64().unwrap_or(0.0);
            aa.partial_cmp(&ba).unwrap_or(std::cmp::Ordering::Equal)
        })
    });
    low_authors.truncate(10);

    json!({
        "total": records.len(),
        "avg_score": ((scores.iter().sum::<f64>() / scores.len() as f64) * 10.0).round() / 10.0,
        "low_score_count": low.len(),
        "authors": authors.len(),
        "projects": projects.len(),
        "additions": additions,
        "deletions": deletions,
        "score_threshold": score_threshold,
        "by_project_count": group_count(records, |r| r.project_name.as_deref()),
        "by_project_score": group_avg(records, |r| r.project_name.as_deref()),
        "by_author_count": group_count(records, |r| r.author.as_deref()),
        "by_author_score": group_avg(records, |r| r.author.as_deref()),
        "by_author_lines": group_lines(records, |r| r.author.as_deref()),
        "by_project_lines": group_lines(records, |r| r.project_name.as_deref()),
        "score_distribution": dist.iter().map(|(n, v)| json!({"name": n, "value": v})).collect::<Vec<_>>(),
        "daily_trend": daily_trend,
        "low_authors": low_authors,
    })
}

fn group_count(records: &[ReviewLog], key: impl Fn(&ReviewLog) -> Option<&str>) -> Vec<Value> {
    let mut bucket: HashMap<String, i64> = HashMap::new();
    for r in records {
        let k = key(r).unwrap_or("unknown").to_string();
        *bucket.entry(k).or_default() += 1;
    }
    let mut out: Vec<_> = bucket
        .into_iter()
        .map(|(name, value)| json!({"name": name, "value": value}))
        .collect();
    out.sort_by(|a, b| {
        b["value"]
            .as_i64()
            .unwrap_or(0)
            .cmp(&a["value"].as_i64().unwrap_or(0))
    });
    out
}

fn group_avg(records: &[ReviewLog], key: impl Fn(&ReviewLog) -> Option<&str>) -> Vec<Value> {
    let mut bucket: HashMap<String, Vec<f64>> = HashMap::new();
    for r in records {
        let k = key(r).unwrap_or("unknown").to_string();
        bucket
            .entry(k)
            .or_default()
            .push(r.score.unwrap_or(0) as f64);
    }
    let mut out: Vec<_> = bucket
        .into_iter()
        .map(|(name, vals)| {
            let avg = vals.iter().sum::<f64>() / vals.len() as f64;
            json!({"name": name, "value": (avg * 10.0).round() / 10.0, "_avg": avg})
        })
        .collect();
    out.sort_by(|a, b| {
        b["_avg"]
            .as_f64()
            .unwrap_or(0.0)
            .partial_cmp(&a["_avg"].as_f64().unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for v in &mut out {
        if let Some(obj) = v.as_object_mut() {
            obj.remove("_avg");
        }
    }
    out
}

fn group_lines(records: &[ReviewLog], key: impl Fn(&ReviewLog) -> Option<&str>) -> Vec<Value> {
    let mut bucket: HashMap<String, (i64, i64)> = HashMap::new();
    for r in records {
        let k = key(r).unwrap_or("unknown").to_string();
        let e = bucket.entry(k).or_insert((0, 0));
        e.0 += r.additions.unwrap_or(0);
        e.1 += r.deletions.unwrap_or(0);
    }
    bucket
        .into_iter()
        .map(|(name, (additions, deletions))| {
            json!({"name": name, "additions": additions, "deletions": deletions})
        })
        .collect()
}
