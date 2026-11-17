use std::collections::HashMap;

use super::order::{
    Intent, Order, OrderError, OrderIntent, OrderPrice, OrderQuantity, OrderType, Side,
};

/// Turns a strategy's output into orders.
///
/// `O` is the strategy's output type and `I` its step type: an executor only
/// accepts an engine that translates exactly the output the strategy produces,
/// so a strategy stays free to emit [`Signal`](super::Signal),
/// [`OrderData`](super::OrderData), or anything an engine of yours handles.
///
/// `transform` runs once per step and returns `None` when the step implies no
/// order; the executor accumulates the rest through [`OrderEngine::create_output`]
/// and [`OrderEngine::append_output`].
pub trait OrderEngine<O, I> {
    /// What the executor ends up with.
    type Order;
    /// What the executor accumulates orders into.
    type Frame;
    /// Per-run scratch state, fresh for every `run`.
    type State: Default;

    /// Translate one strategy output.
    ///
    /// `date` is the step's ordinal from [`View::step_ord_key`], `None` when
    /// the view defines none.
    ///
    /// [`View::step_ord_key`]: super::View::step_ord_key
    fn transform(
        &self,
        output: O,
        step: &I,
        date: Option<i64>,
        state: &mut Self::State,
    ) -> Result<Option<Self::Order>, OrderError>;

    fn create_output(&self) -> Self::Frame;

    fn append_output(&self, frame: &mut Self::Frame, order: Self::Order, step: &I);
}

#[derive(Debug, Default)]
pub struct BaseOrderEngineState {
    pub positions: HashMap<String, f64>,
}

#[derive(Debug, Default)]
pub struct BaseOrderEngine {
    symbol: Option<String>,
}

impl BaseOrderEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_symbol(mut self, symbol: impl Into<String>) -> Self {
        self.symbol = Some(symbol.into());
        self
    }

    fn resolve_symbol(&self, symbol: Option<String>) -> Result<String, OrderError> {
        symbol
            .or_else(|| self.symbol.clone())
            .ok_or(OrderError::MissingSymbol)
    }
}

impl<O: OrderIntent, I> OrderEngine<O, I> for BaseOrderEngine {
    type Order = Order;
    type Frame = Vec<Order>;
    type State = BaseOrderEngineState;

    fn transform(
        &self,
        output: O,
        _step: &I,
        date: Option<i64>,
        state: &mut BaseOrderEngineState,
    ) -> Result<Option<Order>, OrderError> {
        let Some(intent) = output.into_intent() else {
            return Ok(None);
        };

        let (symbol, side, quantity, order_type, price) = match intent {
            Intent::Target { symbol, shares } => {
                if !shares.is_finite() {
                    return Err(OrderError::InvalidQuantity(shares));
                }
                let symbol = self.resolve_symbol(symbol)?;
                let held = state.positions.get(&symbol).copied().unwrap_or(0.0);
                let delta = shares - held;
                if delta == 0.0 {
                    return Ok(None);
                }
                state.positions.insert(symbol.clone(), shares);
                let side = if delta > 0.0 { Side::Buy } else { Side::Sell };
                let quantity = OrderQuantity::Abs(delta.abs());
                (symbol, side, quantity, OrderType::Market, OrderPrice::Mkt)
            }
            Intent::Order(data) => {
                let symbol = self.resolve_symbol(data.symbol)?;
                let Some((signed, side, quantity)) = split_quantity(data.quantity)? else {
                    return Ok(None);
                };
                validate_price(data.order_type, data.price)?;
                if matches!(data.quantity, OrderQuantity::Abs(_)) {
                    *state.positions.entry(symbol.clone()).or_default() += signed;
                }
                (symbol, side, quantity, data.order_type, data.price)
            }
        };

        Ok(Some(Order {
            date,
            symbol,
            side,
            quantity,
            order_type,
            price,
        }))
    }

    fn create_output(&self) -> Vec<Order> {
        Vec::new()
    }

    fn append_output(&self, frame: &mut Vec<Order>, order: Order, _step: &I) {
        frame.push(order);
    }
}

fn split_quantity(
    quantity: OrderQuantity,
) -> Result<Option<(f64, Side, OrderQuantity)>, OrderError> {
    let signed = match quantity {
        OrderQuantity::Abs(value) | OrderQuantity::Pct(value) => value,
    };

    if !signed.is_finite() {
        return Err(OrderError::InvalidQuantity(signed));
    }
    if signed == 0.0 {
        return Ok(None);
    }

    let side = if signed > 0.0 { Side::Buy } else { Side::Sell };
    let magnitude = match quantity {
        OrderQuantity::Abs(_) => OrderQuantity::Abs(signed.abs()),
        OrderQuantity::Pct(_) => OrderQuantity::Pct(signed.abs()),
    };

    Ok(Some((signed, side, magnitude)))
}

fn validate_price(order_type: OrderType, price: OrderPrice) -> Result<(), OrderError> {
    match (order_type, price) {
        (OrderType::Market, OrderPrice::Mkt) => Ok(()),
        (_, OrderPrice::Mkt) => Err(OrderError::IncoherentPrice { order_type, price }),
        (OrderType::Market, OrderPrice::Px(_)) => {
            Err(OrderError::IncoherentPrice { order_type, price })
        }
        (_, OrderPrice::Px(px)) if !px.is_finite() || px <= 0.0 => {
            Err(OrderError::InvalidPrice(px))
        }
        (_, OrderPrice::Px(_)) => Ok(()),
    }
}
