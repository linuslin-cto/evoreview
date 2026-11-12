from biz.utils.im.dingtalk import DingTalkNotifier
from biz.utils.im.feishu import FeishuNotifier
from biz.utils.im.webhook import ExtraWebhookNotifier
from biz.utils.im.wecom import WeComNotifier


def send_notification(content, msg_type='text', title="通知", is_at_all=False, project_name=None, url_slug=None,
                      webhook_data: dict | None = None):
    """向已启用的 IM / 自定义 webhook 发送通知。"""
    if webhook_data is None:
        webhook_data = {}
    DingTalkNotifier().send_message(
        content=content, msg_type=msg_type, title=title, is_at_all=is_at_all,
        project_name=project_name, url_slug=url_slug)
    WeComNotifier().send_message(
        content=content, msg_type=msg_type, title=title, is_at_all=is_at_all,
        project_name=project_name, url_slug=url_slug)
    FeishuNotifier().send_message(
        content=content, msg_type=msg_type, title=title, is_at_all=is_at_all,
        project_name=project_name, url_slug=url_slug)
    ExtraWebhookNotifier().send_message(
        system_data={
            "content": content,
            "msg_type": msg_type,
            "title": title,
            "is_at_all": is_at_all,
            "project_name": project_name,
            "url_slug": url_slug,
        },
        webhook_data=webhook_data,
    )
