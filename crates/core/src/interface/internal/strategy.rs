use std::marker::PhantomData;
use std::sync::{Arc, Mutex, MutexGuard};

use polars::prelude::DataFrame;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::Interface;
use crate::trading::{Signal, Strategy};
use crate::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepArgs {
    pub step: (i32, crate::DataFrame),
    pub history: crate::DataFrame,
}

impl StepArgs {
    pub fn new(step: &(i32, DataFrame), history: &DataFrame) -> Self {
        Self {
            step: (step.0, crate::DataFrame::new(step.1.clone())),
            history: crate::DataFrame::new(history.clone()),
        }
    }

    pub fn into_parts(self) -> ((i32, DataFrame), DataFrame) {
        let (date, frame) = self.step;
        ((date, frame.into_inner()), self.history.into_inner())
    }
}

pub struct StrategyInterface<O = Option<Signal>> {
    interface: Arc<Interface>,
    pending: Mutex<Option<Error>>,
    marker: PhantomData<fn() -> O>,
}

impl<O> StrategyInterface<O> {
    pub fn new(interface: Interface) -> Self {
        Self::from_shared(Arc::new(interface))
    }

    pub fn from_shared(interface: Arc<Interface>) -> Self {
        Self {
            interface,
            pending: Mutex::new(None),
            marker: PhantomData,
        }
    }

    pub fn take_error(&self) -> Option<Error> {
        lock(&self.pending).take()
    }

    pub fn step(&self, step: &(i32, DataFrame), history: &DataFrame) -> Result<O, Error>
    where
        O: DeserializeOwned,
    {
        let args = serde_json::to_value(StepArgs::new(step, history))
            .map_err(|e| Error::Interface(e.to_string()))?;
        let value = self.interface.call("on_step", args)?;
        serde_json::from_value(value).map_err(|e| Error::Interface(e.to_string()))
    }

    fn set_pending(&self, error: Error) {
        let mut pending = lock(&self.pending);
        if pending.is_none() {
            *pending = Some(error);
        }
    }
}

impl<O> Strategy for StrategyInterface<O>
where
    O: DeserializeOwned + Default,
{
    type Input = (i32, DataFrame);
    type State = ();
    type Output = O;

    fn on_step(&self, step: &(i32, DataFrame), history: &DataFrame, _state: &mut ()) -> O {
        match self.step(step, history) {
            Ok(output) => output,
            Err(error) => {
                self.set_pending(error);
                O::default()
            }
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
