//! Financial Modeling Prep bindings.

use pyo3::prelude::*;

use ::dxcore::interface::external::fmp::{BalanceSheet, FmpClient, IncomeStatement, Profile};

use super::to_py_err;

#[pyclass(name = "FmpProfile", module = "dxcore", from_py_object)]
#[derive(Clone)]
pub struct PyFmpProfile {
    inner: Profile,
}

#[pymethods]
impl PyFmpProfile {
    #[getter]
    fn symbol(&self) -> &str {
        &self.inner.symbol
    }

    #[getter]
    fn price(&self) -> Option<f64> {
        self.inner.price
    }

    #[getter]
    fn market_cap(&self) -> Option<f64> {
        self.inner.market_cap
    }

    #[getter]
    fn currency(&self) -> Option<&str> {
        self.inner.currency.as_deref()
    }
}

#[pyclass(name = "FmpBalanceSheet", module = "dxcore", from_py_object)]
#[derive(Clone)]
pub struct PyFmpBalanceSheet {
    inner: BalanceSheet,
}

#[pymethods]
impl PyFmpBalanceSheet {
    #[getter]
    fn symbol(&self) -> &str {
        &self.inner.symbol
    }

    #[getter]
    fn date(&self) -> &str {
        &self.inner.date
    }

    #[getter]
    fn period(&self) -> &str {
        &self.inner.period
    }

    #[getter]
    fn total_assets(&self) -> Option<f64> {
        self.inner.total_assets
    }

    #[getter]
    fn total_liabilities(&self) -> Option<f64> {
        self.inner.total_liabilities
    }

    #[getter]
    fn total_stockholders_equity(&self) -> Option<f64> {
        self.inner.total_stockholders_equity
    }

    #[getter]
    fn total_equity(&self) -> Option<f64> {
        self.inner.total_equity
    }
}

#[pyclass(name = "FmpIncomeStatement", module = "dxcore", from_py_object)]
#[derive(Clone)]
pub struct PyFmpIncomeStatement {
    inner: IncomeStatement,
}

#[pymethods]
impl PyFmpIncomeStatement {
    #[getter]
    fn symbol(&self) -> &str {
        &self.inner.symbol
    }

    #[getter]
    fn date(&self) -> &str {
        &self.inner.date
    }

    #[getter]
    fn period(&self) -> &str {
        &self.inner.period
    }

    #[getter]
    fn revenue(&self) -> Option<f64> {
        self.inner.revenue
    }

    #[getter]
    fn operating_income(&self) -> Option<f64> {
        self.inner.operating_income
    }

    #[getter]
    fn net_income(&self) -> Option<f64> {
        self.inner.net_income
    }

    #[getter]
    fn weighted_average_shs_out(&self) -> Option<f64> {
        self.inner.weighted_average_shs_out
    }
}

/// FMP client. Set the `FMP_API_KEY` environment variable, or pass the key
/// to `FmpClient(api_key)`.
#[pyclass(name = "FmpClient", module = "dxcore")]
pub struct PyFmpClient {
    inner: FmpClient,
}

#[pymethods]
impl PyFmpClient {
    #[new]
    fn new(api_key: String) -> Self {
        Self {
            inner: FmpClient::new(api_key),
        }
    }

    #[staticmethod]
    fn from_env() -> PyResult<Self> {
        FmpClient::from_env().map(|inner| Self { inner }).map_err(to_py_err)
    }

    /// Current market cap, price, and currency for a ticker.
    fn profile(&self, symbol: &str) -> PyResult<PyFmpProfile> {
        self.inner
            .profile(symbol)
            .map(|inner| PyFmpProfile { inner })
            .map_err(to_py_err)
    }

    /// Annual balance sheet statements (most recent first).
    fn balance_sheet(&self, symbol: &str) -> PyResult<Vec<PyFmpBalanceSheet>> {
        self.inner
            .balance_sheet(symbol)
            .map(|rows| {
                rows.into_iter()
                    .map(|inner| PyFmpBalanceSheet { inner })
                    .collect()
            })
            .map_err(to_py_err)
    }

    /// Annual income statements (most recent first).
    fn income_statement(&self, symbol: &str) -> PyResult<Vec<PyFmpIncomeStatement>> {
        self.inner
            .income_statement(symbol)
            .map(|rows| {
                rows.into_iter()
                    .map(|inner| PyFmpIncomeStatement { inner })
                    .collect()
            })
            .map_err(to_py_err)
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFmpClient>()?;
    m.add_class::<PyFmpProfile>()?;
    m.add_class::<PyFmpBalanceSheet>()?;
    m.add_class::<PyFmpIncomeStatement>()?;
    Ok(())
}
