use polars::prelude::*;

use crate::trading::{Signal, Strategy};

#[derive(Debug, Default)]
pub struct State {
    pub cash: f64,
    pub shares: f64,
    pub prev_short: Option<f64>,
    pub prev_long: Option<f64>,
}

/// Long/flat SMA crossover on a single instrument.
pub struct SmaCross {
    pub initial_cash: f64,
    pub short_window: usize,
    pub long_window: usize,
}

impl SmaCross {
    pub fn new(initial_cash: f64, short_window: usize, long_window: usize) -> Self {
        Self { initial_cash, short_window, long_window }
    }
}

impl Strategy for SmaCross {
    type Input = (i32, DataFrame);
    type State = State;
    type Output = Option<Signal>;

    fn on_step(
        &self,
        (_date, day_df): &(i32, DataFrame),
        history: &DataFrame,
        state: &mut State,
    ) -> Option<Signal> {
        let price = day_df
            .column("price")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap_or(f64::NAN);

        let short_sma = column_sma(history, "price", self.short_window);
        let long_sma = column_sma(history, "price", self.long_window);
        let target_before = state.shares;

        match (state.prev_short, state.prev_long, short_sma, long_sma) {
            (Some(ps), Some(pl), Some(s), Some(l)) => {
                if ps <= pl && s > l {
                    if state.cash > 0.0 {
                        let shares_to_buy = (state.cash / price).trunc();
                        state.shares += shares_to_buy;
                        state.cash -= shares_to_buy * price;
                    }
                } else if ps >= pl && s < l && state.shares > 0.0 {
                    state.cash += state.shares * price;
                    state.shares = 0.0;
                }
            }
            _ => {
                if state.shares == 0.0 && state.cash == 0.0 && price.is_finite() {
                    state.cash = self.initial_cash;
                    let shares_to_buy = (state.cash / price).trunc();
                    state.shares += shares_to_buy;
                    state.cash -= shares_to_buy * price;
                }
            }
        };

        state.prev_short = short_sma;
        state.prev_long = long_sma;

        (state.shares != target_before).then(|| Signal {
            symbol: step_symbol(day_df),
            shares: state.shares,
        })
    }
}

fn step_symbol(step: &DataFrame) -> Option<String> {
    step.column("symbol")
        .ok()?
        .str()
        .ok()?
        .get(0)
        .map(str::to_owned)
}

fn column_sma(history: &DataFrame, col: &str, window: usize) -> Option<f64> {
    let series = history.column(col).ok()?.f64().ok()?;
    let vals: Vec<f64> = series.into_iter().flatten().collect();
    if vals.len() < window {
        return None;
    }
    let sum: f64 = vals.iter().rev().take(window).sum();
    Some(sum / window as f64)
}
