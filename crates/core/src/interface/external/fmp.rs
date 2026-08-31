use reqwest::blocking::Client;
use serde::Deserialize;

use crate::Error;

const BASE_URL: &str = "https://financialmodelingprep.com/stable";

pub struct FmpClient {
    api_key: String,
}

impl FmpClient {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }

    pub fn from_env() -> Result<Self, Error> {
        let key = std::env::var("FMP_API_KEY")
            .map_err(|_| Error::Connection("FMP_API_KEY not set".into()))?;
        Ok(Self::new(key))
    }

    /// Current market cap, price, and currency for a ticker.
    pub fn profile(&self, symbol: &str) -> Result<Profile, Error> {
        let url = format!("{BASE_URL}/profile?symbol={symbol}&apikey={}", self.api_key);
        let mut results = get::<Profile>(&url)?;
        results.pop().ok_or_else(|| Error::Http(format!("no profile found for {symbol}")))
    }

    /// Annual balance sheet statements (most recent first).
    pub fn balance_sheet(&self, symbol: &str) -> Result<Vec<BalanceSheet>, Error> {
        let url = format!(
            "{BASE_URL}/balance-sheet-statement?symbol={symbol}&period=annual&apikey={}",
            self.api_key
        );
        get(&url)
    }

    /// Annual income statements (most recent first).
    pub fn income_statement(&self, symbol: &str) -> Result<Vec<IncomeStatement>, Error> {
        let url = format!(
            "{BASE_URL}/income-statement?symbol={symbol}&period=annual&apikey={}",
            self.api_key
        );
        get(&url)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub symbol: String,
    pub price: Option<f64>,
    pub market_cap: Option<f64>,
    pub currency: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceSheet {
    pub symbol: String,
    pub date: String,
    pub period: String,
    pub total_assets: Option<f64>,
    pub total_liabilities: Option<f64>,
    pub total_stockholders_equity: Option<f64>,
    pub total_equity: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomeStatement {
    pub symbol: String,
    pub date: String,
    pub period: String,
    pub revenue: Option<f64>,
    pub operating_income: Option<f64>,
    pub net_income: Option<f64>,
    pub weighted_average_shs_out: Option<f64>,
}

fn get<T: for<'de> Deserialize<'de>>(url: &str) -> Result<Vec<T>, Error> {
    let client = Client::new();
    client
        .get(url)
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.json())
        .map_err(|e| Error::Http(e.to_string()))
}
