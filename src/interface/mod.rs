use std::cell::RefCell;
use std::sync::mpsc::Sender;

use polars::prelude::*;

use ibapi::contracts::Contract;
use ibapi::market_data::historical::{BarSize, Duration};

use crate::core::Portfolio;
use crate::Event;
use crate::Error;
pub mod ibkr;

// ---------------------------------------------------------------------------
// MarketApi — the interface that both real and mock implementations fulfill
// ---------------------------------------------------------------------------

/// Abstraction over a market data and portfolio provider.
pub trait MarketApi {
    /// Fetch historical OHLCV bars for a contract and return them as a
    /// DataFrame with columns: `date`, `open`, `high`, `low`, `close`, `volume`.
    fn market_history(
        &self,
        contract: &Contract,
        bar_size: BarSize,
        duration: Duration,
    ) -> PolarsResult<DataFrame>;

    /// Subscribe to account updates and return the current portfolio snapshot
    /// (metrics + holdings). Blocks until the initial snapshot completes.
    fn portfolio(&self, account_id: &str) -> Result<Portfolio, Error>;

    /// Connect, stream account updates, fetch historical bars for each
    /// position, and push all data as `Event`s through the sender.
    /// Blocks for the session lifetime.
    fn listen(&self, account_id: &str, tx: Sender<Event>) -> Result<(), Error>;
}

// ---------------------------------------------------------------------------
// MockInterface — canned responses for testing
// ---------------------------------------------------------------------------

/// A mock implementation of [`MarketApi`] that returns pre-configured
/// responses. All methods are non-blocking.
#[derive(Debug, Default)]
pub struct MockInterface {
    history: Option<DataFrame>,
    portfolio: Option<Portfolio>,
    events: RefCell<Vec<Event>>,
}

impl MockInterface {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the DataFrame returned by [`market_history`](MarketApi::market_history).
    pub fn with_history(mut self, df: DataFrame) -> Self {
        self.history = Some(df);
        self
    }

    /// Set the [`Portfolio`] returned by [`portfolio`](MarketApi::portfolio).
    pub fn with_portfolio(mut self, p: Portfolio) -> Self {
        self.portfolio = Some(p);
        self
    }

    /// Set the events emitted by [`listen`](MarketApi::listen).
    pub fn with_events(mut self, events: Vec<Event>) -> Self {
        self.events = RefCell::new(events);
        self
    }
}

impl MarketApi for MockInterface {
    fn market_history(
        &self,
        _contract: &Contract,
        _bar_size: BarSize,
        _duration: Duration,
    ) -> PolarsResult<DataFrame> {
        self.history
            .clone()
            .ok_or_else(|| PolarsError::ComputeError("no history configured".into()))
    }

    fn portfolio(&self, _account_id: &str) -> Result<Portfolio, Error> {
        self.portfolio
            .clone()
            .ok_or_else(|| Error::Connection("no portfolio configured".into()))
    }

    fn listen(&self, _account_id: &str, tx: Sender<Event>) -> Result<(), Error> {
        let events: Vec<Event> = self.events.borrow_mut().drain(..).collect();
        for event in events {
            tx.send(event).ok();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
