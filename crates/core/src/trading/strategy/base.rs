use std::collections::HashMap;
use std::hash::Hash;

use polars::prelude::*;

/// Multi-key strategy: one step stream per [`StrategyBase::Key`].
///
/// Like [`Strategy`](super::Strategy), the output is the strategy's only
/// decision; executors hand it to an [`OrderEngine`](crate::trading::OrderEngine).
pub trait StrategyBase {
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
