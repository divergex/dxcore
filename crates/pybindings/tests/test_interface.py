"""Tests for the dxcore interface (external data source) bindings.

Requires the compiled extension: cargo build -p dxcore-pyo3 --features extension-module.
"""

import pytest

import dxcore


def test_fmp_from_env_requires_key(monkeypatch):
    monkeypatch.delenv("FMP_API_KEY", raising=False)
    with pytest.raises(RuntimeError, match="FMP_API_KEY"):
        dxcore.FmpClient.from_env()


def test_data_source_construction():
    assert isinstance(dxcore.FmpClient("dummy-key"), dxcore.FmpClient)
    assert isinstance(dxcore.GuardianClient("dummy-key"), dxcore.GuardianClient)
    assert isinstance(dxcore.XbrlClient(), dxcore.XbrlClient)
