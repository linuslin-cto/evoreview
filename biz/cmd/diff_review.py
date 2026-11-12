"""本地 Diff 审查 CLI 实现。"""
from __future__ import annotations

import argparse
import json
import os
import sys
from dataclasses import asdict
from pathlib import Path

def build_parser(prog: str = "xiezhi review") -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog=prog,
        description="本地 Diff 审查：workspace / commit / 范围。",
    )
    add_review_arguments(p)
    return p


def add_review_arguments(p: argparse.ArgumentParser) -> None:
    """把 review 子命令参数挂到 parser / subparser 上。"""
    p.add_argument("--repo", default=".", help="Git 仓库目录（默认当前目录）")
    mode = p.add_mutually_exclusive_group()
    mode.add_argument("--commit", help="审查指定 commit（git show）")
    mode.add_argument("--from", dest="from_ref", help="范围起点（需配合 --to）")
    p.add_argument("--to", dest="to_ref", help="范围终点（需配合 --from）")
    p.add_argument(
        "--preview",
        action="store_true",
        help="仅预览过滤结果，不调用 LLM（零 Token）",
    )
    p.add_argument(
        "--format",
        choices=("text", "json"),
        default="text",
        help="输出格式（默认 text）",
    )
    p.add_argument(
        "--audience",
        choices=("human", "agent"),
        default="human",
        help="agent 模式下日志不污染 stdout，仅输出 JSON/正文",
    )
    p.add_argument(
        "--no-narrative",
        action="store_true",
        help="跳过叙述性总览",
    )
    p.add_argument(
        "--output",
        "-o",
        help="将结果写入文件（默认打印到 stdout）",
    )


_build_parser = build_parser


def _emit_preview(bundle, fmt: str) -> str:
    included = [i for i in bundle.preview if i.included]
    excluded = [i for i in bundle.preview if not i.included]
    if fmt == "json":
        payload = {
            "status": "preview",
            "mode": bundle.mode,
            "repo_dir": bundle.repo_dir,
            "included": [asdict(i) for i in included],
            "excluded": [asdict(i) for i in excluded],
            "warnings": bundle.warnings,
            "summary": {
                "included": len(included),
                "excluded": len(excluded),
                "additions": sum(i.additions for i in included),
                "deletions": sum(i.deletions for i in included),
            },
        }
        return json.dumps(payload, ensure_ascii=False, indent=2)

    lines = [
        f"# 本地 Diff 预览（{bundle.mode}）",
        f"仓库: {bundle.repo_dir}",
        f"纳入审查: {len(included)} 个文件 · 排除: {len(excluded)} 个文件",
        "",
    ]
    if included:
        lines.append("## [纳入] 将审查")
        for i in included:
            lines.append(f"- `{i.path}`  (+{i.additions}/-{i.deletions})")
        lines.append("")
    if excluded:
        lines.append("## [排除] 已过滤")
        for i in excluded[:50]:
            lines.append(f"- `{i.path}`  ({i.reason})")
        if len(excluded) > 50:
            lines.append(f"- ... 其余 {len(excluded) - 50} 个已省略")
        lines.append("")
    for w in bundle.warnings:
        lines.append(f"> 警告: {w}")
    lines.append("")
    lines.append("_--preview 模式未调用 LLM（零 Token）_")
    return "\n".join(lines)


def _emit_result(bundle, outcome, results, fmt: str) -> str:
    from biz.scr.reviewer import ScrReviewer

    active = ScrReviewer.active_comments(results)
    warnings = list(bundle.warnings)
    for r in results:
        if r.warning:
            warnings.append(f"{r.path}: {r.warning}")

    if fmt == "json":
        payload = {
            "status": "ok",
            "mode": bundle.mode,
            "repo_dir": bundle.repo_dir,
            "score": outcome.score,
            "comments": [
                {
                    "path": c.path,
                    "content": c.content,
                    "existing_code": c.existing_code,
                    "suggestion_code": c.suggestion_code,
                    "start_line": c.start_line,
                    "end_line": c.end_line,
                    "category": c.category,
                    "severity": c.severity,
                }
                for c in active
            ],
            "summary": {
                "files": len(results),
                "comments": len(active),
                "score": outcome.score,
            },
            "warnings": warnings,
            "report": outcome.report_text,
        }
        return json.dumps(payload, ensure_ascii=False, indent=2)
    return outcome.report_text


