#[cfg(feature = "ibkr")]
use std::cell::RefCell;
#[cfg(feature = "ibkr")]
use std::sync::mpsc::Sender;

#[cfg(feature = "ibkr")]
use polars::prelude::*;

#[cfg(feature = "ibkr")]
use ibapi::contracts::Contract;
#[cfg(feature = "ibkr")]
use ibapi::market_data::historical::{BarSize, Duration};

#[cfg(feature = "ibkr")]
use crate::core::Portfolio;
#[cfg(feature = "ibkr")]
use crate::Event;
#[cfg(feature = "ibkr")]
use crate::Error;
pub mod external;
pub mod internal;
pub mod stream;

#[cfg(feature = "ibkr")]
pub trait MarketApi {
    /// DataFrame with columns: `date`, `open`, `high`, `low`, `close`, `volume`.
    fn market_history(
        &self,
        contract: &Contract,
        bar_size: BarSize,
        duration: Duration,
    ) -> PolarsResult<DataFrame>;

    fn portfolio(&self, account_id: &str) -> Result<Portfolio, Error>;

    fn listen(&self, account_id: &str, tx: Sender<Event>) -> Result<(), Error>;
}

#[cfg(feature = "ibkr")]
#[derive(Debug, Default)]
pub struct MockInterface {
    history: Option<DataFrame>,
    portfolio: Option<Portfolio>,
    events: RefCell<Vec<Event>>,
}

#[cfg(feature = "ibkr")]
impl MockInterface {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_history(mut self, df: DataFrame) -> Self {
        self.history = Some(df);
        self
    }

    pub fn with_portfolio(mut self, p: Portfolio) -> Self {
        self.portfolio = Some(p);
        self
    }

    pub fn with_events(mut self, events: Vec<Event>) -> Self {
        self.events = RefCell::new(events);
        self
    }
}

#[cfg(feature = "ibkr")]
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

