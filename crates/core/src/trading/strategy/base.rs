use std::collections::HashMap;
use std::hash::Hash;

use polars::prelude::*;

pub trait StrategyBase {
    type Key: Eq + Hash + Clone;
    type Input;
    type State: Default;
    type Output: Clone;
    type Frame;

    fn on_step(
        &self,
        step: &Self::Input,
        key: &Self::Key,
        history: &HashMap<Self::Key, DataFrame>,
        state: &mut Self::State,
    ) -> Self::Output;

    fn create_output(&self) -> Self::Frame;

    fn append_output(&self, frame: &mut Self::Frame, output: Self::Output, step: &Self::Input);
}
