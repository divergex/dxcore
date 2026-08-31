#[cfg(feature = "integration")]
mod integration {
    use dxcore::interface::external::xbrl;

    #[test]
    fn returns_filings_for_ticker() {
        let filings = xbrl::query_filings(&["ASML"], 5).expect("query_filings failed");

        assert!(!filings.is_empty(), "no filings returned for ASML");
        for f in &filings {
            assert!(!f.entity_name.is_empty(), "entity_name is empty");
            assert!(f.view_url.starts_with("https://filings.xbrl.org/"), "view_url malformed: {}", f.view_url);
        }
    }

    #[test]
    fn respects_limit() {
        let filings = xbrl::query_filings(&["ASML"], 3).expect("query_filings failed");
        assert!(filings.len() <= 3, "limit exceeded: got {}", filings.len());
    }

    #[test]
    fn unknown_ticker_returns_empty() {
        let result = xbrl::query_filings(&["xyznonexistent123456"], 5);
        match result {
            Ok(filings) => assert!(filings.is_empty(), "expected empty, got {} filings", filings.len()),
            Err(_) => {} // API may reject entirely
        }
    }
}
