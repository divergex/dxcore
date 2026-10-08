use std::collections::HashMap;

use polars::prelude::*;
use dxcore::trading::{
    DailyView, OrderEngine, OrderError, Strategy, StreamedStrategy, SyncExecutor, TickView,
};

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

/// Smallest possible engine: every output becomes one order, verbatim.
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

#[test]
fn returns_frame() {
    let df = helpers::ohlc_df();
    let mut executor = SyncExecutor::new(PassThrough, CountStrategy);
    let frame = executor.run(&df, TickView::new("date")).unwrap();
    assert_eq!(frame, vec![1, 2, 3, 4, 5]);
}

#[test]
fn empty_df_yields_empty_frame() {
    let df = helpers::empty_ohlc_df();
    let mut executor = SyncExecutor::new(PassThrough, CountStrategy);
    let frame = executor.run(&df, TickView::new("date")).unwrap();
    assert!(frame.is_empty());
}

#[test]
fn reusable_across_datasets() {
    let df1 = helpers::ohlc_df();
    let mut executor = SyncExecutor::new(PassThrough, CountStrategy);

    let frame1 = executor.run(&df1, TickView::new("date")).unwrap();
    assert_eq!(frame1.len(), 5);

    let df2 = helpers::empty_ohlc_df();
    let frame2 = executor.run(&df2, TickView::new("date")).unwrap();
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

    let mut executor = SyncExecutor::new(PassThrough, MultiCountStrategy);
    let frame = executor.run_multi(&dfs, views).unwrap();

    // 3 AAPL rows + 2 GOOG rows = 5 steps. TickView defines no ordering, so
    // which key goes first depends on map iteration: compare as a multiset.
    let mut sorted = frame.clone();
    sorted.sort();
    assert_eq!(sorted, vec![1, 2, 3, 4, 5]);
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

        fn on_step(
            &self,
            step: &(i32, DataFrame),
            key: &Key,
            _history: &HashMap<Key, DataFrame>,
            _state: &mut (),
        ) -> (Key, i32) {
            (key.clone(), step.0)
        }
    }

    /// The output already carries the key, so the engine just passes it on.
    struct TaggedOrders;

    impl OrderEngine<(Key, i32), (i32, DataFrame)> for TaggedOrders {
        type Order = (Key, i32);
        type Frame = Vec<(Key, i32)>;
        type State = ();

        fn transform(
            &self,
            output: (Key, i32),
            _step: &(i32, DataFrame),
            _date: Option<i64>,
            _state: &mut (),
        ) -> Result<Option<(Key, i32)>, OrderError> {
            Ok(Some(output))
        }

        fn create_output(&self) -> Vec<(Key, i32)> {
            Vec::new()
        }

        fn append_output(
            &self,
            frame: &mut Vec<(Key, i32)>,
            order: (Key, i32),
            _step: &(i32, DataFrame),
        ) {
            frame.push(order);
        }
    }

    let mut dfs = HashMap::new();
    dfs.insert(Key::A, df1);
    dfs.insert(Key::B, df2);

    let views = HashMap::from([
        (Key::A, DailyView::new("date")),
        (Key::B, DailyView::new("date")),
    ]);

    let mut executor = SyncExecutor::new(TaggedOrders, OrderRecorder);
    let frame = executor.run_multi(&dfs, views).unwrap();

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
