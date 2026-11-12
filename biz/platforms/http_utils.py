"""Shared HTTP helpers for Git platform clients."""
from __future__ import annotations

import os
from typing import Any

import requests

DEFAULT_TIMEOUT = (3.05, 27)  # connect, read


def ssl_verify() -> bool | str:
    """PLATFORM_SSL_VERIFY / PLATFORM_CA_BUNDLE。"""
    flag = os.getenv("PLATFORM_SSL_VERIFY", "1").strip().lower()
    if flag in ("0", "false", "no", "off"):
        return False
    ca = os.getenv("PLATFORM_CA_BUNDLE", "").strip()
    if ca:
        return ca
    return True


def platform_request(
    method: str,
    url: str,
    *,
    timeout: tuple[float, float] | float = DEFAULT_TIMEOUT,
    **kwargs: Any,
) -> requests.Response:
    kwargs.setdefault("verify", ssl_verify())
    kwargs.setdefault("timeout", timeout)
    return requests.request(method, url, **kwargs)


def platform_get(url: str, **kwargs: Any) -> requests.Response:
    return platform_request("GET", url, **kwargs)


def platform_post(url: str, **kwargs: Any) -> requests.Response:
    return platform_request("POST", url, **kwargs)
