use std::ops::Deref;
use polars::prelude::*;

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

/// A Polars Schema extended with the notion of key columns.
/// Non-key columns are implicitly value columns (schema fields - keys).
/// Implements Deref<Target = Schema> so all Schema methods are available
/// directly on KeyedSchema without going through .schema.
pub struct KeyedSchema {
    pub keys: Vec<String>,
    schema: Schema,
}

impl KeyedSchema {
    pub fn new(keys: Vec<String>, schema: Schema) -> Self {
        Self { keys, schema }
    }

    pub fn value_cols(&self) -> Vec<String> {
        self.schema
            .iter_fields()
            .map(|f| f.name().to_string())
            .filter(|name| !self.keys.contains(name))
            .collect()
    }
}

impl Deref for KeyedSchema {
    type Target = Schema;

    fn deref(&self) -> &Schema {
        &self.schema
    }
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

/// A View defines how a DataFrame is sliced into steps for the execution loop.
/// Each concrete View declares the type of item it yields via the associated
/// type Item. The Strategy declares what Item it expects as Input. The Executor
/// enforces at compile time that View::Item == Strategy::Input.
pub trait View {
    type Item;

    /// Yield an iterator of steps over the DataFrame.
    fn steps(&self, df: &DataFrame) -> impl Iterator<Item = Self::Item>;

    /// Accumulate a step into the history DataFrame.
    fn append(&self, history: &mut DataFrame, step: &Self::Item);
}

// ---------------------------------------------------------------------------
// TickView
// ---------------------------------------------------------------------------

/// Yields one row per unique timestamp as a single-row DataFrame.
pub struct TickView {
    pub timestamp_col: String,
}

impl TickView {
    pub fn new(timestamp_col: impl Into<String>) -> Self {
        Self {
            timestamp_col: timestamp_col.into(),
        }
    }
}

impl View for TickView {
    type Item = DataFrame;

    fn steps(&self, df: &DataFrame) -> impl Iterator<Item = DataFrame> {
        let df = df.clone();
        let n = df.height();
        (0..n).map(move |i| df.slice(i as i64, 1))
    }

    fn append(&self, history: &mut DataFrame, step: &DataFrame) {
        if history.width() == 0 {
            *history = step.clone();
        } else {
            history.vstack_mut(step).expect("failed to append tick step to history");
        }
    }
}

// ---------------------------------------------------------------------------
// DailyView
// ---------------------------------------------------------------------------

/// Yields one (date_value, DataFrame) per unique date, where the DataFrame
/// contains all rows belonging to that date.
pub struct DailyView {
    pub date_col: String,
}

impl DailyView {
    pub fn new(date_col: impl Into<String>) -> Self {
        Self {
            date_col: date_col.into(),
        }
    }
}

impl View for DailyView {
    type Item = (i32, DataFrame);

    fn steps(&self, df: &DataFrame) -> impl Iterator<Item = (i32, DataFrame)> {
        let date_series = df.column(&self.date_col).unwrap().clone();
        let dates: Vec<i32> = date_series
            .date()
            .unwrap()
            .into_iter()
            .flatten()
            .collect();

        let mut unique_dates = dates.clone();
        unique_dates.sort_unstable();
        unique_dates.dedup();

        let df = df.clone();
        let _date_col = self.date_col.clone();

        unique_dates.into_iter().map(move |date| {
            let mask = date_series.date().unwrap().equal(date);
            let chunk = df.filter(&mask).unwrap();
            (date, chunk)
        })
    }

    fn append(&self, history: &mut DataFrame, step: &(i32, DataFrame)) {
        if history.width() == 0 {
            *history = step.1.clone();
        } else {
            history
                .vstack_mut(&step.1)
                .expect("failed to append daily step to history");
        }
    }
}

// ---------------------------------------------------------------------------
// PanelView
// ---------------------------------------------------------------------------

/// One step contains the key values and a DataFrame slice for a (date, symbol) pair.
pub struct PanelStep {
    pub date: i32,
    pub symbol: String,
    pub data: DataFrame,
}

/// Yields one PanelStep per unique (date, symbol) pair.
pub struct PanelView {
    pub date_col: String,
    pub symbol_col: String,
}

impl PanelView {
    pub fn new(date_col: impl Into<String>, symbol_col: impl Into<String>) -> Self {
        Self {
            date_col: date_col.into(),
            symbol_col: symbol_col.into(),
        }
    }
}

impl View for PanelView {
    type Item = PanelStep;

