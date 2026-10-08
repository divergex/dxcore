use std::collections::HashMap;
use std::hash::Hash;

use polars::prelude::*;

use super::base::StrategyBase;

/// Multi-key strategy that keeps a typed key per source stream.
///
/// [`StrategyBase`] is blanket-implemented for every `StreamedStrategy`, so
/// implementing this trait is enough to run through
/// [`SyncExecutor::run_multi`](crate::trading::SyncExecutor::run_multi) or
/// [`AsyncExecutor::run_multi`](crate::trading::AsyncExecutor::run_multi).
pub trait StreamedStrategy {
    type Key: Eq + Hash + Clone;
    type Input;
    type State: Default;
    type Output;

    fn on_step(
        &self,
        step: &Self::Input,
        key: &Self::Key,
        history: &HashMap<Self::Key, DataFrame>,
        state: &mut Self::State,
    ) -> Self::Output;
}

impl<T: StreamedStrategy> StrategyBase for T {
    type Key = T::Key;
    type Input = T::Input;
    type State = T::State;
    type Output = T::Output;

    fn on_step(
        &self,
        step: &Self::Input,
        key: &Self::Key,
        history: &HashMap<Self::Key, DataFrame>,
        state: &mut Self::State,
    ) -> Self::Output {
        T::on_step(self, step, key, history, state)
    }
}
