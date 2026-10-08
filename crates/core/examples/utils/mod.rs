use polars::prelude::*;

use dxcore::trading::DailyView;

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
