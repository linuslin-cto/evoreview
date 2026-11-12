"""Load Jinja2-rendered prompt templates for the agent."""
from __future__ import annotations

from pathlib import Path
from typing import Any

import yaml
from jinja2 import Template


def load_prompt(prompt_key: str, style: str = "professional") -> dict[str, Any]:
    """Load `prompt_key` from conf/prompt_templates.yml, render with `style`.

    Returns {"system_message": {...}, "user_message": {...}}.
    """
    conf_path = Path(__file__).resolve().parents[2] / "conf" / "prompt_templates.yml"
    try:
        with open(conf_path, "r", encoding="utf-8") as f:
            config = yaml.safe_load(f) or {}
    except FileNotFoundError as e:
        raise RuntimeError(f"Prompt template file not found: {conf_path}") from e
    except yaml.YAMLError as e:
        raise RuntimeError(f"Failed to parse prompt templates at {conf_path}: {e}") from e

    prompts = config.get(prompt_key) or {}
    system = prompts.get("system_prompt")
    user = prompts.get("user_prompt")
    if not system or not user:
        raise KeyError(
            f"Prompt key '{prompt_key}' is missing 'system_prompt' or 'user_prompt' in {conf_path}"
        )

    def render(s: str) -> str:
        return Template(s).render(style=style)

    return {
        "system_message": {"role": "system", "content": render(system)},
        "user_message": {"role": "user", "content": render(user)},
    }
