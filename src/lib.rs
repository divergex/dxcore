
// Re-export the types consumers need so they don't import ibapi directly.
pub use ibapi::accounts::{AccountPortfolioValue, AccountValue};
pub use ibapi::contracts::Contract;
pub use ibapi::market_data::historical::{Bar, BarTimestamp};

// ---------------------------------------------------------------------------
// Event — what the worker thread pushes to the UI
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum Event {
    /// Connection to TWS/Gateway established.
    Connected,
    /// Connection lost or fatal error. Contains the error description.
    Disconnected(String),
    /// An account metric (NetLiquidation, CashBalance, …).
    AccountValue(AccountValue),
    /// A non-zero portfolio position.
    Position(AccountPortfolioValue),
    /// Timestamp of the account snapshot.
    UpdateTime(String),
    /// Successfully fetched historical bars for a contract.
    HistoricalBars {
        contract_id: i32,
        bars: Vec<Bar>,
    },
    /// Historical data request failed for a contract.
    HistoricalError {
        contract_id: i32,
        error: String,
    },
}

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum Error {
    Connection(String),
    Subscription(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Connection(msg) => write!(f, "connection error: {msg}"),
            Error::Subscription(msg) => write!(f, "subscription error: {msg}"),
        }
    }
}

impl std::error::Error for Error {}

// ---------------------------------------------------------------------------
// Modules
// ---------------------------------------------------------------------------

pub mod core;
pub mod interface;
pub mod trading;
