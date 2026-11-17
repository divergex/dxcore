//! Yahoo Finance market-data interface.
//!
//! Rust port of `dxlib.interfaces.external.yfinance.YFinance`. The Python
//! client exposes `quote`/`historical`/`symbols`; those are inherent methods
//! here, and [`MarketInterface::market_history`] adapts the trailing-duration
//! trait to the same chart endpoint.
//!
//! Endpoints:
//! * `v8/finance/chart/{symbol}` — daily OHLCV plus adjusted close
//! * `v7/finance/quote` — delayed quotes for N symbols
//! * `v1/finance/search` — symbol lookup
//!
//! Yahoo gates the quote endpoint on a session cookie plus a `crumb` token.
//! The crumb is scraped from the quote page (falling back to
//! `v1/test/getcrumb`) and cached for the lifetime of the interface, which is
//! why the struct needs interior mutability despite the `&self` trait methods.
//!
//! Frames use a `date` column of polars type `Date` (days since the Unix
//! epoch), matching what [`crate::trading::DailyView`] expects. Adjusted close
//! is reported alongside raw close, which the Python client did not expose.

use std::cell::RefCell;
use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp, ToSpan};
use polars::prelude::*;
use reqwest::blocking::Client;
use reqwest::header::{
    ACCEPT, ACCEPT_LANGUAGE, COOKIE, HeaderMap, HeaderValue, REFERER, USER_AGENT,
};
use serde::Deserialize;

use crate::Error;
use crate::core::Instrument;
use crate::interface::MarketInterface;

const BASE_URL: &str = "https://query2.finance.yahoo.com";
const QUOTE_PAGE: &str = "https://finance.yahoo.com/quote/AAPL";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const BROWSER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36";
const SECONDS_PER_DAY: i64 = 86_400;

/// Bar interval accepted by the Yahoo chart endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interval {
    Minute1,
    Minute5,
    Minute15,
    Minute30,
    Minute60,
    Day1,
    Week1,
    Month1,
    ThreeMonth1,
}

impl Interval {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Minute1 => "1m",
            Self::Minute5 => "5m",
            Self::Minute15 => "15m",
            Self::Minute30 => "30m",
            Self::Minute60 => "60m",
            Self::Day1 => "1d",
            Self::Week1 => "1wk",
            Self::Month1 => "1mo",
            Self::ThreeMonth1 => "3mo",
        }
    }
}

impl FromStr for Interval {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let interval = match s {
            "1m" => Self::Minute1,
            "5m" => Self::Minute5,
            "15m" => Self::Minute15,
            "30m" => Self::Minute30,
            "60m" => Self::Minute60,
            "1d" => Self::Day1,
            "1wk" => Self::Week1,
            "1mo" => Self::Month1,
            "3mo" => Self::ThreeMonth1,
            other => {
                return Err(Error::Interface(format!(
                    "unsupported Yahoo interval: {other}"
                )));
            }
        };
        Ok(interval)
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Yahoo Finance client. Cheap to keep alive: the cookie jar and crumb persist
/// across calls, exactly like the Python client's `httpx` session.
pub struct YahooInterface {
    client: Client,
    crumb: RefCell<Option<String>>,
}

impl YahooInterface {
    pub fn new() -> Result<Self, Error> {
        Self::build(None)
    }

    /// Builds a client with an explicit session cookie, mirroring the Python
    /// constructor's `cookie` argument (set as `A1`/`A3`/`A1S`).
    pub fn with_cookie(cookie: &str) -> Result<Self, Error> {
        Self::build(Some(cookie))
    }

