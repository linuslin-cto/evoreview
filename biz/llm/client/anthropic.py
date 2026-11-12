import os
from typing import Any, Dict, List, Optional

import httpx
from anthropic import Anthropic

from biz.llm.client.base import BaseClient
from biz.llm.types import NotGiven, NOT_GIVEN


def _split_system(messages: List[Dict]) -> tuple[Optional[str], List[Dict]]:
    system_message = None
    anthropic_messages: List[Dict] = []
    for msg in messages:
        role = msg.get("role")
        content = msg.get("content")
        if role == "system":
            system_message = content
        else:
            anthropic_messages.append({"role": role, "content": content})
    return system_message, anthropic_messages


def _to_anthropic_tools(tools: Optional[List[Dict]]) -> List[Dict[str, Any]]:
    """Convert OpenAI-style tool schemas to Anthropic tool definitions."""
    converted: List[Dict[str, Any]] = []
    for item in tools or []:
        fn = item.get("function") if isinstance(item, dict) else None
        if not fn and isinstance(item, dict) and "name" in item:
            fn = item
        if not fn:
            continue
        converted.append({
            "name": fn["name"],
            "description": fn.get("description") or "",
            "input_schema": fn.get("parameters") or {"type": "object", "properties": {}},
        })
    return converted


class AnthropicClient(BaseClient):
    def __init__(self, api_key: str = None):
        self.api_key = api_key or os.getenv("ANTHROPIC_API_KEY")
        self.base_url = os.getenv("ANTHROPIC_API_BASE_URL", None)
        if not self.api_key:
            raise ValueError("API key is required. Please provide it or set ANTHROPIC_API_KEY in the environment variables.")

        http_client = httpx.Client()

        if self.base_url:
            self.client = Anthropic(api_key=self.api_key, base_url=self.base_url, http_client=http_client)
        else:
            self.client = Anthropic(api_key=self.api_key, http_client=http_client)

        self.default_model = os.getenv("ANTHROPIC_API_MODEL", "claude-sonnet-4-5-20250929")

    def completions(self,
                    messages: List[Dict[str, str]],
                    model: Optional[str] | NotGiven = NOT_GIVEN,
                    ) -> str:
        model = model or self.default_model
        system_message, anthropic_messages = _split_system(messages)

        response = self.client.messages.create(
            model=model,
            system=system_message,
            messages=anthropic_messages,
            max_tokens=int(os.getenv("ANTHROPIC_MAX_TOKENS", "4096")),
        )

        if not response.content:
            return ""
        first = response.content[0]
        return getattr(first, "text", "") or ""

    def chat_with_tools(self,
                        messages: List[Dict],
                        tools: Optional[List[Dict]] = None,
                        model: Optional[str] | NotGiven = NOT_GIVEN,
                        ) -> Dict:
        model = model or self.default_model
        system_message, anthropic_messages = _split_system(messages)
        kwargs: Dict[str, Any] = {
            "model": model,
            "messages": anthropic_messages,
            "max_tokens": int(os.getenv("ANTHROPIC_MAX_TOKENS", "4096")),
        }
        if system_message is not None:
            kwargs["system"] = system_message
        anthropic_tools = _to_anthropic_tools(tools)
        if anthropic_tools:
            kwargs["tools"] = anthropic_tools

        completion = self.client.messages.create(**kwargs)

        content_parts: List[str] = []
        tool_calls: List[Dict] = []
        for block in completion.content or []:
            block_type = getattr(block, "type", None)
            if block_type == "text":
                content_parts.append(getattr(block, "text", "") or "")
            elif block_type == "tool_use":
                tool_calls.append({
                    "id": getattr(block, "id", "") or "",
                    "name": getattr(block, "name", "") or "",
                    "arguments": getattr(block, "input", None) or {},
                })

        return {
            "content": "\n".join(p for p in content_parts if p) or None,
            "tool_calls": tool_calls,
            "raw": completion,
        }
