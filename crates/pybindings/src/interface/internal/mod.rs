mod factory;
mod strategy;

use pyo3::prelude::*;

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    factory::register(m)?;
    strategy::register(m)
}
