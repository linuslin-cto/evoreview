"""统一评分（严重级别定档 + 问题密度定位）。"""
from __future__ import annotations

import math
from collections import defaultdict

from biz.scr.models import SEVERITY_PENALTY

DENSITY_FACTOR = 3.0
SPAM_DECAY = 0.7
ANCHOR_DISCOUNT = 0.8

CATEGORY_FACTOR = {
    "security": 1.3,
    "bug": 1.15,
    "performance": 1.0,
    "maintainability": 1.0,
    "test": 0.9,
    "style": 0.7,
    "documentation": 0.7,
    "other": 1.0,
}

UNIT_LINES = 40.0
UNIT_MIN = 0.5
UNIT_MAX = 3.0

BAND_EVIDENCE_MIN = 1.0
ANCHOR_BAND_WEIGHT = 0.5

SEVERITY_BANDS: dict[str, tuple[int, int]] = {
    "critical": (20, 59),
    "high": (55, 79),
    "medium": (70, 92),
    "low": (85, 98),
}
_SEVERITY_ORDER = ("critical", "high", "medium", "low")

CAP_WITH_CRITICAL = SEVERITY_BANDS["critical"][1]
CAP_WITH_HIGH = SEVERITY_BANDS["high"][1]


def _norm_severity(sev: str) -> str:
    return sev if sev in SEVERITY_BANDS else "medium"


def _band_severity(active: list) -> str:
    evidence: dict[str, float] = defaultdict(float)
    for c in active:
        weight = 1.0 if getattr(c, "start_line", 0) > 0 else ANCHOR_BAND_WEIGHT
        evidence[_norm_severity(c.severity)] += weight
    for i, sev in enumerate(_SEVERITY_ORDER):
        if evidence.get(sev, 0.0) <= 0.0:
            continue
        if evidence[sev] >= BAND_EVIDENCE_MIN or i == len(_SEVERITY_ORDER) - 1:
            return sev
        return _SEVERITY_ORDER[i + 1]
    return "medium"


def _effective_penalty(active: list) -> float:
    seen: dict[tuple[str, str], int] = defaultdict(int)
    total = 0.0
    for c in active:
        sev = _norm_severity(c.severity)
        key = (getattr(c, "path", ""), sev)
        k = seen[key]
        seen[key] += 1
        weight = SPAM_DECAY ** k
        weight *= CATEGORY_FACTOR.get(getattr(c, "category", "other"), 1.0)
        if getattr(c, "start_line", 0) <= 0:
            weight *= ANCHOR_DISCOUNT
        total += SEVERITY_PENALTY.get(c.severity, 5) * weight
    return total


def _effective_units(results: list) -> float:
    units = 0.0
    for r in results:
        additions = getattr(r, "additions", 0)
        units += min(UNIT_MAX, max(UNIT_MIN, additions / UNIT_LINES))
    return max(1.0, units)


def _banded_score(active: list, units: float) -> int:
    if not active:
        return 100
    density = _effective_penalty(active) / max(1.0, units)
    density_score = 100.0 * math.exp(-density * DENSITY_FACTOR / 100.0)
    band_low, band_high = SEVERITY_BANDS[_band_severity(active)]
    score = round(band_low + (band_high - band_low) * density_score / 100.0)
    return max(0, min(100, score))


def score_results(results: list) -> int:
    """由 FileReviewResult 列表计算总分（0-100）。"""
    active = [c for r in results for c in r.comments if not getattr(c, "filtered", False)]
    return _banded_score(active, _effective_units(results))


def unified_score(comments: list, files_reviewed: int) -> int:
    """仅有评论与文件数时计算总分（1 文件 = 1 单元）。"""
    active = [c for c in comments if not getattr(c, "filtered", False)]
    return _banded_score(active, float(max(1, int(files_reviewed))))


def score_grade(score: int) -> str:
    if score >= 90:
        return "优秀"
    if score >= 80:
        return "良好"
    if score >= 60:
        return "及格"
    return "需改进"
