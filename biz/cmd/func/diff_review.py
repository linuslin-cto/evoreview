"""交互式菜单入口：本地 Diff 审查。"""
from __future__ import annotations

import os

from biz.cmd.func.base import BaseReviewFunc
from biz.scr.git_diff import collect_local_diffs
from biz.scr.reviewer import ScrReviewer
from biz.review.pipeline import ReviewOutcome, compose_report
from biz.utils.xiezhi_reviewer import XiezhiReviewer


class DiffReviewFunc(BaseReviewFunc):
    """提交前本地 Diff 审查（复用 SCR 结构化 + 叙述总览）。"""

    def process(self):
        repo = self.get_user_input("请输入 Git 仓库路径", default=".")
        mode = self.get_user_input(
            "模式: 1=工作区(默认) 2=commit 3=范围(from..to)",
            default="1",
        )
        preview_only = self.confirm_action("是否仅预览过滤结果（不调用 LLM）? (y/n): ")

        commit = from_ref = to_ref = None
        if mode == "2":
            commit = self.get_user_input("请输入 commit SHA/引用", default="HEAD")
        elif mode == "3":
            from_ref = self.get_user_input("请输入 --from", default="origin/main")
            to_ref = self.get_user_input("请输入 --to", default="HEAD")

        try:
            bundle = collect_local_diffs(
                repo,
                commit=commit,
                from_ref=from_ref,
                to_ref=to_ref,
            )
        except Exception as e:
            print(f"❌ 采集 Diff 失败: {e}")
            return

        included = [i for i in bundle.preview if i.included]
        excluded = [i for i in bundle.preview if not i.included]
        print(f"\n模式={bundle.mode} · 纳入={len(included)} · 排除={len(excluded)}")
        for i in included:
            print(f"  ✅ {i.path} (+{i.additions}/-{i.deletions})")
        for i in excluded[:20]:
            print(f"  ⏭️ {i.path} ({i.reason})")
        if len(excluded) > 20:
            print(f"  ... 其余 {len(excluded) - 20} 个已排除")

        if preview_only:
            print("\n（预览模式，未调用 LLM）")
            return
        if not bundle.changes:
            print("没有可审查的代码变更。")
            return
        if not self.confirm_action(f"确认对 {len(bundle.changes)} 个文件发起审查? (y/n): "):
            return

        no_narrative = not self.confirm_action("是否附加叙述性总览（会多一次 LLM 调用）? (y/n): ")
        print("\n⏳ 审查中...")

        reviewer = ScrReviewer()
        results = reviewer.review_structured(bundle.changes)
        active = reviewer.active_comments(results)
        score = reviewer.score_from_results(results)
        narrative = ""
        if not no_narrative:
            try:
                narrative = XiezhiReviewer().review_and_strip_code(
                    str(bundle.changes), f"local-{bundle.mode}"
                )
            except Exception as e:
                print(f"⚠️ 叙述总览失败: {e}")

        body = compose_report(results, score, narrative)
        outcome = ReviewOutcome(report_text=body, score=score, inline_comments=active)
        print("\n" + outcome.report_text)

        save = self.confirm_action("是否保存到文件? (y/n): ")
        if save:
            out = self.get_user_input("输出路径", default="local-diff-review.md")
            with open(out, "w", encoding="utf-8") as f:
                f.write(outcome.report_text)
            print(f"已保存: {os.path.abspath(out)}")
