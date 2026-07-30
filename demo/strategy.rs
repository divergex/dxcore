//! SMA crossover backtest demo.
//!
//! Run with: `cargo run --example strategy`
//!
//! Key design points:
//! - The View translates source columns ("close") → strategy columns ("price")
//!   via its `col_map`. The strategy never sees the source schema.
//! - The strategy defines its output frame (a DataFrame of signals) and
//!   the executor collects it automatically.

use polars::prelude::*;
use cadlag::trading::{DailyView, Strategy, SyncExecutor};

#[derive(Debug, Clone)]
struct Signal {
    date: i32,
    action: String,
    price: f64,
    shares: f64,
    cash: f64,
    equity: f64,
}
/// Strategy reads from column `"price"` — the View's `col_map` ensures that
/// the source `"close"` column arrives renamed.

#[derive(Debug, Default)]
struct State {
    cash: f64,
    shares: f64,
    prev_short: Option<f64>,
    prev_long: Option<f64>,
}

struct SmaCross {
    initial_cash: f64,
    short_window: usize,
    long_window: usize,
}

impl Strategy for SmaCross {
    type Input = (i32, DataFrame);
    type State = State;
    type Output = Signal;
    type Frame = DataFrame;

    fn on_step(
        &self,
        (_date, day_df): &(i32, DataFrame),
        history: &DataFrame,
        state: &mut State,
    ) -> Signal {
        let price = day_df
            .column("price")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap_or(f64::NAN);

        let date = day_df
            .column("date")
            .unwrap()
            .date()
            .unwrap()
            .get(0)
            .unwrap();

        let short_sma = column_sma(history, "price", self.short_window);
        let long_sma = column_sma(history, "price", self.long_window);

        let action = match (state.prev_short, state.prev_long, short_sma, long_sma) {
            (Some(ps), Some(pl), Some(s), Some(l)) => {
                if ps <= pl && s > l {
                    if state.cash > 0.0 {
                        let shares_to_buy = (state.cash / price).trunc();
                        state.shares += shares_to_buy;
                        state.cash -= shares_to_buy * price;
                        "BUY".into()
                    } else {
                        "-".into()
                    }
                } else if ps >= pl && s < l {
                    if state.shares > 0.0 {
                        state.cash += state.shares * price;
                        state.shares = 0.0;
                        "SELL".into()
                    } else {
                        "-".into()
                    }
                } else {
                    "-".into()
                }
            }
            _ => {
                if state.shares == 0.0 && state.cash == 0.0 && price.is_finite() {
                    state.cash = self.initial_cash;
                    let shares_to_buy = (state.cash / price).trunc();
                    state.shares += shares_to_buy;
                    state.cash -= shares_to_buy * price;
                    "INIT".into()
                } else {
                    "-".into()
                }
            }
        };

        state.prev_short = short_sma;
        state.prev_long = long_sma;

        let equity = state.cash + state.shares * price;

        Signal { date, action, price, shares: state.shares, cash: state.cash, equity }
    }

    fn create_output(&self) -> DataFrame {
        // Columns that match Signal's fields.
        DataFrame::new(vec![
            Column::new_empty("date".into(), &DataType::Int32),
            Column::new_empty("action".into(), &DataType::String),
            Column::new_empty("price".into(), &DataType::Float64),
            Column::new_empty("shares".into(), &DataType::Float64),
            Column::new_empty("cash".into(), &DataType::Float64),
            Column::new_empty("equity".into(), &DataType::Float64),
        ]).unwrap()
    }

    fn append_output(&self, frame: &mut DataFrame, output: Signal, _step: &(i32, DataFrame)) {
        let row = DataFrame::new(vec![
            Column::new("date".into(), &[output.date]),
            Column::new("action".into(), &[output.action.as_str()]),
            Column::new("price".into(), &[output.price]),
            Column::new("shares".into(), &[output.shares]),
            Column::new("cash".into(), &[output.cash]),
            Column::new("equity".into(), &[output.equity]),
        ])
        .unwrap();

        if frame.width() == 0 {
            *frame = row;
        } else {
            frame.vstack_mut(&row).unwrap();
        }
    }
}

/// SMA of column `col` in history. None if column missing or insufficient data.
fn column_sma(history: &DataFrame, col: &str, window: usize) -> Option<f64> {
    let series = history.column(col).ok()?.f64().ok()?;
    let vals: Vec<f64> = series.into_iter().flatten().collect();
    if vals.len() < window {
        return None;
    }
    let sum: f64 = vals.iter().rev().take(window).sum();
    Some(sum / window as f64)
}

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

    // The View translates "close" → "price" via col_map.
    // The strategy only knows about "price".
    let view = DailyView::new("date")
        .with_col_map(vec![("close".into(), "price".into())]);

    let strategy = SmaCross {
        initial_cash: 10_000.0,
        short_window: 5,
        long_window: 10,
    };
    let mut executor = SyncExecutor::new(strategy);

    let signals_df = executor.run(&df, view);

    println!("=== Signals ===\n{:?}", signals_df);
}
