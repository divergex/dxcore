use polars::prelude::*;

use crate::Error;
use crate::core::Instrument;
use crate::interface::quote::Quote;
use crate::interface::{MarketInterface, Span};
use crate::trading::View;

use super::BacktestInterface;

impl<V: View> BacktestInterface<V> {
    pub(super) fn quote_at_current(&self, levels: usize) -> Result<Option<Quote>, Error> {
        let Some(frame) = self.current_frame() else {
            return Ok(None);
        };
        let Some(timestamp) = self.now() else {
            return Ok(None);
        };

        Ok(self.quote_schema.quote(frame, timestamp, levels))
    }
}

impl<V: View> MarketInterface for BacktestInterface<V> {
    /// Returns the current view step's frame unchanged.
    ///
    /// The view owns timestamp, symbol, and column slicing, so `contract`,
    /// `bar_size`, and `duration` are not applied here.
    fn market_history(
        &self,
        _contract: &Instrument,
        _bar_size: Span,
        _duration: Span,
    ) -> PolarsResult<DataFrame> {
        Ok(self
            .current_frame()
            .cloned()
            .unwrap_or_else(|| self.empty.clone()))
    }

    /// Reads the quote from the current view step.
    ///
    /// The view should isolate one instrument per step; otherwise the first row
    /// supplies the quote.
    fn quote(&self, _contract: &Instrument, levels: usize) -> Result<Option<Quote>, Error> {
        self.quote_at_current(levels)
    }
}
