//! XBRL filings bindings.

use pyo3::prelude::*;

use ::dxcore::interface::external::xbrl::{query_filings, XbrlFiling};

use super::to_py_err;

#[pyclass(name = "XbrlFiling", module = "dxcore", from_py_object)]
#[derive(Clone)]
pub struct PyXbrlFiling {
    inner: XbrlFiling,
}

#[pymethods]
impl PyXbrlFiling {
    #[getter]
    fn entity_name(&self) -> &str {
        &self.inner.entity_name
    }

    #[getter]
    fn language(&self) -> &str {
        &self.inner.language
    }

    #[getter]
    fn country(&self) -> &str {
        &self.inner.country
    }

    #[getter]
    fn period_end(&self) -> &str {
        &self.inner.period_end
    }

    #[getter]
    fn date_added(&self) -> &str {
        &self.inner.date_added
    }

    #[getter]
    fn view_url(&self) -> &str {
        &self.inner.view_url
    }
}

#[pyclass(name = "XbrlClient", module = "dxcore")]
pub struct PyXbrlClient;

#[pymethods]
impl PyXbrlClient {
    #[new]
    fn new() -> Self {
        Self
    }

    /// Query filings; each query matches entity name, identifier, language or
    /// file hash (OR-ed together).
    fn query_filings(&self, queries: Vec<String>, limit: usize) -> PyResult<Vec<PyXbrlFiling>> {
        let queries: Vec<&str> = queries.iter().map(String::as_str).collect();
        query_filings(&queries, limit)
            .map(|rows| {
                rows.into_iter()
                    .map(|inner| PyXbrlFiling { inner })
                    .collect()
            })
            .map_err(to_py_err)
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyXbrlClient>()?;
    m.add_class::<PyXbrlFiling>()?;
    Ok(())
}
