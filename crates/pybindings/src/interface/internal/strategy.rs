use std::sync::Arc;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use serde_json::Value;

use ::dxcore::interface::internal::StrategyInterface;

use crate::dataframe::df_from_py;
use crate::trading::output_from_json;

use super::factory::PyInterface;

#[pyclass(name = "StrategyInterface", module = "dxcore")]
pub struct PyStrategyInterface {
    inner: StrategyInterface<Value>,
}

#[pymethods]
impl PyStrategyInterface {
    #[new]
    fn new(interface: PyRef<'_, PyInterface>) -> Self {
        let interface = &*interface;
        Self {
            inner: StrategyInterface::from_shared(Arc::clone(&interface.inner)),
        }
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
        // Held GIL would deadlock: the server thread needs it to run the
        // served Python code while this thread waits on the socket.
        let value = py
            .detach(|| self.inner.step(&(date, step), &history))
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        output_from_json(py, value)
    }

    fn take_error(&self) -> Option<String> {
        self.inner.take_error().map(|err| err.to_string())
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyStrategyInterface>()?;
    Ok(())
}
