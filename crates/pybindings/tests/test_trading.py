"""Tests for the dxcore trading bindings: strategy output -> engine -> orders.

Requires the compiled extension: `maturin develop` or
`cargo build -p dxcore-pyo3 --features extension-module`.
"""

import datetime
import threading
import time
from collections.abc import Iterator

import polars as pl
import pytest

import dxcore


def _ohlc_df(n: int = 10) -> pl.DataFrame:
    dates = [datetime.date(2024, 1, 1) + datetime.timedelta(days=i) for i in range(n)]
    return pl.DataFrame(
        {
            "date": pl.Series(dates, dtype=pl.Date),
            "close": [float(i) for i in range(n)],
        }
    )


def _epoch_days(df: pl.DataFrame) -> list[int]:
    return df.select(pl.col("date").dt.epoch("d")).to_series().to_list()


def _daily_view() -> dxcore.DailyView:
    return dxcore.DailyView("date", [("close", "price")])


def _daily_steps(df: pl.DataFrame) -> Iterator[tuple[int, pl.DataFrame]]:
    # Async stream items reach on_step as-is; the view's col_map is not
    # applied there, so rename before slicing.
    df = df.rename({"close": "price"})
    for day in _epoch_days(df):
        yield day, df.filter(pl.col("date").dt.epoch("d") == day)


class TargetSequence:
    """Emits one target per step (a `Signal`), or nothing for `None`."""

    def __init__(self, targets: list[float | None]) -> None:
        self.targets = targets
        self.calls: list[tuple[int, tuple[str, ...], int, int]] = []

    def on_step(self, date, step, history, state):
        index = state.get("i", 0)
        state["i"] = index + 1
        self.calls.append((date, tuple(step.columns), history.height, index + 1))
        if index >= len(self.targets) or self.targets[index] is None:
            return None
        return dxcore.Signal(shares=self.targets[index])


class AlwaysOrder:
    """Emits an explicit market order on every step."""

    def on_step(self, date, step, history, state):
        return dxcore.OrderData(
            quantity=dxcore.OrderQuantity.Abs(1.0),
            order_type=dxcore.OrderType.Market,
            price=dxcore.OrderPrice.Mkt,
            symbol="AAPL",
        )


def _order_key(order: dxcore.Order):
    return (
        order.date,
        order.symbol,
        order.side,
        order.quantity,
        order.order_type,
        order.price,
    )


def test_daily_view_slices_steps_and_history():
    df = _ohlc_df()
    strategy = TargetSequence([None] * df.height)
    orders = dxcore.Executor(dxcore.BaseOrderEngine("AAPL"), strategy).run(
        df, _daily_view()
    )

    assert orders == []
    n = df.height
    # every step: col_map applied, single row, history grows by one per step
    assert [cols for _, cols, _, _ in strategy.calls] == [("date", "price")] * n
    # history holds prior days only: the first step sees an empty frame
    assert [height for _, _, height, _ in strategy.calls] == list(range(n))
    dates = [date for date, _, _, _ in strategy.calls]
    assert dates == sorted(dates)


def test_state_is_fresh_per_run():
    df = _ohlc_df()
    strategy = TargetSequence([None] * df.height)
    executor = dxcore.Executor(dxcore.BaseOrderEngine(), strategy)

    executor.run(df, _daily_view())
    first = [n for _, _, _, n in strategy.calls]
    strategy.calls.clear()
    executor.run(df, _daily_view())
    second = [n for _, _, _, n in strategy.calls]

    assert first == second == list(range(1, df.height + 1))


def test_signal_targets_diff_against_engine_positions():
    df = _ohlc_df(6)
    # Hold 100 for three days, then flatten: only the two changes trade.
    strategy = TargetSequence([100.0, 100.0, 100.0, 0.0, 0.0, 0.0])
    orders = dxcore.Executor(dxcore.BaseOrderEngine("AAPL"), strategy).run(
        df, _daily_view()
    )

    days = _epoch_days(df)
    assert [(o.date, o.symbol, o.side) for o in orders] == [
        (days[0], "AAPL", dxcore.Side.Buy),
        (days[3], "AAPL", dxcore.Side.Sell),
    ]
    assert all(o.quantity == dxcore.OrderQuantity.Abs(100.0) for o in orders)
    assert all(o.order_type is dxcore.OrderType.Market for o in orders)
    assert all(o.price == dxcore.OrderPrice.Mkt for o in orders)


