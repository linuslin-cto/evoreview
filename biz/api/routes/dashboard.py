"""
Dashboard REST API —— 供 Next.js 前端调用
"""
from __future__ import annotations

import hashlib
import hmac
import math
import os
import time
from datetime import datetime
from functools import wraps

import pandas as pd
from flask import Blueprint, jsonify, request

from biz.service.review_service import ReviewService

dashboard_bp = Blueprint("dashboard", __name__)

DASHBOARD_USER = os.getenv("DASHBOARD_USER", "admin")
DASHBOARD_PASSWORD = os.getenv("DASHBOARD_PASSWORD", "admin")
SECRET_KEY = os.getenv(
    "DASHBOARD_SECRET_KEY",
    "fac8cf149bdd616c07c1a675c4571ccacc40d7f7fe16914cfe0f9f9d966bb773",
)
SCORE_THRESHOLD = int(os.getenv("DASHBOARD_SCORE_THRESHOLD", "60"))
TOKEN_TTL = 30 * 24 * 60 * 60


def _issue_token(username: str) -> str:
    timestamp = str(int(time.time()))
    message = f"{username}:{timestamp}"
    signature = hmac.new(SECRET_KEY.encode(), message.encode(), hashlib.sha256).hexdigest()
    return f"{message}:{signature}"


def _verify_token(token: str | None) -> str | None:
    if not token:
        return None
    try:
        username, timestamp, signature = token.split(":", 2)
        message = f"{username}:{timestamp}"
        expected = hmac.new(SECRET_KEY.encode(), message.encode(), hashlib.sha256).hexdigest()
        if not hmac.compare_digest(expected, signature):
            return None
        if int(time.time()) - int(timestamp) > TOKEN_TTL:
            return None
        if username != DASHBOARD_USER:
            return None
        return username
    except Exception:
        return None


def require_auth(fn):
    @wraps(fn)
    def wrapper(*args, **kwargs):
        auth = request.headers.get("Authorization", "")
        token = auth[7:] if auth.startswith("Bearer ") else request.headers.get("X-Auth-Token")
        username = _verify_token(token)
        if not username:
            return jsonify({"message": "unauthorized"}), 401
        return fn(*args, **kwargs)

    return wrapper


def _parse_ts(date_str: str | None, end_of_day: bool = False) -> int | None:
    if not date_str or len(date_str) < 10:
        return None
    try:
        dt = datetime.strptime(date_str[:10], "%Y-%m-%d")
        if end_of_day:
            dt = dt.replace(hour=23, minute=59, second=59)
        return int(dt.timestamp())
    except ValueError:
        return None


def _split_csv(value: str | None) -> list[str] | None:
    if not value:
        return None
    items = [x.strip() for x in value.split(",") if x.strip()]
    return items or None


def _format_df(df: pd.DataFrame) -> list[dict]:
    if df is None or df.empty:
        return []
    out = df.copy()
    if "updated_at" in out.columns:
        out["updated_at"] = out["updated_at"].apply(
            lambda ts: datetime.fromtimestamp(ts).strftime("%Y-%m-%d %H:%M:%S")
            if isinstance(ts, (int, float)) and not (isinstance(ts, float) and math.isnan(ts))
            else None
        )

    def delta(row):
        try:
            a = row.get("additions")
            d = row.get("deletions")
            if a is None or d is None or (isinstance(a, float) and math.isnan(a)):
                return ""
            return f"+{int(a)}  -{int(d)}"
        except Exception:
            return ""

    if "additions" in out.columns and "deletions" in out.columns:
        out["delta"] = out.apply(delta, axis=1)
    out = out.where(pd.notnull(out), None).infer_objects(copy=False)
    records = out.to_dict(orient="records")
    return records


def _query_logs(kind: str) -> pd.DataFrame:
    authors = _split_csv(request.args.get("authors"))
    projects = _split_csv(request.args.get("projects"))
    gte = _parse_ts(request.args.get("start"))
    lte = _parse_ts(request.args.get("end"), end_of_day=True)
    svc = ReviewService()
    if kind == "push":
        return svc.get_push_review_logs(
            authors=authors, project_names=projects, updated_at_gte=gte, updated_at_lte=lte
        )
    return svc.get_mr_review_logs(
        authors=authors, project_names=projects, updated_at_gte=gte, updated_at_lte=lte
    )


