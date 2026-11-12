import re
from abc import abstractmethod
from typing import List, Dict, Optional

from biz.llm.types import NotGiven, NOT_GIVEN
from biz.utils.log import logger


class BaseClient:
    """Base class for chat model clients."""

    def ping(self) -> bool:
        """Ping the model to check connectivity."""
        try:
            result = self.completions(messages=[{"role": "user", "content": '请仅返回 "ok"。'}])
            cleaned = re.sub(r'<think>.*?</think>', '', result, flags=re.DOTALL)
            return cleaned.strip() == "ok"
        except Exception as e:
            logger.error("尝试连接LLM失败: %s", e, exc_info=True)
            return False

    @abstractmethod
    def completions(self,
                    messages: List[Dict[str, str]],
                    model: Optional[str] | NotGiven = NOT_GIVEN,
                    ) -> str:
        """Chat with the model."""

    def chat_with_tools(self,
                        messages: List[Dict],
                        tools: Optional[List[Dict]] = None,
                        model: Optional[str] | NotGiven = NOT_GIVEN,
                        ) -> Dict:
        """Native tool-use chat; override in providers that support it."""
        raise NotImplementedError(
            f"{type(self).__name__} does not implement native tool-use; "
            "use LLMAdapter's JSON-protocol fallback instead."
        )
