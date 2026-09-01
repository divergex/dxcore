mod executor;
mod strategy;
mod view;

use pyo3::prelude::*;

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    view::register(m)?;
    executor::register(m)?;
    Ok(())
}