def test_order_data_passes_through_with_limit_price():
    df = _ohlc_df(3)

    class Limit:
        def on_step(self, date, step, history, state):
            if state.get("placed"):
                return None
            state["placed"] = True
            return dxcore.OrderData(
                quantity=dxcore.OrderQuantity.Abs(-50.0),
                order_type=dxcore.OrderType.Limit,
                price=dxcore.OrderPrice.Px(101.5),
                symbol="MSFT",
            )

    orders = dxcore.Executor(dxcore.BaseOrderEngine(), Limit()).run(df, _daily_view())

    assert len(orders) == 1
    order = orders[0]
    assert order.date == _epoch_days(df)[0]
    assert order.symbol == "MSFT"
    assert order.side is dxcore.Side.Sell
    assert order.quantity == dxcore.OrderQuantity.Abs(50.0)
    assert order.order_type is dxcore.OrderType.Limit
    assert order.price == dxcore.OrderPrice.Px(101.5)
    assert "Limit" in repr(order)


def test_bogus_strategy_output_raises_type_error():
    df = _ohlc_df(3)

    class Bogus:
        def on_step(self, date, step, history, state):
            return 42

    with pytest.raises(TypeError, match="Signal, or OrderData"):
        dxcore.Executor(dxcore.BaseOrderEngine(), Bogus()).run(df, _daily_view())
    with pytest.raises(TypeError, match="Signal, or OrderData"):
        list(
            dxcore.AsyncExecutor(dxcore.BaseOrderEngine(), Bogus()).run(
                _daily_steps(df), _daily_view()
            )
        )


def test_incoherent_order_raises_value_error():
    df = _ohlc_df(3)

    class Incoherent:
        def on_step(self, date, step, history, state):
            return dxcore.OrderData(
                quantity=dxcore.OrderQuantity.Abs(1.0),
                order_type=dxcore.OrderType.Limit,
                price=dxcore.OrderPrice.Mkt,
                symbol="AAPL",
            )

    with pytest.raises(ValueError, match="limit"):
        dxcore.Executor(dxcore.BaseOrderEngine(), Incoherent()).run(df, _daily_view())


def test_async_executor_matches_sync():
    df = _ohlc_df(6)
    targets = [100.0, 100.0, 0.0, 50.0, 50.0, 50.0]

    sync = dxcore.Executor(dxcore.BaseOrderEngine("AAPL"), TargetSequence(targets)).run(
        df, _daily_view()
    )
    streamed = list(
        dxcore.AsyncExecutor(dxcore.BaseOrderEngine("AAPL"), TargetSequence(targets)).run(
            _daily_steps(df), dxcore.DailyView("date")
        )
    )

    assert [_order_key(o) for o in streamed] == [_order_key(o) for o in sync]
    assert len(sync) == 3


def test_async_rejects_malformed_stream_items():
    df = _ohlc_df(3)
    with pytest.raises(ValueError, match="tuple"):
        list(
            dxcore.AsyncExecutor(dxcore.BaseOrderEngine(), TargetSequence([None])).run(
                (42 for _ in range(3)), _daily_view()
            )
        )


def test_strategy_exception_propagates():
    df = _ohlc_df(3)

    class Boom:
        def on_step(self, date, step, history, state):
            raise ValueError("boom from strategy")

    with pytest.raises(ValueError, match="boom from strategy"):
        dxcore.Executor(dxcore.BaseOrderEngine(), Boom()).run(df, _daily_view())
    with pytest.raises(ValueError, match="boom from strategy"):
        list(
            dxcore.AsyncExecutor(dxcore.BaseOrderEngine(), Boom()).run(
                _daily_steps(df), dxcore.DailyView("date")
            )
        )


def test_async_early_drop_stops_producer():
    df = _ohlc_df().rename({"close": "price"})
    view = dxcore.DailyView("date")
    baseline = threading.active_count()

    def steps():
        for day in _epoch_days(df):
            yield day, df.filter(pl.col("date").dt.epoch("d") == day)
            time.sleep(0.01)

    it = dxcore.AsyncExecutor(dxcore.BaseOrderEngine(), AlwaysOrder()).run(steps(), view)
    first = next(it)
    assert first.symbol == "AAPL"
    del it

    deadline = time.monotonic() + 5
    while threading.active_count() > baseline and time.monotonic() < deadline:
        time.sleep(0.01)
    assert threading.active_count() <= baseline
