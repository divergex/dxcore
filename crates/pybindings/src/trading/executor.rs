//! Bindings for the trading executor: run a Python strategy over a
//! `polars.DataFrame` (backtest) or over a stream of daily steps (live).
//!
//! # Python strategy protocol
//!
//! A strategy is any object implementing three methods:
//!
//! ```python
//! class MyStrategy:
//!     def create_output(self) -> pl.DataFrame:
//!         """Fresh (possibly empty) output frame the executor accumulates into."""
//!         ...
//!
//!     def on_step(self, date: int, step: pl.DataFrame,
//!                 history: pl.DataFrame, state: dict) -> Any:
//!         """One daily step, in date order.
//!
//!         `step` is that day's rows (column map applied), `history` is every
//!         prior day appended so far, `state` is a fresh dict per run.
//!         """
//!         ...
//!
//!     def append_output(self, frame: pl.DataFrame, output: Any,
//!                       date: int, step: pl.DataFrame) -> pl.DataFrame | None:
//!         """Accumulate `output` into `frame`.
//!
//!         Return the new frame, or mutate `frame` in place and return None.
//!         """
//!         ...
//! ```
//!
//! If any method raises, the run aborts and the original exception is
//! re-raised on the caller's thread.

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
use crate::trading::strategy::PyStrategy;
use crate::trading::view::PyDailyView;

#[pyclass(name = "Executor", module = "dxcore")]
pub struct PyExecutor {
    strategy: Py<PyAny>,
}

#[pymethods]
impl PyExecutor {
    #[new]
    #[pyo3(signature = (strategy))]
    fn new(strategy: Py<PyAny>) -> Self {
        Self { strategy }
    }

    fn run(&mut self, df: Bound<'_, PyAny>, view: &PyDailyView) -> PyResult<Py<PyAny>> {
        let py = df.py();
        let df = dataframe::df_from_py(&df)?;
        let adapter = PyStrategy::new(self.strategy.clone_ref(py));
        let mut executor = SyncExecutor::new(adapter);
        let frame = executor.run(&df, view.to_core());
        match executor.strategy.take_pending() {
            Some(err) => Err(err),
            None => Ok(frame),
        }
    }
}

#[pyclass(name = "AsyncExecutor", module = "dxcore")]
pub struct PyAsyncExecutor {
    strategy: Py<PyAny>,
}

#[pymethods]
impl PyAsyncExecutor {
    #[new]
    #[pyo3(signature = (strategy))]
    fn new(strategy: Py<PyAny>) -> Self {
        Self { strategy }
    }

    fn run(&mut self, stream: Bound<'_, PyAny>, view: &PyDailyView) -> PyResult<PyRunIterator> {
        let py = stream.py();
        let iter = stream.try_iter()?;
        let adapter = PyStrategy::new(self.strategy.clone_ref(py));
        let err_slot = adapter.pending_handle();
        let src = PyIterStream::new(iter.unbind().into_any(), err_slot.clone());
        let (tx, rx) = crossbeam_channel::bounded::<Result<Py<PyAny>, PyErr>>(16);
        let view = view.to_core();

        std::thread::spawn(move || {
            let mut executor = AsyncExecutor::new(adapter);
            let mut stream = Box::pin(executor.run(src, view));
            let waker = Waker::noop();
            let mut cx = Context::from_waker(&waker);
            loop {
                match stream.as_mut().poll_next(&mut cx) {
                    Poll::Ready(Some(row)) => {
                        if let Some(err) = err_slot.lock().unwrap().take() {
                            let _ = tx.send(Err(err));
                            break;
                        }
                        if tx.send(Ok(row.output.into_inner())).is_err() {
                            break; // consumer dropped the iterator
                        }
                    }
                    Poll::Ready(None) => {
                        if let Some(err) = err_slot.lock().unwrap().take() {
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
                    *self.err.lock().unwrap() = Some(err);
                    return Poll::Ready(None);
                }
            };
            match item_to_step(&item) {
                Ok(step) => Poll::Ready(Some(step)),
                Err(err) => {
                    *self.err.lock().unwrap() = Some(err);
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
