use std::sync::{Arc, Mutex};

use polars::prelude::*;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use ::dxcore::trading::Strategy;

use crate::dataframe;
use crate::trading::lock;

/// Per-run strategy state: an opaque Python object (a fresh dict by default).
pub struct PyState(Py<PyAny>);

impl Default for PyState {
    fn default() -> Self {
        Python::attach(|py| Self(PyDict::new(py).unbind().into_any()))
    }
}

/// A strategy output crossing the boundary untouched.
pub struct PyOut(Py<PyAny>);

impl PyOut {
    pub fn into_inner(self) -> Py<PyAny> {
        self.0
    }
}

pub struct PyStrategy {
    obj: Py<PyAny>,
    pending: Arc<Mutex<Option<PyErr>>>,
}

impl PyStrategy {
    pub fn new(obj: Py<PyAny>) -> Self {
        Self {
            obj,
            pending: Arc::new(Mutex::new(None)),
        }
    }

    pub fn pending_handle(&self) -> Arc<Mutex<Option<PyErr>>> {
        Arc::clone(&self.pending)
    }

    pub fn take_pending(&self) -> Option<PyErr> {
        lock(&self.pending).take()
    }

    fn has_pending(&self) -> bool {
        lock(&self.pending).is_some()
    }

    fn set_pending(&self, err: PyErr) {
        let mut slot = lock(&self.pending);
        if slot.is_none() {
            *slot = Some(err);
        }
    }
}

impl Strategy for PyStrategy {
    type Input = (i32, DataFrame);
    type State = PyState;
    type Output = PyOut;

    fn on_step(
        &self,
        (date, step_df): &(i32, DataFrame),
        history: &DataFrame,
        state: &mut PyState,
    ) -> PyOut {
        Python::attach(|py| {
            if self.has_pending() {
                return PyOut(py.None());
            }
            let result = (|| -> PyResult<PyOut> {
                let step_py = dataframe::df_to_py(py, step_df)?;
                let hist_py = dataframe::df_to_py(py, history)?;
                let out = self
                    .obj
                    .call_method1(py, "on_step", (*date, step_py, hist_py, state.0.bind(py)))?;
                Ok(PyOut(out))
            })();
            match result {
                Ok(out) => out,
                Err(err) => {
                    self.set_pending(err);
                    PyOut(py.None())
                }
            }
        })
    }
}
