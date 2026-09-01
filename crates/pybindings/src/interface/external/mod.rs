//! Bindings for `dxcore::interface::external` data sources.

mod fmp;
mod guardian;
#[cfg(feature = "ibkr")]
mod ibkr;
mod xbrl;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use ::dxcore::Error as CoreError;

pub(crate) fn to_py_err(err: CoreError) -> PyErr {
    PyRuntimeError::new_err(err.to_string())
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    fmp::register(m)?;
    guardian::register(m)?;
    xbrl::register(m)?;
    #[cfg(feature = "ibkr")]
    ibkr::register(m)?;
    Ok(())
}