def _filter_keyword(records: list[dict], keyword: str | None) -> list[dict]:
    if not keyword:
        return records
    kw = keyword.strip().lower()
    keys = (
        "project_name",
        "author",
        "commit_messages",
        "review_result",
        "source_branch",
        "target_branch",
        "branch",
    )

    def match(row: dict) -> bool:
        return any(kw in str(row.get(k) or "").lower() for k in keys)

    return [r for r in records if match(r)]


def _filter_score(records: list[dict], min_score: int | None, max_score: int | None) -> list[dict]:
    out = records
    if min_score is not None:
        out = [r for r in out if (r.get("score") or 0) >= min_score]
    if max_score is not None:
        out = [r for r in out if (r.get("score") or 0) <= max_score]
    return out


def _stats(records: list[dict]) -> dict:
    if not records:
        return {
            "total": 0,
            "avg_score": 0,
            "low_score_count": 0,
            "authors": 0,
            "projects": 0,
            "additions": 0,
            "deletions": 0,
            "score_threshold": SCORE_THRESHOLD,
            "by_project_count": [],
            "by_project_score": [],
            "by_author_count": [],
            "by_author_score": [],
            "by_author_lines": [],
            "by_project_lines": [],
            "score_distribution": [],
            "daily_trend": [],
            "low_authors": [],
        }

    scores = [float(r.get("score") or 0) for r in records]
    authors = {r.get("author") for r in records if r.get("author")}
    projects = {r.get("project_name") for r in records if r.get("project_name")}
    additions = sum(int(r.get("additions") or 0) for r in records)
    deletions = sum(int(r.get("deletions") or 0) for r in records)
    low = [r for r in records if (r.get("score") or 0) < SCORE_THRESHOLD]

    def group_count(key: str):
        bucket: dict[str, int] = {}
        for r in records:
            k = r.get(key) or "unknown"
            bucket[k] = bucket.get(k, 0) + 1
        return [{"name": k, "value": v} for k, v in sorted(bucket.items(), key=lambda x: -x[1])]

    def group_avg(key: str):
        bucket: dict[str, list[float]] = {}
        for r in records:
            k = r.get(key) or "unknown"
            bucket.setdefault(k, []).append(float(r.get("score") or 0))
        return [
            {"name": k, "value": round(sum(v) / len(v), 1)}
            for k, v in sorted(bucket.items(), key=lambda x: -(sum(x[1]) / len(x[1])))
        ]

    def group_lines(key: str):
        bucket: dict[str, dict] = {}
        for r in records:
            k = r.get(key) or "unknown"
            item = bucket.setdefault(k, {"additions": 0, "deletions": 0})
            item["additions"] += int(r.get("additions") or 0)
            item["deletions"] += int(r.get("deletions") or 0)
        return [{"name": k, **v} for k, v in bucket.items()]

    dist_labels = ["0-39", "40-59", "60-79", "80-100"]
    dist_counts = {x: 0 for x in dist_labels}
    for s in scores:
        if s < 40:
            dist_counts["0-39"] += 1
        elif s < 60:
            dist_counts["40-59"] += 1
        elif s < 80:
            dist_counts["60-79"] += 1
        else:
            dist_counts["80-100"] += 1

    daily: dict[str, dict] = {}
    for r in records:
        day = str(r.get("updated_at") or "")[:10]
        if not day:
            continue
        item = daily.setdefault(day, {"count": 0, "score_sum": 0.0})
        item["count"] += 1
        item["score_sum"] += float(r.get("score") or 0)
    daily_trend = [
        {
            "date": d,
            "count": v["count"],
            "avg_score": round(v["score_sum"] / v["count"], 1) if v["count"] else 0,
        }
        for d, v in sorted(daily.items())
    ]

    low_bucket: dict[str, list[float]] = {}
    for r in low:
        a = r.get("author") or "unknown"
        low_bucket.setdefault(a, []).append(float(r.get("score") or 0))
    low_authors = sorted(
        [
            {"name": k, "count": len(v), "avg": round(sum(v) / len(v), 1)}
            for k, v in low_bucket.items()
        ],
        key=lambda x: (-x["count"], x["avg"]),
    )[:10]

    return {
        "total": len(records),
        "avg_score": round(sum(scores) / len(scores), 1),
        "low_score_count": len(low),
        "authors": len(authors),
        "projects": len(projects),
        "additions": additions,
        "deletions": deletions,
        "score_threshold": SCORE_THRESHOLD,
        "by_project_count": group_count("project_name"),
        "by_project_score": group_avg("project_name"),
        "by_author_count": group_count("author"),
        "by_author_score": group_avg("author"),
        "by_author_lines": group_lines("author"),
        "by_project_lines": group_lines("project_name"),
        "score_distribution": [{"name": k, "value": v} for k, v in dist_counts.items()],
        "daily_trend": daily_trend,
        "low_authors": low_authors,
    }


