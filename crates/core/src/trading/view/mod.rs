use polars::prelude::*;

mod daily;
mod panel;
mod tick;

pub use daily::DailyView;
pub use panel::{PanelStep, PanelView};
pub use tick::TickView;

/// A View defines how a DataFrame is sliced into steps for the execution loop.
/// It is responsible for translating source-column names into the strategy's expected column names.
pub trait View {
    type Item;

    /// Each step has had `col_map` applied, so the strategy sees only its canonical columns.
    fn steps(&self, df: &DataFrame) -> impl Iterator<Item = Self::Item>;

    /// Market data for one step, after `col_map` has been applied.
    fn frame<'a>(&self, step: &'a Self::Item) -> &'a DataFrame;

    fn append(&self, history: &mut DataFrame, step: &Self::Item);

    fn step_ord_key(&self, _step: &Self::Item) -> Option<i64> {
        None
    }

    /// Timestamp of the market snapshot represented by `step`.
    ///
    /// Defaults to [`step_ord_key`](Self::step_ord_key); views whose ordering
    /// key is not a timestamp (such as [`TickView`]) can override it.
    fn step_timestamp(&self, step: &Self::Item) -> Option<i64> {
        self.step_ord_key(step)
    }
}
