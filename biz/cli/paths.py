"""CLI 配置目录解析。"""
from __future__ import annotations

import os
import shutil
import sys
from pathlib import Path


def home_dir() -> Path:
    """用户级配置目录（默认 ~/.xiezhi）。"""
    override = os.getenv("XIEZHI_USER_HOME")
    if override:
        return Path(override).expanduser().resolve()
    return Path.home() / ".xiezhi"


def _is_conf_dir(p: Path) -> bool:
    return (p / "scr").is_dir() and (p / "prompt_templates.yml").is_file()


def _iter_search_bases() -> list[Path]:
    bases: list[Path] = []
    bases.append(home_dir())

    if getattr(sys, "frozen", False):
        bases.append(Path(sys.executable).resolve().parent)
        meipass = getattr(sys, "_MEIPASS", None)
        if meipass:
            bases.append(Path(meipass))

    cwd = Path.cwd().resolve()
    bases.extend([cwd, *cwd.parents])

    try:
        tool_root = Path(__file__).resolve().parents[2]
        bases.append(tool_root)
    except Exception:
        pass

    seen: set[Path] = set()
    ordered: list[Path] = []
    for b in bases:
        try:
            key = b.resolve()
        except Exception:
            key = b
        if key in seen:
            continue
        seen.add(key)
        ordered.append(key)
    return ordered


def find_asset_conf() -> Path | None:
    env_conf = os.getenv("XIEZHI_CONF_DIR") or os.getenv("XIEZHI_HOME")
    if env_conf:
        p = Path(env_conf).expanduser().resolve()
        if _is_conf_dir(p):
            return p
        if (p / "conf").is_dir() and _is_conf_dir(p / "conf"):
            return p / "conf"

    for base in _iter_search_bases():
        if _is_conf_dir(base):
            return base
        conf = base / "conf"
        if _is_conf_dir(conf):
            return conf
    return None


def find_conf_root() -> Path | None:
    return find_asset_conf()


def find_env_file() -> Path | None:
    candidates = [
        home_dir() / ".env",
        home_dir() / "conf" / ".env",
    ]
    asset = find_asset_conf()
    if asset:
        candidates.append(asset / ".env")
        candidates.append(asset.parent / ".env")
    for c in candidates:
        if c.is_file():
            return c
    return None


def init_user_home(force: bool = False) -> Path:
    """初始化 ~/.xiezhi（复制 .env 与 scr 资产）。"""
    home = home_dir()
    home.mkdir(parents=True, exist_ok=True)
    asset = find_asset_conf()
    if asset is None:
        raise FileNotFoundError(
            "未找到内置/旁路配置资产，无法初始化用户目录。\n"
            "请使用完整发布包（含 conf/），或设置 XIEZHI_HOME。"
        )

    dest_env = home / ".env"
    src_env = asset / ".env.dist"
    if not src_env.is_file():
        src_env = asset / ".env"
    if src_env.is_file() and (force or not dest_env.exists()):
        shutil.copy2(src_env, dest_env)

    if force or not (home / "scr").exists():
        if (home / "scr").exists():
            shutil.rmtree(home / "scr")
        shutil.copytree(asset / "scr", home / "scr")
    prompt = asset / "prompt_templates.yml"
    if prompt.is_file() and (force or not (home / "prompt_templates.yml").exists()):
        shutil.copy2(prompt, home / "prompt_templates.yml")

    return home


def ensure_runtime_paths() -> Path:
    conf = find_asset_conf()
    if conf is None:
        raise FileNotFoundError(
            "未找到獬豸配置目录（需要 scr/ 与 prompt_templates.yml）。\n"
            "请使用完整发布包，或执行：\n"
            "  xiezhi config init\n"
            "亦可设置：\n"
            "  XIEZHI_HOME=/path/to/release   # 含 conf/ 的目录\n"
            "  XIEZHI_CONF_DIR=/path/to/conf"
        )

    home = home_dir()
    if _is_conf_dir(home):
        conf = home

    os.environ.setdefault("SCR_RULES_DIR", str(conf / "scr" / "rules"))
    os.environ.setdefault("SCR_ALLOWLIST_DIR", str(conf / "scr" / "allowlist"))
    os.environ.setdefault("XIEZHI_PROMPT_TEMPLATES", str(conf / "prompt_templates.yml"))
    os.environ.setdefault("XIEZHI_CONF_DIR", str(conf))

    env_file = find_env_file()
    if env_file is not None:
        try:
            from dotenv import load_dotenv
            load_dotenv(env_file, override=False)
        except Exception:
            pass

    return conf
