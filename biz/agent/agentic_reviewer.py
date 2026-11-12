"""Top-level entry point for agentic review, used by worker.py."""
from __future__ import annotations

import json
import os
import re
import time
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

from biz.agent.llm_adapter import LLMAdapter
from biz.agent.prompts import load_prompt
from biz.agent.repo_syncer import LocalRepoSyncer
from biz.agent.runner import AgentRunner, TokenBudgetExceeded
from biz.agent.tools import register_default_tools
from biz.agent.tool_registry import ToolRegistry
from biz.llm.factory import Factory
from biz.utils.xiezhi_reviewer import XiezhiReviewer
from biz.utils.log import logger
from biz.utils.im import notifier


_REVIEW_SCORE_RE = re.compile(r"总分[:：]\s*(\d+)分?")


def _slugify_repo_key(provider: str, project: str) -> str:
    return f"{provider}_{project}".replace("/", "_").replace(" ", "_")


def _parse_csv_env(name: str, default: list[str]) -> list[str]:
    raw = os.getenv(name, "").strip()
    if not raw:
        return list(default)
    return [item.strip() for item in raw.split(",") if item.strip()] or list(default)


def _looks_like_review(text: str | None) -> bool:
    """True when output contains the `总分:XX分` marker."""
    if not text:
        return False
    return bool(_REVIEW_SCORE_RE.search(text))


@dataclass
class ReviewLog:
    event: str
    project: str
    ref: str
    strategy: str
    iterations: int
    total_tokens_est: int
    duration_ms: int
    review_result_length: int
    score: int
    degraded: bool
    tool_calls: list[dict]


def _estimate_tokens(messages: list[dict]) -> int:
    """Rough token estimate from assistant message lengths."""
    total = 0
    for m in messages:
        if m.get("role") != "assistant":
            continue
        content = m.get("content") or ""
        if isinstance(content, str):
            total += len(content) // 4
    return total


def _collect_tool_calls(messages: list[dict]) -> list[dict]:
    """Collect tool_calls from assistant messages."""
    calls: list[dict] = []
    for m in messages:
        if m.get("role") != "assistant":
            continue
        for call in m.get("tool_calls") or []:
            calls.append(call)
    return calls


class AgenticReviewer:
    def __init__(
        self,
        *,
        repo_url: str,
        repo_key: str,
        ref: str,
        cache_root: Path | str,
        adapter: LLMAdapter | None = None,
        max_iterations: int = 20,
        total_token_cap: int = 80_000,
    ) -> None:
        self.repo_url = repo_url
        self.repo_key = repo_key
        self.ref = ref
        self.cache_root = Path(cache_root)
        self.adapter = adapter
        self.max_iterations = max_iterations
        self.total_token_cap = total_token_cap

    def _build_adapter(self) -> LLMAdapter:
        if self.adapter is not None:
            return self.adapter
        client = Factory().getClient()
        return LLMAdapter(client)

    def _build_registry(self, repo_root: Path) -> ToolRegistry:
        registry = ToolRegistry()
        from biz.agent.tools.run_command import (
            DEFAULT_ALLOWLIST,
            DEFAULT_BLOCKLIST,
        )
        allow = _parse_csv_env("AGENT_SHELL_ALLOWLIST", DEFAULT_ALLOWLIST)
        block = _parse_csv_env("AGENT_SHELL_BLOCKLIST", DEFAULT_BLOCKLIST)
        register_default_tools(registry, repo_root, allowlist=allow, blocklist=block)
        return registry

    def review(self, diffs_text: str, commits_text: str) -> str:
        start = time.monotonic()
        try:
            syncer = LocalRepoSyncer(cache_root=self.cache_root)
            repo_root = syncer.sync_to(url=self.repo_url, key=self.repo_key, ref=self.ref)
        except (OSError, RuntimeError) as e:
            logger.error("agentic repo sync failed, degrading: %s", e)
            notifier.send_notification(content=f"[agentic] repo sync failed: {e}; falling back to diff_only")
            return XiezhiReviewer().review_and_strip_code(diffs_text, commits_text)

        adapter = self._build_adapter()
        registry = self._build_registry(repo_root)
        runner = AgentRunner(
            adapter=adapter,
            registry=registry,
            max_iterations=self.max_iterations,
            total_token_cap=self.total_token_cap,
        )

        prompts = load_prompt("agentic_xiezhi_review_prompt", style=os.getenv("REVIEW_STYLE", "professional"))
        user_content = prompts["user_message"]["content"].format(
            diffs_text=diffs_text,
            commits_text=commits_text,
            repo_root=str(repo_root),
        )
        messages = [prompts["system_message"], {"role": "user", "content": user_content}]

        run_meta: dict[str, Any] = {}
        result: str
        degraded = False
        try:
            result = runner.run(messages, out=run_meta)
        except (TokenBudgetExceeded, ValueError, RuntimeError, TimeoutError) as e:
            logger.error("agentic run failed, degrading to diff_only: %s", e)
            notifier.send_notification(content=f"[agentic] run failed: {e}; falling back to diff_only")
            degraded = True
            run_meta.setdefault("messages", messages)
            result = XiezhiReviewer().review_and_strip_code(diffs_text, commits_text)
        except Exception as e:
            logger.exception("agentic run unexpected failure, degrading to diff_only: %s", e)
            notifier.send_notification(content=f"[agentic] run failed: {e}; falling back to diff_only")
            degraded = True
            run_meta.setdefault("messages", messages)
            result = XiezhiReviewer().review_and_strip_code(diffs_text, commits_text)

        if not degraded and not _looks_like_review(result):
            logger.warning(
                "agent output missing 总分 marker (len=%d), degrading to diff_only",
                len(result or ""),
            )
            notifier.send_notification(
                content="[agentic] output missing 总分 marker; falling back to diff_only"
            )
            degraded = True
            result = XiezhiReviewer().review_and_strip_code(diffs_text, commits_text)

        run_messages = run_meta.get("messages", messages)
        log_entry = ReviewLog(
            event="agentic_review",
            project=self.repo_key,
            ref=self.ref,
            strategy="agentic",
            iterations=run_meta.get("iterations", 0),
            total_tokens_est=_estimate_tokens(run_messages),
            duration_ms=int((time.monotonic() - start) * 1000),
            review_result_length=len(result),
            score=XiezhiReviewer.parse_review_score(review_text=result),
            degraded=degraded,
            tool_calls=_collect_tool_calls(run_messages),
        )
        logger.info(json.dumps(asdict(log_entry), ensure_ascii=False))
        return result