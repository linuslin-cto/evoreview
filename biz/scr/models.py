"""SCR 审查评论数据模型。"""
from __future__ import annotations

from dataclasses import dataclass, field

VALID_CATEGORIES = {
    "bug",
    "security",
    "performance",
    "maintainability",
    "test",
    "style",
    "documentation",
    "other",
}

VALID_SEVERITIES = {"critical", "high", "medium", "low"}

SEVERITY_PENALTY = {
    "critical": 25,
    "high": 12,
    "medium": 5,
    "low": 2,
}

SEVERITY_EMOJI = {
    "critical": "🔴",
    "high": "🟠",
    "medium": "🟡",
    "low": "🔵",
}


@dataclass
class ScrComment:
    path: str
    content: str
    existing_code: str = ""
    suggestion_code: str = ""
    start_line: int = 0
    end_line: int = 0
    category: str = "other"
    severity: str = "medium"
    filtered: bool = False

    def normalized(self) -> "ScrComment":
        cat = (self.category or "other").strip().lower()
        sev = (self.severity or "medium").strip().lower()
        self.category = cat if cat in VALID_CATEGORIES else "other"
        self.severity = sev if sev in VALID_SEVERITIES else "medium"
        if self.end_line < self.start_line:
            self.end_line = self.start_line
        return self

    @property
    def severity_emoji(self) -> str:
        return SEVERITY_EMOJI.get(self.severity, "🔵")


@dataclass
class FileReviewResult:
    path: str
    comments: list[ScrComment] = field(default_factory=list)
    warning: str = ""
    additions: int = 0
    deletions: int = 0
