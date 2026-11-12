"""SCR 逐文件结构化审查器。"""
from __future__ import annotations

import json
import os
import re
from concurrent.futures import ThreadPoolExecutor, as_completed

import yaml
from jinja2 import Template

from biz.llm.factory import Factory
from biz.scr.anchor import resolve_line_numbers
from biz.scr.env import getenv
from biz.scr.models import SEVERITY_PENALTY, FileReviewResult, ScrComment
from biz.scr.rules import RuleResolver
from biz.utils.log import logger
from biz.utils.token_util import count_tokens, truncate_text_by_tokens

def _prompt_file() -> str:
    return os.getenv("XIEZHI_PROMPT_TEMPLATES", os.path.join("conf", "prompt_templates.yml"))


def _filter_prompt_files() -> tuple[str, str]:
    conf = os.getenv("XIEZHI_CONF_DIR")
    if conf:
        base = os.path.join(conf, "scr", "prompts")
        return (
            os.path.join(base, "review_filter_task_system.md"),
            os.path.join(base, "review_filter_task_user.md"),
        )
    return (
        os.path.join("conf", "scr", "prompts", "review_filter_task_system.md"),
        os.path.join("conf", "scr", "prompts", "review_filter_task_user.md"),
    )


def _extract_json(text: str) -> dict:
    if not text:
        return {}
    cleaned = text.strip()
    fence = re.search(r"```(?:json)?\s*(.*?)```", cleaned, re.DOTALL)
    if fence:
        cleaned = fence.group(1).strip()
    try:
        return json.loads(cleaned)
    except json.JSONDecodeError:
        pass
    start = cleaned.find("{")
    end = cleaned.rfind("}")
    if start != -1 and end != -1 and end > start:
        try:
            return json.loads(cleaned[start:end + 1])
        except json.JSONDecodeError:
            return {}
    return {}


def _extract_json_array(text: str) -> list:
    if not text:
        return []
    cleaned = text.strip()
    fence = re.search(r"```(?:json)?\s*(.*?)```", cleaned, re.DOTALL)
    if fence:
        cleaned = fence.group(1).strip()
    try:
        parsed = json.loads(cleaned)
        return parsed if isinstance(parsed, list) else []
    except json.JSONDecodeError:
        pass
    start = cleaned.find("[")
    end = cleaned.rfind("]")
    if start != -1 and end != -1 and end > start:
        try:
            parsed = json.loads(cleaned[start:end + 1])
            return parsed if isinstance(parsed, list) else []
        except json.JSONDecodeError:
            return []
    return []