def _mask(value: str | None) -> str:
    if not value:
        return "（未配置）"
    if len(value) <= 8:
        return "****"
    return value[:3] + "****" + value[-2:]


@dashboard_bp.route("/api/auth/login", methods=["POST"])
def login():
    data = request.get_json(silent=True) or {}
    username = (data.get("username") or "").strip()
    password = data.get("password") or ""
    if username == DASHBOARD_USER and password == DASHBOARD_PASSWORD:
        token = _issue_token(username)
        return jsonify(
            {
                "token": token,
                "username": username,
                "must_change_password": username == "admin" and password == "admin",
            }
        )
    return jsonify({"message": "用户名或密码错误"}), 401


@dashboard_bp.route("/api/auth/me", methods=["GET"])
@require_auth
def me():
    auth = request.headers.get("Authorization", "")
    token = auth[7:] if auth.startswith("Bearer ") else ""
    username = _verify_token(token)
    return jsonify({"username": username})


@dashboard_bp.route("/api/reviews", methods=["GET"])
@require_auth
def reviews():
    kind = request.args.get("kind", "mr")
    if kind not in ("mr", "push"):
        return jsonify({"message": "invalid kind"}), 400
    df = _query_logs(kind)
    records = _format_df(df)
    records = _filter_keyword(records, request.args.get("q"))
    min_score = request.args.get("min_score", type=int)
    max_score = request.args.get("max_score", type=int)
    records = _filter_score(records, min_score, max_score)
    return jsonify({"items": records, "stats": _stats(records)})


@dashboard_bp.route("/api/filters", methods=["GET"])
@require_auth
def filters():
    kind = request.args.get("kind", "mr")
    df = _query_logs(kind)
    records = _format_df(df)
    authors = sorted({r.get("author") for r in records if r.get("author")})
    projects = sorted({r.get("project_name") for r in records if r.get("project_name")})
    return jsonify({"authors": authors, "projects": projects})


@dashboard_bp.route("/api/members", methods=["GET"])
@require_auth
def members():
    source = request.args.get("source", "mr")  # mr | push | all
    frames = []
    if source in ("mr", "all"):
        frames.append(_format_df(_query_logs("mr")))
    if source in ("push", "all") and os.getenv("PUSH_REVIEW_ENABLED", "0") == "1":
        frames.append(_format_df(_query_logs("push")))
    records = []
    for part in frames:
        records.extend(part)
    records = _filter_keyword(records, request.args.get("q"))

    summary_map: dict[str, dict] = {}
    for r in records:
        author = r.get("author") or "unknown"
        item = summary_map.setdefault(
            author,
            {
                "author": author,
                "review_count": 0,
                "score_sum": 0.0,
                "min_score": 100.0,
                "max_score": 0.0,
                "low_count": 0,
                "additions": 0,
                "deletions": 0,
                "projects": set(),
            },
        )
        score = float(r.get("score") or 0)
        item["review_count"] += 1
        item["score_sum"] += score
        item["min_score"] = min(item["min_score"], score)
        item["max_score"] = max(item["max_score"], score)
        if score < SCORE_THRESHOLD:
            item["low_count"] += 1
        item["additions"] += int(r.get("additions") or 0)
        item["deletions"] += int(r.get("deletions") or 0)
        if r.get("project_name"):
            item["projects"].add(r["project_name"])

    summary = []
    for item in summary_map.values():
        count = item["review_count"] or 1
        summary.append(
            {
                "author": item["author"],
                "review_count": item["review_count"],
                "avg_score": round(item["score_sum"] / count, 1),
                "min_score": item["min_score"],
                "max_score": item["max_score"],
                "low_count": item["low_count"],
                "additions": item["additions"],
                "deletions": item["deletions"],
                "project_count": len(item["projects"]),
            }
        )
    summary.sort(key=lambda x: (-x["low_count"], x["avg_score"]))
    return jsonify(
        {
            "summary": summary,
            "items": records,
            "stats": _stats(records),
            "score_threshold": SCORE_THRESHOLD,
        }
    )


