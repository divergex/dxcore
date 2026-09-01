//! Bindings for `dxcore::network::services`: the `Service` trait bridge.

use std::sync::Arc;

use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use serde_json::Value;

use ::dxcore::network::services::{Request, Response, Service, ServiceError as CoreServiceError};

create_exception!(dxcore, ServiceError, PyException);

pub(crate) fn to_py_err(err: CoreServiceError) -> PyErr {
    ServiceError::new_err(err.to_string())
}

pub(crate) fn into_service(obj: &Bound<'_, PyAny>) -> PyResult<Arc<dyn Service>> {
    if let Ok(mesh) = obj.extract::<PyRef<super::mesh::PyMeshService>>() {
        return Ok(mesh.inner.clone());
    }
    Ok(Arc::new(PyService {
        obj: obj.clone().unbind(),
    }))
}

/// Wraps a Python object as a `Service`: `call(request: dict) -> dict` must
/// return `{"value": ...}`; `name()` and `endpoints()` are optional.
pub struct PyService {
    obj: Py<PyAny>,
}

impl Service for PyService {
    fn call(&self, request: Request) -> Result<Response, CoreServiceError> {
        Python::attach(|py| {
            let request_py = request_to_py(py, &request)
                .map_err(|e| CoreServiceError::Internal(e.to_string()))?;
            match self.obj.call_method1(py, "call", (request_py,)) {
                Ok(out) => response_from_py(out.bind(py))
                    .map_err(|msg| CoreServiceError::BadValue(msg)),
                Err(err) => Err(CoreServiceError::Internal(err.to_string())),
            }
        })
    }

    fn name(&self) -> String {
        Python::attach(|py| {
            self.obj
                .call_method0(py, "name")
                .and_then(|v| v.bind(py).extract())
                .unwrap_or_else(|_| "service".to_string())
        })
    }

    fn endpoints(&self) -> Vec<String> {
        Python::attach(|py| {
            self.obj
                .call_method0(py, "endpoints")
                .and_then(|v| v.bind(py).extract())
                .unwrap_or_default()
        })
    }
}

fn request_to_py(py: Python<'_>, request: &Request) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    match request {
        Request::Get {
            attribute,
            args,
        } => {
            dict.set_item("op", "get")?;
            dict.set_item("attribute", attribute.as_str())?;
            dict.set_item("args", value_to_py_opt(py, args.as_ref())?)?;
        }
        Request::Set {
            attribute,
            value,
        } => {
            dict.set_item("op", "set")?;
            dict.set_item("attribute", attribute.as_str())?;
            dict.set_item("value", value_to_py(py, value)?)?;
        }
        Request::Post {
            attribute,
            value,
        } => {
            dict.set_item("op", "post")?;
            dict.set_item("attribute", attribute.as_str())?;
            dict.set_item("value", value_to_py(py, value)?)?;
        }
    }
    Ok(dict.into_any().unbind())
}

fn response_from_py(obj: &Bound<'_, PyAny>) -> Result<Response, String> {
    let value = value_from_py(obj).map_err(|e| e.to_string())?;
    match value {
        Value::Object(mut map) => map
            .remove("value")
            .map(|value| Response { value })
            .ok_or_else(|| "response must be a dict with a \"value\" key".to_string()),
        _ => Err("response must be a dict with a \"value\" key".to_string()),
    }
}

fn value_to_py_opt(py: Python<'_>, value: Option<&Value>) -> PyResult<Py<PyAny>> {
    match value {
        Some(value) => value_to_py(py, value),
        None => Ok(py.None()),
    }
}

fn value_to_py(py: Python<'_>, value: &Value) -> PyResult<Py<PyAny>> {
    Ok(match value {
        Value::Null => py.None(),
        Value::Bool(b) => (*b).into_pyobject(py)?.to_owned().into_any().unbind(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_pyobject(py)?.into_any().unbind()
            } else if let Some(u) = n.as_u64() {
                u.into_pyobject(py)?.into_any().unbind()
            } else {
                n.as_f64()
                    .expect("json number is representable")
                    .into_pyobject(py)?
                    .into_any()
                    .unbind()
            }
        }
        Value::String(s) => s.clone().into_pyobject(py)?.into_any().unbind(),
        Value::Array(items) => {
            let list = PyList::empty(py);
            for item in items {
                list.append(value_to_py(py, item)?)?;
            }
            list.into_any().unbind()
        }
        Value::Object(map) => {
            let dict = PyDict::new(py);
            for (key, item) in map {
                dict.set_item(key.as_str(), value_to_py(py, item)?)?;
            }
            dict.into_any().unbind()
        }
    })
}

fn value_from_py(obj: &Bound<'_, PyAny>) -> PyResult<Value> {
    if obj.is_none() {
        return Ok(Value::Null);
    }
    if let Ok(b) = obj.extract::<bool>() {
        return Ok(Value::Bool(b));
    }
    if let Ok(i) = obj.extract::<i64>() {
        return Ok(Value::from(i));
    }
    if let Ok(f) = obj.extract::<f64>() {
        return Ok(Value::from(f));
    }
    if let Ok(s) = obj.extract::<String>() {
        return Ok(Value::String(s));
    }
    if let Ok(items) = obj.extract::<Vec<Bound<'_, PyAny>>>() {
        let mut out = Vec::with_capacity(items.len());
        for item in &items {
            out.push(value_from_py(item)?);
        }
        return Ok(Value::Array(out));
    }
    if let Ok(dict) = obj.cast::<PyDict>() {
        let mut out = serde_json::Map::new();
        for (key, item) in dict.iter() {
            out.insert(key.extract()?, value_from_py(&item)?);
        }
        return Ok(Value::Object(out));
    }
    Err(PyTypeError::new_err("value is not JSON-serializable"))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("ServiceError", m.py().get_type::<ServiceError>())?;
    Ok(())
}
