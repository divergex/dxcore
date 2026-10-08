use std::collections::HashMap;

use polars::prelude::*;

use crate::trading::{Signal, StreamedStrategy};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Source {
    Fundamentals,
    Returns,
}

/// Raw accounting data from the most recent fundamentals filing.
#[derive(Debug, Clone, Default)]
pub struct Fundamentals {
    pub outstanding_shares: f64,
    pub book_value: f64,
    pub operating_income: f64,
    pub total_assets: f64,
}

/// Cross-sectional z-score.
#[derive(Debug, Clone, Default)]
pub struct FactorZScore {
    pub smb: f64,
    pub hml: f64,
    pub rmw: f64,
    pub cma: f64,
}

#[derive(Debug, Default)]
pub struct State {
    pub fundamentals: HashMap<String, Fundamentals>,
    /// Prior-period total_assets per symbol, for asset growth computation.
    pub prev_total_assets: HashMap<String, f64>,
    pub zscores: HashMap<String, FactorZScore>,
    pub positions: HashMap<String, f64>,
    pub cash: f64,
    pub top_n: usize,
}

pub struct FiveFactor {
    pub initial_cash: f64,
    pub top_n: usize,
    pub symbol_col: String,
    /// Expected columns in the fundamentals DataFrame (raw accounting values).
    pub outstanding_shares_col: String,
    pub book_value_col: String,
    pub operating_income_col: String,
    pub total_assets_col: String,
    /// Expected columns in the returns DataFrame.
    pub return_col: String,
    pub price_col: String,
}

impl FiveFactor {
    pub fn new(initial_cash: f64, top_n: usize) -> Self {
        Self {
            initial_cash,
            top_n,
            symbol_col: "symbol".into(),
            outstanding_shares_col: "outstanding_shares".into(),
            book_value_col: "book_value".into(),
            operating_income_col: "operating_income".into(),
            total_assets_col: "total_assets".into(),
            return_col: "return".into(),
            price_col: "price".into(),
        }
    }
}

impl StreamedStrategy for FiveFactor {
    type Key = Source;
    type Input = (i32, DataFrame);
    type State = State;
    type Output = Option<Signal>;

    fn on_step(
        &self,
        (_date, df): &(i32, DataFrame),
        key: &Source,
        _history: &HashMap<Source, DataFrame>,
        state: &mut State,
    ) -> Option<Signal> {
        state.top_n = self.top_n;

        match key {
            Source::Fundamentals => {
                self.ingest_fundamentals(df, state);
                None
            }
            Source::Returns => self.handle_returns(df, state),
        }
    }
}

impl FiveFactor {
    fn ingest_fundamentals(&self, df: &DataFrame, state: &mut State) {
        let symbols = df
            .column(&self.symbol_col)
            .map(|c| c.str().unwrap().clone())
            .unwrap();

        let shares = df.column(&self.outstanding_shares_col).unwrap().f64().unwrap();
        let bv = df.column(&self.book_value_col).unwrap().f64().unwrap();
        let oi = df.column(&self.operating_income_col).unwrap().f64().unwrap();
        let ta = df.column(&self.total_assets_col).unwrap().f64().unwrap();

        for i in 0..df.height() {
            let sym = symbols.get(i).unwrap().to_string();
            let total_assets = ta.get(i).unwrap_or(f64::NAN);

            if let Some(prev) = state.fundamentals.get(&sym) {
                state
                    .prev_total_assets
                    .insert(sym.clone(), prev.total_assets);
            }

            state.fundamentals.insert(
                sym,
                Fundamentals {
                    outstanding_shares: shares.get(i).unwrap_or(f64::NAN),
                    book_value: bv.get(i).unwrap_or(f64::NAN),
                    operating_income: oi.get(i).unwrap_or(f64::NAN),
                    total_assets,
                },
            );
        }
    }

