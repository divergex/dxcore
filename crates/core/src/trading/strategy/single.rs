use polars::prelude::*;

pub trait Strategy {
    type Input;
    type State: Default;
    type Output: Clone;
    type Frame;

    fn on_step(
        &self,
        step: &Self::Input,
        history: &DataFrame,
        state: &mut Self::State,
    ) -> Self::Output;

    fn create_output(&self) -> Self::Frame;

    fn append_output(&self, frame: &mut Self::Frame, output: Self::Output, step: &Self::Input);
}
