"""融合审查流水线：SCR 结构化层 + 可选叙述总览。"""
from __future__ import annotations

import os
import re
from dataclasses import dataclass, field

from biz.utils.log import logger
from biz.utils.xiezhi_reviewer import XiezhiReviewer

_SCORE_LINE_RE = re.compile(r"^.*总分[:：]\s*\d+\s*分?.*$", re.MULTILINE)
_SCORE_CUT_RES = (
    re.compile(r"^\s*#{0,6}\s*\**\s*(?:\d+[.\.、)]\s*)?评分明细.*$", re.MULTILINE),
    re.compile(r"^\s*#{0,6}\s*\**\s*(?:\d+[.\.、)]\s*)?总分\s*\**\s*$", re.MULTILINE),
    re.compile(r"^\s*评分项\s*$", re.MULTILINE),
    re.compile(r"^\s*\|?\s*评分项\s*\|", re.MULTILINE),
)
_NARRATIVE_NOISE_RE = re.compile(
    r"^(好的|好，|作为一名|作为资深|我现在|现在，?我|以下是).{0,80}(审查|报告).{0,20}$"
    r"|^\s*#{0,6}\s*\**\s*代码审查报告\s*\**\s*$"
    r"|^\s*问题描述和优化建议\s*$"
)


@dataclass
class ReviewOutcome:
    report_text: str
    score: int
    inline_comments: list = field(default_factory=list)


def resolve_repo_for_event(webhook_data: dict, gitlab_url: str = "") -> tuple[str | None, str | None, str | None]:
    """从 webhook 推断 (repo_url, repo_key, ref)；无法推断则全为 None。"""
    if webhook_data.get("object_kind") == "merge_request":
        repo = webhook_data.get("project", {})
        path = repo.get("path_with_namespace") or repo.get("name")
        url = repo.get("git_http_url") or repo.get("url") or (gitlab_url.rstrip("/") + "/" + path if path and gitlab_url else None)
        attrs = webhook_data.get("object_attributes", {})
        ref = attrs.get("source_branch") or attrs.get("ref")
        sha = (attrs.get("last_commit") or {}).get("id")
        if path and url and (ref or sha):
            return url, path, sha or ref
        return None, None, None
    if webhook_data.get("object_kind") == "push":
        repo = webhook_data.get("project", {})
        path = repo.get("path_with_namespace") or repo.get("name")
        url = repo.get("git_http_url") or repo.get("url") or (gitlab_url.rstrip("/") + "/" + path if path and gitlab_url else None)
        ref = webhook_data.get("after") or webhook_data.get("ref")
        if path and url and ref:
            return url, path, ref
        return None, None, None
    if "repository" in webhook_data and "pull_request" in webhook_data:
        repo = webhook_data["repository"]
        url = repo.get("clone_url") or repo.get("html_url")
        path = repo.get("full_name")
        pr = webhook_data["pull_request"]
        ref = pr.get("head", {}).get("sha") or pr.get("head", {}).get("ref")
        if path and url and ref:
            return url, path, ref
        return None, None, None
    if "repository" in webhook_data and "ref" in webhook_data:
        repo = webhook_data["repository"]
        url = repo.get("clone_url") or repo.get("html_url")
        path = repo.get("full_name")
        ref = webhook_data.get("after") or webhook_data.get("head_commit", {}).get("id")
        if path and url and ref:
            return url, path, ref
        return None, None, None
    return None, None, None


def _narrative_overview(strategy: str, changes: list, commits_text: str, webhook_data: dict, scm_url: str) -> str:
    """产出叙述性总览（diff_only 或 agentic），失败自动降级 diff_only。"""
    if strategy == "agentic":
        try:
            from biz.agent.agentic_reviewer import AgenticReviewer
            repo_url, repo_key, ref = resolve_repo_for_event(webhook_data, scm_url)
            if repo_url and repo_key and ref:
                reviewer = AgenticReviewer(
                    repo_url=repo_url,
                    repo_key=repo_key,
                    ref=ref,
                    cache_root=os.getenv("REPO_CACHE_DIR", "data/repo_cache"),
                )
                return reviewer.review(diffs_text=str(changes), commits_text=commits_text)
            logger.warning("fusion: 无法解析仓库信息，叙述层降级 diff_only")
        except Exception as e:
            logger.error("fusion: agentic 叙述层异常，降级 diff_only: %s", e)
    return XiezhiReviewer().review_and_strip_code(str(changes), commits_text)


def _strip_score_lines(text: str) -> str:
    return _SCORE_LINE_RE.sub("", text or "").strip()


def _clean_narrative(text: str) -> str:
    """去掉叙述层自带评分段落与开场客套。"""
    text = text or ""
    cut_at = None
    for pat in _SCORE_CUT_RES:
        m = pat.search(text)
        if m and (cut_at is None or m.start() < cut_at):
            cut_at = m.start()
    if cut_at is not None:
        text = text[:cut_at]
    text = _strip_score_lines(text)
    lines = text.splitlines()
    cleaned_head = 0
    for line in lines[:6]:
        if not line.strip() or _NARRATIVE_NOISE_RE.match(line.strip()):
            cleaned_head += 1
        else:
            break
    return "\n".join(lines[cleaned_head:]).strip()


