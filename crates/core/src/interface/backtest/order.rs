use std::sync::{Arc, MutexGuard};

use crate::Error;
use crate::interface::order::{
    Execution, OrderId, OrderInterface, OrderRecord, OrderStatus, OrderUpdate, OrderUpdateCallback,
};
use crate::trading::{
    BookOrder, BookOrderKind, Order, OrderBook, OrderPrice, OrderQuantity, OrderType, Price, View,
};

use super::BacktestInterface;

impl<V: View> BacktestInterface<V> {
    fn book(&self) -> Result<MutexGuard<'_, OrderBook>, Error> {
        self.book
            .lock()
            .map_err(|_| Error::Interface("order book lock poisoned".into()))
    }

    fn records(&self) -> Result<MutexGuard<'_, Vec<OrderRecord>>, Error> {
        self.records
            .lock()
            .map_err(|_| Error::Interface("order records lock poisoned".into()))
    }

    fn executions(&self) -> Result<MutexGuard<'_, Vec<Execution>>, Error> {
        self.executions
            .lock()
            .map_err(|_| Error::Interface("executions lock poisoned".into()))
    }

    fn callbacks(&self) -> Result<MutexGuard<'_, Vec<OrderUpdateCallback>>, Error> {
        self.callbacks
            .lock()
            .map_err(|_| Error::Interface("order callback lock poisoned".into()))
    }

    fn next_id(&self) -> Result<u64, Error> {
        let mut next = self
            .next_id
            .lock()
            .map_err(|_| Error::Interface("order id lock poisoned".into()))?;
        let id = *next;
        *next += 1;
        Ok(id)
    }

    fn to_book_order(&self, id: OrderId, order: &Order) -> Result<BookOrder, Error> {
        let quantity = match order.quantity {
            OrderQuantity::Abs(quantity) if quantity.is_finite() && quantity > 0.0 => quantity,
            quantity => {
                return Err(Error::Interface(format!(
                    "order quantity {quantity:?} cannot rest in the book"
                )));
            }
        };

        let kind = match order.order_type {
            OrderType::Market => BookOrderKind::Market,
            OrderType::Limit => BookOrderKind::Limit,
            OrderType::StopLoss => {
                return Err(Error::Interface(
                    "stop orders are not supported by the matching book".into(),
                ));
            }
        };
        let price = match kind {
            BookOrderKind::Market => None,
            BookOrderKind::Limit => match order.price {
                OrderPrice::Px(price) => Some(self.to_price(price)?),
                OrderPrice::Mkt => {
                    return Err(Error::Interface("limit order has no explicit price".into()));
                }
            },
        };

        Ok(BookOrder {
            id,
            side: order.side,
            kind,
            price,
            quantity,
        })
    }

    fn to_price(&self, price: f64) -> Result<Price, Error> {
        if !price.is_finite() || price <= 0.0 {
            return Err(Error::Interface(format!("invalid order price: {price}")));
        }

        let ticks = (price / self.tick_size as f64).round();
        if !ticks.is_finite() || ticks < i64::MIN as f64 || ticks > i64::MAX as f64 {
            return Err(Error::Interface(format!(
                "order price is outside the tick range: {price}"
            )));
        }
        Ok(Price::new(ticks as i64))
    }

    fn cash(&self, price: Price) -> f64 {
        price.ticks() as f64 * self.tick_size as f64
    }

    fn notify(&self, update: &OrderUpdate) -> Result<(), Error> {
        let callbacks = self.callbacks()?.clone();
        for callback in callbacks {
            callback(update);
        }
        Ok(())
    }
}

impl<V: View> OrderInterface for BacktestInterface<V> {
    fn submit_order(&self, order: Order) -> Result<OrderId, Error> {
        let id = OrderId(self.next_id()?);
        let book_order = self.to_book_order(id, &order)?;
        let report = self
            .book()?
            .submit(book_order)
            .map_err(|error| Error::Interface(error.to_string()))?;

        let timestamp = self.now().unwrap_or(0);
        let quantity = match order.quantity {
            OrderQuantity::Abs(quantity) => quantity,
            OrderQuantity::Pct(_) => unreachable!("validated by to_book_order"),
        };
        let filled = report
            .trades
            .iter()
            .map(|trade| trade.quantity)
            .sum::<f64>();
        let status = if filled + f64::EPSILON >= quantity {
            OrderStatus::Filled
        } else if filled > f64::EPSILON {
            OrderStatus::PartiallyFilled
        } else if report.rested {
            OrderStatus::Pending
        } else {
            OrderStatus::Rejected("no liquidity".into())
        };

        let mut updates = Vec::new();
        let mut taker_executions = Vec::new();
        {
            let mut records = self.records()?;
            let mut executions = self.executions()?;

            records.push(OrderRecord {
                id,
                order: order.clone(),
                status: status.clone(),
                filled_quantity: filled,
            });

            for trade in &report.trades {
                let execution = Execution {
                    order_id: id,
                    timestamp,
                    price: self.cash(trade.price),
                    quantity: trade.quantity,
                };
                taker_executions.push(execution.clone());
                executions.push(execution);

                let Some(maker) = records
                    .iter_mut()
                    .find(|record| record.id == trade.maker_order_id)
                else {
                    continue;
                };
                maker.filled_quantity += trade.quantity;
                maker.status = match maker.order.quantity {
                    OrderQuantity::Abs(quantity)
                        if maker.filled_quantity + f64::EPSILON >= quantity =>
                    {
                        OrderStatus::Filled
                    }
                    _ => OrderStatus::PartiallyFilled,
                };

                let execution = Execution {
                    order_id: maker.id,
                    timestamp,
                    price: self.cash(trade.price),
                    quantity: trade.quantity,
                };
                executions.push(execution.clone());
                updates.push(OrderUpdate {
                    order_id: maker.id,
                    status: maker.status.clone(),
                    filled_quantity: maker.filled_quantity,
                    executions: vec![execution],
                });
            }
        }

        updates.push(OrderUpdate {
            order_id: id,
            status,
            filled_quantity: filled,
            executions: taker_executions,
        });

        for update in &updates {
            self.notify(update)?;
        }

        Ok(id)
    }

    fn orders(&self) -> Result<Vec<OrderRecord>, Error> {
        Ok(self.records()?.clone())
    }

    fn executions(&self) -> Result<Vec<Execution>, Error> {
        Ok(self.executions()?.clone())
    }

    fn on_order_update(
        &self,
        callback: Box<dyn Fn(&OrderUpdate) + Send + Sync>,
    ) -> Result<(), Error> {
        self.callbacks()?.push(Arc::from(callback));
        Ok(())
    }
}
