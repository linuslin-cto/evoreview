import os
from typing import Optional

from biz.llm.client.base import BaseClient
from biz.llm.client.anthropic import AnthropicClient
from biz.llm.client.deepseek import DeepSeekClient
from biz.llm.client.ollama_client import OllamaClient
from biz.llm.client.openai import OpenAIClient
from biz.llm.client.qwen import QwenClient
from biz.llm.client.zhipuai import ZhipuAIClient


class Factory:
    @staticmethod
    def get_client(provider: Optional[str] = None) -> BaseClient:
        provider = provider or os.getenv("LLM_PROVIDER", "anthropic")
        chat_model_providers = {
            'anthropic': lambda: AnthropicClient(),
            'zhipuai': lambda: ZhipuAIClient(),
            'openai': lambda: OpenAIClient(),
            'deepseek': lambda: DeepSeekClient(),
            'qwen': lambda: QwenClient(),
            'ollama': lambda: OllamaClient()
        }

        provider_func = chat_model_providers.get(provider)
        if provider_func:
            return provider_func()
        raise ValueError(f'Unknown chat model provider: {provider}')

    getClient = get_client
