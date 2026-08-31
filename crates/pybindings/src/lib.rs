//! Thin Python bindings over the `dxcore` core library.

use ::dxcore::core::{AccountMetric, Instrument, InstrumentStore, Portfolio};
use pyo3::prelude::*;

#[pyclass(name = "Instrument", module = "dxcore", from_py_object)]
#[derive(Clone)]
pub struct PyInstrument {
    inner: Instrument,
}

#[pymethods]
impl PyInstrument {
    #[new]
    #[pyo3(signature = (contract_id, symbol, security_type, exchange, currency))]
    fn new(
        contract_id: i32,
        symbol: String,
        security_type: String,
        exchange: String,
        currency: String,
    ) -> Self {
        Self {
            inner: Instrument {
                contract_id,
                symbol,
                security_type,
                exchange,
                currency,
            },
        }
    }

    #[getter]
    fn contract_id(&self) -> i32 {
        self.inner.contract_id
    }

    #[getter]
    fn symbol(&self) -> &str {
        &self.inner.symbol
    }

    #[getter]
    fn security_type(&self) -> &str {
        &self.inner.security_type
    }

    #[getter]
    fn exchange(&self) -> &str {
        &self.inner.exchange
    }

    #[getter]
    fn currency(&self) -> &str {
        &self.inner.currency
    }

    fn __repr__(&self) -> String {
        format!("{}", self.inner)
    }
}

#[pyclass(name = "AccountMetric", module = "dxcore", from_py_object)]
#[derive(Clone)]
pub struct PyAccountMetric {
    inner: AccountMetric,
}

#[pymethods]
impl PyAccountMetric {
    #[new]
    #[pyo3(signature = (key, value, currency))]
    fn new(key: String, value: String, currency: String) -> Self {
        Self {
            inner: AccountMetric {
                key,
                value,
                currency,
            },
        }
    }

    #[getter]
    fn key(&self) -> &str {
        &self.inner.key
    }

    #[getter]
    fn value(&self) -> &str {
        &self.inner.value
    }

    #[getter]
    fn currency(&self) -> &str {
        &self.inner.currency
    }
}

#[pyclass(name = "Portfolio", module = "dxcore", from_py_object)]
#[derive(Clone)]
pub struct PyPortfolio {
    inner: Portfolio,
}

#[pymethods]
impl PyPortfolio {
    #[new]
    fn new() -> Self {
        Self {
            inner: Portfolio::default(),
        }
    }

    fn upsert_metric(&mut self, key: String, value: String, currency: String) {
        self.inner.upsert_metric(key, value, currency);
    }

    fn set_holding(&mut self, instrument: PyInstrument, quantity: f64) {
        self.inner.set_holding(instrument.inner, quantity);
    }

    fn instrument(&self, contract_id: i32) -> Option<PyInstrument> {
        self.inner
            .instrument(contract_id)
            .cloned()
            .map(|inner| PyInstrument { inner })
    }

    fn quantity(&self, contract_id: i32) -> Option<f64> {
        self.inner.quantity(contract_id)
    }

    fn holding_count(&self) -> usize {
        self.inner.holding_count()
    }

    fn holdings(&self) -> Vec<(PyInstrument, f64)> {
        self.inner
            .holdings()
            .map(|(inst, qty)| (PyInstrument { inner: inst.clone() }, qty))
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "Portfolio(metrics={}, holdings={})",
            self.inner.metrics.len(),
            self.inner.holding_count()
        )
    }
}

#[pyclass(name = "InstrumentStore", module = "dxcore", from_py_object)]
#[derive(Clone)]
pub struct PyInstrumentStore {
    inner: InstrumentStore,
}

#[pymethods]
impl PyInstrumentStore {
    #[new]
    fn new() -> Self {
        Self {
            inner: InstrumentStore::default(),
        }
    }

    fn insert(&mut self, instrument: PyInstrument) {
        self.inner.insert(instrument.inner);
    }

    fn get(&self, contract_id: i32) -> Option<PyInstrument> {
        self.inner
            .get(contract_id)
            .cloned()
            .map(|inner| PyInstrument { inner })
    }

    fn get_by_symbol(&self, symbol: &str) -> Option<PyInstrument> {
        self.inner
            .get_by_symbol(symbol)
            .cloned()
            .map(|inner| PyInstrument { inner })
    }

    fn len(&self) -> usize {
        self.inner.len()
    }

    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }
}

#[pymodule]
fn dxcore(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyInstrument>()?;
    m.add_class::<PyAccountMetric>()?;
    m.add_class::<PyPortfolio>()?;
    m.add_class::<PyInstrumentStore>()?;
    Ok(())
}