    fn build(cookie: Option<&str>) -> Result<Self, Error> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(BROWSER_AGENT));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            ),
        );
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
        headers.insert(
            REFERER,
            HeaderValue::from_static("https://finance.yahoo.com/"),
        );
        if let Some(cookie) = cookie {
            let value = format!("A1={cookie}; A3={cookie}; A1S={cookie}");
            headers.insert(
                COOKIE,
                HeaderValue::from_str(&value).map_err(|e| Error::Connection(e.to_string()))?,
            );
        }

        let client = Client::builder()
            .default_headers(headers)
            .cookie_store(true)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| Error::Connection(e.to_string()))?;

        Ok(Self {
            client,
            crumb: RefCell::new(None),
        })
    }

    /// Session crumb token, scraped once and then cached.
    pub fn crumb(&self) -> Result<String, Error> {
        if let Some(crumb) = self.crumb.borrow().as_deref() {
            return Ok(crumb.to_owned());
        }

        let page = self.get_text(QUOTE_PAGE)?;
        let crumb = match extract_crumb(&page) {
            Some(crumb) => crumb,
            None => {
                let fallback = self.get_text(&format!("{BASE_URL}/v1/test/getcrumb"))?;
                let fallback = fallback.trim().to_owned();
                if fallback.is_empty() {
                    return Err(Error::Http("Yahoo returned an empty crumb".into()));
                }
                fallback
            }
        };

        *self.crumb.borrow_mut() = Some(crumb.clone());
        Ok(crumb)
    }

    /// Delayed quotes for `symbols`.
    ///
    /// Columns: `timestamp` (epoch seconds), `symbol`, `bid`, `ask`,
    /// `market_price`. The Python client returned the same fields with
    /// `marketPrice` as the column name and a `(timestamp, symbol)` index.
    pub fn quote(&self, symbols: &[&str]) -> Result<DataFrame, Error> {
        let crumb = self.crumb()?;
        let joined = symbols.join(",");
        let response: QuoteResponse = self.get_json(
            &format!("{BASE_URL}/v7/finance/quote"),
            &[
                ("symbols", joined.as_str()),
                ("crumb", crumb.as_str()),
                ("lang", "en-US"),
                ("region", "US"),
                ("formatted", "false"),
            ],
        )?;
        quote_frame(&response)
    }

    /// Historical bars for one symbol between `start` and `end` (both UTC dates).
    ///
    /// Columns: `date` (`Date`), `open`, `high`, `low`, `close`, `adjclose`,
    /// `volume`. Rows Yahoo reports as `null` (halts, pre-listing padding) are
    /// kept with null values so the row count matches the timestamp array.
    pub fn historical(
        &self,
        symbol: &str,
        start: Date,
        end: Date,
        interval: Interval,
    ) -> Result<DataFrame, Error> {
        let crumb = self.crumb()?;
        let period1 = epoch_seconds(start)?.to_string();
        let period2 = epoch_seconds(end)?.to_string();
        let url = format!("{BASE_URL}/v8/finance/chart/{symbol}");

        let response: ChartResponse = self.get_json(
            &url,
            &[
                ("interval", interval.as_str()),
                ("period1", period1.as_str()),
                ("period2", period2.as_str()),
                ("crumb", crumb.as_str()),
                ("lang", "en-US"),
                ("region", "US"),
                ("formatted", "false"),
                ("includeAdjustedClose", "true"),
            ],
        )?;

        chart_frame(symbol, &response)
    }

    /// Symbols matching `query`, as returned by Yahoo's search endpoint.
    pub fn symbols(&self, query: &str) -> Result<Vec<String>, Error> {
        let response: SearchResponse = self.get_json(
            &format!("{BASE_URL}/v1/finance/search"),
            &[
                ("q", query),
                ("quotesCount", "10"),
                ("quotesQueryId", "tss_match_phrase_query"),
                ("multiQuotesQueryId", "multi_quote_single_token_query"),
                ("enablePrivateCompany", "true"),
                ("enableLists", "false"),
                ("lang", "en-US"),
                ("region", "US"),
            ],
        )?;
        Ok(response.quotes.into_iter().map(|q| q.symbol).collect())
    }

    fn get_text(&self, url: &str) -> Result<String, Error> {
        self.client
            .get(url)
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.text())
            .map_err(|e| Error::Http(e.to_string()))
    }

    fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        url: &str,
        params: &[(&str, &str)],
    ) -> Result<T, Error> {
        self.client
            .get(url)
            .query(params)
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.json())
            .map_err(|e| Error::Http(e.to_string()))
    }
}

