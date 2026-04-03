from typing import List

class Transaction:
    price: float
    quantity: int
    bid_id: str
    ask_id: str

class Instrument:
    symbol: str
    exchange: str

    def __init__(self, symbol: str) -> None: ...

class Side:
    Buy: "Side"
    Sell: "Side"

class OrderType:
    Market: "OrderType"
    Limit: "OrderType"

class Order:
    id: str
    price: float
    quantity: int
    side: Side

    def __init__(self, type: OrderType, side: Side, price: float, quantity: int) -> None: ...

class OrderBook:
    def __init__(self, instrument: Instrument, tick_size: float = 0.01) -> None: ...
    def insert(self, order: Order) -> List[Transaction]: ...
    def cancel(self, order_id: str) -> None: ...
    def get_mid_price(self) -> float: ...
    def get_best_bid(self) -> float: ...
    def get_best_ask(self) -> float: ...
    def get_level_count(self) -> int: ...
    def get_instrument(self) -> Instrument: ...