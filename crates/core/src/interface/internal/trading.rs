use std::marker::PhantomData;
use std::sync::Arc;

use polars::prelude::{DataFrame as PolarsDataFrame, DataType, PolarsError, PolarsResult};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::http::{
    HttpAccessor, http_events, http_executions, http_listen_async, http_market_history, http_now,
    http_orders, http_portfolio, http_quote, http_step, http_submit_order,
};
use super::{Interface, InterfaceFactory};
use crate::core::{Instrument, Portfolio};
use crate::interface::{
    AccountInterface, BacktestStep, Execution, MarketInterface, OrderId, OrderInterface,
    OrderRecord, Quote, QuoteArgs, Span, StepSnapshot,
};
use crate::network::Protocol;
use crate::trading::Order;
use crate::{Error, Event};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryArgs {
    pub instrument: Instrument,
    pub bar_size: Span,
    pub duration: Span,
}

pub trait StepInput: Sized {
    fn from_snapshot(timestamp: Option<i64>, frame: &PolarsDataFrame) -> Result<Self, Error>;
}

impl StepInput for PolarsDataFrame {
    fn from_snapshot(_timestamp: Option<i64>, frame: &PolarsDataFrame) -> Result<Self, Error> {
        Ok(frame.clone())
    }
}

impl StepInput for (i32, PolarsDataFrame) {
    fn from_snapshot(timestamp: Option<i64>, frame: &PolarsDataFrame) -> Result<Self, Error> {
        let date = timestamp
            .ok_or_else(|| Error::Interface("backtest step has no timestamp".into()))?;
        let date = i32::try_from(date)
            .map_err(|_| Error::Interface(format!("step timestamp {date} is not an i32 date")))?;
        Ok((date, frame.clone()))
    }
}

pub struct TradingProxy {
    interface: Arc<Interface>,
}

impl TradingProxy {
    pub fn new(interface: Interface) -> Self {
        Self::from_shared(Arc::new(interface))
    }

    pub fn from_shared(interface: Arc<Interface>) -> Self {
        Self { interface }
    }

    pub fn connect(url: impl Into<String>) -> Result<Self, Error> {
        let interface = InterfaceFactory::new("trading")
            .accessor(Protocol::Http, HttpAccessor::new(url))
            .get("market_history", Protocol::Http, http_market_history)
            .get("quote", Protocol::Http, http_quote)
            .post("submit_order", Protocol::Http, http_submit_order)
            .get("orders", Protocol::Http, http_orders)
            .get("executions", Protocol::Http, http_executions)
            .get("portfolio", Protocol::Http, http_portfolio)
            .get("listen_async", Protocol::Http, http_listen_async)
            .get("events", Protocol::Http, http_events)
            .get("now", Protocol::Http, http_now)
            .set("step", Protocol::Http, http_step)
            .build()?;
        Ok(Self::new(interface))
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

    pub fn step<I: StepInput>(&self) -> Result<Option<BacktestStep<I>>, Error> {
        let Some(snapshot) = self.call::<(), Option<StepSnapshot>>("step", &())? else {
            return Ok(None);
        };
        let frame = snapshot.frame.into_inner();
        let input = I::from_snapshot(snapshot.timestamp, &frame)?;
        Ok(Some(BacktestStep {
            input,
            frame,
            timestamp: snapshot.timestamp,
        }))
    }

    pub fn now(&self) -> Result<Option<i64>, Error> {
        self.call("now", &())
    }

    pub fn listen<I: StepInput>(&self) -> TradingSteps<'_, I> {
        TradingSteps {
            proxy: self,
            finished: false,
            marker: PhantomData,
        }
    }
}

pub struct TradingSteps<'a, I> {
    proxy: &'a TradingProxy,
    finished: bool,
    marker: PhantomData<fn() -> I>,
}

impl<I: StepInput> Iterator for TradingSteps<'_, I> {
    type Item = Result<BacktestStep<I>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        match self.proxy.step::<I>() {
            Ok(Some(step)) => Some(Ok(step)),
            Ok(None) => {
                self.finished = true;
                None
            }
            Err(error) => {
                self.finished = true;
                Some(Err(error))
            }
        }
    }
}

impl MarketInterface for TradingProxy {
    fn market_history(
        &self,
        contract: &Instrument,
        bar_size: Span,
        duration: Span,
    ) -> PolarsResult<PolarsDataFrame> {
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

    fn quote(&self, contract: &Instrument, levels: usize) -> Result<Option<Quote>, Error> {
        self.call(
            "quote",
            &QuoteArgs {
                instrument: contract.clone(),
                levels,
            },
        )
    }
}

impl AccountInterface for TradingProxy {
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

impl OrderInterface for TradingProxy {
    fn submit_order(&self, order: Order) -> Result<OrderId, Error> {
        self.call("submit_order", &order)
    }

    fn orders(&self) -> Result<Vec<OrderRecord>, Error> {
        self.call("orders", &())
    }

    fn executions(&self) -> Result<Vec<Execution>, Error> {
        self.call("executions", &())
    }
}
