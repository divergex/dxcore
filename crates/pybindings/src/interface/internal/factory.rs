use std::sync::Arc;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use serde_json::Value;

use ::dxcore::interface::internal::{
    registry_guard, HttpAccessor, Interface, InterfaceFactory, MethodKind, ServiceSpec,
};
use ::dxcore::network::mesh::Protocol;
use ::dxcore::Error;

use crate::network::mesh::PyProtocol;
use crate::network::services::{value_from_py, value_to_py};

fn to_py_err(err: Error) -> PyErr {
    PyRuntimeError::new_err(err.to_string())
}

fn to_value(args: Option<&Bound<'_, PyAny>>) -> PyResult<Value> {
    match args {
        Some(args) => value_from_py(args),
        None => Ok(Value::Null),
    }
}

struct PyAccessor {
    obj: Py<PyAny>,
}

fn call_handler(accessor: &PyAccessor, handler: &Py<PyAny>, args: Value) -> Result<Value, Error> {
    Python::attach(|py| {
        let args = value_to_py(py, &args).map_err(|e| Error::Interface(e.to_string()))?;
        let out = handler
            .bind(py)
            .call1((accessor.obj.bind(py), args))
            .map_err(|e| Error::Interface(e.to_string()))?;
        value_from_py(&out).map_err(|e| Error::Interface(e.to_string()))
    })
}

#[pyclass(name = "HttpAccessor", module = "dxcore")]
pub struct PyHttpAccessor {
    inner: HttpAccessor,
}

#[pymethods]
impl PyHttpAccessor {
    #[new]
    fn new(url: &str) -> Self {
        Self {
            inner: HttpAccessor::new(url),
        }
    }

    #[getter]
    fn url(&self) -> &str {
        self.inner.url()
    }

    #[pyo3(signature = (method, args=None))]
    fn get(
        &self,
        py: Python<'_>,
        method: &str,
        args: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let args = to_value(args)?;
        // Held GIL would deadlock: the server thread needs it to run the
        // served Python code while this thread waits on the socket.
        let value: Value = py.detach(|| self.inner.get(method, &args)).map_err(to_py_err)?;
        value_to_py(py, &value)
    }

    #[pyo3(signature = (method, args=None))]
    fn set(
        &self,
        py: Python<'_>,
        method: &str,
        args: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let args = to_value(args)?;
        let value: Value = py.detach(|| self.inner.set(method, &args)).map_err(to_py_err)?;
        value_to_py(py, &value)
    }
}

#[pyclass(name = "InterfaceFactory", module = "dxcore")]
pub struct PyInterfaceFactory {
    service: String,
    accessors: Vec<(Protocol, Py<PyAny>)>,
    methods: Vec<(String, Protocol, MethodKind, Py<PyAny>)>,
}

#[pymethods]
impl PyInterfaceFactory {
    #[new]
    fn new(service: &str) -> Self {
        Self {
            service: service.to_string(),
            accessors: Vec::new(),
            methods: Vec::new(),
        }
    }

    fn accessor(
        slf: Py<Self>,
        py: Python<'_>,
        protocol: PyProtocol,
        accessor: Py<PyAny>,
    ) -> PyResult<Py<Self>> {
        slf.borrow_mut(py).accessors.push((protocol.to_core(), accessor));
        Ok(slf)
    }

    fn get(
        slf: Py<Self>,
        py: Python<'_>,
        method: &str,
        protocol: PyProtocol,
        handler: Py<PyAny>,
    ) -> PyResult<Py<Self>> {
        slf.borrow_mut(py).methods.push((
            method.to_string(),
            protocol.to_core(),
            MethodKind::Get,
            handler,
        ));
        Ok(slf)
    }

    fn set(
        slf: Py<Self>,
        py: Python<'_>,
        method: &str,
        protocol: PyProtocol,
        handler: Py<PyAny>,
    ) -> PyResult<Py<Self>> {
        slf.borrow_mut(py).methods.push((
            method.to_string(),
            protocol.to_core(),
            MethodKind::Set,
            handler,
        ));
        Ok(slf)
    }

    fn build(&self, py: Python<'_>) -> PyResult<PyInterface> {
        let mut factory = InterfaceFactory::new(self.service.clone());
        for (protocol, accessor) in &self.accessors {
            factory = factory.accessor(
                *protocol,
                PyAccessor {
                    obj: accessor.clone_ref(py),
                },
            );
        }
        for (method, protocol, kind, handler) in &self.methods {
            let handler = handler.clone_ref(py);
            let invoke = move |accessor: &PyAccessor, args: Value| call_handler(accessor, &handler, args);
            factory = match kind {
                MethodKind::Get => factory.get(method, *protocol, invoke),
                MethodKind::Set => factory.set(method, *protocol, invoke),
            };
        }
        Ok(PyInterface {
            inner: Arc::new(factory.build().map_err(to_py_err)?),
        })
    }
}

#[pyclass(name = "Interface", module = "dxcore")]
pub struct PyInterface {
    pub(crate) inner: Arc<Interface>,
}

#[pymethods]
impl PyInterface {
    #[getter]
    fn service(&self) -> &str {
        self.inner.service()
    }

    #[pyo3(signature = (method, args=None))]
    fn call(
        &self,
        py: Python<'_>,
        method: &str,
        args: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let args = to_value(args)?;
        let value = self.inner.call(method, args).map_err(to_py_err)?;
        value_to_py(py, &value)
    }
}

#[pyclass(name = "ServiceSpec", module = "dxcore")]
pub struct PyServiceSpec {
    inner: ServiceSpec,
}

#[pymethods]
impl PyServiceSpec {
    #[new]
    fn new(name: &str) -> Self {
        Self {
            inner: ServiceSpec::new(name),
        }
    }

    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    fn get(&self, method: &str, protocols: Vec<PyProtocol>) -> PyServiceSpec {
        PyServiceSpec {
            inner: self
                .inner
                .clone()
                .with_get(method, &core_protocols(protocols)),
        }
    }

    fn set(&self, method: &str, protocols: Vec<PyProtocol>) -> PyServiceSpec {
        PyServiceSpec {
            inner: self
                .inner
                .clone()
                .with_set(method, &core_protocols(protocols)),
        }
    }

    fn register(&self) {
        registry_guard().register(self.inner.clone());
    }
}

fn core_protocols(protocols: Vec<PyProtocol>) -> Vec<Protocol> {
    protocols.into_iter().map(PyProtocol::to_core).collect()
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyHttpAccessor>()?;
    m.add_class::<PyInterfaceFactory>()?;
    m.add_class::<PyInterface>()?;
    m.add_class::<PyServiceSpec>()?;
    Ok(())
}
