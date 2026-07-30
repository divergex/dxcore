use polars::prelude::*;

use cadlag::trading::*;


fn ohlc_df() -> DataFrame {
    let date_col = Column::new(
        "date".into(),
        Series::new("date".into(), &[19000i32, 19000, 19001, 19001, 19002])
            .cast(&DataType::Date)
            .unwrap(),
    );
    let symbol_col = Column::new("symbol".into(), &["AAPL", "GOOG", "AAPL", "GOOG", "AAPL"]);
    let open_col = Column::new("open".into(), &[150.0f64, 140.0, 151.0, 141.0, 152.0]);
    let close_col = Column::new("close".into(), &[155.0f64, 145.0, 156.0, 146.0, 153.0]);

    DataFrame::new(vec![date_col, symbol_col, open_col, close_col]).unwrap()
}

fn empty_ohlc_df() -> DataFrame {
    DataFrame::new(vec![
        Column::new_empty("date".into(), &DataType::Date),
        Column::new_empty("symbol".into(), &DataType::String),
        Column::new_empty("open".into(), &DataType::Float64),
        Column::new_empty("close".into(), &DataType::Float64),
    ]).unwrap()
}

#[test]
fn keyed_schema_value_cols_returns_non_key_columns() {
    let ks = KeyedSchema::new(
        vec!["date".into(), "symbol".into()],
        Schema::from_iter([
            Field::new("date".into(), DataType::Date),
            Field::new("symbol".into(), DataType::String),
            Field::new("open".into(), DataType::Float64),
            Field::new("close".into(), DataType::Float64),
        ]),
    );

    let mut vals = ks.value_cols();
    vals.sort();
    assert_eq!(vals, vec!["close", "open"]);
}

#[test]
fn keyed_schema_value_cols_empty_when_all_are_keys() {
    let ks = KeyedSchema::new(
        vec!["a".into(), "b".into()],
        Schema::from_iter([
            Field::new("a".into(), DataType::Int32),
            Field::new("b".into(), DataType::String),
        ]),
    );

    assert!(ks.value_cols().is_empty());
}

#[test]
fn keyed_schema_deref_accesses_schema_methods() {
    let ks = KeyedSchema::new(
        vec!["date".into()],
        Schema::from_iter([Field::new("date".into(), DataType::Date)]),
    );

    assert_eq!(ks.get("date"), Some(&DataType::Date));
    assert_eq!(ks.get("nonexistent"), None);
}

#[test]
fn tick_view_yields_one_row_per_original_row() {
    let df = ohlc_df();
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
fn tick_view_on_empty_df_yields_no_steps() {
    let df = empty_ohlc_df();
    let view = TickView::new("date");
    let steps: Vec<DataFrame> = view.steps(&df).collect();
    assert!(steps.is_empty());
}

#[test]
fn tick_view_append_accumulates_rows() {
    let df = ohlc_df();
    let view = TickView::new("date");
    let mut history = DataFrame::empty();

    for step in view.steps(&df) {
        view.append(&mut history, &step);
    }

    assert_eq!(history.height(), 5);
}

#[test]
fn daily_view_groups_by_date() {
    let df = ohlc_df();
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
fn daily_view_on_empty_df_yields_no_steps() {
    let df = empty_ohlc_df();
    let view = DailyView::new("date");
    let steps: Vec<(i32, DataFrame)> = view.steps(&df).collect();
    assert!(steps.is_empty());
}

#[test]
fn daily_view_append_accumulates_all_groups() {
    let df = ohlc_df();
    let view = DailyView::new("date");
    let mut history = DataFrame::empty();

    for step in view.steps(&df) {
        view.append(&mut history, &step);
    }

    // All 5 rows accumulated (2 + 2 + 1).
    assert_eq!(history.height(), 5);
}

#[test]
fn daily_view_single_date_returns_one_step() {
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
fn panel_view_groups_by_date_and_symbol() {
    let df = ohlc_df();
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
fn panel_view_on_empty_df_yields_no_steps() {
    let df = empty_ohlc_df();
    let view = PanelView::new("date", "symbol");
    let steps: Vec<PanelStep> = view.steps(&df).collect();
    assert!(steps.is_empty());
}

#[test]
fn panel_view_append_accumulates_all_groups() {
    let df = ohlc_df();
    let view = PanelView::new("date", "symbol");
    let mut history = DataFrame::empty();

    for step in view.steps(&df) {
        view.append(&mut history, &step);
    }

    assert_eq!(history.height(), 5);
}

struct CountStrategy;

impl Strategy for CountStrategy {
    type Input = DataFrame;
    type State = u32;
    type Output = u32;
    type Frame = Vec<u32>;

    fn on_step(
        &self,
        _step: &DataFrame,
        _history: &DataFrame,
        state: &mut u32,
    ) -> u32 {
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
fn sync_executor_returns_frame() {
    let df = ohlc_df();
    let mut executor = SyncExecutor::new(CountStrategy);

    let frame = executor.run(&df, TickView::new("date"));

    assert_eq!(frame, vec![1, 2, 3, 4, 5]);
}

#[test]
fn sync_executor_empty_dataframe_yields_empty_frame() {
    let df = empty_ohlc_df();
    let mut executor = SyncExecutor::new(CountStrategy);

    let frame = executor.run(&df, TickView::new("date"));

    assert!(frame.is_empty());
}

#[test]
fn sync_executor_reusable_across_datasets() {
    let df1 = ohlc_df();
    let mut executor = SyncExecutor::new(CountStrategy);

    let frame1 = executor.run(&df1, TickView::new("date"));
    assert_eq!(frame1.len(), 5);

    let df2 = empty_ohlc_df();
    let frame2 = executor.run(&df2, TickView::new("date"));
    assert!(frame2.is_empty());
}
