"""Agent tool implementations."""
from biz.agent.tool_registry import ToolRegistry
from biz.agent.tools.read_file import ReadFileTool
from biz.agent.tools.run_command import RunCommandTool


def register_default_tools(
    registry: ToolRegistry,
    repo_root,
    *,
    allowlist: list[str] | None = None,
    blocklist: list[str] | None = None,
) -> None:
    """Register default tools bound to repo_root."""
    registry.register(ReadFileTool(repo_root))
    registry.register(RunCommandTool(repo_root, allowlist=allowlist, blocklist=blocklist))


__all__ = [
    "ReadFileTool",
    "RunCommandTool",
    "register_default_tools",
]