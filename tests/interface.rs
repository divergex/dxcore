use polars::prelude::*;

use cadlag::core::{Instrument, Portfolio};
use cadlag::interface::MockInterface;
use cadlag::interface::MarketApi;
use cadlag::{Error, Event};

fn make_history_df() -> DataFrame {
    DataFrame::new(vec![
        Column::new("date".into(), &["20240528", "20240529"]),
        Column::new("open".into(), &[149.0f64, 150.0]),
        Column::new("high".into(), &[151.0f64, 153.0]),
        Column::new("low".into(), &[148.0f64, 149.0]),
        Column::new("close".into(), &[150.0f64, 152.0]),
        Column::new("volume".into(), &[1000.0f64, 1200.0]),
    ])
    .unwrap()
}

fn make_portfolio() -> Portfolio {
    let mut p = Portfolio::default();
    p.upsert_metric("NetLiquidation".into(), "100000".into(), "USD".into());
    p.set_holding(
        Instrument {
            contract_id: 1,
            symbol: "AAPL".into(),
            security_type: "STK".into(),
            exchange: "SMART".into(),
            currency: "USD".into(),
        },
        100.0,
    );
    p
}

#[test]
fn mock_market_history_returns_configured_df() {
    let df = make_history_df();
    let mock = MockInterface::new().with_history(df.clone());

    let result = mock
        .market_history(
            &ibapi::contracts::Contract::default(),
            ibapi::market_data::historical::BarSize::Day,
            ibapi::market_data::historical::Duration::days(5),
        )
        .unwrap();

    assert_eq!(result.height(), 2);
    assert_eq!(
        result
            .column("date")
            .unwrap()
            .str()
            .unwrap()
            .get(0)
            .unwrap(),
        "20240528"
    );
    assert_eq!(
        result.column("close").unwrap().f64().unwrap().get(0),
        Some(150.0)
    );
}

#[test]
fn mock_market_history_errors_when_no_history_configured() {
    let mock = MockInterface::new();
    let result = mock.market_history(
        &ibapi::contracts::Contract::default(),
        ibapi::market_data::historical::BarSize::Day,
        ibapi::market_data::historical::Duration::days(5),
    );
    assert!(result.is_err());
}

#[test]
fn mock_portfolio_returns_configured() {
    let portfolio = make_portfolio();
    let mock = MockInterface::new().with_portfolio(portfolio.clone());

    let result = mock.portfolio("U123").unwrap();

    assert_eq!(result.holding_count(), 1);
    assert_eq!(result.quantity(1), Some(100.0));
    assert_eq!(result.instrument(1).unwrap().symbol, "AAPL");
    assert_eq!(result.metrics["NetLiquidation"].value, "100000");
}

#[test]
fn mock_portfolio_errors_when_no_portfolio_configured() {
    let mock = MockInterface::new();
    let result = mock.portfolio("U123");
    assert!(matches!(result, Err(Error::Connection(_))));
}

#[test]
fn mock_listen_sends_configured_events() {
    use std::sync::mpsc;

    let events = vec![
        Event::Connected,
        Event::UpdateTime("12:00:00".into()),
        Event::Disconnected("done".into()),
    ];
    let count = events.len();
    let mock = MockInterface::new().with_events(events);

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        mock.listen("U123", tx).unwrap();
    });

    let received: Vec<Event> = rx.iter().collect();
    assert_eq!(received.len(), count);
    assert!(matches!(received[0], Event::Connected));
    assert!(matches!(received[2], Event::Disconnected(_)));
}

#[test]
fn mock_listen_empty_events_sends_nothing() {
    use std::sync::mpsc;

    let mock = MockInterface::new().with_events(vec![]);

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        mock.listen("U123", tx).unwrap();
    });

    // Channel closes immediately — no events sent.
    let received: Vec<Event> = rx.iter().collect();
    assert!(received.is_empty());
}


#[cfg(feature = "integration")]
mod integration {
    use std::env;
    use std::sync::mpsc;

    use ibapi::contracts::Contract;

    use cadlag::interface::ibkr::IbkrInterface;
    use cadlag::interface::MarketApi;
    use cadlag::Event;

    fn account_id() -> String {
        env::var("IB_ACCOUNT_ID").expect("IB_ACCOUNT_ID must be set for integration tests")
    }

    /// Requires a running TWS/Gateway with the test account.
    #[test]
    fn ibkr_market_history_returns_dataframe() {
        let interface = IbkrInterface::new("127.0.0.1:7496".into(), 1);

        let contract = Contract::stock("AAPL").build();
        let df = interface
            .market_history(
                &contract,
                ibapi::market_data::historical::BarSize::Day,
                ibapi::market_data::historical::Duration::days(5),
            )
            .expect("market_history failed");

        assert!(df.height() > 0, "expected at least one bar");
        let cols: Vec<&str> = df.get_column_names().iter().map(|n| n.as_str()).collect();
        assert!(cols.contains(&"date"));
        assert!(cols.contains(&"open"));
        assert!(cols.contains(&"close"));
        assert!(cols.contains(&"volume"));
    }

    /// Requires a running TWS/Gateway with the test account.
    #[test]
    fn ibkr_portfolio_returns_holdings() {
        let interface = IbkrInterface::new("127.0.0.1:7496".into(), 1);
        let account = account_id();

        let _portfolio = interface.portfolio(&account).expect("portfolio failed");

    }

    /// Requires a running TWS/Gateway with the test account.
    #[test]
    fn ibkr_listen_streams_events() {

        let interface = IbkrInterface::new("127.0.0.1:7496".into(), 1);
        let account = account_id();

        let (tx, rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            interface.listen(&account, tx)
        });

        // Collect events with a timeout — we just want to verify the stream
        // starts and sends a Connected event, not drain the full session.
        let mut connected = false;
        let mut saw_position_or_value = false;

        for event in rx.iter().take(50) {
            match event {
                Event::Connected => connected = true,
                Event::AccountValue(_) | Event::Position(_) => {
                    saw_position_or_value = true;
                }
                Event::HistoricalBars { .. } | Event::HistoricalError { .. } => {
                    break; // history fetched, done
                }
                Event::Disconnected(_) => break,
                _ => {}
            }
        }

        handle.join().ok();

        assert!(connected, "did not receive Connected event");
        assert!(
            saw_position_or_value,
            "did not receive any account values or positions"
        );
    }
}
