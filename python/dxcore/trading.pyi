from collections.abc import Iterable
from typing import Any

import polars as pl


class DailyView:
    def __init__(
        self, date_col: str, col_map: list[tuple[str, str]] | None = None
    ) -> None: ...
    @property
    def col_map(self) -> list[tuple[str, str]]: ...


class Executor:
    def __init__(self, strategy: object) -> None: ...
    def run(self, df: pl.DataFrame, view: DailyView) -> pl.DataFrame: ...


class AsyncExecutor:
    def __init__(self, strategy: object) -> None: ...
    def run(
        self, stream: Iterable[tuple[int, pl.DataFrame]], view: DailyView
    ) -> RunIterator: ...


class RunIterator:
    def __iter__(self) -> RunIterator: ...
    # Yields each strategy's `on_step` return value, which is unconstrained.
    def __next__(self) -> Any: ...
