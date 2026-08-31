use polars::prelude::*;
use futures::StreamExt;
use dxlib::trading::{AsyncExecutor, Strategy, TickView, View};

use super::helpers;

struct CountStrategy;

impl Strategy for CountStrategy {
    type Input = DataFrame;
    type State = u32;
    type Output = u32;
    type Frame = Vec<u32>;

    fn on_step(&self, _step: &DataFrame, _history: &DataFrame, state: &mut u32) -> u32 {
        *state += 1;
        *state
    }

    fn create_output(&self) -> Vec<u32> {
        Vec::new()
    }

    fn append_output(&self, frame: &mut Vec<u32>, output: u32, _step: &DataFrame) {
        frame.push(output);
    }
}

#[test]
fn yields_output_rows() {
    let df = helpers::ohlc_df();
    let view = TickView::new("date");

    let steps: Vec<DataFrame> = view.steps(&df).collect();
    let stream = futures::stream::iter(steps);

    let mut executor = AsyncExecutor::new(CountStrategy);
    let mut output_stream = executor.run(stream, view);

    let mut outputs = Vec::new();
    while let Some(row) = futures::executor::block_on_stream(&mut output_stream).next() {
        outputs.push(row.output);
    }

    assert_eq!(outputs, vec![1, 2, 3, 4, 5]);
}

#[test]
fn empty_stream_yields_nothing() {
    let df = helpers::empty_ohlc_df();
    let view = TickView::new("date");

    let steps: Vec<DataFrame> = view.steps(&df).collect();
    let stream = futures::stream::iter(steps);

    let mut executor = AsyncExecutor::new(CountStrategy);
    let mut output_stream = executor.run(stream, view);

    let mut outputs = Vec::new();
    while let Some(row) = futures::executor::block_on_stream(&mut output_stream).next() {
        outputs.push(row.output);
    }

    assert!(outputs.is_empty());
}
