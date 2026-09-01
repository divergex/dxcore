//! Interactive Brokers (TWS/Gateway) bindings.

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

use crate::core::PyPortfolio;
use crate::dataframe;

use ::dxcore::interface::external::ibkr::IbkrInterface;
use ::dxcore::interface::MarketApi;

use ibapi::contracts::{Contract, ContractBuilder, SecurityType};

use super::to_py_err;

/// A contract descriptor used for IBKR historical data requests.
#[pyclass(name = "Contract", module = "dxcore", from_py_object)]
#[derive(Clone)]
pub struct PyContract {
    contract_id: i32,
    symbol: String,
    security_type: String,
    exchange: String,
    currency: String,
}

#[pymethods]
impl PyContract {
    #[new]
    #[pyo3(signature = (contract_id, symbol, security_type="STK", exchange="SMART", currency="USD"))]
    fn new(
        contract_id: i32,
        symbol: &str,
        security_type: &str,
        exchange: &str,
        currency: &str,
    ) -> Self {
        Self {
            contract_id,
            symbol: symbol.to_string(),
            security_type: security_type.to_string(),
            exchange: exchange.to_string(),
            currency: currency.to_string(),
        }
    }

    #[getter]
    fn contract_id(&self) -> i32 {
        self.contract_id
    }

    #[getter]
    fn symbol(&self) -> &str {
        &self.symbol
    }

    #[getter]
    fn security_type(&self) -> &str {
        &self.security_type
    }

    #[getter]
    fn exchange(&self) -> &str {
        &self.exchange
    }

    #[getter]
    fn currency(&self) -> &str {
        &self.currency
    }

    fn __repr__(&self) -> String {
        format!(
            "Contract({}, {}, {}, {})",
            self.contract_id, self.symbol, self.exchange, self.currency
        )
    }
}

impl PyContract {
    fn to_ibapi(&self) -> Result<Contract, PyErr> {
        let security_type = match self.security_type.to_ascii_uppercase().as_str() {
            "STK" => SecurityType::Stock,
            "OPT" => SecurityType::Option,
            "FUT" => SecurityType::Future,
            "CONTFUT" => SecurityType::ContinuousFuture,
            "IND" => SecurityType::Index,
            "FOP" => SecurityType::FuturesOption,
            "CASH" => SecurityType::ForexPair,
            "COMBO" => SecurityType::Spread,
            "WAR" => SecurityType::Warrant,
            "BOND" => SecurityType::Bond,
            "CMDTY" => SecurityType::Commodity,
            other => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "unknown security type: {other}"
                )))
            }
        };
        ContractBuilder::new()
            .contract_id(self.contract_id)
            .symbol(&self.symbol)
            .security_type(security_type)
            .exchange(&self.exchange)
            .currency(&self.currency)
            .build()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }
}

#[pyclass(name = "IbkrInterface", module = "dxcore")]
pub struct PyIbkrInterface {
    inner: IbkrInterface,
}

#[pymethods]
impl PyIbkrInterface {
    #[new]
    fn new(host: String, client_id: i32) -> Self {
        Self {
            inner: IbkrInterface::new(host, client_id),
        }
    }

    /// Fetch historical bars for `contract` as a DataFrame with columns
    /// `date`, `open`, `high`, `low`, `close`, `volume`.
    ///
    /// `bar_size` and `duration` accept the same strings as IB's API,
    /// e.g. `"1 day"` and `"30 D"`.
    fn market_history(
        &self,
        contract: &PyContract,
        bar_size: &str,
        duration: &str,
    ) -> PyResult<Py<PyAny>> {
        let contract = contract.to_ibapi()?;
        let bar_size = bar_size
            .parse()
            .map_err(|e| PyValueError::new_err(format!("bad bar_size: {e}")))?;
        let duration = duration
            .parse()
            .map_err(|e| PyValueError::new_err(format!("bad duration: {e}")))?;
        let df = self
            .inner
            .market_history(&contract, bar_size, duration)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Python::attach(|py| dataframe::df_to_py(py, &df))
    }

    /// Fetch the current account portfolio (metrics + holdings).
    fn portfolio(&self, account_id: &str) -> PyResult<PyPortfolio> {
        self.inner
            .portfolio(account_id)
            .map(|inner| PyPortfolio { inner })
            .map_err(to_py_err)
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyIbkrInterface>()?;
    m.add_class::<PyContract>()?;
    Ok(())
}
