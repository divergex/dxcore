from collections.abc import Iterable
from typing import Any

import polars as pl

class Side:
    Buy: Side
    Sell: Side

class OrderType:
    Market: OrderType
    Limit: OrderType
    StopLoss: OrderType

class OrderQuantity:
    class Pct(OrderQuantity):
        _0: float
        def __init__(self, value: float) -> None: ...

    class Abs(OrderQuantity):
        _0: float
        def __init__(self, value: float) -> None: ...

class OrderPrice:
    Mkt: OrderPrice
    @staticmethod
    def Px(price: float) -> OrderPrice: ...

class Signal:
    def __init__(self, shares: float, symbol: str | None = None) -> None: ...
    @property
    def symbol(self) -> str | None: ...
    @property
    def shares(self) -> float: ...

class OrderData:
    def __init__(
        self,
        quantity: OrderQuantity,
        order_type: OrderType,
        price: OrderPrice,
        symbol: str | None = None,
    ) -> None: ...
    @property
    def symbol(self) -> str | None: ...
    @property
    def quantity(self) -> OrderQuantity: ...
    @property
    def order_type(self) -> OrderType: ...
    @property
    def price(self) -> OrderPrice: ...

class Order:
    @property
    def date(self) -> int | None: ...
    @property
    def symbol(self) -> str: ...
    @property
    def side(self) -> Side: ...
    @property
    def quantity(self) -> OrderQuantity: ...
    @property
    def order_type(self) -> OrderType: ...
    @property
    def price(self) -> OrderPrice: ...

class BaseOrderEngine:
    def __init__(self, symbol: str | None = None) -> None: ...

class DailyView:
    def __init__(
        self, date_col: str, col_map: list[tuple[str, str]] | None = None
    ) -> None: ...
    @property
    def col_map(self) -> list[tuple[str, str]]: ...

class Executor:
    def __init__(self, engine: BaseOrderEngine, strategy: object) -> None: ...
    def run(self, df: pl.DataFrame, view: DailyView) -> list[Order]: ...

class AsyncExecutor:
    def __init__(self, engine: BaseOrderEngine, strategy: object) -> None: ...
    def run(
        self, stream: Iterable[tuple[int, pl.DataFrame]], view: DailyView
    ) -> RunIterator: ...

class RunIterator:
    def __iter__(self) -> RunIterator: ...
    def __next__(self) -> Order: ...
