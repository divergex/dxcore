import datetime
import math

import polars as pl
import pytest

import dxcore


class Cross:
    def __init__(self, short: int = 2, long: int = 3) -> None:
        self.short = short
        self.long = long

    def on_step(self, date, step, history, state):
        price = float(step["price"][0])
        short = _sma(history, self.short)
        long = _sma(history, self.long)
        before = state.get("shares", 0.0)
        prev_short, prev_long = state.get("prev_short"), state.get("prev_long")

        if short is None or prev_short is None or prev_long is None:
            state["shares"] = float(int(1000.0 / price))
        elif prev_short <= prev_long and short > long:
            state["shares"] = float(int(1000.0 / price))
        elif prev_short >= prev_long and short < long:
            state["shares"] = 0.0

        state["prev_short"] = short
        state["prev_long"] = long
        if state["shares"] != before:
            return dxcore.Signal(shares=state["shares"], symbol="DEMO")
        return None


def _sma(history: pl.DataFrame, window: int) -> float | None:
    if history.height < window:
        return None
    return float(history["price"].tail(window).mean())


def _prices(n: int = 30) -> list[float]:
    return [round(100.0 + 10.0 * math.sin(i * 0.5 * math.pi / 2.0)) for i in range(n)]


def _step(date: int, price: float) -> pl.DataFrame:
    return pl.DataFrame({"price": [price]})


def _empty() -> pl.DataFrame:
    return pl.DataFrame(schema={"price": pl.Float64})


def _serve(strategy: Cross) -> dxcore.ServerHandle:
    state = {}

    def on_step(args):
        date, rows = args["step"]
        step = pl.DataFrame(rows)
        history = pl.DataFrame(args["history"])
        return strategy.on_step(date, step, history, state)

    service = dxcore.FunctionalService("strategy").set("on_step", on_step)
    return dxcore.HttpServer("127.0.0.1:0", service).spawn()


def _on_step(accessor, args):
    return accessor.set("on_step", args)


def _interface(url: str) -> dxcore.StrategyInterface:
    factory = dxcore.InterfaceFactory("strategy")
    factory.accessor(dxcore.Protocol.Http, dxcore.HttpAccessor(url))
    interface = factory.set("on_step", dxcore.Protocol.Http, _on_step).build()
    return dxcore.StrategyInterface(interface)


def _signal_key(signal) -> tuple | None:
    return None if signal is None else (signal.shares, signal.symbol)


def test_interface_matches_a_local_strategy():
    handle = _serve(Cross())
    try:
        interface = _interface(f"http://{handle.addr()}")
        local = Cross()

        local_state = {}
        history = _empty()
        signals = 0
        for i, price in enumerate(_prices()):
            step = _step(19000 + i, price)
            expected = local.on_step(19000 + i, step, history, local_state)
            # A fresh state dict per call: only the served side can be keeping it.
            actual = interface.on_step(19000 + i, step, history, {})

            assert _signal_key(actual) == _signal_key(expected)
            signals += actual is not None
            history = step if history.height == 0 else history.vstack(step)

        assert signals > 0
        assert interface.take_error() is None
    finally:
        handle.stop()


def _ohlc(n: int = 30) -> pl.DataFrame:
    return pl.DataFrame(
        {
            "date": [datetime.date(2024, 1, 1) + datetime.timedelta(days=i) for i in range(n)],
            "symbol": ["DEMO"] * n,
            "close": _prices(n),
        }
    )


def _order_key(order: dxcore.Order) -> tuple:
    return (order.date, order.symbol, order.side, order.quantity, order.order_type, order.price)


def test_interface_runs_in_the_executor():
    df = _ohlc()
    view = dxcore.DailyView("date", [("close", "price")])

    local = dxcore.Executor(dxcore.BaseOrderEngine(), Cross()).run(df, view)
    assert local

    handle = _serve(Cross())
    try:
        remote = dxcore.Executor(
            dxcore.BaseOrderEngine(), _interface(f"http://{handle.addr()}")
        ).run(df, view)
    finally:
        handle.stop()

    assert [_order_key(o) for o in remote] == [_order_key(o) for o in local]


def test_request_failure_raises():
    iface = _interface("http://127.0.0.1:1")
    with pytest.raises(RuntimeError):
        iface.on_step(19000, _step(19000, 100.0), _empty(), {})


def test_build_rejects_a_method_the_service_does_not_declare():
    factory = dxcore.InterfaceFactory("strategy")
    factory.accessor(dxcore.Protocol.Http, dxcore.HttpAccessor("http://127.0.0.1:1"))
    with pytest.raises(RuntimeError, match="no method"):
        factory.set("nope", dxcore.Protocol.Http, _on_step).build()


def test_build_rejects_an_unregistered_service():
    factory = dxcore.InterfaceFactory("nope")
    factory.accessor(dxcore.Protocol.Http, dxcore.HttpAccessor("http://127.0.0.1:1"))
    with pytest.raises(RuntimeError, match="not registered"):
        factory.build()


def test_a_user_registered_spec_gives_access():
    dxcore.ServiceSpec("thing").get("read", [dxcore.Protocol.Http]).register()

    factory = dxcore.InterfaceFactory("thing")
    factory.accessor(dxcore.Protocol.Http, dxcore.HttpAccessor("http://127.0.0.1:1"))
    interface = factory.get("read", dxcore.Protocol.Http, lambda accessor, args: args).build()
    assert interface.service == "thing"

    # The spec declares a getter; registering it as a setter is caught.
    bad = dxcore.InterfaceFactory("thing")
    bad.accessor(dxcore.Protocol.Http, dxcore.HttpAccessor("http://127.0.0.1:1"))
    with pytest.raises(RuntimeError, match="registered as a Set"):
        bad.set("read", dxcore.Protocol.Http, lambda accessor, args: args).build()
