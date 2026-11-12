"""SCR 结构化代码审查。"""

from biz.scr.reviewer import ScrReviewer
from biz.scr.git_diff import collect_local_diffs

__all__ = ["ScrReviewer", "collect_local_diffs"]
