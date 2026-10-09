use std::sync::Arc;

use polars::prelude::{DataType, DataFrame, PolarsError, PolarsResult};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::Interface;
use crate::core::{Instrument, Portfolio};
use crate::interface::{AccountInterface, MarketInterface, OrderInterface, Span};
use crate::{Error, Event};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryArgs {
    pub instrument: Instrument,
    pub bar_size: Span,
    pub duration: Span,
}

pub struct TradingInterface {
    interface: Arc<Interface>,
}

impl TradingInterface {
    pub fn new(interface: Interface) -> Self {
        Self::from_shared(Arc::new(interface))
    }

    pub fn from_shared(interface: Arc<Interface>) -> Self {
        Self { interface }
    }

    fn call<T, R>(&self, method: &str, args: &T) -> Result<R, Error>
    where
        T: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        let args = serde_json::to_value(args).map_err(|e| Error::Interface(e.to_string()))?;
        let value = self.interface.call(method, args)?;
        serde_json::from_value(value).map_err(|e| Error::Interface(e.to_string()))
    }
}

impl MarketInterface for TradingInterface {
    fn market_history(
        &self,
        contract: &Instrument,
        bar_size: Span,
        duration: Span,
    ) -> PolarsResult<DataFrame> {
        let args = HistoryArgs {
            instrument: contract.clone(),
            bar_size,
            duration,
        };
        let frame: crate::DataFrame = self
            .call("market_history", &args)
            .map_err(|e| PolarsError::ComputeError(e.to_string().into()))?;
        let mut frame = frame.into_inner();
        frame.try_apply("date", |dates| dates.cast(&DataType::Date))?;
        Ok(frame)
    }
}

impl AccountInterface for TradingInterface {
    fn portfolio(&self, account_id: &str) -> Result<Portfolio, Error> {
        self.call("portfolio", account_id)
    }

    fn listen_async(&self, account_id: &str) -> Result<(), Error> {
        let _: bool = self.call("listen_async", account_id)?;
        Ok(())
    }

    fn events(&self, account_id: &str) -> Result<Vec<Event>, Error> {
        self.call("events", account_id)
    }
}

impl OrderInterface for TradingInterface {}
