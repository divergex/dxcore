use polars::prelude::*;
use dxlib::trading::{TickView, View};

use super::helpers;

#[test]
fn yields_one_row_per_original_row() {
    let df = helpers::ohlc_df();
    let view = TickView::new("date");
    let steps: Vec<DataFrame> = view.steps(&df).collect();

    assert_eq!(steps.len(), 5);
    for (i, step) in steps.iter().enumerate() {
        assert_eq!(step.height(), 1);
        assert_eq!(
            step.column("symbol").unwrap().str().unwrap().get(0),
            df.column("symbol").unwrap().str().unwrap().get(i),
        );
    }
}

#[test]
fn on_empty_df_yields_no_steps() {
    let df = helpers::empty_ohlc_df();
    let view = TickView::new("date");
    let steps: Vec<DataFrame> = view.steps(&df).collect();
    assert!(steps.is_empty());
}

#[test]
fn append_accumulates_rows() {
    let df = helpers::ohlc_df();
    let view = TickView::new("date");
    let mut history = DataFrame::empty();

    for step in view.steps(&df) {
        view.append(&mut history, &step);
    }

    assert_eq!(history.height(), 5);
}

#[test]
fn step_ord_key_returns_none() {
    let df = helpers::ohlc_df();
    let view = TickView::new("date");
    let step = view.steps(&df).next().unwrap();
    assert_eq!(view.step_ord_key(&step), None);
}
