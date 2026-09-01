//! Bindings for `dxcore::network`: services, the HTTP server and the mesh.

pub mod mesh;
pub mod servers;
pub mod services;

use pyo3::prelude::*;

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    services::register(m)?;
    servers::register(m)?;
    mesh::register(m)?;
    Ok(())
}
