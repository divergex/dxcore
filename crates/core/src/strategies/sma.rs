use polars::prelude::*;
use crate::trading::Strategy;

#[derive(Debug, Clone)]
pub struct Signal {
    pub date: i32,
    pub action: String,
    pub price: f64,
    pub shares: f64,
    pub cash: f64,
    pub equity: f64,
}

#[derive(Debug, Default)]
pub struct State {
    pub cash: f64,
    pub shares: f64,
    pub prev_short: Option<f64>,
    pub prev_long: Option<f64>,
}

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

/// None if `col` is missing or has fewer than `window` rows.
fn column_sma(history: &DataFrame, col: &str, window: usize) -> Option<f64> {
    let series = history.column(col).ok()?.f64().ok()?;
    let vals: Vec<f64> = series.into_iter().flatten().collect();
    if vals.len() < window {
        return None;
    }
    let sum: f64 = vals.iter().rev().take(window).sum();
    Some(sum / window as f64)
}