impl MarketInterface for YahooInterface {
    fn market_history(
        &self,
        contract: &Instrument,
        bar_size: Span,
        duration: Span,
    ) -> PolarsResult<DataFrame> {
        let interval = interval_for(&bar_size).ok_or_else(|| {
            PolarsError::InvalidOperation(format!("yahoo: unsupported bar size {bar_size}").into())
        })?;

        // The trait asks for a trailing window ending now; Yahoo takes absolute
        // bounds, so today is the inclusive end.
        let today = Timestamp::now().to_zoned(TimeZone::UTC).date();
        let start = today.saturating_sub(duration);

        self.historical(&contract.symbol, start, today, interval)
            .map_err(|e| PolarsError::ComputeError(format!("yahoo: {e}").into()))
    }
}

/// Maps a span to the Yahoo interval it corresponds to, if any.
fn interval_for(bar_size: &Span) -> Option<Interval> {
    let candidates = [
        (1.minutes(), Interval::Minute1),
        (5.minutes(), Interval::Minute5),
        (15.minutes(), Interval::Minute15),
        (30.minutes(), Interval::Minute30),
        (1.hours(), Interval::Minute60),
        (1.days(), Interval::Day1),
        (1.weeks(), Interval::Week1),
        (1.months(), Interval::Month1),
        (3.months(), Interval::ThreeMonth1),
    ];
    candidates
        .into_iter()
        .find(|(span, _)| bar_size.fieldwise() == *span)
        .map(|(_, interval)| interval)
}

fn epoch_seconds(date: Date) -> Result<i64, Error> {
    date.to_zoned(TimeZone::UTC)
        .map(|zoned| zoned.timestamp().as_second())
        .map_err(|e| Error::Interface(format!("date out of range: {e}")))
}

