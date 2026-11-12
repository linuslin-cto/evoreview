import requests
import os
from biz.utils.log import logger


class FeishuNotifier:
    def __init__(self, webhook_url=None):
        self.default_webhook_url = webhook_url or os.environ.get('FEISHU_WEBHOOK_URL', '')
        self.enabled = os.environ.get('FEISHU_ENABLED', '0') == '1'

    def _get_webhook_url(self, project_name=None, url_slug=None):
        """按项目名 / url_slug 解析 webhook，否则用默认。"""
        if not project_name:
            if self.default_webhook_url:
                return self.default_webhook_url
            raise ValueError("未提供项目名称，且未设置默认的 飞书 Webhook URL。")

        target_key_project = f"FEISHU_WEBHOOK_URL_{project_name.upper()}"
        target_key_url_slug = f"FEISHU_WEBHOOK_URL_{url_slug.upper()}" if url_slug else None
        for env_key, env_value in os.environ.items():
            env_key_upper = env_key.upper()
            if env_key_upper == target_key_project:
                return env_value
            if target_key_url_slug and env_key_upper == target_key_url_slug:
                return env_value

        if self.default_webhook_url:
            return self.default_webhook_url
        raise ValueError(f"未找到项目 '{project_name}' 对应的 Feishu Webhook URL，且未设置默认的 Webhook URL。")

    def send_message(self, content, msg_type='text', title=None, is_at_all=False, project_name=None, url_slug=None):
        if not self.enabled:
            logger.info("飞书推送未启用")
            return

        try:
            post_url = self._get_webhook_url(project_name=project_name, url_slug=url_slug)
            if msg_type == 'markdown':
                data = {
                    "msg_type": "interactive",
                    "card": {
                        "schema": "2.0",
                        "config": {
                            "update_multi": True,
                            "style": {
                                "text_size": {
                                    "normal_v2": {
                                        "default": "normal",
                                        "pc": "normal",
                                        "mobile": "heading"
                                    }
                                }
                            }
                        },
                        "body": {
                            "direction": "vertical",
                            "padding": "12px 12px 12px 12px",
                            "elements": [
                                {
                                    "tag": "markdown",
                                    "content": content,
                                    "text_align": "left",
                                    "text_size": "normal_v2",
                                    "margin": "0px 0px 0px 0px"
                                }
                            ]
                        },
                        "header": {
                            "title": {
                                "tag": "plain_text",
                                "content": title
                            },
                            "template": "blue",
                            "padding": "12px 12px 12px 12px"
                        }
                    }
                }
            else:
                data = {
                    "msg_type": "text",
                    "content": {
                        "text": content
                    },
                }

            response = requests.post(
                url=post_url,
                json=data,
                headers={'Content-Type': 'application/json'},
                timeout=10,
            )

            if not response.ok:
                logger.error(f"飞书消息发送失败! webhook_url:{post_url}, error_msg:{response.text}")
                return

            result = response.json()
            if result.get('msg') != "success":
                logger.error(f"发送飞书消息失败! webhook_url:{post_url},errmsg:{result}")
            else:
                logger.info(f"飞书消息发送成功! webhook_url:{post_url}")

        except ValueError as e:
            logger.error(f"飞书配置错误: {e}")
        except requests.RequestException as e:
            logger.error(f"飞书消息发送失败! webhook_url:{post_url if 'post_url' in locals() else ''}, error:{e}")
        except Exception as e:
            logger.error(f"飞书消息发送失败! ", e)
