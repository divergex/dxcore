#[cfg(feature = "integration")]
mod integration {
    use dxcore::interface::external::fred::FredInterface;

    #[test]
    fn dtb3_returns_a_long_history() {
        let series = FredInterface::new()
            .expect("failed to build FRED client")
            .dtb3()
            .expect("DTB3 fetch failed");

        assert!(series.len() > 1_000, "only {} observations", series.len());
        assert!(series.values().all(|value| value.is_finite()));

        let latest = series.values().next_back().copied().unwrap_or_default();
        assert!(
            (-5.0..=25.0).contains(&latest),
            "implausible bill rate: {latest}"
        );
    }

    #[test]
    fn unknown_series_is_rejected() {
        let result = FredInterface::new()
            .expect("failed to build FRED client")
            .series("NOT_A_REAL_SERIES");
        assert!(result.is_err(), "unexpected success: {result:?}");
    }
}