    fn steps(&self, df: &DataFrame) -> impl Iterator<Item = PanelStep> {
        let df = df.clone();
        let date_col = self.date_col.clone();
        let symbol_col = self.symbol_col.clone();

        let pairs: Vec<(i32, String)> = {
            let dates = df.column(&date_col).unwrap().date().unwrap().clone();
            let symbols = df.column(&symbol_col).unwrap().str().unwrap().clone();
            let mut pairs: Vec<(i32, String)> = dates
                .into_iter()
                .zip(symbols.into_iter())
                .filter_map(|(d, s)| Some((d?, s?.to_string())))
                .collect();
            pairs.sort_unstable();
            pairs.dedup();
            pairs
        };

        pairs.into_iter().map(move |(date, symbol)| {
            let date_series = df.column(&date_col).unwrap();
            let symbol_series = df.column(&symbol_col).unwrap();
            let date_mask = date_series.date().unwrap().equal(date);
            let symbol_strs = symbol_series.str().unwrap();
            let symbol_values: Vec<bool> = symbol_strs
                .into_iter()
                .map(|s| s == Some(symbol.as_ref()))
                .collect();
            let symbol_mask = BooleanChunked::from_slice("".into(), &symbol_values);
            let mask = date_mask & symbol_mask;
            let data = df.filter(&mask).unwrap();
            PanelStep { date, symbol, data }
        })
    }

    fn append(&self, history: &mut DataFrame, step: &PanelStep) {
        if history.width() == 0 {
            *history = step.data.clone();
        } else {
            history
                .vstack_mut(&step.data)
                .expect("failed to append panel step to history");
        }
    }
}

// ---------------------------------------------------------------------------
// Strategy
// ---------------------------------------------------------------------------

/// A Strategy processes one step at a time, with access to the accumulated
/// history and mutable state. It has no knowledge of iteration or data
/// sourcing.
pub trait Strategy {
    /// The type of input expected from a View (must match View::Item).
    type Input;
    /// Mutable state carried across steps (receive a default before step 0).
    type State: Default;
    /// The value produced per step, collected into a Vec by the Executor.
    type Output;

    /// Process one step. Receives the current step payload, the history
    /// accumulated so far (before this step is appended), and mutable state.
    fn on_step(&self, step: &Self::Input, history: &DataFrame, state: &mut Self::State) -> Self::Output;
}

// ---------------------------------------------------------------------------
// SyncExecutor
// ---------------------------------------------------------------------------

/// Batch executor: runs a strategy over a DataFrame sliced by a View.
/// Returns the accumulated history DataFrame. State resets each call.
pub struct SyncExecutor<S: Strategy> {
    pub strategy: S,
}

impl<S: Strategy> SyncExecutor<S> {
    pub fn new(strategy: S) -> Self {
        Self { strategy }
    }

    pub fn run<V: View<Item = S::Input>>(&mut self, df: &DataFrame, view: V) -> DataFrame {
        let mut history = DataFrame::empty();
        let mut state = S::State::default();

        for step in view.steps(df) {
            self.strategy.on_step(&step, &history, &mut state);
            view.append(&mut history, &step);
        }

        history
    }
}

// ---------------------------------------------------------------------------
// OutputRow
// ---------------------------------------------------------------------------

/// A single output produced by the [`AsyncExecutor`] for each stream item.
pub struct OutputRow<O> {
    pub output: O,
}

// ---------------------------------------------------------------------------
// AsyncExecutor
// ---------------------------------------------------------------------------

/// Streaming executor: processes items from a [`futures::Stream`], appending
/// each to history via the given View, and yielding an [`OutputRow`] per item.
pub struct AsyncExecutor<S: Strategy> {
    pub strategy: S,
}

impl<S: Strategy> AsyncExecutor<S> {
    pub fn new(strategy: S) -> Self {
        Self { strategy }
    }

    pub fn run<V, St>(
        &mut self,
        stream: St,
        view: V,
    ) -> impl futures::Stream<Item = OutputRow<S::Output>> + '_
    where
        V: View<Item = S::Input> + 'static,
        St: futures::Stream<Item = S::Input> + Unpin + 'static,
        S::Input: 'static,
    {
        use std::pin::Pin;
        use std::task::{Context, Poll};
        use futures::Stream;

        struct RunStream<'a, S2: Strategy, V2: View<Item = S2::Input>, St2: Stream<Item = S2::Input> + Unpin> {
            strategy: &'a mut S2,
            view: V2,
            stream: St2,
            history: DataFrame,
            state: S2::State,
        }

        impl<'a, S2, V2, St2> Stream for RunStream<'a, S2, V2, St2>
        where
            S2: Strategy,
            V2: View<Item = S2::Input>,
            St2: Stream<Item = S2::Input> + Unpin,
        {
            type Item = OutputRow<S2::Output>;

            fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
                // SAFETY: stream is Unpin, and field accesses are disjoint (strategy,
                // view, history, state are never pinned).
                let this = unsafe { self.get_unchecked_mut() };
                match Pin::new(&mut this.stream).poll_next(cx) {
                    Poll::Ready(Some(step)) => {
                        let output = this.strategy.on_step(&step, &this.history, &mut this.state);
                        this.view.append(&mut this.history, &step);
                        Poll::Ready(Some(OutputRow { output }))
                    }
                    Poll::Ready(None) => Poll::Ready(None),
                    Poll::Pending => Poll::Pending,
                }
            }
        }

        RunStream {
            strategy: &mut self.strategy,
            view,
            stream,
            history: DataFrame::empty(),
            state: S::State::default(),
        }
    }
}

#[cfg(test)]
mod tests;