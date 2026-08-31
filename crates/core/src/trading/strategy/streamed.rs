use std::collections::HashMap;
use std::hash::Hash;

use polars::prelude::*;

use super::base::StrategyBase;

pub trait StreamedStrategy {
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

impl<T: StreamedStrategy> StrategyBase for T {
    type Key = T::Key;
    type Input = T::Input;
    type State = T::State;
    type Output = T::Output;
    type Frame = T::Frame;

    fn on_step(
        &self,
        step: &Self::Input,
        key: &Self::Key,
        history: &HashMap<Self::Key, DataFrame>,
        state: &mut Self::State,
    ) -> Self::Output {
        T::on_step(self, step, key, history, state)
    }

    fn create_output(&self) -> Self::Frame {
        T::create_output(self)
    }

    fn append_output(
        &self,
        frame: &mut Self::Frame,
        output: Self::Output,
        step: &Self::Input,
    ) {
        T::append_output(self, frame, output, step)
    }
}
