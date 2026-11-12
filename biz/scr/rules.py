"""按路径 glob 解析审查规则文档。"""
from __future__ import annotations

import json
import os
import re
from functools import lru_cache

from biz.utils.log import logger

_DEFAULT_RULES_DIR = os.path.join("conf", "scr", "rules")
_DEFAULT_ALLOWLIST_DIR = os.path.join("conf", "scr", "allowlist")


def _expand_braces(pattern: str) -> list[str]:
    """展开一层花括号，如 `*.{ts,js}` -> [`*.ts`, `*.js`]。"""
    match = re.search(r"\{([^{}]*)\}", pattern)
    if not match:
        return [pattern]
    prefix = pattern[: match.start()]
    suffix = pattern[match.end():]
    results: list[str] = []
    for option in match.group(1).split(","):
        results.extend(_expand_braces(prefix + option + suffix))
    return results


def _glob_to_regex(pattern: str) -> str:
    """把 glob（支持 ** / * / ?）转换为正则，按 POSIX 路径匹配。"""
    i = 0
    n = len(pattern)
    out = ["^"]
    while i < n:
        c = pattern[i]
        if c == "*":
            if i + 1 < n and pattern[i + 1] == "*":
                i += 2
                if i < n and pattern[i] == "/":
                    i += 1
                    out.append("(?:.*/)?")
                else:
                    out.append(".*")
                continue
            out.append("[^/]*")
        elif c == "?":
            out.append("[^/]")
        elif c == ".":
            out.append(r"\.")
        else:
            out.append(re.escape(c))
        i += 1
    out.append("$")
    return "".join(out)


class RuleResolver:
    """加载并缓存规则映射、规则文档与扩展名白名单。"""

    def __init__(self, rules_dir: str | None = None, allowlist_dir: str | None = None):
        from biz.scr.env import getenv
        self.rules_dir = rules_dir or getenv("SCR_RULES_DIR", _DEFAULT_RULES_DIR)
        self.allowlist_dir = allowlist_dir or getenv("SCR_ALLOWLIST_DIR", _DEFAULT_ALLOWLIST_DIR)
        self._system_rules = self._load_system_rules()
        self._compiled: list[tuple[re.Pattern, str]] = []
        for glob_pattern, rule_file in self._system_rules.get("path_rule_map", {}).items():
            for expanded in _expand_braces(glob_pattern):
                try:
                    self._compiled.append((re.compile(_glob_to_regex(expanded), re.IGNORECASE), rule_file))
                except re.error as exc:  # pragma: no cover - defensive
                    logger.warning("SCR 规则 glob 编译失败 %s: %s", expanded, exc)
        self._default_rule_file = self._system_rules.get("default_rule", "default.md")

    def _load_system_rules(self) -> dict:
        path = os.path.join(self.rules_dir, "system_rules.json")
        try:
            with open(path, "r", encoding="utf-8") as f:
                return json.load(f)
        except (FileNotFoundError, json.JSONDecodeError) as exc:
            logger.warning("SCR system_rules.json 加载失败(%s)，使用兜底默认规则: %s", path, exc)
            return {"default_rule": "default.md", "path_rule_map": {}}

    @lru_cache(maxsize=64)
    def _read_rule_doc(self, rule_file: str) -> str:
        path = os.path.join(self.rules_dir, "rule_docs", rule_file)
        try:
            with open(path, "r", encoding="utf-8") as f:
                return f.read().strip()
        except FileNotFoundError:
            logger.warning("SCR 规则文档缺失: %s", path)
            return ""

    def rule_file_for(self, path: str) -> str:
        norm = path.replace("\\", "/")
        while norm.startswith("./"):
            norm = norm[2:]
        for regex, rule_file in self._compiled:
            if regex.match(norm):
                return rule_file
        return self._default_rule_file

    def resolve(self, path: str) -> str:
        rule_file = self.rule_file_for(path)
        doc = self._read_rule_doc(rule_file)
        if not doc and rule_file != self._default_rule_file:
            doc = self._read_rule_doc(self._default_rule_file)
        return doc

    @lru_cache(maxsize=1)
    def supported_extensions(self) -> frozenset[str]:
        path = os.path.join(self.allowlist_dir, "supported_file_types.json")
        try:
            with open(path, "r", encoding="utf-8") as f:
                return frozenset(json.load(f))
        except (FileNotFoundError, json.JSONDecodeError):
            return frozenset()

    @lru_cache(maxsize=1)
    def default_exclude_patterns(self) -> tuple[re.Pattern, ...]:
        path = os.path.join(self.allowlist_dir, "default_exclude_patterns.json")
        try:
            with open(path, "r", encoding="utf-8") as f:
                patterns = json.load(f)
        except (FileNotFoundError, json.JSONDecodeError):
            return ()
        compiled: list[re.Pattern] = []
        for pattern in patterns:
            for expanded in _expand_braces(pattern):
                try:
                    compiled.append(re.compile(_glob_to_regex(expanded), re.IGNORECASE))
                except re.error as exc:  # pragma: no cover
                    logger.warning("SCR exclude glob 编译失败 %s: %s", expanded, exc)
        return tuple(compiled)

    def is_excluded_by_default(self, path: str) -> bool:
        norm = path.replace("\\", "/")
        while norm.startswith("./"):
            norm = norm[2:]
        return any(rx.match(norm) for rx in self.default_exclude_patterns())

    def is_supported_path(self, path: str) -> bool:
        """扩展名白名单检测；若白名单为空则回退环境变量 SUPPORTED_EXTENSIONS。"""
        norm = path.replace("\\", "/")
        exts = self.supported_extensions()
        if not exts:
            raw = os.getenv("SUPPORTED_EXTENSIONS", ".java,.py,.php,.ts,.tsx,.js,.jsx,.go")
            exts = frozenset(item.strip() for item in raw.split(",") if item.strip())
        return any(norm.endswith(ext) for ext in exts)
