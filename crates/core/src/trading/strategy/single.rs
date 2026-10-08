use polars::prelude::*;

/// Strategy logic over one stream of steps.
///
/// A strategy decides *what* it wants and nothing else: it declares its output
/// type and [`Strategy::on_step`] returns it. Turning that output into
/// executable orders is the [`OrderEngine`](crate::trading::OrderEngine)'s job, so the
/// same strategy can be run through different engines (backtest, live) without
/// changing a line.
pub trait Strategy {
    type Input;
    type State: Default;
    type Output;

    fn on_step(
        &self,
        step: &Self::Input,
        history: &DataFrame,
        state: &mut Self::State,
    ) -> Self::Output;
}