    fn handle_returns(&self, df: &DataFrame, state: &mut State) -> Option<Signal> {
        let symbols = df
            .column(&self.symbol_col)
            .map(|c| c.str().unwrap().clone())
            .unwrap();

        let prices = df.column(&self.price_col).unwrap().f64().unwrap();

        // (symbol, price, market_cap, book_to_market, op_profitability, asset_growth)
        let mut cross: Vec<(String, f64, f64, f64, f64, f64)> = Vec::new();

        for i in 0..df.height() {
            let sym = symbols.get(i).unwrap().to_string();
            let price = prices.get(i).unwrap_or(f64::NAN);
            let fund = match state.fundamentals.get(&sym) {
                Some(f) => f,
                None => continue,
            };

            let market_cap = price * fund.outstanding_shares;
            if market_cap <= 0.0 {
                continue;
            }

            let book_to_market = if market_cap > 0.0 {
                fund.book_value / market_cap
            } else {
                0.0
            };
            let op_profitability = if fund.book_value > 0.0 {
                fund.operating_income / fund.book_value
            } else {
                0.0
            };
            let asset_growth = state
                .prev_total_assets
                .get(&sym)
                .map(|prev| {
                    if *prev > 0.0 {
                        (fund.total_assets - prev) / prev
                    } else {
                        0.0
                    }
                })
                .unwrap_or(0.0);

            cross.push((sym, price, market_cap, book_to_market, op_profitability, asset_growth));
        }

        if cross.len() < 2 {
            return None;
        }

        let n_f = cross.len() as f64;

        let mean_mc = cross.iter().map(|(_, _, mc, _, _, _)| mc).sum::<f64>() / n_f;
        let mean_btm = cross.iter().map(|(_, _, _, btm, _, _)| btm).sum::<f64>() / n_f;
        let mean_op = cross.iter().map(|(_, _, _, _, op, _)| op).sum::<f64>() / n_f;
        let mean_ag = cross.iter().map(|(_, _, _, _, _, ag)| ag).sum::<f64>() / n_f;

        let var_mc = cross.iter().map(|(_, _, mc, _, _, _)| (mc - mean_mc).powi(2)).sum::<f64>() / n_f;
        let var_btm = cross.iter().map(|(_, _, _, btm, _, _)| (btm - mean_btm).powi(2)).sum::<f64>() / n_f;
        let var_op = cross.iter().map(|(_, _, _, _, op, _)| (op - mean_op).powi(2)).sum::<f64>() / n_f;
        let var_ag = cross.iter().map(|(_, _, _, _, _, ag)| (ag - mean_ag).powi(2)).sum::<f64>() / n_f;

        state.zscores.clear();
        let mut scored: Vec<(String, f64, f64)> = Vec::new();

        for (sym, price, mc, btm, op, ag) in &cross {
            let z = FactorZScore {
                smb: if var_mc > 0.0 {
                    -(mc - mean_mc) / var_mc.sqrt()
                } else {
                    0.0
                },
                hml: if var_btm > 0.0 {
                    (btm - mean_btm) / var_btm.sqrt()
                } else {
                    0.0
                },
                rmw: if var_op > 0.0 {
                    (op - mean_op) / var_op.sqrt()
                } else {
                    0.0
                },
                cma: if var_ag > 0.0 {
                    -(ag - mean_ag) / var_ag.sqrt()
                } else {
                    0.0
                },
            };
            let composite = z.smb + z.hml + z.rmw + z.cma;
            state.zscores.insert(sym.clone(), z);
            scored.push((sym.clone(), *price, composite));
        }

        let n = scored.len().min(state.top_n);
        scored.sort_unstable_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

        let total_value: f64 = state.cash
            + state
                .positions
                .iter()
                .map(|(sym, shares)| {
                    let price = scored
                        .iter()
                        .find(|(s, _, _)| s == sym)
                        .map(|(_, p, _)| *p)
                        .unwrap_or(0.0);
                    shares * price
                })
                .sum::<f64>();

        let target_value = if total_value <= 0.0 {
            self.initial_cash / (2.0 * n as f64)
        } else {
            total_value / (2.0 * n as f64)
        };

        // One trade per step keeps signal granularity at the step level.
        let top_n: Vec<&(String, f64, f64)> = scored.iter().take(n).collect();
        let bottom_n: Vec<&(String, f64, f64)> = scored.iter().rev().take(n).collect();

        for (sym, price, _score) in &top_n {
            let current_shares = state.positions.get(sym).copied().unwrap_or(0.0);
            let target_shares = (target_value / price).trunc();
            if (target_shares - current_shares).abs() >= 1.0 {
                let delta = target_shares - current_shares;
                state.cash -= delta * price;
                state.positions.insert(sym.to_string(), target_shares);
                return Some(Signal { symbol: Some(sym.clone()), shares: target_shares });
            }
        }

        for (sym, price, _score) in &bottom_n {
            let current_shares = state.positions.get(sym).copied().unwrap_or(0.0);
            let target_shares = -(target_value / price).trunc();
            if (current_shares - target_shares).abs() >= 1.0 {
                let delta = target_shares - current_shares;
                state.cash -= delta * price;
                state.positions.insert(sym.to_string(), target_shares);
                return Some(Signal { symbol: Some(sym.clone()), shares: target_shares });
            }
        }

        None
    }
}
