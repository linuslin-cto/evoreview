"""Path sandboxing and sensitive path detection for agent tools."""
import re
from pathlib import Path

SENSITIVE_PATH_PATTERNS: tuple[re.Pattern, ...] = tuple(
    re.compile(p) for p in (
        r"(^|/)(\.env|\.env\.[^/]+)$",
        r"(^|/)id_rsa(\.pub)?$",
        r"(^|/)\.git/",
        r"(^|/)\.ssh/",
        r"\.pem$",
        r"\.key$",
    )
)


def is_path_safe(candidate: Path | str, repo_root: Path | str) -> bool:
    """候选路径是否落在仓库根目录内。"""
    try:
        candidate_resolved = Path(candidate).resolve(strict=False)
        root_resolved = Path(repo_root).resolve(strict=False)
        return candidate_resolved.is_relative_to(root_resolved)
    except (OSError, ValueError):
        return False


def is_sensitive_path(path: Path | str) -> bool:
    """是否匹配敏感路径模式。"""
    return any(pat.search(Path(path).as_posix()) for pat in SENSITIVE_PATH_PATTERNS)
