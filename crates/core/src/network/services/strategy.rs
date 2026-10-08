use polars::prelude::DataFrame;
use serde::{Deserialize, Serialize};

use super::{ClassService, Request, Response, Service, ServiceError};
use crate::trading::Strategy;

/// Frames cross as [`crate::DataFrame`] the serializable wrapper.
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

    pub(crate) fn into_parts(self) -> ((i32, DataFrame), DataFrame) {
        let (date, frame) = self.step;
        ((date, frame.into_inner()), self.history.into_inner())
    }
}

struct Runner<S: Strategy> {
    strategy: S,
    state: S::State,
}

pub struct StrategyService<S: Strategy> {
    inner: ClassService<Runner<S>>,
}

impl<S> StrategyService<S>
where
    S: Strategy<Input = (i32, DataFrame)> + Send + Sync + 'static,
    S::State: Send + Sync + 'static,
    S::Output: Serialize + Send + Sync + 'static,
{
    pub fn new(name: impl Into<String>, strategy: S) -> Self {
        let runner = Runner {
            strategy,
            state: S::State::default(),
        };
        let inner = ClassService::new(name, runner).with_set(
            "on_step",
            |runner: &mut Runner<S>, args: StepArgs| {
                let (step, history) = args.into_parts();
                Ok(runner
                    .strategy
                    .on_step(&step, &history, &mut runner.state))
            },
        );
        Self { inner }
    }
}

impl<S> Service for StrategyService<S>
where
    S: Strategy<Input = (i32, DataFrame)> + Send + Sync + 'static,
    S::State: Send + Sync + 'static,
    S::Output: Serialize + Send + Sync + 'static,
{
    fn call(&self, request: Request) -> Result<Response, ServiceError> {
        self.inner.call(request)
    }

    fn name(&self) -> String {
        self.inner.name()
    }

    fn endpoints(&self) -> Vec<String> {
        self.inner.endpoints()
    }
}
