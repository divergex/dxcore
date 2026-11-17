#[cfg(feature = "integration")]
mod integration {
    use dxcore::core::Instrument;
    use dxcore::interface::external::yahoo::{Interval, YahooInterface};
    use dxcore::interface::{MarketInterface, Span};
    use jiff::civil::Date;
    use std::str::FromStr;

    fn interface() -> YahooInterface {
        YahooInterface::new().expect("failed to build Yahoo client")
    }

    fn instrument(symbol: &str) -> Instrument {
        Instrument {
            contract_id: 0,
            symbol: symbol.to_string(),
            security_type: "STK".into(),
            exchange: "SMART".into(),
            currency: "USD".into(),
        }
    }

    #[test]
    fn crumb_is_not_empty() {
        let crumb = interface().crumb().expect("crumb failed");
        assert!(!crumb.is_empty(), "empty crumb");
        assert!(!crumb.contains(' '), "crumb looks like HTML: {crumb}");
    }

    #[test]
    fn daily_history_returns_bars() {
        let start = Date::from_str("2024-05-01").unwrap();
        let end = Date::from_str("2024-05-15").unwrap();
        let frame = interface()
            .historical("AAPL", start, end, Interval::Day1)
            .expect("historical failed");

        assert!(!frame.is_empty(), "no bars returned");
        assert_eq!(frame.get_column_names()[0], "date");
        assert!(frame.column("close").is_ok());
        assert!(frame.column("adjclose").is_ok());

        let closes = frame.column("close").unwrap().f64().unwrap();
        assert!(closes.into_iter().flatten().all(|c| c > 0.0), "bad close");
        assert!(closes.into_iter().flatten().count() >= 5, "too few bars");
    }

    #[test]
    fn quotes_return_requested_symbols() {
        let frame = interface().quote(&["MSFT", "AMD"]).expect("quote failed");

        let symbols: Vec<String> = frame
            .column("symbol")
            .unwrap()
            .str()
            .unwrap()
            .into_iter()
            .flatten()
            .map(str::to_string)
            .collect();
        assert_eq!(symbols.len(), 2, "expected two quotes, got {symbols:?}");
        assert!(symbols.iter().any(|s| s == "MSFT"));
        assert!(symbols.iter().any(|s| s == "AMD"));
    }

    #[test]
    fn search_finds_symbols() {
        let symbols = interface().symbols("microsoft").expect("search failed");
        assert!(!symbols.is_empty(), "no symbols returned");
        assert!(
            symbols.iter().any(|s| s == "MSFT"),
            "MSFT missing from {symbols:?}"
        );
    }

    #[test]
    fn market_history_accepts_trailing_duration() {
        let frame = interface()
            .market_history(
                &instrument("SPY"),
                Span::new().days(1),
                Span::new().months(1),
            )
            .expect("market_history failed");

        assert!(!frame.is_empty(), "no rows for a one-month window");
        assert!(frame.column("volume").is_ok());
    }

    #[test]
    fn market_history_rejects_unsupported_bar_size() {
        let error = interface()
            .market_history(
                &instrument("SPY"),
                Span::new().days(2),
                Span::new().months(1),
            )
            .unwrap_err();
        assert!(
            error.to_string().contains("unsupported bar size"),
            "unexpected error: {error}"
        );
    }
}
