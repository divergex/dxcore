use std::cell::RefCell;

use jiff::Span;
use polars::prelude::*;

use crate::core::{Instrument, Portfolio};
use crate::{Error, Event};

pub trait MarketInterface {
    fn market_history(
        &self,
        contract: &Instrument,
        bar_size: Span,
        duration: Span,
    ) -> PolarsResult<DataFrame>;
}

pub trait AccountInterface {
    fn portfolio(&self, account_id: &str) -> Result<Portfolio, Error>;

    /// Starts a background listen loop for `account_id`, recording events for
    /// [`events`](Self::events).
    fn listen_async(&self, account_id: &str) -> Result<(), Error>;

    /// Drains the events recorded by [`listen_async`](Self::listen_async).
    fn events(&self, account_id: &str) -> Result<Vec<Event>, Error>;
}

pub trait OrderInterface {}

pub trait TradingInterface: MarketInterface + AccountInterface + OrderInterface {}

impl<T: MarketInterface + AccountInterface + OrderInterface> TradingInterface for T {}

/// In-memory interface returning preconfigured data, for tests and demos.
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

impl MarketInterface for MockInterface {
    fn market_history(
        &self,
        _contract: &Instrument,
        _bar_size: Span,
        _duration: Span,
    ) -> PolarsResult<DataFrame> {
        self.history
            .clone()
            .ok_or_else(|| PolarsError::ComputeError("no history configured".into()))
    }
}

impl AccountInterface for MockInterface {
    fn portfolio(&self, _account_id: &str) -> Result<Portfolio, Error> {
        self.portfolio
            .clone()
            .ok_or_else(|| Error::Connection("no portfolio configured".into()))
    }

    fn listen_async(&self, _account_id: &str) -> Result<(), Error> {
        Ok(())
    }

    fn events(&self, _account_id: &str) -> Result<Vec<Event>, Error> {
        Ok(self.events.borrow_mut().drain(..).collect())
    }
}

impl OrderInterface for MockInterface {}
