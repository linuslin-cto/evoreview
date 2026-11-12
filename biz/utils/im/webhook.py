import os
from biz.utils.log import logger
import requests


class ExtraWebhookNotifier:
    def __init__(self, webhook_url=None):
        self.default_webhook_url = webhook_url or os.environ.get('EXTRA_WEBHOOK_URL', '')
        self.enabled = os.environ.get('EXTRA_WEBHOOK_ENABLED', '0') == '1'

    def send_message(self, system_data: dict, webhook_data: dict):
        """Forward review payload and raw webhook data to a custom URL."""
        if not self.enabled:
            logger.info("ExtraWebhook推送未启用")
            return

        try:
            data = {
                "xiezhi_data": system_data,
                "webhook_data": webhook_data
            }
            response = requests.post(
                url=self.default_webhook_url,
                json=data,
                headers={'Content-Type': 'application/json'},
                timeout=10,
            )

            if response.status_code != 200:
                logger.error(f"ExtraWebhook消息发送失败! webhook_url:{self.default_webhook_url}, error_msg:{response.text}")
                return

        except Exception as e:
            logger.error(f"ExtraWebhook消息发送失败! ", e)
