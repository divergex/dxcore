use std::sync::{Arc, Mutex};

use polars::prelude::*;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use ::dxcore::trading::Strategy;

use crate::dataframe;

pub struct PyState(Py<PyAny>);

impl Default for PyState {
    fn default() -> Self {
        Python::attach(|py| Self(PyDict::new(py).unbind().into_any()))
    }
}

pub struct PyOut(Py<PyAny>);

impl Clone for PyOut {
    fn clone(&self) -> Self {
        Python::attach(|py| PyOut(self.0.clone_ref(py)))
    }
}

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
        self.pending.lock().unwrap().take()
    }

    fn has_pending(&self) -> bool {
        self.pending.lock().unwrap().is_some()
    }

    fn set_pending(&self, err: PyErr) {
        let mut slot = self.pending.lock().unwrap();
        if slot.is_none() {
            *slot = Some(err);
        }
    }
}

impl Strategy for PyStrategy {
    type Input = (i32, DataFrame);
    type State = PyState;
    type Output = PyOut;
    type Frame = Py<PyAny>;

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

    fn create_output(&self) -> Py<PyAny> {
        Python::attach(|py| {
            if self.has_pending() {
                return py.None();
            }
            let result = (|| -> PyResult<Py<PyAny>> {
                let out = self.obj.call_method0(py, "create_output")?;
                if out.is_none(py) {
                    dataframe::empty_df_py(py)
                } else {
                    Ok(out)
                }
            })();
            match result {
                Ok(out) => out,
                Err(err) => {
                    self.set_pending(err);
                    py.None()
                }
            }
        })
    }

    fn append_output(
        &self,
        frame: &mut Py<PyAny>,
        output: PyOut,
        (date, step_df): &(i32, DataFrame),
    ) {
        Python::attach(|py| {
            if self.has_pending() {
                return;
            }
            let result = (|| -> PyResult<()> {
                let step_py = dataframe::df_to_py(py, step_df)?;
                let new_frame = self.obj.call_method1(
                    py,
                    "append_output",
                    (frame.bind(py), output.0.bind(py), *date, step_py),
                )?;
                if !new_frame.is_none(py) {
                    *frame = new_frame;
                }
                Ok(())
            })();
            if let Err(err) = result {
                self.set_pending(err);
            }
        })
    }
}
