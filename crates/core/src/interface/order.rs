pub use crate::trading::OrderId;

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::Error;
use crate::interface::quote::Quote;
use crate::trading::{Order, OrderQuantity, OrderType, Side};

/// Lifecycle state of an accepted order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    Pending,
    PartiallyFilled,
    Filled,
    Rejected(String),
}

impl OrderStatus {
    pub fn is_open(&self) -> bool {
        matches!(self, Self::Pending | Self::PartiallyFilled)
    }
}

/// An order as stored by an [`OrderInterface`], with its current lifecycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderRecord {
    pub id: OrderId,
    pub order: Order,
    pub status: OrderStatus,
    /// Cumulative quantity filled so far.
    pub filled_quantity: f64,
}

/// One executed quantity at a specific price and time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Execution {
    pub order_id: OrderId,
    pub timestamp: i64,
    pub price: f64,
    pub quantity: f64,
}

/// Change published to [`OrderInterface::on_order_update`] callbacks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderUpdate {
    pub order_id: OrderId,
    pub status: OrderStatus,
    pub filled_quantity: f64,
    /// Executions produced by this update, empty for pending/rejected updates.
    pub executions: Vec<Execution>,
}

/// A fill proposed by an [`ExecutionEngine`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fill {
    pub timestamp: i64,
    pub price: f64,
    pub quantity: f64,
}

/// What an execution engine wants an [`OrderInterface`] to do with an order.
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionResult {
    /// Leave the order open; the engine may fill it on a later step.
    Pending,
    /// Apply these fills and keep the order open.
    PartiallyFilled(Vec<Fill>),
    /// Apply these fills and mark the order complete.
    Filled(Vec<Fill>),
    /// Reject the order.
    Rejected(String),
}

/// Market state supplied to an [`ExecutionEngine`].
#[derive(Debug, Clone, Copy)]
pub struct ExecutionContext<'a> {
    /// Current backtest timestamp, when one is set.
    pub timestamp: Option<i64>,
    /// Current best bid/ask book, when the market has one for the order.
    pub quote: Option<&'a Quote>,
}

/// Decides whether, when, and at what price an order executes.
///
/// Implementations are called when an order is submitted and again on every
/// backtest step for orders that remain open.
pub trait ExecutionEngine: Send + Sync {
    fn execute(
        &self,
        order: &Order,
        context: &ExecutionContext<'_>,
    ) -> Result<ExecutionResult, Error>;
}

impl ExecutionEngine for Box<dyn ExecutionEngine> {
    fn execute(
        &self,
        order: &Order,
        context: &ExecutionContext<'_>,
    ) -> Result<ExecutionResult, Error> {
        (**self).execute(order, context)
    }
}

/// Fills market orders immediately at the best available quote.
///
/// Buy orders cross at the best ask and sell orders at the best bid. A
/// midprice-only quote supplies `bid == ask`, so both sides use the midprice.
/// Limit and stop orders, percentage-sized orders, and orders without a quote
/// remain pending for another engine or a later step.
#[derive(Debug, Clone, Copy, Default)]
pub struct ImmediateExecutionEngine;

impl ImmediateExecutionEngine {
    pub fn new() -> Self {
        Self
    }
}

impl ExecutionEngine for ImmediateExecutionEngine {
    fn execute(
        &self,
        order: &Order,
        context: &ExecutionContext<'_>,
    ) -> Result<ExecutionResult, Error> {
        if order.order_type != OrderType::Market {
            return Ok(ExecutionResult::Pending);
        }

        let Some(quote) = context.quote else {
            return Ok(ExecutionResult::Pending);
        };
        let Some(level) = quote.best() else {
            return Ok(ExecutionResult::Pending);
        };

        let price = match order.side {
            Side::Buy => level.ask.or_else(|| level.mid()),
            Side::Sell => level.bid.or_else(|| level.mid()),
        };
        let Some(price) = price else {
            return Ok(ExecutionResult::Pending);
        };

        let quantity = match order.quantity {
            OrderQuantity::Abs(quantity) => quantity,
            OrderQuantity::Pct(_) => return Ok(ExecutionResult::Pending),
        };

        if !price.is_finite() || price <= 0.0 {
            return Ok(ExecutionResult::Rejected(format!(
                "invalid execution price: {price}"
            )));
        }
        if !quantity.is_finite() || quantity <= 0.0 {
            return Ok(ExecutionResult::Rejected(format!(
                "invalid execution quantity: {quantity}"
            )));
        }

        Ok(ExecutionResult::Filled(vec![Fill {
            timestamp: quote.timestamp,
            price,
            quantity,
        }]))
    }
}

/// A sink for orders and their execution updates.
pub trait OrderInterface {
    /// Accept and store an order, returning its identifier.
    fn submit_order(&self, _order: Order) -> Result<OrderId, Error> {
        Err(Error::Interface(
            "order submission is not supported by this interface".into(),
        ))
    }

    /// All orders accepted so far, in submission order.
    fn orders(&self) -> Result<Vec<OrderRecord>, Error> {
        Err(Error::Interface(
            "order listing is not supported by this interface".into(),
        ))
    }

    /// All executions produced so far, in execution order.
    fn executions(&self) -> Result<Vec<Execution>, Error> {
        Err(Error::Interface(
            "execution listing is not supported by this interface".into(),
        ))
    }

    /// Register a callback invoked for every fill or rejection.
    fn on_order_update(
        &self,
        _callback: Box<dyn Fn(&OrderUpdate) + Send + Sync>,
    ) -> Result<(), Error> {
        Err(Error::Interface(
            "order updates are not supported by this interface".into(),
        ))
    }
}

pub(crate) type OrderUpdateCallback = Arc<dyn Fn(&OrderUpdate) + Send + Sync>;
