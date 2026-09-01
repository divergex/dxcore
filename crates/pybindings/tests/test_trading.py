"""Tests for the dxcore trading executor bindings.

Requires the compiled extension: cargo build -p dxcore-pyo3 --features extension-module.
"""

import datetime
import threading
import time
from typing import Any
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


def _daily_steps(df: pl.DataFrame) -> Iterator[tuple[int, pl.DataFrame]]:
    # Async stream items reach on_step as-is; the view's col_map is not
    # applied there, so rename before slicing.
    df = df.rename({"close": "price"})
    for day in _epoch_days(df):
        yield day, df.filter(pl.col("date").dt.epoch("d") == day)


class Recorder:
    def __init__(self) -> None:
        self.steps: list[tuple[int, tuple[str, ...], int]] = []

    def create_output(self) -> pl.DataFrame:
        return pl.DataFrame(schema={"date": pl.Int32, "n": pl.Int32})

    def on_step(
        self, date: int, step: pl.DataFrame, history: pl.DataFrame, state: dict
    ) -> dict[str, Any]:
        state["n"] = state.get("n", 0) + 1
        self.steps.append((date, tuple(step.columns), history.height))
        return {"date": date, "n": state["n"]}

    def append_output(
        self, frame: pl.DataFrame, output: Any, _date: int, _step: pl.DataFrame
    ) -> pl.DataFrame:
        row = pl.DataFrame([output], schema=frame.schema)
        return pl.concat([frame, row]) if frame.height else row


class Boom:
    def create_output(self) -> pl.DataFrame:
        return pl.DataFrame(schema={"x": pl.Int32})

    def on_step(
        self, _date: int, _step: pl.DataFrame, _history: pl.DataFrame, _state: dict
    ) -> dict[str, Any]:
        raise ValueError("boom from strategy")

    def append_output(
        self, _frame: pl.DataFrame, _output: Any, _date: int, _step: pl.DataFrame
    ) -> None:
        pass


def test_sync_executor_runs_protocol():
    df = _ohlc_df()
    strategy = Recorder()
    frame = dxcore.Executor(strategy).run(
        df, dxcore.DailyView("date", [("close", "price")])
    )

    n = df.height
    assert frame.height == n
    # state dict persists across steps within a run
    assert frame["n"].to_list() == list(range(1, n + 1))
    assert frame["date"].to_list() == _epoch_days(df)
    # every step: col_map applied, single row, history grows by one per step
    assert [cols for _, cols, _ in strategy.steps] == [("date", "price")] * n
    assert [h for _, _, h in strategy.steps] == list(range(n))
    dates = [d for d, _, _ in strategy.steps]
    assert dates == sorted(dates)


def test_state_is_fresh_per_run():
    df = _ohlc_df()
    view = dxcore.DailyView("date")
    executor = dxcore.Executor(Recorder())
    first = executor.run(df, view)
    second = executor.run(df, view)
    expected = list(range(1, df.height + 1))
    assert first["n"].to_list() == second["n"].to_list() == expected


def test_async_executor_matches_sync():
    df = _ohlc_df()
    view = dxcore.DailyView("date", [("close", "price")])

    sync = dxcore.Executor(Recorder()).run(df, view)
    outputs = list(dxcore.AsyncExecutor(Recorder()).run(_daily_steps(df), view))

    assert [o["n"] for o in outputs] == sync["n"].to_list()
    assert [o["date"] for o in outputs] == sync["date"].to_list()


def test_strategy_exception_propagates():
    df = _ohlc_df()
    view = dxcore.DailyView("date")

    with pytest.raises(ValueError, match="boom from strategy"):
        dxcore.Executor(Boom()).run(df, view)
    with pytest.raises(ValueError, match="boom from strategy"):
        list(dxcore.AsyncExecutor(Boom()).run(_daily_steps(df), view))


def test_async_rejects_malformed_stream_items():
    view = dxcore.DailyView("date")
    with pytest.raises(ValueError, match="tuple"):
        list(dxcore.AsyncExecutor(Recorder()).run((42 for _ in range(3)), view))


def test_async_early_drop_stops_producer():
    df = _ohlc_df().rename({"close": "price"})
    view = dxcore.DailyView("date")
    baseline = threading.active_count()

    def steps():
        for day in _epoch_days(df):
            yield day, df.filter(pl.col("date").dt.epoch("d") == day)
            time.sleep(0.01)

    it = dxcore.AsyncExecutor(Recorder()).run(steps(), view)
    next(it)
    del it

    deadline = time.monotonic() + 5
    while threading.active_count() > baseline and time.monotonic() < deadline:
        time.sleep(0.01)
    assert threading.active_count() <= baseline
