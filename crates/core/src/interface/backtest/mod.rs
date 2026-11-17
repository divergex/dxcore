mod account;
mod market;
mod order;
mod quote;
mod service;

pub use quote::QuoteSchema;
pub use service::BacktestService;

use std::fmt;
use std::sync::Mutex;

use polars::prelude::*;
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::core::Portfolio;
use crate::interface::order::{Execution, OrderRecord, OrderUpdateCallback};
use crate::trading::{OrderBook, View};

/// One step yielded by [`BacktestInterface::listen`].
#[derive(Debug, Clone)]
pub struct BacktestStep<I> {
    pub input: I,
    pub frame: DataFrame,
    pub timestamp: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepSnapshot {
    pub timestamp: Option<i64>,
    pub frame: crate::DataFrame,
}

/// Optional observer invoked after each step is produced.
pub type StepObserver<'a, V> = Box<
    dyn FnMut(&BacktestInterface<V>, &BacktestStep<<V as View>::Item>) -> Result<(), Error> + 'a,
>;

/// Iterator returned by [`BacktestInterface::listen`].
pub struct BacktestSteps<'a, V: View> {
    interface: &'a BacktestInterface<V>,
    observer: Option<StepObserver<'a, V>>,
    finished: bool,
}

/// A steppable in-memory trading interface for backtests.
///
/// The steps come from a [`View`], which owns the dataset's timestamp and
/// column layout. `BacktestInterface` stores the view's steps, advances a
/// cursor through them, and exposes the current step's frame and timestamp.
///
/// A view should isolate one instrument per step when quotes or order
/// execution are used; [`PanelView`](crate::trading::PanelView) does this for
/// date/symbol datasets. `QuoteSchema` controls how quote columns are read from
/// the current step.
pub struct BacktestInterface<V: View> {
    view: V,
    steps: Vec<V::Item>,
    empty: DataFrame,
    cursor: Mutex<Option<usize>>,
    quote_schema: QuoteSchema,
    portfolio: Portfolio,
    tick_size: i64,
    book: Mutex<OrderBook>,
    next_id: Mutex<u64>,
    records: Mutex<Vec<OrderRecord>>,
    executions: Mutex<Vec<Execution>>,
    callbacks: Mutex<Vec<OrderUpdateCallback>>,
}

impl<V: View> BacktestInterface<V> {
    /// Build an interface from a dataset and the view that slices it.
    ///
    /// The quote schema is inferred from the first step; use
    /// [`with_quote_schema`](Self::with_quote_schema) to override it. Prices
    /// use a tick size of 1 until [`with_tick_size`](Self::with_tick_size).
    pub fn new(history: DataFrame, view: V) -> Self {
        let steps: Vec<V::Item> = view.steps(&history).collect();
        let empty = steps
            .first()
            .map(|step| view.frame(step).slice(0, 0))
            .unwrap_or_else(|| history.slice(0, 0));
        let quote_schema = steps
            .first()
            .and_then(|step| QuoteSchema::infer(view.frame(step)))
            .unwrap_or_default();

        Self {
            view,
            steps,
            empty,
            cursor: Mutex::new(None),
            quote_schema,
            portfolio: Portfolio::default(),
            tick_size: 1,
            book: Mutex::new(OrderBook::default()),
            next_id: Mutex::new(1),
            records: Mutex::new(Vec::new()),
            executions: Mutex::new(Vec::new()),
            callbacks: Mutex::new(Vec::new()),
        }
    }

    pub fn with_quote_schema(mut self, schema: QuoteSchema) -> Self {
        self.quote_schema = schema;
        self
    }

    pub fn with_tick_size(mut self, tick_size: i64) -> Result<Self, Error> {
        let book =
            OrderBook::new(tick_size).map_err(|error| Error::Interface(error.to_string()))?;
        self.tick_size = tick_size;
        self.book = Mutex::new(book);
        Ok(self)
    }

    /// Current step timestamp, `None` before the first [`step`](Self::step).
    pub fn now(&self) -> Option<i64> {
        self.current_step()
            .and_then(|step| self.view.step_timestamp(step))
    }

    /// Advance to the next view step.
    ///
    /// Returns `false` once every step has been visited. Open orders are
    /// offered the new quote before the method returns.
    pub fn step(&self) -> Result<bool, Error> {
        let mut cursor = self
            .cursor
            .lock()
            .map_err(|_| Error::Interface("backtest cursor lock poisoned".into()))?;
        let next = match *cursor {
            Some(index) => index + 1,
            None => 0,
        };

        if next >= self.steps.len() {
            return Ok(false);
        }

        *cursor = Some(next);
        Ok(true)
    }

    /// Iterate over the remaining view steps.
    ///
    /// The optional observer runs after each step is produced; it can read the
    /// current state or submit orders. `None` yields steps only.
    pub fn listen<'a>(&'a self, observer: Option<StepObserver<'a, V>>) -> BacktestSteps<'a, V> {
        BacktestSteps {
            interface: self,
            observer,
            finished: false,
        }
    }

    /// The view item for the current step, once [`step`](Self::step) has run.
    pub fn current_step(&self) -> Option<&V::Item> {
        let index = (*self.cursor.lock().ok()?)?;
        self.steps.get(index)
    }

    /// Market data for the current step, once [`step`](Self::step) has run.
    pub fn current_frame(&self) -> Option<&DataFrame> {
        self.current_step().map(|step| self.view.frame(step))
    }
}

impl<V> Iterator for BacktestSteps<'_, V>
where
    V: View,
    V::Item: Clone,
{
    type Item = Result<BacktestStep<V::Item>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        match self.interface.step() {
            Ok(true) => {}
            Ok(false) => {
                self.finished = true;
                return None;
            }
            Err(error) => {
                self.finished = true;
                return Some(Err(error));
            }
        }

        let Some(input) = self.interface.current_step().cloned() else {
            self.finished = true;
            return Some(Err(Error::Interface(
                "backtest step has no current step".into(),
            )));
        };
        let Some(frame) = self.interface.current_frame().cloned() else {
            self.finished = true;
            return Some(Err(Error::Interface(
                "backtest step has no current frame".into(),
            )));
        };

        let step = BacktestStep {
            input,
            frame,
            timestamp: self.interface.now(),
        };

        if let Some(observer) = &mut self.observer {
            if let Err(error) = observer(self.interface, &step) {
                self.finished = true;
                return Some(Err(error));
            }
        }

        Some(Ok(step))
    }
}

impl<V: View> fmt::Debug for BacktestInterface<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BacktestInterface")
            .field("steps", &self.steps.len())
            .field("now", &self.now())
            .field("portfolio", &self.portfolio)
            .finish_non_exhaustive()
    }
}
