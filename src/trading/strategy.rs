use polars::prelude::*;

/// A Strategy processes one step at a time with access to accumulated history and mutable state.  
pub trait Strategy {
    /// The type of input expected from a View (must match View::Item).
    type Input;
    /// Mutable state carried across steps (initialised via Default each run).
    type State: Default;
    /// The per-step value produced by [`on_step`](Strategy::on_step).
    type Output: Clone;
    /// The type that accumulates per-step outputs.
    type Frame;

    fn on_step(
        &self,
        step: &Self::Input,
        history: &DataFrame,
        state: &mut Self::State,
    ) -> Self::Output;

    fn create_output(&self) -> Self::Frame;

    /// Append a per-step output to the output frame, given the step that produced it (so the strategy can inspect the observation for context, e.g. extract a date).
    fn append_output(
        &self,
        frame: &mut Self::Frame,
        output: Self::Output,
        step: &Self::Input,
    );
}
