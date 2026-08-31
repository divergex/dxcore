
#[cfg(feature = "ibkr")]
pub use ibapi::accounts::{AccountPortfolioValue, AccountValue};
#[cfg(feature = "ibkr")]
pub use ibapi::contracts::Contract;
#[cfg(feature = "ibkr")]
pub use ibapi::market_data::historical::{Bar, BarTimestamp};


#[derive(Debug)]
pub enum Event {
    /// Connection to TWS/Gateway established.
    Connected,
    /// Connection lost or fatal error. Contains the error description.
    Disconnected(String),
    /// An account metric (NetLiquidation, CashBalance, …).
    #[cfg(feature = "ibkr")]
    AccountValue(AccountValue),
    /// A non-zero portfolio position.
    #[cfg(feature = "ibkr")]
    Position(AccountPortfolioValue),
    /// Timestamp of the account snapshot.
    UpdateTime(String),
    /// Successfully fetched historical bars for a contract.
    #[cfg(feature = "ibkr")]
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


#[derive(Debug)]
pub enum Error {
    Connection(String),
    Subscription(String),
    Http(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Connection(msg) => write!(f, "connection error: {msg}"),
            Error::Subscription(msg) => write!(f, "subscription error: {msg}"),
            Error::Http(msg) => write!(f, "http error: {msg}"),
        }
    }
}

impl std::error::Error for Error {}

pub mod core;
pub mod dataframe;
pub mod interface;
pub mod network;
pub mod serialization;
pub mod trading;
#[cfg(feature = "strategies")]
pub mod strategies;

pub use dataframe::DataFrame;
