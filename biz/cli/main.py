"""獬豸 CLI 入口：`xiezhi <subcommand>`。"""
from __future__ import annotations

import argparse
import sys

from biz.cli import __version__


def _cmd_review(args: argparse.Namespace) -> int:
    from biz.cmd.diff_review import run
    return run(args)


def _cmd_menu(_: argparse.Namespace) -> int:
    """交互式菜单（原 python -m biz.cmd.review）。"""
    from biz.cli.paths import ensure_runtime_paths
    from biz.cmd.review import get_func_choice, welcome_message

    try:
        ensure_runtime_paths()
    except FileNotFoundError as e:
        print(str(e), file=sys.stderr)
        return 2

    welcome_message()
    FuncClass = get_func_choice()
    FuncClass().process()
    return 0


def _cmd_version(_: argparse.Namespace) -> int:
    print(f"xiezhi {__version__}")
    return 0


def _cmd_config(args: argparse.Namespace) -> int:
    """配置管理（init / path）。"""
    from biz.cli.paths import find_asset_conf, find_env_file, home_dir, init_user_home

    action = getattr(args, "config_action", None) or "path"
    if action == "init":
        try:
            home = init_user_home(force=bool(args.force))
        except FileNotFoundError as e:
            print(str(e), file=sys.stderr)
            return 2
        print(f"已初始化用户配置目录: {home}")
        print(f"请编辑密钥文件: {home / '.env'}")
        print("至少填写 LLM_PROVIDER 与对应 API Key（如 DEEPSEEK_API_KEY）。")
        return 0

    if action == "path":
        print(f"user_home={home_dir()}")
        env = find_env_file()
        print(f"env_file={env if env else '(未找到)'}")
        asset = find_asset_conf()
        print(f"asset_conf={asset if asset else '(未找到)'}")
        return 0

    print(f"未知 config 动作: {action}", file=sys.stderr)
    return 2


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="xiezhi",
        description="獬豸 AI 代码审查 CLI —— 本地 Diff 审查。",
    )
    parser.add_argument(
        "-V", "--version",
        action="version",
        version=f"xiezhi {__version__}",
    )
    sub = parser.add_subparsers(dest="command", metavar="COMMAND")

    from biz.cmd.diff_review import add_review_arguments
    p_review = sub.add_parser(
        "review",
        help="本地 Diff 审查（workspace / commit / 范围）",
        description="提交前自审、commit 审查或范围 Diff，复用 SCR 融合能力。",
    )
    add_review_arguments(p_review)
    p_review.set_defaults(func=_cmd_review)

    p_menu = sub.add_parser("menu", help="打开交互式审查菜单")
    p_menu.set_defaults(func=_cmd_menu)

    p_cfg = sub.add_parser("config", help="配置管理（init / path）")
    cfg_sub = p_cfg.add_subparsers(dest="config_action", metavar="ACTION")
    p_init = cfg_sub.add_parser("init", help="初始化 ~/.xiezhi 用户配置目录")
    p_init.add_argument("--force", action="store_true", help="覆盖已有配置资产")
    p_init.set_defaults(func=_cmd_config, config_action="init")
    p_path = cfg_sub.add_parser("path", help="打印当前配置路径")
    p_path.set_defaults(func=_cmd_config, config_action="path")
    p_cfg.set_defaults(func=_cmd_config, config_action="path")

    p_ver = sub.add_parser("version", help="显示版本号")
    p_ver.set_defaults(func=_cmd_version)

    return parser


def main(argv: list[str] | None = None) -> int:
    try:
        sys.stdout.reconfigure(encoding="utf-8")
        sys.stderr.reconfigure(encoding="utf-8")
    except Exception:
        pass

    argv = list(sys.argv[1:] if argv is None else argv)
    parser = build_parser()

    if not argv or argv[0] in ("-h", "--help"):
        parser.print_help()
        return 0
    if argv[0] in ("-V", "--version"):
        return _cmd_version(argparse.Namespace())

    known = {"review", "menu", "version", "config", "help"}
    if argv[0] not in known and not argv[0].startswith("-"):
        pass
    elif argv[0] not in known:
        argv = ["review", *argv]

    args = parser.parse_args(argv)
    if not hasattr(args, "func"):
        parser.print_help()
        return 0
    return int(args.func(args))


if __name__ == "__main__":
    raise SystemExit(main())
