from blinker import Signal

from biz.entity.review_entity import MergeRequestReviewEntity, PushReviewEntity
from biz.service.review_service import ReviewService
from biz.utils.im import notifier

event_manager = {
    "merge_request_reviewed": Signal(),
    "push_reviewed": Signal(),
}


def _brief_commits(commits: list, max_items: int = 3, max_chars: int = 160) -> str:
    """提交信息摘要（前几条首行，过长截断）。"""
    firsts = []
    for commit in commits[:max_items]:
        first_line = (commit.get("message") or "").strip().splitlines()[0] if commit.get("message") else ""
        if first_line:
            firsts.append(first_line[:max_chars])
    more = len(commits) - max_items
    text = "；".join(firsts) or "（无提交信息）"
    if more > 0:
        text += f" …（共 {len(commits)} 个提交）"
    return text


def _has_report(review_result) -> bool:
    return bool(review_result) and "总分" in str(review_result)


def on_merge_request_reviewed(mr_review_entity: MergeRequestReviewEntity):
    im_msg = f"""### 🔀 {mr_review_entity.project_name}: Merge Request

- **提交者:** {mr_review_entity.author}
- **分支**: `{mr_review_entity.source_branch}` → `{mr_review_entity.target_branch}`
- **提交:** {_brief_commits(mr_review_entity.commits)}
- [查看合并详情]({mr_review_entity.url})

---

{mr_review_entity.review_result}
"""
    title = f"MR 审查 · {mr_review_entity.project_name}"
    if _has_report(mr_review_entity.review_result):
        title += f" · {int(mr_review_entity.score)}分"
    notifier.send_notification(content=im_msg, msg_type='markdown', title=title,
                               project_name=mr_review_entity.project_name, url_slug=mr_review_entity.url_slug,
                               webhook_data=mr_review_entity.webhook_data or {})
    ReviewService().insert_mr_review_log(mr_review_entity)


def on_push_reviewed(entity: PushReviewEntity):
    first_url = next((c.get('url') for c in entity.commits if c.get('url')), None)
    im_msg = f"""### 🚀 {entity.project_name}: Push

- **提交者:** {entity.author}
- **分支**: `{entity.branch}`
- **提交:** {_brief_commits(entity.commits)}
"""
    if first_url:
        im_msg += f"- [查看提交详情]({first_url})\n"
    if entity.review_result:
        im_msg += f"\n---\n\n{entity.review_result}\n"

    title = f"Push 审查 · {entity.project_name}"
    if _has_report(entity.review_result):
        title += f" · {int(entity.score)}分"
    notifier.send_notification(content=im_msg, msg_type='markdown', title=title,
                               project_name=entity.project_name, url_slug=entity.url_slug,
                               webhook_data=entity.webhook_data or {})
    ReviewService().insert_push_review_log(entity)


event_manager["merge_request_reviewed"].connect(on_merge_request_reviewed)
event_manager["push_reviewed"].connect(on_push_reviewed)
