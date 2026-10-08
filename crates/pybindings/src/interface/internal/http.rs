use std::sync::Arc;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;

use ::dxcore::interface::internal::http::{HttpStrategy, StepArgs};
use ::dxcore::network::services::{Request, Response, Service, ServiceError};

use crate::dataframe::{df_from_py, df_to_py};
use crate::trading::{output_from_json, output_to_json};

#[pyclass(name = "HttpStrategy", module = "dxcore")]
pub struct PyHttpStrategy {
    inner: HttpStrategy<Value>,
}

#[pymethods]
impl PyHttpStrategy {
    #[new]
    fn new(url: &str) -> Self {
        Self {
            inner: HttpStrategy::connect(url),
        }
    }

    #[getter]
    fn url(&self) -> String {
        self.inner.url().to_string()
    }

    #[pyo3(signature = (date, step, history, state=None))]
    fn on_step(
        &self,
        py: Python<'_>,
        date: i32,
        step: &Bound<'_, PyAny>,
        history: &Bound<'_, PyAny>,
        state: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let _ = state;
        let step = df_from_py(step)?;
        let history = df_from_py(history)?;
        let value = py
            .detach(|| self.inner.step(&(date, step), &history))
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        output_from_json(py, value)
    }
}

pub(crate) struct StrategyServer {
    strategy: Py<PyAny>,
    state: Py<PyAny>,
}

impl StrategyServer {
    fn new(py: Python<'_>, strategy: Py<PyAny>) -> Self {
        Self {
            strategy,
            state: PyDict::new(py).unbind().into_any(),
        }
    }
}

impl Service for StrategyServer {
    fn call(&self, request: Request) -> Result<Response, ServiceError> {
        let Request::Set { attribute, value } = request else {
            return Err(ServiceError::WriteOnly("strategy serves PUT /on_step".into()));
        };
        if attribute != "on_step" {
            return Err(ServiceError::UnknownAttribute(attribute));
        }
        let args: StepArgs =
            serde_json::from_value(value).map_err(|e| ServiceError::BadValue(e.to_string()))?;
        let ((date, step), history) = args.into_parts();

        Python::attach(|py| -> PyResult<Value> {
            let step = df_to_py(py, &step)?;
            let history = df_to_py(py, &history)?;
            let output = self.strategy.call_method1(
                py,
                "on_step",
                (date, step, history, self.state.bind(py)),
            )?;
            output_to_json(output.bind(py))
        })
        .map(|value| Response { value })
        .map_err(|e| ServiceError::Internal(e.to_string()))
    }

    fn name(&self) -> String {
        "strategy".into()
    }

    fn endpoints(&self) -> Vec<String> {
        vec!["/on_step".into()]
    }
}

#[pyclass(name = "StrategyService", module = "dxcore")]
pub struct PyStrategyService {
    pub(crate) inner: Arc<StrategyServer>,
}

#[pymethods]
impl PyStrategyService {
    #[new]
    fn new(strategy: Py<PyAny>) -> Self {
        Python::attach(|py| Self {
            inner: Arc::new(StrategyServer::new(py, strategy)),
        })
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyHttpStrategy>()?;
    m.add_class::<PyStrategyService>()?;
    Ok(())
}
