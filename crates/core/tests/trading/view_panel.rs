use polars::prelude::*;
use dxcore::trading::{PanelStep, PanelView, View};

use super::helpers;

#[test]
fn groups_by_date_and_symbol() {
    let df = helpers::ohlc_df();
    let view = PanelView::new("date", "symbol");
    let steps: Vec<PanelStep> = view.steps(&df).collect();

    assert_eq!(steps.len(), 5);

    for step in &steps {
        assert_eq!(
            step.data.height(),
            1,
            "step ({}, {}) expected 1 row",
            step.date,
            step.symbol,
        );
    }

    let step = steps
        .iter()
        .find(|s| s.date == 19000 && s.symbol == "AAPL")
        .unwrap();
    assert_eq!(
        step.data.column("open").unwrap().f64().unwrap().get(0),
        Some(150.0),
    );
}

#[test]
fn on_empty_df_yields_no_steps() {
    let df = helpers::empty_ohlc_df();
    let view = PanelView::new("date", "symbol");
    let steps: Vec<PanelStep> = view.steps(&df).collect();
    assert!(steps.is_empty());
}

#[test]
fn append_accumulates_all_groups() {
    let df = helpers::ohlc_df();
    let view = PanelView::new("date", "symbol");
    let mut history = DataFrame::empty();

    for step in view.steps(&df) {
        view.append(&mut history, &step);
    }

    assert_eq!(history.height(), 5);
}

#[test]
fn step_ord_key_returns_date() {
    let df = helpers::ohlc_df();
    let view = PanelView::new("date", "symbol");
    let steps: Vec<PanelStep> = view.steps(&df).collect();
    let by_date: Vec<i64> = steps.iter().map(|s| view.step_ord_key(s).unwrap()).collect();
    assert_eq!(by_date, vec![19000, 19000, 19001, 19001, 19002]);
}
