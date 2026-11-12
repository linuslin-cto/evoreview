"""Lazy local clone/fetch of repositories for agentic review."""
from __future__ import annotations

import errno
import fcntl
import os
import re
import subprocess
import time
from contextlib import contextmanager
from pathlib import Path
from urllib.parse import quote, urlparse, urlunparse

from biz.utils.log import logger


def _sanitize_key(key: str) -> str:
    """Turn a project key into a safe directory name."""
    return re.sub(r"[^A-Za-z0-9._-]", "_", key)


def _pick_token_for_host(host: str | None) -> str | None:
    """按 host 选择对应平台的 access token。"""
    if not host:
        return None
    h = host.lower()
    if h == "github.com" or h.endswith(".github.com"):
        return os.getenv("GITHUB_ACCESS_TOKEN")
    if "gitea" in h:
        return os.getenv("GITEA_ACCESS_TOKEN")
    return os.getenv("GITLAB_ACCESS_TOKEN")


def _auth_url(url: str) -> str:
    """在 HTTPS URL 中注入当前 host 对应的 token。"""
    parsed = urlparse(url)
    if parsed.scheme not in ("http", "https"):
        return url
    if "@" in (parsed.netloc or ""):
        logger.warning("URL already contains credentials; skipping token injection")
        return url
    host = parsed.hostname
    if not host:
        return url
    token = _pick_token_for_host(host)
    if not token:
        return url
    userinfo = f"oauth2:{quote(token, safe='')}"
    return urlunparse(parsed._replace(netloc=f"{userinfo}@{parsed.netloc}"))


class LocalRepoSyncer:
    """惰性 clone / fetch+checkout；缓存目录带文件锁。"""

    def __init__(
        self,
        cache_root: Path | str,
        *,
        clone_timeout: int = 300,
        lock_wait_seconds: int = 60,
    ) -> None:
        self.cache_root = Path(cache_root)
        self.cache_root.mkdir(parents=True, exist_ok=True)
        self.clone_timeout = clone_timeout
        self.lock_wait_seconds = lock_wait_seconds

    def sync_to(self, *, url: str, key: str, ref: str) -> Path:
        """确保本地有该仓库并 checkout 到 ref。"""
        safe = _sanitize_key(key)
        target = self.cache_root / safe
        lock_path = self.cache_root / f"{safe}.lock"

        with self._lock(lock_path):
            if not (target / ".git").exists():
                self._clone(_auth_url(url), target)
            self._ensure_authenticated_remote(target)
            self._fetch_and_checkout(target, ref)
        return target

    def _ensure_authenticated_remote(self, target: Path) -> None:
        """用当前 token 刷新 origin URL。"""
        try:
            r = subprocess.run(
                ["git", "remote", "get-url", "origin"],
                cwd=target,
                check=True,
                capture_output=True,
                text=True,
                timeout=30,
            )
        except (subprocess.CalledProcessError, subprocess.TimeoutExpired, FileNotFoundError):
            return
        existing = r.stdout.strip()
        new_url = _auth_url(existing)
        if new_url == existing:
            return
        try:
            subprocess.run(
                ["git", "remote", "set-url", "origin", new_url],
                cwd=target,
                check=True,
                capture_output=True,
                text=True,
                timeout=30,
            )
        except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as e:
            stderr = (e.stderr or "").strip() if hasattr(e, "stderr") else str(e)
            raise RuntimeError(f"git remote set-url failed: {stderr}") from e

    @contextmanager
    def _lock(self, lock_path: Path):
        lock_path.touch(exist_ok=True)
        f = open(lock_path, "w")
        deadline = time.monotonic() + self.lock_wait_seconds
        while True:
            try:
                fcntl.flock(f.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except OSError as e:
                if e.errno not in (errno.EWOULDBLOCK, errno.EAGAIN):
                    raise
                if time.monotonic() >= deadline:
                    logger.warning("could not acquire lock %s within %ds, proceeding without lock",
                                   lock_path, self.lock_wait_seconds)
                    break
                time.sleep(0.5)
        try:
            yield
        finally:
            try:
                fcntl.flock(f.fileno(), fcntl.LOCK_UN)
            except OSError:
                pass
            f.close()

    def _clone(self, url: str, target: Path) -> None:
        logger.debug("cloning %s -> %s", url, target)
        try:
            subprocess.run(
                ["git", "clone", url, str(target)],
                check=True,
                capture_output=True,
                text=True,
                timeout=self.clone_timeout,
            )
        except subprocess.TimeoutExpired as e:
            raise RuntimeError(f"git clone timed out after {self.clone_timeout}s") from e
        except subprocess.CalledProcessError as e:
            raise RuntimeError(f"git clone failed: {e.stderr.strip()}") from e

    def _fetch_and_checkout(self, target: Path, ref: str) -> None:
        try:
            subprocess.run(
                ["git", "fetch", "--all", "--prune"],
                cwd=target, check=True, capture_output=True, text=True, timeout=self.clone_timeout,
            )
        except subprocess.CalledProcessError as e:
            raise RuntimeError(f"git fetch failed: {e.stderr.strip()}") from e
        is_sha = bool(re.fullmatch(r"[0-9a-fA-F]{7,40}", ref))
        cmd = ["git", "reset", "--hard", ref] if is_sha else ["git", "reset", "--hard", f"origin/{ref}"]
        try:
            subprocess.run(
                cmd, cwd=target, check=True, capture_output=True, text=True, timeout=self.clone_timeout,
            )
        except subprocess.CalledProcessError as e:
            raise RuntimeError(f"git checkout failed: {e.stderr.strip()}") from e
