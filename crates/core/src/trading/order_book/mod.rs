mod levels;

use std::collections::HashMap;
use std::fmt;

use super::{OrderId, Side};
use levels::{LevelId, NodeId, PriceLevels};

/// Price in integer ticks.
///
/// Integer prices avoid the equality/hash problems of `f64` and make tick
/// comparison exact. Convert from currency prices at the adapter boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Price(i64);

impl Price {
    pub const fn new(ticks: i64) -> Self {
        Self(ticks)
    }

    pub const fn ticks(self) -> i64 {
        self.0
    }
}

impl fmt::Display for Price {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Whether an order rests on the book or takes liquidity only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookOrderKind {
    Limit,
    Market,
}

/// An order submitted to the book.
///
/// `price` is required for limit orders and ignored for market orders.
#[derive(Debug, Clone, PartialEq)]
pub struct BookOrder {
    pub id: OrderId,
    pub side: Side,
    pub kind: BookOrderKind,
    pub price: Option<Price>,
    pub quantity: f64,
}

impl BookOrder {
    pub fn limit(id: OrderId, side: Side, price: Price, quantity: f64) -> Self {
        Self {
            id,
            side,
            kind: BookOrderKind::Limit,
            price: Some(price),
            quantity,
        }
    }

    pub fn market(id: OrderId, side: Side, quantity: f64) -> Self {
        Self {
            id,
            side,
            kind: BookOrderKind::Market,
            price: None,
            quantity,
        }
    }
}

/// One execution between a taker and a resting maker.
#[derive(Debug, Clone, PartialEq)]
pub struct Trade {
    pub taker_order_id: OrderId,
    pub maker_order_id: OrderId,
    pub price: Price,
    pub quantity: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FillReport {
    pub trades: Vec<Trade>,
    /// Quantity not filled. For a rested limit order this is the open quantity;
    /// for a market order it is discarded.
    pub remaining: f64,
    /// Whether the unfilled remainder was added to the book.
    pub rested: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DepthLevel {
    pub price: Price,
    pub quantity: f64,
    pub orders: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OrderBookError {
    DuplicateOpenOrder(OrderId),
    InvalidQuantity(f64),
    InvalidTickSize(i64),
    MissingLimitPrice,
}

impl fmt::Display for OrderBookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateOpenOrder(id) => {
                write!(f, "order {} is already open", id.0)
            }
            Self::InvalidQuantity(quantity) => {
                write!(f, "invalid order quantity: {quantity}")
            }
            Self::InvalidTickSize(tick_size) => {
                write!(f, "invalid tick size: {tick_size}")
            }
            Self::MissingLimitPrice => write!(f, "limit order has no price"),
        }
    }
}

impl std::error::Error for OrderBookError {}

#[derive(Debug, Clone, Copy)]
struct OrderLocation {
    side: Side,
    level: LevelId,
    node: NodeId,
}

/// A price-time-priority order book.
///
/// Prices are kept as integer tick indices. `tick_size` converts raw integer
/// prices through [`normalize_price`](Self::normalize_price). The implementation
/// is single-writer; its arena and index layers are the replacement seam for a
/// future lock-free version.
#[derive(Debug)]
pub struct OrderBook {
    tick_size: i64,
    bids: PriceLevels,
    asks: PriceLevels,
    orders: HashMap<OrderId, OrderLocation>,
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new(1).expect("default tick size is valid")
    }
}

impl OrderBook {
    pub fn new(tick_size: i64) -> Result<Self, OrderBookError> {
        if tick_size <= 0 {
            return Err(OrderBookError::InvalidTickSize(tick_size));
        }

        Ok(Self {
            tick_size,
            bids: PriceLevels::new(Side::Buy),
            asks: PriceLevels::new(Side::Sell),
            orders: HashMap::new(),
        })
    }

    pub fn tick_size(&self) -> i64 {
        self.tick_size
    }

    /// Convert a raw integer price into a tick index.
    ///
    /// Uses Euclidean division, so values are floored toward negative infinity.
    pub fn normalize_price(&self, raw: i64) -> Price {
        Price::new(raw.div_euclid(self.tick_size))
    }

