use polars::prelude::*;
use dxcore::trading::{AsyncExecutor, OrderEngine, OrderError, Strategy, TickView, View};

use super::helpers;

struct CountStrategy;

impl Strategy for CountStrategy {
    type Input = DataFrame;
    type State = u32;
    type Output = u32;

    fn on_step(&self, _step: &DataFrame, _history: &DataFrame, state: &mut u32) -> u32 {
        *state += 1;
        *state
    }
}

/// Every output becomes one order.
struct PassThrough;

impl<I> OrderEngine<u32, I> for PassThrough {
    type Order = u32;
    type Frame = Vec<u32>;
    type State = ();

    fn transform(
        &self,
        output: u32,
        _step: &I,
        _date: Option<i64>,
        _state: &mut (),
    ) -> Result<Option<u32>, OrderError> {
        Ok(Some(output))
    }

    fn create_output(&self) -> Vec<u32> {
        Vec::new()
    }

    fn append_output(&self, frame: &mut Vec<u32>, order: u32, _step: &I) {
        frame.push(order);
    }
}

/// Drops outputs it has no order for.
struct EvensOnly;

impl<I> OrderEngine<u32, I> for EvensOnly {
    type Order = u32;
    type Frame = Vec<u32>;
    type State = ();

    fn transform(
        &self,
        output: u32,
        _step: &I,
        _date: Option<i64>,
        _state: &mut (),
    ) -> Result<Option<u32>, OrderError> {
        Ok((output % 2 == 0).then_some(output))
    }

    fn create_output(&self) -> Vec<u32> {
        Vec::new()
    }

    fn append_output(&self, frame: &mut Vec<u32>, order: u32, _step: &I) {
        frame.push(order);
    }
}

fn collect<E: OrderEngine<u32, DataFrame, Order = u32>>(
    engine: E,
    df: &DataFrame,
) -> Vec<Result<u32, OrderError>>
where
    E::State: Unpin,
{
    let view = TickView::new("date");
    let steps: Vec<DataFrame> = view.steps(df).collect();

    let mut executor = AsyncExecutor::new(engine, CountStrategy);
    let mut output_stream = executor.run(futures::stream::iter(steps), view);

    let mut rows = Vec::new();
    while let Some(row) = futures::executor::block_on_stream(&mut output_stream).next() {
        rows.push(row.map(|row| row.output));
    }
    rows
}

#[test]
fn yields_output_rows() {
    let outputs: Vec<u32> = collect(PassThrough, &helpers::ohlc_df())
        .into_iter()
        .map(|row| row.unwrap())
        .collect();

    assert_eq!(outputs, vec![1, 2, 3, 4, 5]);
}

#[test]
fn steps_without_orders_yield_nothing() {
    let outputs: Vec<u32> = collect(EvensOnly, &helpers::ohlc_df())
        .into_iter()
        .map(|row| row.unwrap())
        .collect();

    assert_eq!(outputs, vec![2, 4]);
}

#[test]
fn empty_stream_yields_nothing() {
    assert!(collect(PassThrough, &helpers::empty_ohlc_df()).is_empty());
}
