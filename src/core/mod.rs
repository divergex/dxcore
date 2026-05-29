use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Instrument
// ---------------------------------------------------------------------------

/// A tradable instrument independent of any broker-specific representation.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Instrument {
    /// Broker-issued unique contract identifier.
    pub contract_id: i32,
    /// Ticker symbol (e.g. "AAPL", "ES").
    pub symbol: String,
    /// Security type code (e.g. "STK", "OPT", "FUT").
    pub security_type: String,
    /// Primary exchange (e.g. "SMART", "NYSE", "CME").
    pub exchange: String,
    /// Trading currency (e.g. "USD", "EUR").
    pub currency: String,
}

impl std::fmt::Display for Instrument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.symbol, self.security_type)
    }
}

// ---------------------------------------------------------------------------
// AccountMetric
// ---------------------------------------------------------------------------

/// A single keyed account metric (e.g. NetLiquidation, CashBalance).
#[derive(Debug, Clone)]
pub struct AccountMetric {
    pub key: String,
    pub value: String,
    pub currency: String,
}

// ---------------------------------------------------------------------------
// Portfolio
// ---------------------------------------------------------------------------

/// The aggregate state of an account: metrics and holdings.
///
/// Holdings map a contract ID to the instrument and the quantity held.
/// A zero or absent quantity means the instrument is not held.
/// Portfolio is independent of market state — it does not store prices or P&L.
#[derive(Debug, Clone, Default)]
pub struct Portfolio {
    /// Latest account metrics keyed by metric name.
    pub metrics: HashMap<String, AccountMetric>,
    /// Holdings keyed by contract_id: (instrument, quantity).
    holdings: HashMap<i32, (Instrument, f64)>,
}

impl Portfolio {
    /// Insert or update an account metric.
    pub fn upsert_metric(&mut self, key: String, value: String, currency: String) {
        self.metrics.insert(
            key.clone(),
            AccountMetric {
                key,
                value,
                currency,
            },
        );
    }

    /// Set the quantity held for an instrument. A quantity of zero removes the holding.
    pub fn set_holding(&mut self, instrument: Instrument, quantity: f64) {
        if quantity == 0.0 {
            self.holdings.remove(&instrument.contract_id);
        } else {
            self.holdings
                .insert(instrument.contract_id, (instrument, quantity));
        }
    }

    /// Look up the instrument for a given contract ID.
    pub fn instrument(&self, contract_id: i32) -> Option<&Instrument> {
        self.holdings.get(&contract_id).map(|(inst, _)| inst)
    }

    /// Look up the quantity held for a given contract ID.
    pub fn quantity(&self, contract_id: i32) -> Option<f64> {
        self.holdings.get(&contract_id).map(|(_, qty)| *qty)
    }

    /// Number of distinct instruments held.
    pub fn holding_count(&self) -> usize {
        self.holdings.len()
    }

    /// Iterate over all holdings.
    pub fn holdings(&self) -> impl Iterator<Item = (&Instrument, f64)> {
        self.holdings.values().map(|(inst, qty)| (inst, *qty))
    }
}

// ---------------------------------------------------------------------------
// InstrumentStore
// ---------------------------------------------------------------------------

/// A bidirectional registry of instruments: look up by contract ID or symbol.
#[derive(Debug, Clone, Default)]
pub struct InstrumentStore {
    by_id: HashMap<i32, Instrument>,
    by_symbol: HashMap<String, Instrument>,
}

impl InstrumentStore {
    /// Register an instrument. Replaces any existing entry with the same
    /// contract_id or symbol.
    pub fn insert(&mut self, instrument: Instrument) {
        self.by_id
            .insert(instrument.contract_id, instrument.clone());
        self.by_symbol
            .insert(instrument.symbol.clone(), instrument);
    }

    /// Look up an instrument by its contract ID.
    pub fn get(&self, contract_id: i32) -> Option<&Instrument> {
        self.by_id.get(&contract_id)
    }

    /// Look up an instrument by its symbol.
    pub fn get_by_symbol(&self, symbol: &str) -> Option<&Instrument> {
        self.by_symbol.get(symbol)
    }

    /// Number of registered instruments.
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
