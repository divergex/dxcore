//! SMA crossover backtest demo.
//!
//! Run with: `cargo run --example strategy`
//!
//! Key design point: the View is the translation layer between source data
//! columns and the strategy's expected column names. Here the source DataFrame
//! uses `"close"`, the strategy expects `"price"`, and the View handles the
//! rename at the boundary.

use polars::prelude::*;
use dxcore_rs::trading::{DailyView, Strategy, View};

// ---------------------------------------------------------------------------
// Signal
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct Signal {
    date: i32,
    action: String,
    price: f64,
    shares: f64,
    cash: f64,
    equity: f64,
}

// ---------------------------------------------------------------------------
// Strategy: 5/10 SMA crossover
// ---------------------------------------------------------------------------

/// The strategy declares its column contract: it reads from a column named
/// `"price"`. It has no knowledge of what the source DataFrame calls that
/// column — the View is responsible for renaming.
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

    fn on_step(
        &self,
        (_date, day_df): &(i32, DataFrame),
        history: &DataFrame,
        state: &mut State,
    ) -> Signal {
        // Strategy always reads from column "price" — the canonical name.
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
                if state.cash == self.initial_cash && state.shares == 0.0 && price.is_finite() {
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

// ---------------------------------------------------------------------------
// Rename helper — the View's translation boundary
// ---------------------------------------------------------------------------

/// Rename a column in a DataFrame. Returns a new DataFrame.
fn rename_column(df: &DataFrame, from: &str, to: &str) -> DataFrame {
    let mut out = df.clone();
    out.rename(from, to.into()).unwrap();
    out
}

// ---------------------------------------------------------------------------
// Synthetic data
// ---------------------------------------------------------------------------

/// Deterministic price path. Source columns use `"close"` — the application's
/// native naming. The View translates `"close"` → `"price"` at the boundary.
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

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let df = generate_ohlc(30);
    println!("=== OHLC Data (source columns: 'close') ===\n{:?}\n", df.head(Some(5)));

    // -- Manual loop: View translates source "close" → strategy "price" --
    let signals = run_with_signals(&df);

    println!(
        "{:>6}  {:>6}  {:>8}  {:>8}  {:>10}  {:>10}",
        "DATE", "ACTION", "PRICE", "SHARES", "CASH", "EQUITY"
    );
    println!("{}", "-".repeat(65));

    let mut final_equity = 0.0;
    for s in &signals {
        println!(
            "{:>6}  {:>6}  {:>8.2}  {:>8.0}  {:>10.2}  {:>10.2}",
            s.date, s.action, s.price, s.shares, s.cash, s.equity
        );
        final_equity = s.equity;
    }

    println!("\nFinal equity: ${final_equity:.2}");
    println!("Return: {:+.2}%", (final_equity - 10_000.0) / 100.0);
}

/// Run the strategy step-by-step, showing the View as translation boundary:
/// source data has `"close"` → View yields step → rename `"close"` to
/// `"price"` → strategy processes step → step (with `"price"`) appended to
/// history.  The history accumulates in strategy format.
fn run_with_signals(df: &DataFrame) -> Vec<Signal> {
    let strategy = SmaCross {
        initial_cash: 10_000.0,
        short_window: 5,
        long_window: 10,
    };
    let view = DailyView::new("date");

    let mut history = DataFrame::empty();
    let mut state = State { cash: 10_000.0, ..State::default() };
    let mut signals = Vec::new();

    for (date, day_df) in view.steps(df) {
        // --- View translation boundary ---
        // Source data uses "close"; strategy expects "price".
        let step = rename_column(&day_df, "close", "price");
        // ---------------------------------

        let signal = strategy.on_step(&(date, step.clone()), &history, &mut state);
        view.append(&mut history, &(date, step));
        signals.push(signal);
    }

    signals
}
