//! Thin Python bindings over the `dxcore` core library.

mod core;
mod dataframe;
mod interface;
mod trading;

use pyo3::prelude::*;

#[pymodule]
fn dxcore(m: &Bound<'_, PyModule>) -> PyResult<()> {
    core::register(m)?;
    trading::register(m)?;
    interface::register(m)?;
    Ok(())
}
