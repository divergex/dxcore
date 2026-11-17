//! Federal Reserve Economic Data (FRED) interface.
//!
//! Wraps FRED's CSV export endpoint (`graph/fredgraph.csv?id=<SERIES>`), which
//! returns `date,value` observations for a single series. The response uses `.`
//! for days without an observation (weekends, holidays); those rows are skipped.
//!
//! The series this crate needs is `DTB3`, the 3-month Treasury bill secondary
//! market rate, used as the fixed-rate leg of strategy backtests. Any other
//! series id works through [`FredInterface::series`].

use std::collections::BTreeMap;
use std::time::Duration;

use jiff::civil::Date;
use reqwest::blocking::Client;

use crate::Error;

pub const BASE_URL: &str = "https://fred.stlouisfed.org/graph/fredgraph.csv";
/// FRED answers `200` for a normal browser agent and drops requests that carry
/// no `User-Agent` at all, which is the default for `reqwest`.
const USER_AGENT: &str = "dxcore/0.1 (+https://github.com/divergex/dxcore)";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

pub struct FredInterface {
    client: Client,
}

impl FredInterface {
    pub fn new() -> Result<Self, Error> {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|error| Error::Connection(error.to_string()))?;
        Ok(Self { client })
    }

    /// Observations for `series_id`, keyed by observation date.
    pub fn series(&self, series_id: &str) -> Result<BTreeMap<Date, f64>, Error> {
        let body = self
            .client
            .get(BASE_URL)
            .query(&[("id", series_id)])
            .send()
            .and_then(|response| response.error_for_status())
            .and_then(|response| response.text())
            .map_err(|error| Error::Http(format!("{series_id}: {error}")))?;

        parse(&body, series_id)
    }

    /// 3-month Treasury bill secondary market rate, annualized percent.
    pub fn dtb3(&self) -> Result<BTreeMap<Date, f64>, Error> {
        self.series("DTB3")
    }
}

/// Parses a FRED CSV payload, verifying it belongs to `series_id`.
pub fn parse(body: &str, series_id: &str) -> Result<BTreeMap<Date, f64>, Error> {
    let mut lines = body.lines().filter(|line| !line.trim().is_empty());
    let header = lines
        .next()
        .ok_or_else(|| Error::Interface(format!("empty {series_id} response")))?;
    if !header.contains(series_id) {
        return Err(Error::Interface(format!(
            "response is not {series_id}: {header}"
        )));
    }

    let mut series = BTreeMap::new();
    for line in lines {
        let mut fields = line.split(',');
        let date = fields.next().unwrap_or_default().trim();
        let value = fields.next().unwrap_or_default().trim();
        if date.is_empty() || value.is_empty() || value == "." {
            continue;
        }

        let date: Date = date
            .parse()
            .map_err(|error| Error::Interface(format!("bad {series_id} date {date:?}: {error}")))?;
        let value: f64 = value.parse().map_err(|error| {
            Error::Interface(format!("bad {series_id} value {value:?}: {error}"))
        })?;
        series.insert(date, value);
    }

    if series.is_empty() {
        return Err(Error::Interface(format!(
            "{series_id} response had no observations"
        )));
    }
    Ok(series)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_observations_and_skips_holidays() {
        let body = "observation_date,DTB3\n\
                    2024-05-01,5.25\n\
                    2024-05-02,.\n\
                    2024-05-03,5.21\n";
        let series = parse(body, "DTB3").unwrap();

        assert_eq!(series.len(), 2);
        assert_eq!(series[&"2024-05-01".parse().unwrap()], 5.25);
        assert_eq!(series[&"2024-05-03".parse().unwrap()], 5.21);
    }

    #[test]
    fn rejects_a_different_series() {
        assert!(parse("observation_date,GS10\n2024-05-01,4.5\n", "DTB3").is_err());
    }

    #[test]
    fn rejects_empty_payloads() {
        assert!(parse("", "DTB3").is_err());
        assert!(parse("observation_date,DTB3\n", "DTB3").is_err());
    }

    #[test]
    fn rejects_malformed_values() {
        assert!(parse("observation_date,DTB3\n2024-05-01,oops\n", "DTB3").is_err());
        assert!(parse("observation_date,DTB3\nnot-a-date,5.0\n", "DTB3").is_err());
    }
}
