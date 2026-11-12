"""本地 Git Diff 采集（workspace / commit / range）。"""
from __future__ import annotations

import os
import re
import subprocess
from dataclasses import dataclass, field
from pathlib import Path
from typing import Literal

from biz.scr.rules import RuleResolver

ProviderNoiseDirs = (
    "vendor/",
    "node_modules/",
    "target/",
    "dist/",
    "build/",
    ".git/",
    "__pycache__/",
    ".venv/",
    "venv/",
)


@dataclass
class DiffPreviewItem:
    path: str
    additions: int = 0
    deletions: int = 0
    included: bool = True
    reason: str = ""


@dataclass
class LocalDiffBundle:
    mode: Literal["workspace", "commit", "range"]
    repo_dir: str
    changes: list[dict] = field(default_factory=list)
    preview: list[DiffPreviewItem] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)


def _run_git(repo_dir: str, args: list[str], check: bool = True) -> str:
    # core.quotepath=false keeps non-ASCII paths readable for filtering/anchoring
    result = subprocess.run(
        ["git", "-c", "core.quotepath=false", *args],
        cwd=repo_dir,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    if check and result.returncode != 0:
        msg = (result.stderr or result.stdout or "").strip()
        raise RuntimeError(f"git {' '.join(args)} 失败: {msg}")
    return result.stdout


def _is_noise_path(path: str) -> bool:
    norm = path.replace("\\", "/")
    return any(part in norm for part in ProviderNoiseDirs) or any(
        f"/{d}" in f"/{norm}" for d in ("vendor", "node_modules", "target", "dist", "build", "__pycache__")
    )


def _count_lines(diff: str) -> tuple[int, int]:
    additions = len(re.findall(r"^\+(?!\+\+)", diff, re.MULTILINE))
    deletions = len(re.findall(r"^-(?!--)", diff, re.MULTILINE))
    return additions, deletions


def _split_unified_diffs(raw: str) -> list[tuple[str, str]]:
    """Split multi-file `git diff` output into (path, diff_text) pairs."""
    if not raw.strip():
        return []
    chunks = re.split(r"(?=^diff --git )", raw, flags=re.MULTILINE)
    results: list[tuple[str, str]] = []
    for chunk in chunks:
        chunk = chunk.strip("\n")
        if not chunk.startswith("diff --git "):
            continue
        m = re.match(r"diff --git a/(.+?) b/(.+)$", chunk.splitlines()[0])
        if not m:
            continue
        path = m.group(2)
        hunk_start = chunk.find("\n@@")
        if hunk_start == -1:
            if "Binary files" in chunk or "GIT binary patch" in chunk:
                results.append((path, ""))
            continue
        meta = chunk.find("\n--- ")
        if meta != -1 and meta < hunk_start:
            diff_body = chunk[meta + 1 :]
        else:
            diff_body = chunk[hunk_start + 1 :]
        results.append((path, diff_body))
    return results


def _untracked_as_diff(repo_dir: str, path: str) -> str:
    """Synthesize a unified diff for an untracked file."""
    abs_path = os.path.join(repo_dir, path)
    try:
        with open(abs_path, "r", encoding="utf-8", errors="replace") as f:
            content = f.read()
    except OSError:
        return ""
    if "\x00" in content:
        return ""
    lines = content.splitlines()
    header = [
        f"--- /dev/null",
        f"+++ b/{path}",
        f"@@ -0,0 +1,{len(lines)} @@",
    ]
    body = [f"+{line}" for line in lines]
    return "\n".join(header + body) + ("\n" if body else "")


def _decide(path: str, diff: str, rules: RuleResolver) -> DiffPreviewItem:
    additions, deletions = _count_lines(diff) if diff else (0, 0)
    item = DiffPreviewItem(path=path, additions=additions, deletions=deletions)
    if _is_noise_path(path):
        item.included = False
        item.reason = "noise_dir"
        return item
    if not diff:
        item.included = False
        item.reason = "binary_or_empty"
        return item
    if rules.is_excluded_by_default(path):
        item.included = False
        item.reason = "default_path"
        return item
    if not rules.is_supported_path(path):
        item.included = False
        item.reason = "unsupported_ext"
        return item
    item.included = True
    item.reason = ""
    return item


def _to_changes(preview: list[DiffPreviewItem], diff_map: dict[str, str]) -> list[dict]:
    changes = []
    for item in preview:
        if not item.included:
            continue
        changes.append(
            {
                "new_path": item.path,
                "diff": diff_map.get(item.path, ""),
                "additions": item.additions,
                "deletions": item.deletions,
            }
        )
    return changes


def collect_workspace_diffs(repo_dir: str, rules: RuleResolver | None = None) -> LocalDiffBundle:
    """Workspace vs HEAD (staged + unstaged) plus untracked files."""
    rules = rules or RuleResolver()
    repo_dir = os.path.abspath(repo_dir)
    warnings: list[str] = []

    head_raw = _run_git(repo_dir, ["diff", "HEAD", "--no-ext-diff", "--text", "--unified=3"], check=False)
    untracked_raw = _run_git(repo_dir, ["ls-files", "--others", "--exclude-standard"], check=False)

    diff_map: dict[str, str] = dict(_split_unified_diffs(head_raw))

    for line in untracked_raw.splitlines():
        path = line.strip()
        if not path or path in diff_map:
            continue
        diff_map[path] = _untracked_as_diff(repo_dir, path)

    preview = [_decide(path, diff, rules) for path, diff in sorted(diff_map.items())]
    return LocalDiffBundle(
        mode="workspace",
        repo_dir=repo_dir,
        changes=_to_changes(preview, diff_map),
        preview=preview,
        warnings=warnings,
    )


def collect_commit_diffs(repo_dir: str, commit: str, rules: RuleResolver | None = None) -> LocalDiffBundle:
    rules = rules or RuleResolver()
    repo_dir = os.path.abspath(repo_dir)
    raw = _run_git(repo_dir, ["show", "--pretty=format:", "--no-ext-diff", "--text", "--unified=3", commit])
    diff_map = dict(_split_unified_diffs(raw))
    preview = [_decide(path, diff, rules) for path, diff in sorted(diff_map.items())]
    return LocalDiffBundle(
        mode="commit",
        repo_dir=repo_dir,
        changes=_to_changes(preview, diff_map),
        preview=preview,
        warnings=[],
    )


def collect_range_diffs(
    repo_dir: str,
    from_ref: str,
    to_ref: str,
    rules: RuleResolver | None = None,
) -> LocalDiffBundle:
    """Range diff via merge-base(from,to)..to when possible."""
    rules = rules or RuleResolver()
    repo_dir = os.path.abspath(repo_dir)
    warnings: list[str] = []
    base = _run_git(repo_dir, ["merge-base", from_ref, to_ref], check=False).strip()
    if not base:
        warnings.append(f"无法计算 merge-base({from_ref}, {to_ref})，回退直连 {from_ref}..{to_ref}")
        base = from_ref
    raw = _run_git(
        repo_dir,
        ["diff", "--no-ext-diff", "--text", "--unified=3", f"{base}...{to_ref}"],
    )
    diff_map = dict(_split_unified_diffs(raw))
    preview = [_decide(path, diff, rules) for path, diff in sorted(diff_map.items())]
    return LocalDiffBundle(
        mode="range",
        repo_dir=repo_dir,
        changes=_to_changes(preview, diff_map),
        preview=preview,
        warnings=warnings,
    )


def _resolve_git_root(path: str) -> str:
    cur = Path(os.path.abspath(path))
    for p in [cur, *cur.parents]:
        if (p / ".git").exists():
            return str(p)
    return str(cur)


def _filter_to_subdir(bundle: LocalDiffBundle, subdir: str) -> LocalDiffBundle:
    """When repo_dir is a subdirectory, keep only paths under that prefix."""
    root = Path(bundle.repo_dir).resolve()
    target = Path(os.path.abspath(subdir)).resolve()
    try:
        rel = target.relative_to(root).as_posix()
    except ValueError:
        return bundle
    if rel in (".", ""):
        return bundle
    prefix = rel if rel.endswith("/") else rel + "/"

    def under(path: str) -> bool:
        norm = path.replace("\\", "/")
        return norm == rel or norm.startswith(prefix)

    preview = [i for i in bundle.preview if under(i.path)]
    changes = [c for c in bundle.changes if under(c.get("new_path", ""))]
    bundle.preview = preview
    bundle.changes = changes
    if not any(w.startswith("scope=") for w in bundle.warnings):
        bundle.warnings.append(f"scope={rel}（仅审查子目录内变更）")
    return bundle


def collect_local_diffs(
    repo_dir: str = ".",
    *,
    commit: str | None = None,
    from_ref: str | None = None,
    to_ref: str | None = None,
) -> LocalDiffBundle:
    """Entry point: workspace, single commit, or ref range."""
    user_path = os.path.abspath(repo_dir)
    git_root = _resolve_git_root(user_path)

    if commit:
        bundle = collect_commit_diffs(git_root, commit)
    elif from_ref or to_ref:
        if not (from_ref and to_ref):
            raise ValueError("--from 与 --to 需同时提供")
        bundle = collect_range_diffs(git_root, from_ref, to_ref)
    else:
        bundle = collect_workspace_diffs(git_root)

    return _filter_to_subdir(bundle, user_path)
