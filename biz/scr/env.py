"""SCR 环境变量与策略读取。"""
from __future__ import annotations

import os


def getenv(name: str, default: str | None = None) -> str | None:
    return os.getenv(name, default)


def review_strategy() -> str:
    return (os.getenv("REVIEW_STRATEGY") or "diff_only").strip().lower()
