use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use futures::Stream;
use polars::prelude::*;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyTuple;

use ::dxcore::trading::{AsyncExecutor, SyncExecutor};

use crate::dataframe;
use crate::trading::engine::PyBaseOrderEngine;
use crate::trading::lock;
use crate::trading::orders::PyOrder;
use crate::trading::strategy::PyStrategy;
use crate::trading::view::PyDailyView;

fn take_pending(slot: &Arc<Mutex<Option<PyErr>>>) -> Option<PyErr> {
    lock(slot).take()
}

#[pyclass(name = "Executor", module = "dxcore")]
pub struct PyExecutor {
    engine: Py<PyBaseOrderEngine>,
    strategy: Py<PyAny>,
}

#[pymethods]
impl PyExecutor {
    #[new]
    #[pyo3(signature = (engine, strategy))]
    fn new(engine: Py<PyBaseOrderEngine>, strategy: Py<PyAny>) -> Self {
        Self { engine, strategy }
    }

    fn run(&self, df: Bound<'_, PyAny>, view: &PyDailyView) -> PyResult<Vec<PyOrder>> {
        let py = df.py();
        let df = dataframe::df_from_py(&df)?;
        let engine = {
            let engine = self.engine.bind(py).borrow();
            engine.clear_pending();
            engine.clone_for_run()
        };
        let strategy = PyStrategy::new(self.strategy.clone_ref(py));
        let mut executor = SyncExecutor::new(engine, strategy);
        let frame = executor.run(&df, view.to_core());

        // A stashed Python error (a bogus strategy output, or an exception from
        // `on_step`) wins over `Ok`/`Err` from the run.
        if let Some(err) = executor.engine.take_pending() {
            return Err(err);
        }
        if let Some(err) = executor.strategy.take_pending() {
            return Err(err);
        }

        match frame {
            Ok(orders) => Ok(orders.into_iter().map(PyOrder::from_core).collect()),
            Err(err) => Err(PyValueError::new_err(err.to_string())),
        }
    }
}

#[pyclass(name = "AsyncExecutor", module = "dxcore")]
pub struct PyAsyncExecutor {
    engine: Py<PyBaseOrderEngine>,
    strategy: Py<PyAny>,
}

#[pymethods]
impl PyAsyncExecutor {
    #[new]
    #[pyo3(signature = (engine, strategy))]
    fn new(engine: Py<PyBaseOrderEngine>, strategy: Py<PyAny>) -> Self {
        Self { engine, strategy }
    }

    fn run(&self, stream: Bound<'_, PyAny>, view: &PyDailyView) -> PyResult<PyRunIterator> {
        let py = stream.py();
        let iter = stream.try_iter()?;
        let engine = {
            let engine = self.engine.bind(py).borrow();
            engine.clear_pending();
            engine.clone_for_run()
        };
        let strategy = PyStrategy::new(self.strategy.clone_ref(py));
        let engine_pending = engine.pending_handle();
        let strategy_pending = strategy.pending_handle();
        let src = PyIterStream::new(iter.unbind().into_any(), Arc::clone(&strategy_pending));
        let (tx, rx) = crossbeam_channel::bounded::<Result<Py<PyAny>, PyErr>>(16);
        let view = view.to_core();

        std::thread::spawn(move || {
            let mut executor = AsyncExecutor::new(engine, strategy);
            let mut stream = Box::pin(executor.run(src, view));
            let waker = Waker::noop();
            let mut cx = Context::from_waker(&waker);
            loop {
                match stream.as_mut().poll_next(&mut cx) {
                    Poll::Ready(Some(row)) => {
                        if let Some(err) = take_pending(&engine_pending) {
                            let _ = tx.send(Err(err));
                            break;
                        }
                        if let Some(err) = take_pending(&strategy_pending) {
                            let _ = tx.send(Err(err));
                            break;
                        }
                        match row {
                            Ok(row) => {
                                let order = Python::attach(|py| {
                                    Py::new(py, PyOrder::from_core(row.output))
                                        .map(|order| order.into_any())
                                });
                                match order {
                                    Ok(order) => {
                                        if tx.send(Ok(order)).is_err() {
                                            break; // consumer dropped the iterator
                                        }
                                    }
                                    Err(err) => {
                                        let _ = tx.send(Err(err));
                                        break;
                                    }
                                }
                            }
                            Err(err) => {
                                let _ = tx.send(Err(PyValueError::new_err(err.to_string())));
                                break;
                            }
                        }
                    }
                    Poll::Ready(None) => {
                        if let Some(err) = take_pending(&engine_pending) {
                            let _ = tx.send(Err(err));
                        } else if let Some(err) = take_pending(&strategy_pending) {
                            let _ = tx.send(Err(err));
                        }
                        break;
                    }
                    Poll::Pending => std::thread::yield_now(),
                }
            }
        });

        Ok(PyRunIterator { rx: Some(rx) })
    }
}

#[pyclass(name = "RunIterator", module = "dxcore")]
pub struct PyRunIterator {
    rx: Option<crossbeam_channel::Receiver<Result<Py<PyAny>, PyErr>>>,
}

#[pymethods]
impl PyRunIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let rx = match self.rx.as_ref() {
            Some(rx) => rx,
            None => return Ok(None),
        };
        // Block without the GIL so the producer thread can run.
        match py.detach(move || rx.recv()) {
            Ok(Ok(obj)) => Ok(Some(obj)),
            Ok(Err(err)) => Err(err),
            Err(_) => Ok(None), // producer finished: StopIteration
        }
    }
}

impl Drop for PyRunIterator {
    fn drop(&mut self) {
        // Dropping the receiver makes the producer's sends fail, so the
        // background thread exits promptly if iteration is abandoned.
        self.rx = None;
    }
}

struct PyIterStream {
    iter: Py<PyAny>,
    err: Arc<Mutex<Option<PyErr>>>,
}

impl PyIterStream {
    fn new(iter: Py<PyAny>, err: Arc<Mutex<Option<PyErr>>>) -> Self {
        Self { iter, err }
    }
}

impl Stream for PyIterStream {
    type Item = (i32, DataFrame);

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Python::attach(|py| {
            let item = match self.iter.bind(py).call_method0("__next__") {
                Ok(item) => item,
                Err(err) => {
                    if err.is_instance_of::<pyo3::exceptions::PyStopIteration>(py) {
                        return Poll::Ready(None);
                    }
                    *lock(&self.err) = Some(err);
                    return Poll::Ready(None);
                }
            };
            match item_to_step(&item) {
                Ok(step) => Poll::Ready(Some(step)),
                Err(err) => {
                    *lock(&self.err) = Some(err);
                    Poll::Ready(None)
                }
            }
        })
    }
}

fn item_to_step(item: &Bound<'_, PyAny>) -> PyResult<(i32, DataFrame)> {
    let tup = item
        .cast::<PyTuple>()
        .map_err(|_| PyValueError::new_err("stream items must be (date, DataFrame) tuples"))?;
    if tup.len() != 2 {
        return Err(PyValueError::new_err(
            "stream items must be (date, DataFrame) tuples",
        ));
    }
    let date: i32 = tup.get_item(0)?.extract()?;
    let df = dataframe::df_from_py(&tup.get_item(1)?)?;
    Ok((date, df))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyExecutor>()?;
    m.add_class::<PyAsyncExecutor>()?;
    m.add_class::<PyRunIterator>()?;
    Ok(())
}
