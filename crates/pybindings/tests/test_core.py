"""Tests for the dxcore core model bindings.

Requires the compiled extension: cargo build -p dxcore-pyo3 --features extension-module.
"""

import dxcore


def test_core_model():
    p = dxcore.Portfolio()
    p.upsert_metric("NetLiquidation", "100000", "USD")
    inst = dxcore.Instrument(1, "AAPL", "STK", "SMART", "USD")
    p.set_holding(inst, 100.0)
    assert p.quantity(1) == 100.0
    assert p.holding_count() == 1
    assert p.metrics()[0].key == "NetLiquidation"
    assert p.holdings()[0][0].symbol == "AAPL"

    store = dxcore.InstrumentStore()
    store.insert(inst)
    assert store.get(1).symbol == "AAPL"
    assert store.get_by_symbol("AAPL").contract_id == 1
    assert len(store) == 1
