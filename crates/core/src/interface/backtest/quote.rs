use polars::prelude::*;

use crate::interface::quote::{Quote, QuoteLevel};

/// Column names used to read quotes from the current view step.
///
/// A bid/ask schema can describe several book levels; a midprice schema is a
/// single level with `bid == ask`. Both can be present, in which case bid/ask
/// is preferred and the midprice is used for missing sides.
#[derive(Debug, Clone, Default)]
pub struct QuoteSchema {
    pub bids: Vec<String>,
    pub asks: Vec<String>,
    pub mid: Option<String>,
}

impl QuoteSchema {
    pub fn bid_ask(bids: Vec<String>, asks: Vec<String>) -> Self {
        Self {
            bids,
            asks,
            mid: None,
        }
    }

    pub fn mid(column: impl Into<String>) -> Self {
        Self {
            bids: Vec::new(),
            asks: Vec::new(),
            mid: Some(column.into()),
        }
    }

    /// Infer a schema from common column names.
    ///
    /// Level 1 accepts `bid`/`ask`, `bid_1`/`ask_1`, or `bid1`/`ask1`; deeper
    /// levels accept the numbered forms. A lone midprice accepts `mid`,
    /// `midprice`, or `mid_price`.
    pub fn infer(df: &DataFrame) -> Option<Self> {
        let mut bids = Vec::new();
        let mut asks = Vec::new();
        for level in 1.. {
            let Some((bid, ask)) = level_columns(df, level) else {
                break;
            };
            bids.push(bid);
            asks.push(ask);
        }

        let mid = ["mid", "midprice", "mid_price"]
            .into_iter()
            .find(|column| has_column(df, column))
            .map(str::to_owned);

        if bids.is_empty() && asks.is_empty() && mid.is_none() {
            None
        } else {
            Some(Self { bids, asks, mid })
        }
    }

    pub(super) fn quote(&self, frame: &DataFrame, timestamp: i64, levels: usize) -> Option<Quote> {
        if levels == 0 || self.is_empty() || levels > self.level_count() {
            return None;
        }

        let mid = self
            .mid
            .as_deref()
            .and_then(|column| f64_at(frame, column, 0));
        let bid_ask_levels = if self.bids.is_empty() || self.asks.is_empty() {
            0
        } else {
            self.bids.len().min(self.asks.len())
        };

        let mut out = Vec::with_capacity(levels);
        for level in 0..levels {
            let (mut bid, mut ask) = if level < bid_ask_levels {
                (
                    f64_at(frame, &self.bids[level], 0),
                    f64_at(frame, &self.asks[level], 0),
                )
            } else {
                (None, None)
            };

            if bid.is_none() {
                bid = mid;
            }
            if ask.is_none() {
                ask = mid;
            }
            if bid.is_none() && ask.is_none() {
                return None;
            }
            out.push(QuoteLevel::new(bid, ask));
        }

        Some(Quote {
            timestamp,
            levels: out,
        })
    }

    fn level_count(&self) -> usize {
        let bid_ask = if self.bids.is_empty() || self.asks.is_empty() {
            0
        } else {
            self.bids.len().min(self.asks.len())
        };
        bid_ask.max(usize::from(self.mid.is_some()))
    }

    fn is_empty(&self) -> bool {
        self.bids.is_empty() && self.asks.is_empty() && self.mid.is_none()
    }
}

fn level_columns(df: &DataFrame, level: usize) -> Option<(String, String)> {
    let candidates = if level == 1 {
        vec![
            ("bid".to_owned(), "ask".to_owned()),
            ("bid_1".to_owned(), "ask_1".to_owned()),
            ("bid1".to_owned(), "ask1".to_owned()),
        ]
    } else {
        vec![
            (format!("bid_{level}"), format!("ask_{level}")),
            (format!("bid{level}"), format!("ask{level}")),
        ]
    };

    candidates
        .into_iter()
        .find(|(bid, ask)| has_column(df, bid) && has_column(df, ask))
}

fn has_column(df: &DataFrame, column: &str) -> bool {
    df.column(column).is_ok()
}

fn f64_at(df: &DataFrame, column: &str, row: usize) -> Option<f64> {
    let series = df.column(column).ok()?;
    let series = series.cast(&DataType::Float64).ok()?;
    series.f64().ok()?.get(row)
}
