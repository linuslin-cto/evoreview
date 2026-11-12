import base64
import hashlib
import hmac
import json
import os
import time
from urllib.parse import parse_qsl, quote_plus, urlencode, urlparse, urlunparse

import requests

from biz.utils.log import logger


class DingTalkNotifier:
    def __init__(self, webhook_url=None):
        self.enabled = os.environ.get('DINGTALK_ENABLED', '0') == '1'
        self._secret_enabled = os.environ.get('DINGTALK_SECRET_ENABLED', '0') == '1'
        self._secret = os.environ.get('DINGTALK_SECRET') or ''
        self.default_webhook_url = webhook_url or os.environ.get('DINGTALK_WEBHOOK_URL')

    @staticmethod
    def _sign_url(base_url: str, secret: str) -> str:
        timestamp = str(round(time.time() * 1000))
        string_to_sign = f'{timestamp}\n{secret}'
        hmac_code = hmac.new(
            secret.encode('utf-8'),
            string_to_sign.encode('utf-8'),
            digestmod=hashlib.sha256,
        ).digest()
        sign = quote_plus(base64.b64encode(hmac_code))
        parsed = urlparse(base_url)
        query = dict(parse_qsl(parsed.query, keep_blank_values=True))
        query.update({'timestamp': timestamp, 'sign': sign})
        return urlunparse(parsed._replace(query=urlencode(query)))

    def _get_webhook_url(self, project_name=None, url_slug=None):
        """按项目名 / url_slug 解析 webhook，否则用默认。"""
        if not project_name:
            if self.default_webhook_url:
                return self.default_webhook_url
            raise ValueError("未提供项目名称，且未设置默认的钉钉 Webhook URL。")

        target_key_project = f"DINGTALK_WEBHOOK_URL_{project_name.upper()}"
        target_key_url_slug = f"DINGTALK_WEBHOOK_URL_{url_slug.upper()}" if url_slug else None
        for env_key, env_value in os.environ.items():
            env_key_upper = env_key.upper()
            if env_key_upper == target_key_project:
                return env_value
            if target_key_url_slug and env_key_upper == target_key_url_slug:
                return env_value

        if self.default_webhook_url:
            return self.default_webhook_url
        raise ValueError(f"未找到项目 '{project_name}' 对应的钉钉Webhook URL，且未设置默认的 Webhook URL。")

    def send_message(self, content: str, msg_type='text', title='通知', is_at_all=False, project_name=None, url_slug=None):
        if not self.enabled:
            logger.info("钉钉推送未启用")
            return

        try:
            post_url = self._get_webhook_url(project_name=project_name, url_slug=url_slug)
            if self._secret_enabled:
                if not self._secret:
                    logger.error("钉钉加签已启用但未配置 DINGTALK_SECRET")
                    return
                post_url = self._sign_url(post_url, self._secret)
            headers = {
                "Content-Type": "application/json",
                "Charset": "UTF-8"
            }
            if msg_type == 'markdown':
                message = {
                    "msgtype": "markdown",
                    "markdown": {
                        "title": title,
                        "text": content
                    },
                    "at": {
                        "isAtAll": is_at_all
                    }
                }
            else:
                message = {
                    "msgtype": "text",
                    "text": {
                        "content": content
                    },
                    "at": {
                        "isAtAll": is_at_all
                    }
                }
            response = requests.post(
                url=post_url, data=json.dumps(message), headers=headers, timeout=10
            )
            response.raise_for_status()
            try:
                response_data = response.json()
            except ValueError:
                logger.error("钉钉消息发送失败! webhook_url:%s, 响应非 JSON: %s", post_url, response.text[:200])
                return
            if response_data.get('errmsg') == 'ok':
                logger.info("钉钉消息发送成功! webhook_url:%s", post_url)
            else:
                logger.error("钉钉消息发送失败! webhook_url:%s,errmsg:%s", post_url, response_data.get('errmsg'))
        except Exception as e:
            logger.error("钉钉消息发送失败! %s", e, exc_info=True)
