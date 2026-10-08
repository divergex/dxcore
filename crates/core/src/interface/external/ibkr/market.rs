use ibapi::contracts::{Contract, Currency, Exchange, SecurityType};
use ibapi::market_data::historical::{
    BarSize as IbapiBarSize, Duration as IbapiDuration, WhatToShow,
};
use polars::prelude::*;

use super::IbkrInterface;
use crate::core::Instrument;
use crate::interface::{MarketInterface, Span};

impl MarketInterface for IbkrInterface {
    fn market_history(
        &self,
        contract: &Instrument,
        bar_size: Span,
        duration: Span,
    ) -> PolarsResult<DataFrame> {
        let bar_size = to_ibapi_bar_size(bar_size).ok_or_else(|| {
            PolarsError::ComputeError(format!("unsupported bar size: {bar_size}").into())
        })?;
        let duration = to_ibapi_duration(duration).ok_or_else(|| {
            PolarsError::ComputeError(format!("unsupported duration: {duration}").into())
        })?;

        let client = self
            .connect()
            .map_err(|e| PolarsError::ComputeError(e.to_string().into()))?;

        let contract = to_ibapi_contract(contract);

        let data = client
            .historical_data(&contract, bar_size)
            .what_to_show(WhatToShow::Trades)
            .duration(duration)
            .fetch()
            .map_err(|e| PolarsError::ComputeError(e.to_string().into()))?;

        bars_to_dataframe(&data.bars)
    }
}

fn to_ibapi_contract(instrument: &Instrument) -> Contract {
    Contract {
        contract_id: instrument.contract_id,
        symbol: instrument.symbol.clone().into(),
        security_type: to_security_type(&instrument.security_type),
        exchange: Exchange::from(instrument.exchange.clone()),
        currency: Currency::from(instrument.currency.clone()),
        ..Contract::default()
    }
}

/// Unknown types are preserved as [`SecurityType::Other`] rather than rejected.
fn to_security_type(s: &str) -> SecurityType {
    match s.to_ascii_uppercase().as_str() {
        "STK" => SecurityType::Stock,
        "OPT" => SecurityType::Option,
        "FUT" => SecurityType::Future,
        "CONTFUT" => SecurityType::ContinuousFuture,
        "IND" => SecurityType::Index,
        "FOP" => SecurityType::FuturesOption,
        "CASH" => SecurityType::ForexPair,
        "BAG" | "COMBO" => SecurityType::Spread,
        "WAR" => SecurityType::Warrant,
        "BOND" => SecurityType::Bond,
        "CMDTY" => SecurityType::Commodity,
        other => SecurityType::Other(other.to_string()),
    }
}

/// `None` when the span is not one of IBKR's discrete bar sizes.
fn to_ibapi_bar_size(span: Span) -> Option<IbapiBarSize> {
    use IbapiBarSize::*;
    let units = (
        span.get_years(),
        span.get_months(),
        span.get_weeks(),
        span.get_days(),
        span.get_hours(),
        span.get_minutes(),
        span.get_seconds(),
    );
    Some(match units {
        (0, 0, 0, 0, 0, 0, 1) => Sec,
        (0, 0, 0, 0, 0, 0, 5) => Sec5,
        (0, 0, 0, 0, 0, 0, 10) => Sec10,
        (0, 0, 0, 0, 0, 0, 15) => Sec15,
        (0, 0, 0, 0, 0, 0, 30) => Sec30,
        (0, 0, 0, 0, 0, 1, 0) => Min,
        (0, 0, 0, 0, 0, 2, 0) => Min2,
        (0, 0, 0, 0, 0, 3, 0) => Min3,
        (0, 0, 0, 0, 0, 4, 0) => Min4,
        (0, 0, 0, 0, 0, 5, 0) => Min5,
        (0, 0, 0, 0, 0, 10, 0) => Min10,
        (0, 0, 0, 0, 0, 15, 0) => Min15,
        (0, 0, 0, 0, 0, 20, 0) => Min20,
        (0, 0, 0, 0, 0, 30, 0) => Min30,
        (0, 0, 0, 0, 1, 0, 0) => Hour,
        (0, 0, 0, 0, 2, 0, 0) => Hour2,
        (0, 0, 0, 0, 3, 0, 0) => Hour3,
        (0, 0, 0, 0, 4, 0, 0) => Hour4,
        (0, 0, 0, 0, 8, 0, 0) => Hour8,
        (0, 0, 0, 1, 0, 0, 0) => Day,
        (0, 0, 1, 0, 0, 0, 0) => Week,
        (0, 1, 0, 0, 0, 0, 0) => Month,
        _ => return None,
    })
}

