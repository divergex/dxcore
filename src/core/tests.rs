use super::*;

// ---------------------------------------------------------------------------
// Instrument
// ---------------------------------------------------------------------------

fn make_instrument(id: i32, symbol: &str) -> Instrument {
    Instrument {
        contract_id: id,
        symbol: symbol.into(),
        security_type: "STK".into(),
        exchange: "SMART".into(),
        currency: "USD".into(),
    }
}

#[test]
fn instrument_display_shows_symbol_and_type() {
    let inst = Instrument {
        contract_id: 42,
        symbol: "AAPL".into(),
        security_type: "STK".into(),
        exchange: "NYSE".into(),
        currency: "USD".into(),
    };
    assert_eq!(inst.to_string(), "AAPL (STK)");
}

#[test]
fn instrument_eq_depends_on_all_fields() {
    let a = Instrument {
        contract_id: 1,
        symbol: "A".into(),
        security_type: "STK".into(),
        exchange: "X".into(),
        currency: "USD".into(),
    };
    let mut b = a.clone();
    assert_eq!(a, b);

    b.contract_id = 2;
    assert_ne!(a, b);
}

#[test]
fn instrument_hash_consistent_with_eq() {
    use std::collections::HashSet;
    let a = make_instrument(1, "AAPL");
    let b = make_instrument(1, "AAPL");
    let c = make_instrument(2, "GOOG");

    let mut set = HashSet::new();
    set.insert(a.clone());
    set.insert(b.clone());
    assert_eq!(set.len(), 1);
    set.insert(c);
    assert_eq!(set.len(), 2);
}

// ---------------------------------------------------------------------------
// Portfolio
// ---------------------------------------------------------------------------

#[test]
fn portfolio_upsert_metric_adds_and_updates() {
    let mut p = Portfolio::default();
    assert!(p.metrics.is_empty());

    p.upsert_metric("NetLiquidation".into(), "100000".into(), "USD".into());
    assert_eq!(p.metrics.len(), 1);
    assert_eq!(p.metrics["NetLiquidation"].value, "100000");

    p.upsert_metric("NetLiquidation".into(), "105000".into(), "USD".into());
    assert_eq!(p.metrics.len(), 1);
    assert_eq!(p.metrics["NetLiquidation"].value, "105000");
}

#[test]
fn portfolio_set_holding_adds_replaces_and_removes() {
    let mut p = Portfolio::default();
    assert_eq!(p.holding_count(), 0);

    let inst = make_instrument(1, "AAPL");
    p.set_holding(inst.clone(), 100.0);
    assert_eq!(p.holding_count(), 1);
    assert_eq!(p.quantity(1), Some(100.0));
    assert_eq!(p.instrument(1).unwrap().symbol, "AAPL");

    // Update quantity
    p.set_holding(inst.clone(), 200.0);
    assert_eq!(p.holding_count(), 1);
    assert_eq!(p.quantity(1), Some(200.0));

    // Zero removes
    p.set_holding(inst, 0.0);
    assert_eq!(p.holding_count(), 0);
    assert_eq!(p.quantity(1), None);
    assert_eq!(p.instrument(1), None);
}

#[test]
fn portfolio_holdings_iterator() {
    let mut p = Portfolio::default();
    p.set_holding(make_instrument(1, "AAPL"), 50.0);
    p.set_holding(make_instrument(2, "GOOG"), 30.0);

    let mut holdings: Vec<_> = p.holdings().collect();
    holdings.sort_by_key(|(inst, _)| inst.symbol.clone());

    assert_eq!(holdings.len(), 2);
    assert_eq!(holdings[0].0.symbol, "AAPL");
    assert_eq!(holdings[0].1, 50.0);
    assert_eq!(holdings[1].0.symbol, "GOOG");
    assert_eq!(holdings[1].1, 30.0);
}

#[test]
fn portfolio_negative_quantity_preserved() {
    let mut p = Portfolio::default();
    p.set_holding(make_instrument(1, "SHORT"), -100.0);
    assert_eq!(p.holding_count(), 1);
    assert_eq!(p.quantity(1), Some(-100.0));
}

// ---------------------------------------------------------------------------
// InstrumentStore
// ---------------------------------------------------------------------------

#[test]
fn store_insert_and_lookup_by_id() {
    let mut store = InstrumentStore::default();
    store.insert(make_instrument(42, "AAPL"));
    store.insert(make_instrument(99, "GOOG"));

    assert_eq!(store.len(), 2);
    assert_eq!(store.get(42).unwrap().symbol, "AAPL");
    assert_eq!(store.get(99).unwrap().symbol, "GOOG");
    assert!(store.get(7).is_none());
}

#[test]
fn store_lookup_by_symbol() {
    let mut store = InstrumentStore::default();
    store.insert(make_instrument(1, "ES"));
    store.insert(make_instrument(2, "NQ"));

    assert_eq!(store.get_by_symbol("ES").unwrap().contract_id, 1);
    assert_eq!(store.get_by_symbol("NQ").unwrap().contract_id, 2);
    assert!(store.get_by_symbol("MISSING").is_none());
}

#[test]
fn store_insert_replaces_existing() {
    let mut store = InstrumentStore::default();
    store.insert(make_instrument(1, "AAPL"));

    store.insert(Instrument {
        contract_id: 1,
        symbol: "AAPL".into(),
        security_type: "OPT".into(),
        exchange: "NYSE".into(),
        currency: "USD".into(),
    });

    assert_eq!(store.len(), 1);
    assert_eq!(store.get(1).unwrap().security_type, "OPT");

    store.insert(make_instrument(2, "AAPL"));
    assert_eq!(store.len(), 2);
    assert_eq!(store.get_by_symbol("AAPL").unwrap().contract_id, 2);
}

#[test]
fn store_is_empty_and_len() {
    let store = InstrumentStore::default();
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);
}