def compose_report(results: list, score: int, narrative: str = "", *, degraded: bool = False) -> str:
    """合成报告：总分 + 总览 + 问题明细。"""
    from biz.scr.reviewer import ScrReviewer
    from biz.review.scoring import score_grade

    active = ScrReviewer.active_comments(results)
    filtered_count = sum(1 for r in results for c in r.comments if c.filtered)
    counts = ScrReviewer.severity_counts(results)

    lines: list[str] = []
    lines.append("## 🛡️ 獬豸 AI 代码审查")
    lines.append("")
    lines.append(f"**总分:{score}分（{score_grade(score)}）**")
    lines.append("")

    if degraded:
        lines.append("> ⚠️ 结构化审查层不可用，本次为叙述回退审查（总分解析自叙述层）")
        lines.append("")
    else:
        lines.append(
            "**问题分布**："
            f"🔴 critical {counts['critical']} · "
            f"🟠 high {counts['high']} · "
            f"🟡 medium {counts['medium']} · "
            f"🔵 low {counts['low']}"
        )
        lines.append("")
        lines.append(
            f"> 审查文件 {len(results)} · 有效问题 {len(active)}"
            + (f" · 已过滤误报 {filtered_count}" if filtered_count else "")
        )
        lines.append("")

    narrative = _clean_narrative(narrative)
    if narrative:
        lines.append("### 🧭 整体总览")
        lines.append("")
        lines.append(narrative)
        lines.append("")

    if not degraded:
        if active:
            lines.append("### 📋 问题明细")
            lines.append("")
            lines.append(ScrReviewer.render_findings(results, heading="####"))
        else:
            lines.append("✅ 未发现明显问题，代码质量良好。")

    try:
        from biz.cli import __version__ as _ver
    except Exception:
        _ver = "unknown"
    lines.append("")
    lines.append(f"> xiezhi {_ver}")

    return "\n".join(lines).rstrip()


def _legacy_review(changes: list, commits_text: str, webhook_data: dict, scm_url: str) -> ReviewOutcome:
    from biz.scr.env import review_strategy
    strategy = review_strategy()
    if strategy == "scr":
        try:
            from biz.scr.reviewer import ScrReviewer
            reviewer = ScrReviewer()
            results = reviewer.review_structured(changes)
            score = reviewer.score_from_results(results)
            active = reviewer.active_comments(results)
            return ReviewOutcome(
                report_text=compose_report(results, score),
                score=score,
                inline_comments=[c for c in active if c.start_line > 0],
            )
        except Exception as e:
            logger.error("scr reviewer failed, falling back to diff_only: %s", e)
            return _narrative_only_outcome(
                XiezhiReviewer().review_and_strip_code(str(changes), commits_text)
            )
    if strategy == "agentic":
        return _narrative_only_outcome(
            _narrative_overview("agentic", changes, commits_text, webhook_data, scm_url)
        )
    return _narrative_only_outcome(
        XiezhiReviewer().review_and_strip_code(str(changes), commits_text)
    )


def _narrative_only_outcome(text: str) -> ReviewOutcome:
    score = XiezhiReviewer.parse_review_score(text)
    return ReviewOutcome(
        report_text=compose_report([], score, text, degraded=True),
        score=score,
        inline_comments=[],
    )


def review_changes(changes: list, commits_text: str, webhook_data: dict, scm_url: str = "") -> ReviewOutcome:
    if not changes:
        return ReviewOutcome(report_text="关注的文件没有修改", score=0, inline_comments=[])

    fusion_enabled = os.getenv("REVIEW_FUSION_ENABLED", "1") == "1"
    if not fusion_enabled:
        return _legacy_review(changes, commits_text, webhook_data, scm_url)

    try:
        from biz.scr.reviewer import ScrReviewer
        reviewer = ScrReviewer()
        results = reviewer.review_structured(changes)
    except Exception as e:
        logger.error("fusion: SCR 结构化层异常，整体降级 diff_only: %s", e)
        return _narrative_only_outcome(
            XiezhiReviewer().review_and_strip_code(str(changes), commits_text)
        )

    active = reviewer.active_comments(results)
    score = reviewer.score_from_results(results)
    inline_comments = [c for c in active if c.start_line > 0]

    narrative = ""
    strategy = os.getenv("REVIEW_STRATEGY", "diff_only")
    narrative_enabled = os.getenv("REVIEW_NARRATIVE_ENABLED", "1") == "1"
    if narrative_enabled and strategy in ("diff_only", "agentic"):
        try:
            narrative = _narrative_overview(strategy, changes, commits_text, webhook_data, scm_url)
        except Exception as e:
            logger.warning("fusion: 叙述层生成失败，仅用结构化结果: %s", e)

    body = compose_report(results, score, narrative)

    logger.info(
        "fusion review: files=%d inline=%d score=%d strategy=%s narrative=%s",
        len(results), len(inline_comments), score, strategy, narrative_enabled,
    )
    return ReviewOutcome(report_text=body, score=score, inline_comments=inline_comments)
