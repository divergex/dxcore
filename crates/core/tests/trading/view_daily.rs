use polars::prelude::*;
use dxlib::trading::{DailyView, View};

use super::helpers;

#[test]
fn groups_by_date() {
    let df = helpers::ohlc_df();
    let view = DailyView::new("date");
    let steps: Vec<(i32, DataFrame)> = view.steps(&df).collect();

    assert_eq!(steps.len(), 3);
    assert_eq!(steps[0].0, 19000);
    assert_eq!(steps[1].0, 19001);
    assert_eq!(steps[2].0, 19002);

    assert_eq!(steps[0].1.height(), 2);
    assert_eq!(steps[1].1.height(), 2);
    assert_eq!(steps[2].1.height(), 1);
}

#[test]
fn on_empty_df_yields_no_steps() {
    let df = helpers::empty_ohlc_df();
    let view = DailyView::new("date");
    let steps: Vec<(i32, DataFrame)> = view.steps(&df).collect();
    assert!(steps.is_empty());
}

#[test]
fn append_accumulates_all_groups() {
    let df = helpers::ohlc_df();
    let view = DailyView::new("date");
    let mut history = DataFrame::empty();

    for step in view.steps(&df) {
        view.append(&mut history, &step);
    }

    assert_eq!(history.height(), 5);
}

#[test]
fn single_date_returns_one_step() {
    let date_col = Column::new(
        "date".into(),
        Series::new("date".into(), &[19000i32])
            .cast(&DataType::Date)
            .unwrap(),
    );
    let value_col = Column::new("value".into(), &[42.0f64]);
    let df = DataFrame::new(vec![date_col, value_col]).unwrap();

    let view = DailyView::new("date");
    let steps: Vec<(i32, DataFrame)> = view.steps(&df).collect();
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].0, 19000);
    assert_eq!(steps[0].1.height(), 1);
}

#[test]
fn step_ord_key_returns_date() {
    let df = helpers::ohlc_df();
    let view = DailyView::new("date");
    let steps: Vec<(i32, DataFrame)> = view.steps(&df).collect();
    assert_eq!(view.step_ord_key(&steps[0]), Some(19000));
    assert_eq!(view.step_ord_key(&steps[1]), Some(19001));
    assert_eq!(view.step_ord_key(&steps[2]), Some(19002));
}
