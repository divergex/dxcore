use std::sync::{Arc, Mutex};

use polars::prelude::*;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;

use ::dxcore::trading::{
    BaseOrderEngine, BaseOrderEngineState, Order, OrderData, OrderEngine, OrderError, Signal,
};

use crate::trading::lock;
use crate::trading::orders::{PyOrderData, PySignal};
use crate::trading::strategy::PyOut;

/// The stock engine over Python strategy outputs.
#[pyclass(name = "BaseOrderEngine", module = "dxcore")]
pub struct PyBaseOrderEngine {
    inner: BaseOrderEngine,
    symbol: Option<String>,
    pending: Arc<Mutex<Option<PyErr>>>,
}

impl PyBaseOrderEngine {
    /// A clone that talks to the same pending-error slot; used to hand an owned
    /// engine to an executor without giving up the Python object.
    pub(crate) fn clone_for_run(&self) -> Self {
        let inner = match &self.symbol {
            Some(symbol) => BaseOrderEngine::new().with_symbol(symbol.clone()),
            None => BaseOrderEngine::new(),
        };
        Self {
            inner,
            symbol: self.symbol.clone(),
            pending: Arc::clone(&self.pending),
        }
    }

    pub(crate) fn pending_handle(&self) -> Arc<Mutex<Option<PyErr>>> {
        Arc::clone(&self.pending)
    }

    pub(crate) fn take_pending(&self) -> Option<PyErr> {
        lock(&self.pending).take()
    }

    pub(crate) fn clear_pending(&self) {
        *lock(&self.pending) = None;
    }
}

#[pymethods]
impl PyBaseOrderEngine {
    #[new]
    #[pyo3(signature = (symbol = None))]
    fn new(symbol: Option<String>) -> Self {
        let inner = match &symbol {
            Some(symbol) => BaseOrderEngine::new().with_symbol(symbol.clone()),
            None => BaseOrderEngine::new(),
        };
        Self {
            inner,
            symbol,
            pending: Arc::new(Mutex::new(None)),
        }
    }
}

impl OrderEngine<PyOut, (i32, DataFrame)> for PyBaseOrderEngine {
    type Order = Order;
    type Frame = Vec<Order>;
    type State = BaseOrderEngineState;

    fn transform(
        &self,
        output: PyOut,
        step: &(i32, DataFrame),
        date: Option<i64>,
        state: &mut BaseOrderEngineState,
    ) -> Result<Option<Order>, OrderError> {
        if lock(&self.pending).is_some() {
            return Ok(None);
        }

        Python::attach(|py| {
            let obj = output.into_inner();
            let obj = obj.bind(py);
            if obj.is_none() {
                return Ok(None);
            }

            if let Ok(signal) = obj.cast::<PySignal>() {
                let signal = signal.borrow().to_core();
                return <BaseOrderEngine as OrderEngine<Signal, (i32, DataFrame)>>::transform(
                    &self.inner,
                    signal,
                    step,
                    date,
                    state,
                );
            }

            if let Ok(data) = obj.cast::<PyOrderData>() {
                let data = data.borrow().to_core();
                return <BaseOrderEngine as OrderEngine<OrderData, (i32, DataFrame)>>::transform(
                    &self.inner,
                    data,
                    step,
                    date,
                    state,
                );
            }

            let type_name = obj
                .get_type()
                .name()
                .map(|name| name.to_string())
                .unwrap_or_else(|_| "<unknown>".to_string());
            *lock(&self.pending) = Some(PyTypeError::new_err(format!(
                "strategy output must be None, Signal, or OrderData, got {type_name}"
            )));
            Ok(None)
        })
    }

    fn create_output(&self) -> Vec<Order> {
        Vec::new()
    }

    fn append_output(&self, frame: &mut Vec<Order>, order: Order, _step: &(i32, DataFrame)) {
        frame.push(order);
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBaseOrderEngine>()?;
    Ok(())
}
