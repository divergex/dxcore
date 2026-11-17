use crate::interface::{StepSnapshot, TradingService};
use crate::network::services::{ClassService, ServiceError};
use crate::trading::View;

use super::BacktestInterface;

pub struct BacktestService<V: View> {
    service: ClassService<BacktestInterface<V>>,
}

impl<V> BacktestService<V>
where
    V: View + Send + Sync + 'static,
    V::Item: Send + Sync + 'static,
{
    pub fn new(backtest: BacktestInterface<V>) -> Self {
        let service = TradingService::named("backtest", backtest)
            .service()
            .with_set("step", |bt: &mut BacktestInterface<V>, _: ()| {
                if !bt
                    .step()
                    .map_err(|e| ServiceError::Internal(e.to_string()))?
                {
                    return Ok(None);
                }

                let frame = bt
                    .current_frame()
                    .ok_or_else(|| ServiceError::Internal("step has no current frame".into()))?
                    .clone();
                Ok(Some(StepSnapshot {
                    timestamp: bt.now(),
                    frame: crate::DataFrame::new(frame),
                }))
            })
            .with_get("now", |bt: &BacktestInterface<V>, _: ()| Ok(bt.now()));
        Self { service }
    }

    pub fn service(self) -> ClassService<BacktestInterface<V>> {
        self.service
    }
}