/// Scrapes `"crumb":"<token>"` out of the quote page's embedded JSON.
fn extract_crumb(html: &str) -> Option<String> {
    const KEY: &str = "\"crumb\"";
    let start = html.find(KEY)? + KEY.len();
    let rest = html[start..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    let crumb = &rest[..end];
    (!crumb.is_empty()).then(|| crumb.to_owned())
}

fn chart_frame(symbol: &str, response: &ChartResponse) -> Result<DataFrame, Error> {
    if let Some(error) = response.chart.error.as_ref() {
        return Err(Error::Interface(format!(
            "yahoo chart error for {symbol}: {error}"
        )));
    }

    let result = response
        .chart
        .result
        .as_ref()
        .and_then(|results| results.first())
        .ok_or_else(|| Error::Interface(format!("no chart data for {symbol}")))?;

    let timestamps = result.timestamp.as_deref().unwrap_or_default();
    let offset = result
        .meta
        .as_ref()
        .and_then(|meta| meta.gmtoffset)
        .unwrap_or(0);
    let quote = result
        .indicators
        .quote
        .first()
        .ok_or_else(|| Error::Interface(format!("no quote series for {symbol}")))?;
    let adjclose = result
        .indicators
        .adjclose
        .as_ref()
        .and_then(|series| series.first())
        .and_then(|series| series.adjclose.as_deref());

    let days: Vec<i32> = timestamps.iter().map(|ts| epoch_day(*ts, offset)).collect();

    let column = |name: &str, values: Option<&[Option<f64>]>, len: usize| -> Column {
        let values = values.unwrap_or(&[]);
        let mut padded: Vec<Option<f64>> = values.to_vec();
        padded.resize(len, None);
        Column::new(name.into(), &padded)
    };

    let len = days.len();
    DataFrame::new(vec![
        Column::new("date".into(), &days)
            .cast(&DataType::Date)
            .map_err(|e| Error::Interface(e.to_string()))?,
        column("open", quote.open.as_deref(), len),
        column("high", quote.high.as_deref(), len),
        column("low", quote.low.as_deref(), len),
        column("close", quote.close.as_deref(), len),
        column("adjclose", adjclose, len),
        column("volume", quote.volume.as_deref(), len),
    ])
    .map_err(|e| Error::Interface(e.to_string()))
}

fn quote_frame(response: &QuoteResponse) -> Result<DataFrame, Error> {
    let results = response
        .quote_response
        .as_ref()
        .map(|body| body.result.as_slice())
        .unwrap_or_default();

    let timestamps: Vec<i64> = results
        .iter()
        .map(|row| row.regular_market_time.unwrap_or(0))
        .collect();
    let symbols: Vec<String> = results.iter().map(|row| row.symbol.clone()).collect();
    let bids: Vec<Option<f64>> = results.iter().map(|row| row.bid).collect();
    let asks: Vec<Option<f64>> = results.iter().map(|row| row.ask).collect();
    let prices: Vec<Option<f64>> = results.iter().map(|row| row.regular_market_price).collect();

    DataFrame::new(vec![
        Column::new("timestamp".into(), &timestamps),
        Column::new("symbol".into(), &symbols),
        Column::new("bid".into(), &bids),
        Column::new("ask".into(), &asks),
        Column::new("market_price".into(), &prices),
    ])
    .map_err(|e| Error::Interface(e.to_string()))
}

/// Converts an epoch-second timestamp to a day number in the exchange's local
/// time, which is how Yahoo labels its bars.
fn epoch_day(timestamp: i64, offset_seconds: i64) -> i32 {
    let local = timestamp.saturating_add(offset_seconds);
    i32::try_from(local.div_euclid(SECONDS_PER_DAY)).unwrap_or(0)
}

#[derive(Debug, Deserialize)]
struct ChartResponse {
    chart: ChartBody,
}

#[derive(Debug, Deserialize)]
struct ChartBody {
    result: Option<Vec<ChartResult>>,
    error: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ChartResult {
    meta: Option<ChartMeta>,
    timestamp: Option<Vec<i64>>,
    indicators: Indicators,
}

#[derive(Debug, Deserialize)]
struct ChartMeta {
    gmtoffset: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct Indicators {
    #[serde(default)]
    quote: Vec<QuoteSeries>,
    adjclose: Option<Vec<AdjCloseSeries>>,
}

#[derive(Debug, Deserialize)]
struct QuoteSeries {
    open: Option<Vec<Option<f64>>>,
    high: Option<Vec<Option<f64>>>,
    low: Option<Vec<Option<f64>>>,
    close: Option<Vec<Option<f64>>>,
    volume: Option<Vec<Option<f64>>>,
}

#[derive(Debug, Deserialize)]
struct AdjCloseSeries {
    adjclose: Option<Vec<Option<f64>>>,
}

#[derive(Debug, Deserialize)]
struct QuoteResponse {
    #[serde(rename = "quoteResponse")]
    quote_response: Option<QuoteBody>,
}

#[derive(Debug, Deserialize)]
struct QuoteBody {
    result: Vec<QuoteRow>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuoteRow {
    symbol: String,
    regular_market_time: Option<i64>,
    bid: Option<f64>,
    ask: Option<f64>,
    regular_market_price: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    quotes: Vec<SearchHit>,
}

#[derive(Debug, Deserialize)]
struct SearchHit {
    symbol: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHART_JSON: &str = r#"{
        "chart": {
            "result": [{
                "meta": {"symbol": "AAPL", "gmtoffset": -18000},
                "timestamp": [1716910200, 1716996600, 1717083000],
                "indicators": {
                    "quote": [{
                        "open": [190.0, 191.5, null],
                        "high": [192.0, 193.0, null],
                        "low": [189.0, 190.5, null],
                        "close": [191.0, 192.5, null],
                        "volume": [1000.0, 1200.0, null]
                    }],
                    "adjclose": [{"adjclose": [190.5, 192.0, null]}]
                }
            }],
            "error": null
        }
    }"#;

    #[test]
    fn interval_round_trips_through_yahoo_codes() {
        for interval in [
            Interval::Minute1,
            Interval::Minute5,
            Interval::Minute15,
            Interval::Minute30,
            Interval::Minute60,
            Interval::Day1,
            Interval::Week1,
            Interval::Month1,
            Interval::ThreeMonth1,
        ] {
            let parsed: Interval = interval.as_str().parse().unwrap();
            assert_eq!(parsed, interval);
        }
        assert!("4h".parse::<Interval>().is_err());
    }

    #[test]
    fn bar_size_maps_to_interval() {
        assert_eq!(interval_for(&1.days()), Some(Interval::Day1));
        assert_eq!(interval_for(&1.weeks()), Some(Interval::Week1));
        assert_eq!(interval_for(&15.minutes()), Some(Interval::Minute15));
        assert_eq!(interval_for(&2.days()), None);
    }

    #[test]
    fn crumb_is_scraped_from_quote_page() {
        let html = r#"<script>{"context":{"crumb":"AbC-123","x":1}}</script>"#;
        assert_eq!(extract_crumb(html).as_deref(), Some("AbC-123"));
        assert_eq!(
            extract_crumb(r#"{"crumb" : "spaced"}"#).as_deref(),
            Some("spaced")
        );
        assert_eq!(extract_crumb("no token here"), None);
        assert_eq!(extract_crumb(r#"{"crumb":""}"#), None);
    }

    #[test]
    fn chart_json_becomes_dated_frame() {
        let response: ChartResponse = serde_json::from_str(CHART_JSON).unwrap();
        let frame = chart_frame("AAPL", &response).unwrap();

        assert_eq!(
            frame.get_column_names(),
            ["date", "open", "high", "low", "close", "adjclose", "volume"]
        );
        assert_eq!(frame.height(), 3);

        let dates = frame.column("date").unwrap().date().unwrap();
        // 1716910200 at UTC-5 is 2024-05-28; UTC would report the next day.
        let days: Vec<i32> = dates.into_iter().flatten().collect();
        assert_eq!(days, vec![19_871, 19_872, 19_873]);

        let closes: Vec<Option<f64>> = frame
            .column("close")
            .unwrap()
            .f64()
            .unwrap()
            .into_iter()
            .collect();
        assert_eq!(closes, vec![Some(191.0), Some(192.5), None]);

        let adj: Vec<Option<f64>> = frame
            .column("adjclose")
            .unwrap()
            .f64()
            .unwrap()
            .into_iter()
            .collect();
        assert_eq!(adj, vec![Some(190.5), Some(192.0), None]);
    }

    #[test]
    fn chart_error_surfaces_as_interface_error() {
        let response: ChartResponse = serde_json::from_str(
            r#"{"chart":{"result":null,"error":{"code":"Not Found","description":"No data found"}}}"#,
        )
        .unwrap();
        let err = chart_frame("NOPE", &response).unwrap_err();
        assert!(matches!(err, Error::Interface(message) if message.contains("No data found")));
    }

    #[test]
    fn quote_json_becomes_frame() {
        let response: QuoteResponse = serde_json::from_str(
            r#"{"quoteResponse":{"result":[
                {"symbol":"MSFT","regularMarketTime":1716910200,"bid":415.1,"ask":415.5,"regularMarketPrice":415.3},
                {"symbol":"AMD","regularMarketTime":1716910200}
            ],"error":null}}"#,
        )
        .unwrap();
        let frame = quote_frame(&response).unwrap();

        assert_eq!(
            frame.get_column_names(),
            ["timestamp", "symbol", "bid", "ask", "market_price"]
        );
        assert_eq!(frame.height(), 2);
        let symbols: Vec<&str> = frame
            .column("symbol")
            .unwrap()
            .str()
            .unwrap()
            .into_iter()
            .flatten()
            .collect();
        assert_eq!(symbols, vec!["MSFT", "AMD"]);
        let prices: Vec<Option<f64>> = frame
            .column("market_price")
            .unwrap()
            .f64()
            .unwrap()
            .into_iter()
            .collect();
        assert_eq!(prices, vec![Some(415.3), None]);
    }

    #[test]
    fn search_json_yields_symbols() {
        let response: SearchResponse = serde_json::from_str(
            r#"{"quotes":[{"symbol":"MSFT"},{"symbol":"MSFT.MX"}],"news":[]}"#,
        )
        .unwrap();
        let symbols: Vec<String> = response.quotes.into_iter().map(|q| q.symbol).collect();
        assert_eq!(symbols, vec!["MSFT", "MSFT.MX"]);
    }

    #[test]
    fn epoch_day_uses_exchange_offset() {
        // 1716910200 == 2024-05-28T14:30:00Z; UTC-5 keeps it on the 28th.
        assert_eq!(epoch_day(1_716_910_200, -18_000), 19_871);
        assert_eq!(epoch_day(1_716_910_200, 0), 19_871);
        // 2024-05-29T02:00:00Z is the 28th in UTC-5 but the 29th in UTC.
        assert_eq!(epoch_day(1_716_948_000, -18_000), 19_871);
        assert_eq!(epoch_day(1_716_948_000, 0), 19_872);
    }
}
