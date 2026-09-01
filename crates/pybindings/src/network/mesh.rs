//! Bindings for `dxcore::network::mesh`: the service mesh and registry types.

use std::collections::HashMap;
use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::PyType;

use ::dxcore::network::mesh::{Endpoint, MeshService, Protocol, Registration};

use super::services::{into_service, to_py_err};

#[pyclass(name = "Protocol", module = "dxcore", eq, from_py_object)]
#[derive(Clone, Copy, PartialEq)]
pub enum PyProtocol {
    Http,
}

impl PyProtocol {
    fn to_core(self) -> Protocol {
        match self {
            PyProtocol::Http => Protocol::Http,
        }
    }

    fn from_core(protocol: Protocol) -> Self {
        match protocol {
            Protocol::Http => PyProtocol::Http,
        }
    }
}

#[pymethods]
impl PyProtocol {
    #[classmethod]
    fn parse(_cls: &Bound<'_, PyType>, name: &str) -> Option<PyProtocol> {
        Protocol::parse(name).map(Self::from_core)
    }

    fn __str__(&self) -> &'static str {
        self.to_core().as_str()
    }
}

#[pyclass(name = "Endpoint", module = "dxcore", from_py_object)]
#[derive(Clone)]
pub struct PyEndpoint {
    inner: Endpoint,
}

#[pymethods]
impl PyEndpoint {
    #[getter]
    fn protocols(&self) -> Vec<PyProtocol> {
        self.inner
            .protocols
            .iter()
            .map(|protocol| PyProtocol::from_core(*protocol))
            .collect()
    }

    #[getter]
    fn description(&self) -> Option<String> {
        self.inner.description.clone()
    }
}

#[pyclass(name = "Registration", module = "dxcore", from_py_object)]
#[derive(Clone)]
pub struct PyRegistration {
    inner: Registration,
}

#[pymethods]
impl PyRegistration {
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    #[getter]
    fn url(&self) -> &str {
        &self.inner.url
    }

    #[getter]
    fn protocols(&self) -> Vec<PyProtocol> {
        self.inner
            .protocols
            .iter()
            .map(|protocol| PyProtocol::from_core(*protocol))
            .collect()
    }

    #[getter]
    fn endpoints(&self) -> HashMap<String, PyEndpoint> {
        self.inner
            .endpoints
            .iter()
            .map(|(name, endpoint)| (name.clone(), PyEndpoint { inner: endpoint.clone() }))
            .collect()
    }
}

#[pyclass(name = "MeshService", module = "dxcore")]
pub struct PyMeshService {
    pub(crate) inner: Arc<MeshService>,
}

#[pymethods]
impl PyMeshService {
    #[new]
    fn new() -> Self {
        Self {
            inner: Arc::new(MeshService::new()),
        }
    }

    fn register(
        &self,
        service: Bound<'_, PyAny>,
        url: &str,
        protocol: PyProtocol,
    ) -> PyResult<String> {
        self.inner
            .register(into_service(&service)?, url, protocol.to_core())
            .map_err(to_py_err)
    }

    fn registrations(&self) -> PyResult<HashMap<String, PyRegistration>> {
        self.inner
            .registrations()
            .map(|registrations| {
                registrations
                    .into_iter()
                    .map(|(uuid, registration)| (uuid, PyRegistration { inner: registration }))
                    .collect()
            })
            .map_err(to_py_err)
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyProtocol>()?;
    m.add_class::<PyEndpoint>()?;
    m.add_class::<PyRegistration>()?;
    m.add_class::<PyMeshService>()?;
    Ok(())
}
