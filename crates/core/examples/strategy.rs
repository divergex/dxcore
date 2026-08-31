//! SMA crossover backtest demo.
//!
//! Run with: `cargo run --example strategy --features strategies`

use polars::prelude::*;
use dxcore::strategies::SmaCross;
use dxcore::trading::{DailyView, SyncExecutor};

fn generate_ohlc(n_days: usize) -> DataFrame {
    let prices: Vec<f64> = (0..n_days)
        .map(|i| {
            let t = i as f64;
            let trend = 100.0 + t * 0.3;
            let wave = 5.0 * (t * 0.4).sin() - 3.0 * (t * 0.15).cos();
            let dip = -15.0 * (-((t - 15.0).powi(2) / 30.0)).exp();
            ((trend + wave + dip) * 100.0).round() / 100.0
        })
        .collect();

    let base_date = 19000i32;
    let dates: Vec<i32> = (0..n_days).map(|i| base_date + i as i32).collect();
    let opens: Vec<f64> = prices.clone();
    let closes: Vec<f64> = prices
        .iter()
        .enumerate()
        .map(|(i, p)| if i < n_days - 1 { prices[i + 1] } else { *p })
        .collect();
    let highs: Vec<f64> = opens
        .iter()
        .zip(closes.iter())
        .map(|(o, c)| o.max(*c) + 0.5)
        .collect();
    let lows: Vec<f64> = opens
        .iter()
        .zip(closes.iter())
        .map(|(o, c)| o.min(*c) - 0.5)
        .collect();
    let volumes: Vec<f64> = (0..n_days).map(|_| 10_000.0).collect();

    let date_col = Column::new(
        "date".into(),
        Series::new("date".into(), dates)
            .cast(&DataType::Date)
            .unwrap(),
    );
    let symbol_col = Column::new("symbol".into(), vec!["DEMO"; n_days]);

    DataFrame::new(vec![
        date_col,
        symbol_col,
        Column::new("open".into(), opens),
        Column::new("high".into(), highs),
        Column::new("low".into(), lows),
        Column::new("close".into(), closes),
        Column::new("volume".into(), volumes),
    ])
    .unwrap()
}

fn main() {
    let df = generate_ohlc(30);
    println!("=== OHLC Data (source columns: 'close') ===\n{:?}\n", df.head(Some(5)));

    let view = DailyView::new("date")
        .with_col_map(vec![("close".into(), "price".into())]);

    let strategy = SmaCross::new(10_000.0, 5, 10);
    let mut executor = SyncExecutor::new(strategy);

    let signals_df = executor.run(&df, view);

    println!("=== Signals ===\n{:?}", signals_df);
}