def _write_output_files(
    output: str,
    primary_text: str,
    *,
    markdown_text: str | None = None,
    log=None,
) -> list[Path]:
    """写入 -o 主产物；若目标不是 .md，再旁路生成同名 .md（审查报告）。"""
    written: list[Path] = []
    out = Path(output)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(primary_text, encoding="utf-8")
    written.append(out)
    if log:
        log(f"已写入 {out}")

    md_body = markdown_text if markdown_text is not None else primary_text
    if out.suffix.lower() != ".md":
        md_path = out.with_suffix(".md")
        md_path.write_text(md_body, encoding="utf-8")
        written.append(md_path)
        if log:
            log(f"已写入 {md_path}")
    return written


def run(args: argparse.Namespace) -> int:
    """执行已解析的 review 参数（供 CLI 子命令与 -m 入口复用）。"""
    quiet = args.audience == "agent" or args.format == "json"

    def log(msg: str) -> None:
        if not quiet:
            print(msg, file=sys.stderr)

    try:
        from biz.cli.paths import ensure_runtime_paths, find_env_file
        conf = ensure_runtime_paths()
        log(f"配置目录: {conf}")
        env_path = find_env_file()
        if env_path:
            log(f"已加载环境: {env_path}")
    except FileNotFoundError as e:
        print(str(e), file=sys.stderr)
        return 2

    repo_dir = os.path.abspath(args.repo)
    found_git = None
    for p in [Path(repo_dir), *Path(repo_dir).parents]:
        if (p / ".git").exists():
            found_git = p
            break
    if found_git is None:
        print(f"错误: {repo_dir} 不在任何 Git 仓库内", file=sys.stderr)
        return 2

    if args.from_ref and not args.to_ref:
        print("错误: --from 需要同时提供 --to", file=sys.stderr)
        return 2
    if args.to_ref and not args.from_ref:
        print("错误: --to 需要同时提供 --from", file=sys.stderr)
        return 2

    from biz.scr.git_diff import collect_local_diffs

    try:
        bundle = collect_local_diffs(
            repo_dir,
            commit=args.commit,
            from_ref=args.from_ref,
            to_ref=args.to_ref,
        )
    except Exception as e:
        print(f"采集 Diff 失败: {e}", file=sys.stderr)
        return 2

    if args.preview:
        text = _emit_preview(bundle, args.format)
        if args.output:
            md_side = text if args.format == "text" else _emit_preview(bundle, "text")
            _write_output_files(args.output, text, markdown_text=md_side, log=log)
        else:
            print(text)
        return 0

    if not bundle.changes:
        msg = "没有可审查的代码变更（可能被过滤，或工作区干净）。可用 --preview 查看详情。"
        if args.format == "json":
            print(json.dumps({"status": "empty", "message": msg, "warnings": bundle.warnings}, ensure_ascii=False, indent=2))
        else:
            print(msg)
        return 0

    log(f"开始审查: mode={bundle.mode} files={len(bundle.changes)}")

    from biz.scr.reviewer import ScrReviewer
    from biz.review.pipeline import ReviewOutcome, compose_report

    reviewer = ScrReviewer()
    results = reviewer.review_structured(bundle.changes)
    active = reviewer.active_comments(results)
    score = reviewer.score_from_results(results)

    narrative = ""
    if not args.no_narrative and os.getenv("REVIEW_NARRATIVE_ENABLED", "1") == "1":
        try:
            from biz.utils.xiezhi_reviewer import XiezhiReviewer

            narrative = XiezhiReviewer().review_and_strip_code(
                str(bundle.changes), f"local-{bundle.mode}"
            )
        except Exception as e:
            log(f"叙述总览失败，仅用结构化结果: {e}")

    body = compose_report(results, score, narrative)
    outcome = ReviewOutcome(report_text=body, score=score, inline_comments=active)
    text = _emit_result(bundle, outcome, results, args.format)

    if args.output:
        _write_output_files(
            args.output,
            text,
            markdown_text=outcome.report_text,
            log=log,
        )
    else:
        print(text)
    return 0


def main(argv: list[str] | None = None) -> int:
    try:
        sys.stdout.reconfigure(encoding="utf-8")
        sys.stderr.reconfigure(encoding="utf-8")
    except Exception:
        pass
    return run(build_parser().parse_args(argv))


if __name__ == "__main__":
    raise SystemExit(main())
