"""Unified diff 解析与行级锚定。"""
from __future__ import annotations

import re

from biz.scr.models import ScrComment

_HUNK_RE = re.compile(r"^@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@")


def parse_new_line_map(diff_text: str) -> list[tuple[int, str, bool]]:
    """返回新文件侧 (line_no, text, is_added)。"""
    result: list[tuple[int, str, bool]] = []
    new_line = 0
    in_hunk = False
    for raw in diff_text.splitlines():
        hunk = _HUNK_RE.match(raw)
        if hunk:
            new_line = int(hunk.group(1))
            in_hunk = True
            continue
        if not in_hunk:
            continue
        if raw.startswith("+++") or raw.startswith("---"):
            continue
        if raw.startswith("+"):
            result.append((new_line, raw[1:], True))
            new_line += 1
        elif raw.startswith("-"):
            continue
        elif raw.startswith("\\"):
            continue
        else:
            text = raw[1:] if raw.startswith(" ") else raw
            result.append((new_line, text, False))
            new_line += 1
    return result


def _split_snippet(snippet: str) -> list[str]:
    lines = [ln.strip() for ln in (snippet or "").splitlines()]
    return [ln for ln in lines if ln]


def anchor_comment(comment: ScrComment, line_map: list[tuple[int, str, bool]]) -> ScrComment:
    """用 existing_code 匹配行号。"""
    snippet = _split_snippet(comment.existing_code)
    if not snippet or not line_map:
        return comment

    stripped_map = [(ln_no, text.strip(), added) for ln_no, text, added in line_map]
    first = snippet[0]

    for prefer_added in (True, False):
        for idx, (ln_no, text, added) in enumerate(stripped_map):
            if added != prefer_added:
                continue
            if text != first:
                continue
            matched = 1
            j = idx + 1
            k = 1
            while k < len(snippet) and j < len(stripped_map):
                if stripped_map[j][1] == snippet[k]:
                    matched += 1
                    k += 1
                    j += 1
                else:
                    break
            comment.start_line = ln_no
            comment.end_line = stripped_map[min(idx + matched - 1, len(stripped_map) - 1)][0]
            return comment

    for ln_no, text, _added in stripped_map:
        if first and first in text:
            comment.start_line = ln_no
            comment.end_line = ln_no
            return comment
    return comment


def resolve_line_numbers(comments: list[ScrComment], diff_text: str) -> list[ScrComment]:
    line_map = parse_new_line_map(diff_text)
    for comment in comments:
        if comment.existing_code:
            anchor_comment(comment, line_map)
    return comments
