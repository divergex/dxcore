use serde::{Deserialize, Serialize};

use crate::core::Instrument;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteArgs {
    pub instrument: Instrument,
    pub levels: usize,
}

/// One level of a bid/ask book.
///
/// A side is `None` when the dataset does not provide it. A midprice-only
/// source is represented as a single level with `bid == ask`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct QuoteLevel {
    pub bid: Option<f64>,
    pub ask: Option<f64>,
}

impl QuoteLevel {
    pub fn new(bid: Option<f64>, ask: Option<f64>) -> Self {
        Self { bid, ask }
    }

    /// Midpoint of the level, when both sides are present.
    pub fn mid(&self) -> Option<f64> {
        match (self.bid, self.ask) {
            (Some(bid), Some(ask)) => Some((bid + ask) / 2.0),
            _ => None,
        }
    }
}

/// A quote snapshot for one instrument.
///
/// `levels[0]` is the best bid/ask; later entries are deeper book levels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quote {
    pub timestamp: i64,
    pub levels: Vec<QuoteLevel>,
}

impl Quote {
    pub fn best(&self) -> Option<&QuoteLevel> {
        self.levels.first()
    }
}
