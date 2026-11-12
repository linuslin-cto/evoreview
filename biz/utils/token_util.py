from typing import Optional

import tiktoken

from biz.utils.log import logger

_encoding_cache: Optional[tiktoken.Encoding] = None


def _get_encoding_cached(encoding_name: str = "cl100k_base") -> Optional[tiktoken.Encoding]:
    global _encoding_cache
    if _encoding_cache is not None:
        return _encoding_cache
    try:
        _encoding_cache = tiktoken.get_encoding(encoding_name)
        return _encoding_cache
    except Exception as e:
        logger.warning(f"Failed to load tiktoken encoding '{encoding_name}': {e}")
        return None


def _estimate_tokens_simple(text: str) -> int:
    """离线估算：中文约 2 字/token，其它约 4 字/token。"""
    if not text:
        return 0
    chinese_chars = sum(1 for c in text if '\u4e00' <= c <= '\u9fff')
    other_chars = len(text) - chinese_chars
    return max(chinese_chars // 2 + other_chars // 4, 1)


def count_tokens(text: str) -> int:
    encoding = _get_encoding_cached()
    if encoding is not None:
        return len(encoding.encode(text))
    return _estimate_tokens_simple(text)


def truncate_text_by_tokens(text: str, max_tokens: int, encoding_name: str = "cl100k_base") -> str:
    encoding = _get_encoding_cached(encoding_name)
    if encoding is not None:
        tokens = encoding.encode(text)
        if len(tokens) > max_tokens:
            return encoding.decode(tokens[:max_tokens])
        return text

    if not text:
        return text
    chinese_chars = sum(1 for c in text if '\u4e00' <= c <= '\u9fff')
    other_chars = len(text) - chinese_chars
    chars_per_token = 2 if chinese_chars > other_chars else 4
    max_chars = max_tokens * chars_per_token
    return text if len(text) <= max_chars else text[:max_chars]
