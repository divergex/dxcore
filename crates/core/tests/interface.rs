use polars::prelude::*;

use dxcore::core::{Instrument, Portfolio};
use dxcore::interface::{AccountInterface, MarketInterface, MockInterface, Span};
use dxcore::{Error, Event};

fn dummy_instrument() -> Instrument {
    Instrument {
        contract_id: 0,
        symbol: String::new(),
        security_type: String::new(),
        exchange: String::new(),
        currency: String::new(),
    }
}

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
            &dummy_instrument(),
            Span::new().days(1),
            Span::new().days(5),
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
        &dummy_instrument(),
        Span::new().days(1),
        Span::new().days(5),
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
fn mock_events_returns_configured() {
    let mock = MockInterface::new().with_events(vec![
        Event::Connected,
        Event::UpdateTime("12:00:00".into()),
        Event::Disconnected("done".into()),
    ]);

    let events = mock.events("U123").unwrap();

    assert_eq!(events.len(), 3);
    assert!(matches!(events[0], Event::Connected));
    assert!(matches!(events[1], Event::UpdateTime(_)));
    assert!(matches!(events[2], Event::Disconnected(_)));
}

#[test]
fn mock_listen_async_starts_the_loop() {
    let mock = MockInterface::new();

    mock.listen_async("U123").unwrap();

    assert!(mock.events("U123").unwrap().is_empty());
}

#[cfg(all(feature = "integration", feature = "ibkr"))]
mod integration {
    use std::env;

    use dxcore::core::Instrument;
    use dxcore::interface::external::ibkr::IbkrInterface;
    use dxcore::interface::{AccountInterface, MarketInterface, Span};
    use dxcore::Event;

    fn account_id() -> String {
        env::var("IB_ACCOUNT_ID").expect("IB_ACCOUNT_ID must be set for integration tests")
    }

    /// Requires a running TWS/Gateway with the test account.
    #[test]
    fn ibkr_market_history_returns_dataframe() {
        let interface = IbkrInterface::new("127.0.0.1:7496".into(), 1);

        let contract = Instrument {
            contract_id: 0,
            symbol: "AAPL".into(),
            security_type: "STK".into(),
            exchange: "SMART".into(),
            currency: "USD".into(),
        };
        let df = interface
            .market_history(&contract, Span::new().days(1), Span::new().days(5))
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
    fn ibkr_listen_async_records_events() {
        let interface = IbkrInterface::new("127.0.0.1:7496".into(), 1);
        let account = account_id();

        interface
            .listen_async(&account)
            .expect("listen_async failed");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut connected = false;
        let mut saw_position_or_value = false;

        while std::time::Instant::now() < deadline && !saw_position_or_value {
            for event in interface.events(&account).expect("events failed") {
                match event {
                    Event::Connected => connected = true,
                    Event::AccountValue(_) | Event::Position(_) => saw_position_or_value = true,
                    _ => {}
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }

        assert!(connected, "did not receive Connected event");
        assert!(
            saw_position_or_value,
            "did not receive any account values or positions"
        );
    }
}
