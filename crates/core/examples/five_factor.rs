//! Five-factor strategy demo with live FMP fundamentals + IBKR daily returns.
//!
//! Prerequisites:
//! - FMP_API_KEY environment variable
//! - IB Gateway or TWS running on IB_HOST (default 127.0.0.1:4002)
//!
//! Run with: `cargo run --example five_factor --features strategies`

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use polars::prelude::*;

use dxlib::interface::external::fmp::FmpClient;
use dxlib::interface::external::ibkr::IbkrInterface;
use dxlib::interface::stream::poll;
use dxlib::interface::MarketApi;
use dxlib::strategies::five_factor::{FiveFactor, Source};
use dxlib::trading::{AsyncExecutor, DailyView};
use ibapi::contracts::Contract;
use ibapi::market_data::historical::{BarSize, ToDuration};

const SYMBOLS: &[&str] = &["AAPL", "MSFT", "GOOGL"];
const INITIAL_CASH: f64 = 100_000.0;
const TOP_N: usize = 3;

#[tokio::main]
async fn main() {
    let fmp = Arc::new(FmpClient::from_env().expect("FMP_API_KEY must be set"));
    let ib_host = std::env::var("IB_HOST").unwrap_or_else(|_| "127.0.0.1:4002".into());
    let ibkr = Arc::new(IbkrInterface::new(ib_host, 1));

    let strategy = FiveFactor::new(INITIAL_CASH, TOP_N);
    let mut executor = AsyncExecutor::new(strategy);


    let fundamentals = {
        let fmp = fmp.clone();
        poll(Duration::from_secs(180 * 24 * 60 * 60), true, move || {
            let fmp = fmp.clone();
            async move {
                tokio::task::spawn_blocking(move || fetch_fundamentals(&fmp))
                    .await
                    .map_err(|e| dxlib::Error::Connection(e.to_string()))?
            }
        })
        .map(|r| r.expect("fundamentals fetch"))
        .boxed()
    };


    let returns = {
        let ibkr = ibkr.clone();
        poll(Duration::from_secs(24 * 60 * 60), false, move || {
            let ibkr = ibkr.clone();
            async move {
                tokio::task::spawn_blocking(move || fetch_returns(&ibkr))
                    .await
                    .map_err(|e| dxlib::Error::Connection(e.to_string()))?
            }
        })
        .map(|r| r.expect("returns fetch"))
        .boxed()
    };


    let views: HashMap<Source, DailyView> = HashMap::from([
        (Source::Fundamentals, DailyView::new("date")),
        (Source::Returns, DailyView::new("date")),
    ]);

    let streams = HashMap::from([
        (Source::Fundamentals, fundamentals),
        (Source::Returns, returns),
    ]);

    let mut output = executor.run_multi(streams, views);

    println!("symbols: {:?}", SYMBOLS);
    println!("initial cash: ${INITIAL_CASH}");
    println!("top N: {TOP_N}\n");

    while let Some(row) = output.next().await {
        let s = &row.output;
        if s.action != "-" {
            println!(
                "date={} symbol={} action={} weight={:.4} price={:.2}",
                s.date, s.symbol, s.action, s.weight, s.price
            );
        }
    }
}


fn fetch_fundamentals(fmp: &FmpClient) -> Result<(i32, DataFrame), dxlib::Error> {
    let mut symbols: Vec<String> = Vec::new();
    let mut dates: Vec<i32> = Vec::new();
    let mut shares: Vec<f64> = Vec::new();
    let mut book_values: Vec<f64> = Vec::new();
    let mut op_incomes: Vec<f64> = Vec::new();
    let mut total_assets: Vec<f64> = Vec::new();

    for &sym in SYMBOLS {
        let bs = fmp.balance_sheet(sym)?;
        let inc = fmp.income_statement(sym)?;

        let bs_latest = match bs.first() {
            Some(b) => b,
            None => continue,
        };
        let inc_latest = match inc.first() {
            Some(i) => i,
            None => continue,
        };

        let date = parse_fmp_date(&bs_latest.date);
        let bv = bs_latest
            .total_stockholders_equity
            .or(bs_latest.total_equity)
            .unwrap_or(0.0);
        let shares_out = inc_latest.weighted_average_shs_out.unwrap_or(0.0);
        let oi = inc_latest.operating_income.unwrap_or(0.0);
        let ta = bs_latest.total_assets.unwrap_or(0.0);

        symbols.push(sym.to_string());
        dates.push(date);
        shares.push(shares_out);
        book_values.push(bv);
        op_incomes.push(oi);
        total_assets.push(ta);
    }

    let step_date = dates.iter().copied().max().unwrap_or(0);
    let date_col = Column::new("date".into(), dates);
    let sym_col = Column::new("symbol".into(), symbols);
    let df = DataFrame::new(vec![
        date_col,
        sym_col,
        Column::new("outstanding_shares".into(), shares),
        Column::new("book_value".into(), book_values),
        Column::new("operating_income".into(), op_incomes),
        Column::new("total_assets".into(), total_assets),
    ])
    .map_err(|e| dxlib::Error::Connection(e.to_string()))?;


    Ok((step_date, df))
}

/// "2024-09-30" → approximate days since 1970-01-01.
fn parse_fmp_date(s: &str) -> i32 {
    let parts: Vec<i32> = s.split('-').filter_map(|p| p.parse().ok()).collect();
    if parts.len() != 3 {
        return 0;
    }
    let (y, m, d) = (parts[0], parts[1], parts[2]);

    let mut days = (y - 1970) * 365;
    days += (y - 1).saturating_sub(1968) / 4
        - (y - 1).saturating_sub(1900) / 100
        + (y - 1).saturating_sub(1600) / 400;
    // Month offsets (non-leap year).
    let month_offsets = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    days += month_offsets.get(m as usize - 1).copied().unwrap_or(0);
    // Leap year correction for Jan/Feb.
    let is_leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    if m > 2 && is_leap {
        days += 1;
    }
    days + d - 1
}


fn fetch_returns(ibkr: &IbkrInterface) -> Result<(i32, DataFrame), dxlib::Error> {
    let mut symbols: Vec<String> = Vec::new();
    let mut dates: Vec<i32> = Vec::new();
    let mut prices: Vec<f64> = Vec::new();

    for &sym in SYMBOLS {
        let contract = Contract::stock(sym).build();
        let df = ibkr
            .market_history(&contract, BarSize::Day, 2.days())
            .map_err(|e| dxlib::Error::Connection(e.to_string()))?;

        if df.height() == 0 {
            continue;
        }

        let last = df.slice((df.height() - 1) as i64, 1);
        let close = last
            .column("close")
            .and_then(|c| c.f64())
            .ok()
            .and_then(|s| s.get(0))
            .unwrap_or(f64::NAN);

        let date_str = last
            .column("date")
            .ok()
            .and_then(|c| c.str().ok())
            .and_then(|s| s.get(0).map(|v| v.to_string()))
            .unwrap_or_default();

        let date = parse_fmp_date(&date_str);

        symbols.push(sym.to_string());
        dates.push(date);
        prices.push(close);
    }

    let step_date = dates.iter().copied().max().unwrap_or(0);
    let df = DataFrame::new(vec![
        Column::new("date".into(), dates),
        Column::new("symbol".into(), symbols),
        Column::new("price".into(), prices),
    ])
    .map_err(|e| dxlib::Error::Connection(e.to_string()))?;


    Ok((step_date, df))
}
