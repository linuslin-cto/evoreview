from typing import Optional, List, Literal

from pydantic import BaseModel
from typing_extensions import override


class NotGiven:
    """Sentinel for omitted keyword arguments (distinct from explicit None)."""

    def __bool__(self) -> Literal[False]:
        return False

    @override
    def __repr__(self) -> str:
        return "NOT_GIVEN"

    def to_json(self):
        raise NotImplementedError("Serialization is not implemented for NotGiven.")


NOT_GIVEN = NotGiven()


class Function(BaseModel):
    arguments: str
    name: str


class CompletionMessageToolCall(BaseModel):
    id: str
    function: Function
    type: Literal["function"]


class CompletionMessage(BaseModel):
    content: Optional[str] = None
    role: str
    refusal: Optional[str] = None
