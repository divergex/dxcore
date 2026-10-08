mod engine;
mod executor;
mod orders;
mod strategy;
mod view;

use std::sync::{Mutex, MutexGuard};

use pyo3::prelude::*;

pub(crate) use orders::{output_from_json, PyOrderData, PySignal};

/// Lock a mutex, recovering the guard if a previous holder panicked.
///
/// The pending-error slots are shared between an executor and its worker
/// thread; a poisoned lock must never turn a Python error into a panic.
pub(crate) fn lock<'a, T>(mutex: &'a Mutex<T>) -> MutexGuard<'a, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    view::register(m)?;
    orders::register(m)?;
    engine::register(m)?;
    executor::register(m)?;
    Ok(())
}
