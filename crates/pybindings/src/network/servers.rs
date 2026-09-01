//! Bindings for `dxcore::network::servers`: the HTTP server and its handle.

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use ::dxcore::network::servers::{HttpServer, ServerHandle};

use super::services::{into_service, to_py_err};

#[pyclass(name = "HttpServer", module = "dxcore")]
pub struct PyHttpServer {
    inner: Option<HttpServer>,
}

#[pymethods]
impl PyHttpServer {
    #[new]
    fn new(addr: &str, service: Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: Some(HttpServer::bind(addr, into_service(&service)?).map_err(to_py_err)?),
        })
    }

    fn addr(&self) -> PyResult<String> {
        self.inner
            .as_ref()
            .map(|server| server.addr().to_string())
            .ok_or_else(already_started)
    }

    fn spawn(slf: Py<Self>, py: Python<'_>) -> PyResult<PyServerHandle> {
        let inner = slf.borrow_mut(py).inner.take().ok_or_else(already_started)?;
        Ok(PyServerHandle {
            inner: Some(inner.spawn()),
        })
    }
}

fn already_started() -> PyErr {
    PyRuntimeError::new_err("server already started")
}

#[pyclass(name = "ServerHandle", module = "dxcore")]
pub struct PyServerHandle {
    inner: Option<ServerHandle>,
}

#[pymethods]
impl PyServerHandle {
    fn addr(&self) -> PyResult<String> {
        self.inner
            .as_ref()
            .map(|handle| handle.addr().to_string())
            .ok_or_else(already_stopped)
    }

    fn stop(slf: Py<Self>, py: Python<'_>) -> PyResult<()> {
        let inner = slf.borrow_mut(py).inner.take().ok_or_else(already_stopped)?;
        py.detach(move || inner.stop()).map_err(to_py_err)
    }
}

fn already_stopped() -> PyErr {
    PyRuntimeError::new_err("server already stopped")
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyHttpServer>()?;
    m.add_class::<PyServerHandle>()?;
    Ok(())
}