@dashboard_bp.route("/api/system", methods=["GET"])
@require_auth
def system_info():
    log_file = os.getenv("LOG_FILE", "log/app.log")
    log_tail = ""
    try:
        from pathlib import Path

        path = Path(log_file)
        if path.exists():
            lines = path.read_text(encoding="utf-8", errors="ignore").splitlines()
            log_tail = "\n".join(lines[-120:])
    except Exception as e:
        log_tail = f"读取日志失败: {e}"

    return jsonify(
        {
            "llm_provider": os.getenv("LLM_PROVIDER", "deepseek"),
            "review_strategy": os.getenv("REVIEW_STRATEGY", "diff_only"),
            "review_style": os.getenv("REVIEW_STYLE", "professional"),
            "push_review_enabled": os.getenv("PUSH_REVIEW_ENABLED", "0") == "1",
            "protected_branches_only": os.getenv(
                "MERGE_REVIEW_ONLY_PROTECTED_BRANCHES_ENABLED", "0"
            )
            == "1",
            "supported_extensions": os.getenv("SUPPORTED_EXTENSIONS", ""),
            "review_max_tokens": os.getenv("REVIEW_MAX_TOKENS", "10000"),
            "repo_cache_dir": os.getenv("REPO_CACHE_DIR", "data/repo_cache"),
            "score_threshold": SCORE_THRESHOLD,
            "report_crontab": os.getenv("REPORT_CRONTAB_EXPRESSION", "0 18 * * 1-5"),
            "tokens": {
                "gitlab": _mask(os.getenv("GITLAB_ACCESS_TOKEN")),
                "github": _mask(os.getenv("GITHUB_ACCESS_TOKEN")),
                "gitea": _mask(os.getenv("GITEA_ACCESS_TOKEN")),
            },
            "notify": {
                "dingtalk": os.getenv("DINGTALK_ENABLED") == "1",
                "wecom": os.getenv("WECOM_ENABLED") == "1",
                "feishu": os.getenv("FEISHU_ENABLED") == "1",
                "extra_webhook": os.getenv("EXTRA_WEBHOOK_ENABLED") == "1",
            },
            "log_tail": log_tail,
        }
    )


@dashboard_bp.route("/api/reports/summary", methods=["POST"])
@require_auth
def report_summary():
    data = request.get_json(silent=True) or {}
    kind = data.get("kind", "mr")
    authors = data.get("authors")
    projects = data.get("projects")
    gte = _parse_ts(data.get("start"))
    lte = _parse_ts(data.get("end"), end_of_day=True)
    svc = ReviewService()
    if kind == "push":
        df = svc.get_push_review_logs(
            authors=authors, project_names=projects, updated_at_gte=gte, updated_at_lte=lte
        )
    else:
        df = svc.get_mr_review_logs(
            authors=authors, project_names=projects, updated_at_gte=gte, updated_at_lte=lte
        )
    records = _format_df(df)
    if not records:
        return jsonify({"message": "No data to process.", "content": None}), 200
    payload = [
        {
            "author": r.get("author"),
            "project_name": r.get("project_name"),
            "commit_messages": r.get("commit_messages"),
            "score": r.get("score"),
            "updated_at": r.get("updated_at"),
        }
        for r in records
    ]
    try:
        import json

        from biz.utils.reporter import Reporter

        content = Reporter().generate_report(json.dumps(payload, ensure_ascii=False))
        return jsonify({"content": content})
    except Exception as e:
        return jsonify({"message": f"生成失败: {e}"}), 500
