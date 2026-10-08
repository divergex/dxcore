#![allow(dead_code)]

use std::sync::Arc;

use polars::prelude::*;
use serde::Serialize;

use dxcore::interface::internal::StepArgs;
use dxcore::network::servers::{HttpServer, ServerHandle};
use dxcore::network::services::{FunctionalService, ServiceError};
use dxcore::trading::{DailyView, Strategy};

struct Runner<S: Strategy> {
    strategy: S,
    state: S::State,
}

pub fn serve<S>(strategy: S) -> Result<(String, ServerHandle), ServiceError>
where
    S: Strategy<Input = (i32, DataFrame)> + Send + Sync + 'static,
    S::State: Send + Sync + 'static,
    S::Output: Serialize + Send + Sync + 'static,
{
    let runner = Runner {
        strategy,
        state: S::State::default(),
    };
    let service = FunctionalService::new("strategy", runner).with_set(
        "on_step",
        |runner: &mut Runner<S>, args: StepArgs| {
            let (step, history) = args.into_parts();
            Ok(runner
                .strategy
                .on_step(&step, &history, &mut runner.state))
        },
    );

    let server = HttpServer::bind("127.0.0.1:0", Arc::new(service))?;
    let addr = server.addr();
    let handle = server.spawn();
    Ok((format!("http://{addr}"), handle))
}

pub fn generate_ohlc(n_days: usize) -> DataFrame {
    let prices: Vec<f64> = (0..n_days)
        .map(|i| {
            let t = i as f64;
            let trend = 100.0 + t * 0.3;
            let wave = 5.0 * (t * 0.4).sin() - 3.0 * (t * 0.15).cos();
            let dip = -15.0 * (-((t - 15.0).powi(2) / 30.0)).exp();
            ((trend + wave + dip) * 100.0).round() / 100.0
        })
        .collect();

    let base_date = 19000i32;
    let dates: Vec<i32> = (0..n_days).map(|i| base_date + i as i32).collect();
    let opens: Vec<f64> = prices.clone();
    let closes: Vec<f64> = prices
        .iter()
        .enumerate()
        .map(|(i, p)| if i < n_days - 1 { prices[i + 1] } else { *p })
        .collect();
    let highs: Vec<f64> = opens
        .iter()
        .zip(closes.iter())
        .map(|(o, c)| o.max(*c) + 0.5)
        .collect();
    let lows: Vec<f64> = opens
        .iter()
        .zip(closes.iter())
        .map(|(o, c)| o.min(*c) - 0.5)
        .collect();
    let volumes: Vec<f64> = (0..n_days).map(|_| 10_000.0).collect();

    DataFrame::new(vec![
        Column::new(
            "date".into(),
            Series::new("date".into(), dates)
                .cast(&DataType::Date)
                .unwrap(),
        ),
        Column::new("symbol".into(), vec!["DEMO"; n_days]),
        Column::new("open".into(), opens),
        Column::new("high".into(), highs),
        Column::new("low".into(), lows),
        Column::new("close".into(), closes),
        Column::new("volume".into(), volumes),
    ])
    .unwrap()
}

pub fn daily_view() -> DailyView {
    DailyView::new("date").with_col_map(vec![("close".into(), "price".into())])
}
