//! Bindings for `dxcore::interface` — the external data sources.

pub mod external;

use pyo3::prelude::*;

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    external::register(m)
}