    /// Submit an order, matching any crossing liquidity first.
    ///
    /// A limit remainder rests at its limit price; a market remainder is
    /// discarded. IDs must be unique among open orders.
    pub fn submit(&mut self, order: BookOrder) -> Result<FillReport, OrderBookError> {
        validate_order(&order)?;
        if self.orders.contains_key(&order.id) {
            return Err(OrderBookError::DuplicateOpenOrder(order.id));
        }

        let opposite = match order.side {
            Side::Buy => Side::Sell,
            Side::Sell => Side::Buy,
        };
        let mut remaining = order.quantity;
        let mut trades = Vec::new();

        while remaining > f64::EPSILON {
            let Some((price, level)) = self.side(opposite).best() else {
                break;
            };

            if let Some(limit) = order.price {
                let crosses = match order.side {
                    Side::Buy => price <= limit,
                    Side::Sell => price >= limit,
                };
                if !crosses {
                    break;
                }
            }

            let Some((maker_id, maker_quantity)) = self.side(opposite).head(level) else {
                break;
            };
            let quantity = remaining.min(maker_quantity);
            if quantity <= f64::EPSILON {
                break;
            }

            remaining -= quantity;
            trades.push(Trade {
                taker_order_id: order.id,
                maker_order_id: maker_id,
                price,
                quantity,
            });
            let filled = self.side_mut(opposite).fill_head(level, quantity);
            if filled {
                self.orders.remove(&maker_id);
            }
            if self.side(opposite).level_orders(level) == 0 {
                self.handle_empty_level(opposite, level);
            }
        }

        let mut rested = false;
        if order.kind == BookOrderKind::Limit && remaining > f64::EPSILON {
            let price = order.price.expect("limit price validated above");
            let (level, node) = self.side_mut(order.side).append(price, order.id, remaining);
            self.orders.insert(
                order.id,
                OrderLocation {
                    side: order.side,
                    level,
                    node,
                },
            );
            rested = true;
        }

        Ok(FillReport {
            trades,
            remaining,
            rested,
        })
    }

    /// Cancel an open order, returning its remaining quantity if present.
    pub fn cancel(&mut self, id: OrderId) -> Option<BookOrder> {
        let location = self.orders.remove(&id)?;
        let (price, quantity, empty) = self
            .side_mut(location.side)
            .remove(location.level, location.node)?;

        if empty {
            self.handle_empty_level(location.side, location.level);
        }

        Some(BookOrder::limit(id, location.side, price, quantity))
    }

    /// Look up an open order without removing it.
    pub fn order(&self, id: OrderId) -> Option<BookOrder> {
        let location = *self.orders.get(&id)?;
        let (price, quantity) = self
            .side(location.side)
            .order_info(location.level, location.node)?;

        Some(BookOrder::limit(id, location.side, price, quantity))
    }

    pub fn best_bid(&self) -> Option<Price> {
        self.bids.best().map(|(price, _)| price)
    }

    pub fn best_ask(&self) -> Option<Price> {
        self.asks.best().map(|(price, _)| price)
    }

    pub fn depth(&self, side: Side) -> Vec<DepthLevel> {
        self.side(side)
            .depth()
            .into_iter()
            .map(|(price, quantity, orders)| DepthLevel {
                price,
                quantity,
                orders,
            })
            .collect()
    }

    pub fn level_quantity(&self, side: Side, price: Price) -> Option<f64> {
        self.side(side).level_quantity(price)
    }

    pub fn open_orders(&self) -> usize {
        self.orders.len()
    }

    pub fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }

    fn side(&self, side: Side) -> &PriceLevels {
        match side {
            Side::Buy => &self.bids,
            Side::Sell => &self.asks,
        }
    }

    fn side_mut(&mut self, side: Side) -> &mut PriceLevels {
        match side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        }
    }

    fn handle_empty_level(&mut self, side: Side, level: LevelId) {
        self.side_mut(side).remove_level(level);
    }
}

fn validate_order(order: &BookOrder) -> Result<(), OrderBookError> {
    if !order.quantity.is_finite() || order.quantity <= 0.0 {
        return Err(OrderBookError::InvalidQuantity(order.quantity));
    }
    if order.kind == BookOrderKind::Limit && order.price.is_none() {
        return Err(OrderBookError::MissingLimitPrice);
    }
    Ok(())
}