/// `None` when the span is finer-grained than IBKR's request-window units.
fn to_ibapi_duration(span: Span) -> Option<IbapiDuration> {
    let units = (
        span.get_years(),
        span.get_months(),
        span.get_weeks(),
        span.get_days(),
        span.get_hours(),
        span.get_minutes(),
        span.get_seconds(),
    );
    Some(match units {
        (y, 0, 0, 0, 0, 0, 0) if y > 0 => IbapiDuration::years(i32::from(y)),
        (0, mo, 0, 0, 0, 0, 0) if mo > 0 => IbapiDuration::months(mo),
        (0, 0, w, 0, 0, 0, 0) if w > 0 => IbapiDuration::weeks(w),
        (0, 0, 0, d, 0, 0, 0) if d > 0 => IbapiDuration::days(d),
        (0, 0, 0, 0, 0, 0, s) if s > 0 => IbapiDuration::seconds(i32::try_from(s).ok()?),
        _ => return None,
    })
}

fn bars_to_dataframe(bars: &[ibapi::market_data::historical::Bar]) -> PolarsResult<DataFrame> {
    let dates: Vec<String> = bars.iter().map(|b| b.date.to_string()).collect();
    let opens: Vec<f64> = bars.iter().map(|b| b.open).collect();
    let highs: Vec<f64> = bars.iter().map(|b| b.high).collect();
    let lows: Vec<f64> = bars.iter().map(|b| b.low).collect();
    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let volumes: Vec<f64> = bars.iter().map(|b| b.volume).collect();

    DataFrame::new(vec![
        Column::new("date".into(), &dates),
        Column::new("open".into(), &opens),
        Column::new("high".into(), &highs),
        Column::new("low".into(), &lows),
        Column::new("close".into(), &closes),
        Column::new("volume".into(), &volumes),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_size_span_maps_to_ibkr_variants() {
        assert_eq!(
            to_ibapi_bar_size(Span::new().seconds(5)),
            Some(IbapiBarSize::Sec5)
        );
        assert_eq!(
            to_ibapi_bar_size(Span::new().minutes(5)),
            Some(IbapiBarSize::Min5)
        );
        assert_eq!(
            to_ibapi_bar_size(Span::new().hours(8)),
            Some(IbapiBarSize::Hour8)
        );
        assert_eq!(
            to_ibapi_bar_size(Span::new().days(1)),
            Some(IbapiBarSize::Day)
        );
        assert_eq!(
            to_ibapi_bar_size(Span::new().months(1)),
            Some(IbapiBarSize::Month)
        );
        assert_eq!(to_ibapi_bar_size(Span::new().days(3)), None);
        assert_eq!(to_ibapi_bar_size(Span::new().years(1)), None);
    }

    #[test]
    fn window_span_maps_to_ibkr_units() {
        assert_eq!(
            to_ibapi_duration(Span::new().days(30)),
            Some(IbapiDuration::days(30))
        );
        assert_eq!(
            to_ibapi_duration(Span::new().months(6)),
            Some(IbapiDuration::months(6))
        );
        assert_eq!(
            to_ibapi_duration(Span::new().years(1)),
            Some(IbapiDuration::years(1))
        );
        assert_eq!(to_ibapi_duration(Span::new().minutes(5)), None);
        assert_eq!(to_ibapi_duration(Span::new().months(1).days(2)), None);
    }
}
