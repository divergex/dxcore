pub mod external;
pub(crate) mod internal;

use pyo3::prelude::*;

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    external::register(m)?;
    internal::register(m)
}
