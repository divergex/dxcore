use crate::core::Portfolio;
use crate::interface::AccountInterface;
use crate::trading::View;
use crate::{Error, Event};

use super::BacktestInterface;

impl<V: View> AccountInterface for BacktestInterface<V> {
    fn portfolio(&self, _account_id: &str) -> Result<Portfolio, Error> {
        Ok(self.portfolio.clone())
    }

    /// No live account feed exists during a backtest.
    fn listen_async(&self, _account_id: &str) -> Result<(), Error> {
        Ok(())
    }

    /// No account events are produced during a backtest.
    fn events(&self, _account_id: &str) -> Result<Vec<Event>, Error> {
        Ok(Vec::new())
    }
}
