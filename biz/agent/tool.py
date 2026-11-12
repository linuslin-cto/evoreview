"""Tool abstract base class for the agent."""
from __future__ import annotations

import abc
from dataclasses import dataclass


@dataclass
class ToolResult:
    """Result returned by a tool execution."""
    success: bool
    output: str
    error: str | None = None

    def __bool__(self) -> bool:
        return self.success and bool(self.output)


class Tool(abc.ABC):
    """Agent tool base: define name, description, parameters, and execute()."""
    name: str = ""
    description: str = ""
    parameters: dict = {}

    @abc.abstractmethod
    def execute(self, **kwargs) -> ToolResult:
        ...

    def to_schema(self) -> dict:
        """OpenAI-style tool schema."""
        return {
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": self.parameters,
            },
        }