class ScrReviewer:
    def __init__(self):
        self.client = Factory().getClient()
        self.rules = RuleResolver()
        self.style = os.getenv("REVIEW_STYLE", "professional")
        self.concurrency = max(1, int(getenv("SCR_CONCURRENCY", "6")))
        self.filter_enabled = getenv("SCR_REVIEW_FILTER_ENABLED", "1") == "1"
        self.max_file_diff_tokens = int(getenv("SCR_MAX_FILE_DIFF_TOKENS", "6000"))
        self.max_files = int(getenv("SCR_MAX_FILES", "50"))
        self._prompts = self._load_prompts()

    def _load_prompts(self) -> dict:
        with open(_prompt_file(), "r", encoding="utf-8") as f:
            block = (yaml.safe_load(f) or {}).get("scr_review_prompt") or {}
        system = Template(block["system_prompt"]).render(style=self.style)
        user = Template(block["user_prompt"]).render(style=self.style)
        return {"system": system, "user": user}

    @staticmethod
    def _read_text(path: str) -> str:
        try:
            with open(path, "r", encoding="utf-8") as f:
                return f.read()
        except FileNotFoundError:
            return ""

    def _review_file(self, item: dict, change_files: str) -> FileReviewResult:
        path = item.get("new_path") or item.get("path") or "unknown"
        diff = item.get("diff", "") or ""
        additions = int(item.get("additions") or 0)
        deletions = int(item.get("deletions") or 0)
        if diff and not additions and not deletions:
            additions = len(re.findall(r"^\+(?!\+\+)", diff, re.MULTILINE))
            deletions = len(re.findall(r"^-(?!--)", diff, re.MULTILINE))
        result = FileReviewResult(
            path=path,
            additions=additions,
            deletions=deletions,
        )
        if not diff.strip():
            return result

        if count_tokens(diff) > self.max_file_diff_tokens:
            diff = truncate_text_by_tokens(diff, self.max_file_diff_tokens)
            result.warning = "diff 过大，已截断后审查"

        system_rule = self.rules.resolve(path) or ""
        user_content = self._prompts["user"].format(
            change_files=change_files,
            current_file_path=path,
            diff=diff,
            system_rule=system_rule,
        )
        messages = [
            {"role": "system", "content": self._prompts["system"]},
            {"role": "user", "content": user_content},
        ]
        try:
            raw = self.client.completions(messages=messages)
        except Exception as exc:  # 单文件失败隔离，不影响整体
            logger.error("SCR 审查文件失败 %s: %s", path, exc)
            result.warning = f"审查调用失败: {exc}"
            return result

        payload = _extract_json(raw)
        for raw_comment in payload.get("comments", []) or []:
            if not isinstance(raw_comment, dict):
                continue
            content = (raw_comment.get("content") or "").strip()
            if not content:
                continue
            comment = ScrComment(
                path=path,
                content=content,
                existing_code=(raw_comment.get("existing_code") or "").strip(),
                suggestion_code=(raw_comment.get("suggestion_code") or "").strip(),
                start_line=int(raw_comment.get("start_line") or 0),
                category=raw_comment.get("category") or "other",
                severity=raw_comment.get("severity") or "medium",
            ).normalized()
            result.comments.append(comment)

        resolve_line_numbers(result.comments, diff)

        if self.filter_enabled and result.comments:
            self._apply_review_filter(path, diff, result.comments)
        return result

    def _apply_review_filter(self, path: str, diff: str, comments: list[ScrComment]) -> None:
        filter_system, filter_user = _filter_prompt_files()
        system = self._read_text(filter_system)
        user_tmpl = self._read_text(filter_user)
        if not system or not user_tmpl:
            return
        indexed = []
        for idx, c in enumerate(comments):
            cid = f"c-{idx}"
            snippet = c.existing_code or ""
            indexed.append(f'- id: {cid}\n  comment: {c.content}\n  code: {snippet}')
        comments_block = "\n".join(indexed)
        user = (
            user_tmpl.replace("{{path}}", path)
            .replace("{{diff}}", diff)
            .replace("{{comments}}", comments_block)
        )
        try:
            raw = self.client.completions(
                messages=[
                    {"role": "system", "content": system},
                    {"role": "user", "content": user},
                ]
            )
        except Exception as exc:
            logger.warning("SCR review filter 调用失败(%s)，跳过过滤: %s", path, exc)
            return
        bad_ids = set(_extract_json_array(raw))
        for idx, c in enumerate(comments):
            if f"c-{idx}" in bad_ids:
                c.filtered = True

    @staticmethod
    def score_from_results(results: list[FileReviewResult]) -> int:
        from biz.review.scoring import score_results
        return score_results(results)

    @staticmethod
    def compute_score(comments: list[ScrComment], files_reviewed: int | None = None) -> int:
        from biz.review.scoring import unified_score
        active = [c for c in comments if not c.filtered]
        if files_reviewed is None:
            files_reviewed = max(1, len({c.path for c in active}))
        return unified_score(active, files_reviewed)

    _compute_score = compute_score

    @staticmethod
    def severity_counts(results: list[FileReviewResult]) -> dict[str, int]:
        counts = {"critical": 0, "high": 0, "medium": 0, "low": 0}
        for r in results:
            for c in r.comments:
                if not c.filtered:
                    counts[c.severity] = counts.get(c.severity, 0) + 1
        return counts

    @staticmethod
    def render_findings(results: list[FileReviewResult], heading: str = "##") -> str:
        severity_order = {"critical": 0, "high": 1, "medium": 2, "low": 3}
        lines: list[str] = []
        for result in results:
            file_comments = [c for c in result.comments if not c.filtered]
            if not file_comments and not result.warning:
                continue
            lines.append(f"{heading} 📄 `{result.path}`")
            if result.warning:
                lines.append(f"> ⚠️ {result.warning}")
            file_comments.sort(key=lambda c: severity_order.get(c.severity, 9))
            for c in file_comments:
                loc = f"L{c.start_line}" if c.start_line > 0 else "未锚定"
                if c.end_line > c.start_line > 0:
                    loc = f"L{c.start_line}-L{c.end_line}"
                lines.append(
                    f"- {c.severity_emoji} **[{c.category}/{c.severity}]** ({loc}) {c.content}"
                )
                if c.suggestion_code:
                    lines.append("")
                    lines.append("  建议：")
                    lines.append("")
                    lines.append("  ```")
                    for sline in c.suggestion_code.splitlines():
                        lines.append(f"  {sline}")
                    lines.append("  ```")
            lines.append("")
        return "\n".join(lines).rstrip()

    def _render(self, results: list[FileReviewResult], score: int) -> str:
        active = [c for r in results for c in r.comments if not c.filtered]
        filtered_count = sum(1 for r in results for c in r.comments if c.filtered)
        counts = self.severity_counts(results)

        lines: list[str] = []
        lines.append("# 🛡️ SCR 结构化代码审查报告")
        lines.append("")
        lines.append(
            f"> 文件数: {len(results)} · 有效问题: {len(active)}"
            + (f" · 已过滤误报: {filtered_count}" if filtered_count else "")
        )
        lines.append("")
        lines.append(
            "**问题分布**："
            f"🔴 critical {counts['critical']} · "
            f"🟠 high {counts['high']} · "
            f"🟡 medium {counts['medium']} · "
            f"🔵 low {counts['low']}"
        )
        lines.append("")

        if not active:
            lines.append("✅ 未发现明显问题，代码质量良好。")
        else:
            lines.append(self.render_findings(results))
            lines.append("")

        from biz.review.scoring import score_grade

        lines.append("---")
        lines.append(f"总分:{score}分（{score_grade(score)}）")
        return "\n".join(lines)

    def review_structured(self, changes: list) -> list[FileReviewResult]:
        if not changes:
            return []

        changes = changes[: self.max_files]
        change_files = "\n".join(
            f"- {item.get('new_path') or item.get('path')}" for item in changes
        )

        results: list[FileReviewResult] = [None] * len(changes)  # type: ignore
        with ThreadPoolExecutor(max_workers=min(self.concurrency, len(changes))) as pool:
            future_map = {
                pool.submit(self._review_file, item, change_files): idx
                for idx, item in enumerate(changes)
            }
            for future in as_completed(future_map):
                idx = future_map[future]
                try:
                    results[idx] = future.result()
                except Exception as exc:  # pragma: no cover - defensive
                    item = changes[idx]
                    path = item.get("new_path") or item.get("path") or "unknown"
                    logger.error("SCR 子任务异常 %s: %s", path, exc)
                    results[idx] = FileReviewResult(path=path, warning=f"子任务异常: {exc}")

        return [r for r in results if r is not None]

    def render_report(self, results: list[FileReviewResult], score: int | None = None) -> str:
        if score is None:
            score = self.score_from_results(results)
        return self._render(results, score)

    @staticmethod
    def active_comments(results: list[FileReviewResult]) -> list[ScrComment]:
        return [c for r in results for c in r.comments if not c.filtered]

    def review(self, changes: list, commits_text: str = "") -> str:
        results = self.review_structured(changes)
        if not results:
            return "代码为空\n\n总分:100分"
        active = self.active_comments(results)
        score = self.score_from_results(results)
        from biz.review.pipeline import compose_report

        report = compose_report(results, score)
        logger.info(
            "SCR 审查完成: files=%d comments=%d score=%d",
            len(results),
            len(active),
            score,
        )
        return report
