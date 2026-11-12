"""Generate golden fixtures without installing full Python deps."""
from __future__ import annotations

import importlib.util
import json
import sys
import types
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


def load(name: str, rel: str):
    path = ROOT / rel
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


def main() -> None:
    biz = types.ModuleType("biz")
    scr = types.ModuleType("biz.scr")
    review = types.ModuleType("biz.review")
    sys.modules["biz"] = biz
    sys.modules["biz.scr"] = scr
    sys.modules["biz.review"] = review

    models = load("biz.scr.models", "biz/scr/models.py")
    scr.models = models
    sys.modules["biz.scr.models"] = models

    scoring = load("biz.review.scoring", "biz/review/scoring.py")
    review.scoring = scoring
    sys.modules["biz.review.scoring"] = scoring

    # prevent biz.scr.__init__ side effects if somehow imported
    scr.__path__ = [str(ROOT / "biz" / "scr")]  # type: ignore[attr-defined]
    anchor = load("biz.scr.anchor", "biz/scr/anchor.py")
    sys.modules["biz.scr.anchor"] = anchor

    ScrComment = models.ScrComment
    FileReviewResult = models.FileReviewResult
    score_results = scoring.score_results
    score_grade = scoring.score_grade
    unified_score = scoring.unified_score
    parse_new_line_map = anchor.parse_new_line_map
    anchor_comment = anchor.anchor_comment

    def one_comment(**kwargs):
        defaults = dict(
            path="a.py",
            content="x",
            severity="medium",
            category="bug",
            start_line=1,
            end_line=1,
            filtered=False,
        )
        defaults.update(kwargs)
        defaults["end_line"] = defaults.get("end_line", defaults["start_line"])
        return ScrComment(**defaults)

    cases = []
    cases.append({"name": "empty", "score": score_results([]), "grade": score_grade(100)})

    r = [
        FileReviewResult(
            path="a.py",
            additions=40,
            comments=[
                one_comment(severity="critical", category="security", start_line=10)
            ],
        )
    ]
    s = score_results(r)
    cases.append({"name": "critical_security", "score": s, "grade": score_grade(s)})

    r = [
        FileReviewResult(
            path="a.py",
            additions=80,
            comments=[one_comment(severity="low", category="style", start_line=3)],
        )
    ]
    s = score_results(r)
    cases.append({"name": "low_style", "score": s, "grade": score_grade(s)})

    r = [
        FileReviewResult(
            path="a.py",
            additions=40,
            comments=[
                one_comment(severity="high", category="bug", start_line=1, filtered=True)
            ],
        )
    ]
    cases.append({"name": "filtered", "score": score_results(r)})

    one = [
        FileReviewResult(
            path="a.py",
            additions=40,
            comments=[one_comment(severity="high", category="bug", start_line=1)],
        )
    ]
    many = [
        FileReviewResult(
            path="a.py",
            additions=40,
            comments=[
                one_comment(severity="high", category="bug", start_line=i + 1)
                for i in range(5)
            ],
        )
    ]
    cases.append(
        {
            "name": "spam",
            "score_one": score_results(one),
            "score_five": score_results(many),
        }
    )

    cases.append(
        {
            "name": "unified",
            "score": unified_score(
                [one_comment(severity="medium", category="bug", start_line=1)], 1
            ),
        }
    )

    r = [
        FileReviewResult(
            path="a.py",
            additions=10,
            comments=[one_comment(path="a.py", severity="medium", category="bug", start_line=1)],
        ),
        FileReviewResult(
            path="b.py",
            additions=100,
            comments=[
                one_comment(
                    path="b.py", severity="high", category="security", start_line=2
                )
            ],
        ),
    ]
    s = score_results(r)
    cases.append({"name": "multi_file", "score": s, "grade": score_grade(s)})

    r = [
        FileReviewResult(
            path="a.py",
            additions=40,
            comments=[one_comment(severity="high", category="bug", start_line=0)],
        )
    ]
    cases.append({"name": "unanchored", "score": score_results(r)})

    diff = "@@ -1,2 +1,3 @@\n keep\n+fn bad() {}\n keep2\n"
    line_map = parse_new_line_map(diff)
    c = ScrComment(path="a.rs", content="issue", existing_code="fn bad() {}")
    anchor_comment(c, line_map)

    golden = {
        "source": "biz/review/scoring.py + biz/scr/anchor.py",
        "scoring_cases": cases,
        "anchor": {
            "diff": diff,
            "existing_code": "fn bad() {}",
            "start_line": c.start_line,
            "end_line": c.end_line,
            "line_map": [[n, t, a] for n, t, a in line_map],
        },
    }

    out = Path(__file__).with_name("python_golden.json")
    out.write_text(json.dumps(golden, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {out}")
    print(json.dumps(golden, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
