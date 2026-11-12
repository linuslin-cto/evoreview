"""行级 inline 评论渲染。"""
from __future__ import annotations


def format_inline_body(comment) -> str:
    emoji = getattr(comment, "severity_emoji", "🔵")
    category = getattr(comment, "category", "other")
    severity = getattr(comment, "severity", "medium")
    lines = [f"{emoji} **[{category}/{severity}]** {comment.content}"]
    suggestion = getattr(comment, "suggestion_code", "") or ""
    if suggestion.strip():
        lines.append("")
        lines.append("**建议：**")
        lines.append("```suggestion")
        lines.extend(suggestion.splitlines())
        lines.append("```")
    lines.append("")
    lines.append("<sub>🤖 由獬豸生成</sub>")
    return "\n".join(lines)
