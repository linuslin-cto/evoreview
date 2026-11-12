"""统一各 LLM 供应商的 tool-use 调用协议。"""
from __future__ import annotations

import json
import re
from dataclasses import dataclass
from typing import Any

from biz.agent.tool import ToolResult
from biz.llm.client.base import BaseClient
from biz.utils.log import logger


@dataclass
class ToolCall:
    id: str
    name: str
    arguments: dict


@dataclass
class LLMResponse:
    content: str | None
    tool_calls: list[ToolCall]
    raw: Any = None


_JSON_TOOL_BLOCK = re.compile(
    r"\{(?:[^{}]|\{[^{}]*\})*?\"tool\"\s*:\s*\"[^\"]+\"(?:[^{}]|\{[^{}]*\})*?\}",
    re.DOTALL,
)

_JSON_PROTOCOL_INSTRUCTION = (
    "\n\nWhen you need to call a tool, emit a single JSON object on its own "
    "line, of the form:\n"
    '{"tool": "<tool_name>", "args": {<json_args>}, "id": "<unique_id>"}\n'
    "Otherwise respond with plain text. Do NOT wrap JSON in markdown fences."
)


class LLMAdapter:
    def __init__(self, client, use_native: bool | None = None):
        self.client = client
        if use_native is None:
            use_native = (
                hasattr(type(client), "chat_with_tools")
                and getattr(type(client), "chat_with_tools", None)
                is not BaseClient.chat_with_tools
            )
        self.use_native = use_native


    def completions_with_tools(
        self,
        messages: list[dict],
        tools: list[dict] | None = None,
    ) -> LLMResponse:
        if self.use_native:
            return self._native(messages, tools)
        return self._json_fallback(messages, tools)

    def build_assistant_message(self, resp: LLMResponse) -> dict:
        if self.use_native:
            return {
                "role": "assistant",
                "content": resp.content or "",
                "tool_calls": [
                    {
                        "id": tc.id,
                        "type": "function",
                        "function": {
                            "name": tc.name,
                            "arguments": json.dumps(tc.arguments, ensure_ascii=False),
                        },
                    }
                    for tc in resp.tool_calls
                ],
            }
        content = resp.content or ""
        for tc in resp.tool_calls:
            content = content.replace(tc.raw_block, "") if hasattr(tc, "raw_block") else content
        return {"role": "assistant", "content": content.strip()}

    def build_tool_message(self, call: ToolCall, result: ToolResult) -> dict:
        body = result.output if result.success else f"ERROR: {result.error}"
        return {
            "role": "tool",
            "tool_call_id": call.id,
            "content": body,
        }


    def _native(self, messages, tools):
        raw = self.client.chat_with_tools(messages=messages, tools=tools)
        return LLMResponse(
            content=raw.get("content"),
            tool_calls=[
                ToolCall(id=tc["id"], name=tc["name"], arguments=tc.get("arguments") or {})
                for tc in raw.get("tool_calls", [])
            ],
            raw=raw.get("raw"),
        )

    def _json_fallback(self, messages, tools):
        if messages and messages[0].get("role") == "system":
            messages = list(messages)
            messages[0] = {**messages[0], "content": messages[0]["content"] + _JSON_PROTOCOL_INSTRUCTION}
        else:
            messages = [{"role": "system", "content": _JSON_PROTOCOL_INSTRUCTION.lstrip()}] + list(messages)
        text = self.client.completions(messages=messages)
        tool_calls: list[ToolCall] = []
        for m in _JSON_TOOL_BLOCK.finditer(text):
            block = m.group(0)
            try:
                obj = json.loads(block)
            except json.JSONDecodeError:
                continue
            name = obj.get("tool")
            args = obj.get("args", {})
            call_id = obj.get("id") or f"call_{len(tool_calls)}"
            if not name or not isinstance(args, dict):
                continue
            tc = ToolCall(id=call_id, name=name, arguments=args)
            tc.raw_block = block
            tool_calls.append(tc)
        cleaned = _JSON_TOOL_BLOCK.sub("", text).strip()
        return LLMResponse(content=cleaned, tool_calls=tool_calls, raw=text)
