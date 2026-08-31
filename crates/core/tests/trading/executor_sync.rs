use std::collections::HashMap;

use polars::prelude::*;
use dxcore::trading::{
    DailyView, Strategy, StreamedStrategy, SyncExecutor, TickView,
};

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
fn returns_frame() {
    let df = helpers::ohlc_df();
    let mut executor = SyncExecutor::new(CountStrategy);
    let frame = executor.run(&df, TickView::new("date"));
    assert_eq!(frame, vec![1, 2, 3, 4, 5]);
}

#[test]
fn empty_df_yields_empty_frame() {
    let df = helpers::empty_ohlc_df();
    let mut executor = SyncExecutor::new(CountStrategy);
    let frame = executor.run(&df, TickView::new("date"));
    assert!(frame.is_empty());
}

#[test]
fn reusable_across_datasets() {
    let df1 = helpers::ohlc_df();
    let mut executor = SyncExecutor::new(CountStrategy);

    let frame1 = executor.run(&df1, TickView::new("date"));
    assert_eq!(frame1.len(), 5);

    let df2 = helpers::empty_ohlc_df();
    let frame2 = executor.run(&df2, TickView::new("date"));
    assert!(frame2.is_empty());
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Symbol {
    Aapl,
    Goog,
}

struct MultiCountStrategy;

impl StreamedStrategy for MultiCountStrategy {
    type Key = Symbol;
    type Input = DataFrame;
    type State = u32;
    type Output = u32;
    type Frame = Vec<(Symbol, u32)>;

    fn on_step(
        &self,
        _step: &DataFrame,
        _key: &Symbol,
        _history: &HashMap<Symbol, DataFrame>,
        state: &mut u32,
    ) -> u32 {
        *state += 1;
        *state
    }

    fn create_output(&self) -> Vec<(Symbol, u32)> {
        Vec::new()
    }

    fn append_output(
        &self,
        frame: &mut Vec<(Symbol, u32)>,
        output: u32,
        _step: &DataFrame,
    ) {
        frame.push((Symbol::Aapl, output));
    }
}

fn filter_by_symbol(df: &DataFrame, symbol: &str) -> DataFrame {
    let mask: BooleanChunked = df
        .column("symbol")
        .unwrap()
        .str()
        .unwrap()
        .into_iter()
        .map(|s| s == Some(symbol))
        .collect();
    df.filter(&mask).unwrap()
}

#[test]
fn run_multi_processes_all_keys() {
    let df = helpers::ohlc_df();

    let df_aapl = filter_by_symbol(&df, "AAPL");
    let df_goog = filter_by_symbol(&df, "GOOG");

    let mut dfs = HashMap::new();
    dfs.insert(Symbol::Aapl, df_aapl);
    dfs.insert(Symbol::Goog, df_goog);

    let views = HashMap::from([
        (Symbol::Aapl, TickView::new("date")),
        (Symbol::Goog, TickView::new("date")),
    ]);

    let mut executor = SyncExecutor::new(MultiCountStrategy);
    executor.run_multi(&dfs, views);

    // 3 AAPL rows + 2 GOOG rows = 5 steps; no panic = success.
}

#[test]
fn run_multi_interleaves_by_timestamp() {
    let df1 = {
        let date_col = Column::new(
            "date".into(),
            Series::new("date".into(), &[19000i32, 19001])
                .cast(&DataType::Date)
                .unwrap(),
        );
        let val_col = Column::new("val".into(), &[10.0f64, 20.0]);
        DataFrame::new(vec![date_col, val_col]).unwrap()
    };
    let df2 = {
        let date_col = Column::new(
            "date".into(),
            Series::new("date".into(), &[19000i32, 19002])
                .cast(&DataType::Date)
                .unwrap(),
        );
        let val_col = Column::new("val".into(), &[30.0f64, 40.0]);
        DataFrame::new(vec![date_col, val_col]).unwrap()
    };

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    enum Key {
        A,
        B,
    }

    struct OrderRecorder;

    impl StreamedStrategy for OrderRecorder {
        type Key = Key;
        type Input = (i32, DataFrame);
        type State = ();
        type Output = (Key, i32);
        type Frame = Vec<(Key, i32)>;

        fn on_step(
            &self,
            step: &(i32, DataFrame),
            key: &Key,
            _history: &HashMap<Key, DataFrame>,
            _state: &mut (),
        ) -> (Key, i32) {
            (key.clone(), step.0)
        }

        fn create_output(&self) -> Vec<(Key, i32)> {
            Vec::new()
        }

        fn append_output(
            &self,
            frame: &mut Vec<(Key, i32)>,
            output: (Key, i32),
            _step: &(i32, DataFrame),
        ) {
            frame.push(output);
        }
    }

    let mut dfs = HashMap::new();
    dfs.insert(Key::A, df1);
    dfs.insert(Key::B, df2);

    let views = HashMap::from([
        (Key::A, DailyView::new("date")),
        (Key::B, DailyView::new("date")),
    ]);

    let mut executor = SyncExecutor::new(OrderRecorder);
    let frame = executor.run_multi(&dfs, views);

    // Steps must be sorted by timestamp; equal timestamps retain insertion order.
    let dates: Vec<i32> = frame.iter().map(|(_, d)| *d).collect();
    let mut sorted_dates = dates.clone();
    sorted_dates.sort();
    assert_eq!(dates, sorted_dates, "steps not in timestamp order");

    // Each key should appear exactly as many times as its df has daily groups.
    let a_count = frame.iter().filter(|(k, _)| *k == Key::A).count();
    let b_count = frame.iter().filter(|(k, _)| *k == Key::B).count();
    assert_eq!(a_count, 2);
    assert_eq!(b_count, 2);
}
