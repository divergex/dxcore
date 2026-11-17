use crate::interface::{HistoryArgs, MarketInterface, OrderInterface, QuoteArgs};
use crate::network::services::{ClassService, ServiceError};
use crate::trading::Order;

pub struct TradingService<T> {
    service: ClassService<T>,
}

impl<T> TradingService<T>
where
    T: MarketInterface + OrderInterface + Send + Sync + 'static,
{
    pub fn new(inner: T) -> Self {
        Self::named("trading", inner)
    }

    pub fn named(name: impl Into<String>, inner: T) -> Self {
        let service = ClassService::new(name, inner)
            .with_get("market_history", |t: &T, args: HistoryArgs| {
                t.market_history(&args.instrument, args.bar_size, args.duration)
                    .map(crate::DataFrame::new)
                    .map_err(|e| ServiceError::Internal(e.to_string()))
            })
            .with_get("quote", |t: &T, args: QuoteArgs| {
                t.quote(&args.instrument, args.levels)
                    .map_err(|e| ServiceError::Internal(e.to_string()))
            })
            .with_post("submit_order", |t: &mut T, order: Order| {
                t.submit_order(order)
                    .map_err(|e| ServiceError::BadValue(e.to_string()))
            })
            .with_get("orders", |t: &T, _: ()| {
                t.orders().map_err(|e| ServiceError::Internal(e.to_string()))
            })
            .with_get("executions", |t: &T, _: ()| {
                t.executions()
                    .map_err(|e| ServiceError::Internal(e.to_string()))
            });
        Self { service }
    }

    pub fn service(self) -> ClassService<T> {
        self.service
    }
}
