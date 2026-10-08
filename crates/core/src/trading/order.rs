use std::fmt;

use serde::{Deserialize, Serialize};

/// Direction of an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
}

/// Size of an order.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum OrderQuantity {
    /// Fraction of the account (`0.5` = 50 %).
    ///
    /// The engine passes the fraction through unresolved: turning it into
    /// shares needs account equity, which belongs to the execution layer, not
    /// to the translation layer.
    Pct(f64),
    /// Absolute number of shares.
    ///
    /// A strategy may sign it (`-100.0` = sell 100); [`Order`] normalises the
    /// sign into a magnitude plus a [`Side`].
    Abs(f64),
}

/// How an order is specified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit,
    StopLoss,
}

/// Price an order is specified at.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum OrderPrice {
    /// Execute at whatever the market gives.
    Mkt,
    /// Execute at a fixed price (a limit price, or a stop level).
    Px(f64),
}

impl fmt::Display for OrderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderType::Market => write!(f, "market"),
            OrderType::Limit => write!(f, "limit"),
            OrderType::StopLoss => write!(f, "stop loss"),
        }
    }
}

impl fmt::Display for OrderPrice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderPrice::Mkt => write!(f, "the market price"),
            OrderPrice::Px(px) => write!(f, "{px}"),
        }
    }
}

/// An order a strategy authored directly.
///
/// Unlike [`Signal`], this is not a target: the engine validates it and passes
/// it through. `Abs` sizes are booked onto the engine's position ledger, so
/// repeated specs accumulate the way the orders themselves would.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderData {
    /// Instrument to trade. Falls back to
    /// [`BaseOrderEngine::with_symbol`](super::BaseOrderEngine::with_symbol).
    pub symbol: Option<String>,
    /// Signed size: negative trades out, positive trades in.
    pub quantity: OrderQuantity,
    pub order_type: OrderType,
    pub price: OrderPrice,
}

/// The position a strategy wants to hold in one instrument after a step.
///
/// The engine diffs consecutive targets per symbol and trades the difference,
/// so repeating a target produces no order. Holding is expressed by simply not
/// emitting a signal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Signal {
    /// Instrument the target refers to. Falls back to
    /// [`BaseOrderEngine::with_symbol`](super::BaseOrderEngine::with_symbol).
    pub symbol: Option<String>,
    /// Shares the strategy wants to hold once this step is done.
    pub shares: f64,
}

/// A validated order, as produced by an [`OrderEngine`](super::OrderEngine).
#[derive(Debug, Clone, PartialEq)]
pub struct Order {
    /// Ordinal of the step that produced it, from [`View::step_ord_key`]
    /// (epoch days for [`DailyView`] and [`PanelView`]); `None` for views that
    /// define no ordering, such as [`TickView`].
    ///
    /// [`View::step_ord_key`]: super::View::step_ord_key
    /// [`DailyView`]: super::DailyView
    /// [`PanelView`]: super::PanelView
    /// [`TickView`]: super::TickView
    pub date: Option<i64>,
    pub symbol: String,
    pub side: Side,
    /// Unsigned: the direction lives in [`Order::side`].
    pub quantity: OrderQuantity,
    pub order_type: OrderType,
    pub price: OrderPrice,
}

/// What a strategy output means to an engine.
#[derive(Debug, Clone, PartialEq)]
pub enum Intent {
    /// Trade towards a position, sized by the difference to the last target.
    Target { symbol: Option<String>, shares: f64 },
    /// An explicit order spec, passed through after validation.
    Order(OrderData),
}

/// A strategy output a [`BaseOrderEngine`](super::BaseOrderEngine) understands.
///
/// Implemented for [`Signal`], [`OrderData`], and `Option<T>` of either — so a
/// strategy can say "nothing to do this step" with `None`. Engines of your own
/// can define their own output contract instead.
pub trait OrderIntent {
    /// `None` when the step implies no order at all.
    fn into_intent(self) -> Option<Intent>;
}

impl OrderIntent for Signal {
    fn into_intent(self) -> Option<Intent> {
        Some(Intent::Target { symbol: self.symbol, shares: self.shares })
    }
}

impl OrderIntent for OrderData {
    fn into_intent(self) -> Option<Intent> {
        Some(Intent::Order(self))
    }
}

impl<T: OrderIntent> OrderIntent for Option<T> {
    fn into_intent(self) -> Option<Intent> {
        self.and_then(OrderIntent::into_intent)
    }
}

/// Why an engine refused to produce an order.
#[derive(Debug, Clone, PartialEq)]
pub enum OrderError {
    /// Neither the strategy output nor the engine names an instrument.
    MissingSymbol,
    /// Quantity is not a finite number.
    InvalidQuantity(f64),
    /// Fixed price is not a finite, positive number.
    InvalidPrice(f64),
    /// Order type and price spec disagree: a market order must carry
    /// [`OrderPrice::Mkt`], a limit or stop order a concrete price.
    IncoherentPrice { order_type: OrderType, price: OrderPrice },
}

impl fmt::Display for OrderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderError::MissingSymbol => write!(
                f,
                "order has no instrument: name it in the strategy output or \
                 configure the engine with `with_symbol`"
            ),
            OrderError::InvalidQuantity(quantity) => {
                write!(f, "invalid order quantity: {quantity}")
            }
            OrderError::InvalidPrice(price) => write!(f, "invalid order price: {price}"),
            OrderError::IncoherentPrice { order_type, price } => write!(
                f,
                "a {order_type} order cannot be specified at {price}"
            ),
        }
    }
}

impl std::error::Error for OrderError {}
