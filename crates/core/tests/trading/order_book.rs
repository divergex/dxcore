use dxcore::trading::{BookOrder, OrderBook, OrderBookError, OrderId, Price, Side};

fn limit(id: u64, side: Side, ticks: i64, quantity: f64) -> BookOrder {
    BookOrder::limit(OrderId(id), side, Price::new(ticks), quantity)
}

fn market(id: u64, side: Side, quantity: f64) -> BookOrder {
    BookOrder::market(OrderId(id), side, quantity)
}

#[test]
fn levels_are_ordered_best_to_worst() {
    let mut book = OrderBook::new(1).unwrap();

    book.submit(limit(1, Side::Buy, 100, 10.0)).unwrap();
    book.submit(limit(2, Side::Buy, 102, 5.0)).unwrap();
    book.submit(limit(3, Side::Buy, 101, 7.0)).unwrap();
    book.submit(limit(4, Side::Sell, 105, 3.0)).unwrap();
    book.submit(limit(5, Side::Sell, 103, 4.0)).unwrap();
    book.submit(limit(6, Side::Sell, 104, 2.0)).unwrap();

    assert_eq!(book.best_bid(), Some(Price::new(102)));
    assert_eq!(book.best_ask(), Some(Price::new(103)));

    let bids = book.depth(Side::Buy);
    assert_eq!(
        bids.iter()
            .map(|level| level.price.ticks())
            .collect::<Vec<_>>(),
        vec![102, 101, 100]
    );
    let asks = book.depth(Side::Sell);
    assert_eq!(
        asks.iter()
            .map(|level| level.price.ticks())
            .collect::<Vec<_>>(),
        vec![103, 104, 105]
    );
    assert_eq!(book.level_quantity(Side::Buy, Price::new(101)), Some(7.0));
    assert_eq!(book.open_orders(), 6);
}

#[test]
fn cancel_unlinks_order_and_removes_empty_level() {
    let mut book = OrderBook::new(1).unwrap();
    book.submit(limit(1, Side::Buy, 100, 5.0)).unwrap();
    book.submit(limit(2, Side::Buy, 99, 3.0)).unwrap();

    let cancelled = book.cancel(OrderId(1)).expect("open order");
    assert_eq!(cancelled.quantity, 5.0);
    assert_eq!(book.best_bid(), Some(Price::new(99)));
    assert_eq!(book.depth(Side::Buy).len(), 1);
    assert_eq!(book.open_orders(), 1);

    book.cancel(OrderId(2)).expect("open order");
    assert_eq!(book.best_bid(), None);
    assert!(book.is_empty());
    assert!(book.cancel(OrderId(2)).is_none());
}

#[test]
fn matching_is_price_then_time_priority() {
    let mut book = OrderBook::new(1).unwrap();
    book.submit(limit(1, Side::Sell, 101, 3.0)).unwrap();
    book.submit(limit(2, Side::Sell, 101, 4.0)).unwrap();
    book.submit(limit(3, Side::Sell, 102, 5.0)).unwrap();

    let report = book.submit(market(10, Side::Buy, 5.0)).unwrap();

    assert_eq!(report.remaining, 0.0);
    assert!(!report.rested);
    assert_eq!(report.trades.len(), 2);
    assert_eq!(report.trades[0].maker_order_id, OrderId(1));
    assert_eq!(report.trades[0].price, Price::new(101));
    assert_eq!(report.trades[0].quantity, 3.0);
    assert_eq!(report.trades[1].maker_order_id, OrderId(2));
    assert_eq!(report.trades[1].quantity, 2.0);

    let asks = book.depth(Side::Sell);
    assert_eq!(asks[0].price, Price::new(101));
    assert_eq!(asks[0].quantity, 2.0);
    assert_eq!(asks[0].orders, 1);
}

#[test]
fn market_remainder_is_not_rested() {
    let mut book = OrderBook::new(1).unwrap();

    let report = book.submit(market(10, Side::Buy, 5.0)).unwrap();

    assert!(report.trades.is_empty());
    assert_eq!(report.remaining, 5.0);
    assert!(!report.rested);
    assert!(book.is_empty());
}

#[test]
fn limit_remainder_rests_after_matching() {
    let mut book = OrderBook::new(1).unwrap();
    book.submit(limit(1, Side::Sell, 101, 4.0)).unwrap();

    let report = book.submit(limit(2, Side::Buy, 101, 10.0)).unwrap();

    assert_eq!(report.trades.len(), 1);
    assert_eq!(report.trades[0].quantity, 4.0);
    assert_eq!(report.remaining, 6.0);
    assert!(report.rested);
    assert_eq!(book.best_bid(), Some(Price::new(101)));
    assert_eq!(book.level_quantity(Side::Buy, Price::new(101)), Some(6.0));
    assert_eq!(book.open_orders(), 1);
}

#[test]
fn normalizes_raw_prices_by_tick_size() {
    let book = OrderBook::new(5).unwrap();

    assert_eq!(book.tick_size(), 5);
    assert_eq!(book.normalize_price(105), Price::new(21));
    assert_eq!(book.normalize_price(-5), Price::new(-1));
    assert!(matches!(
        OrderBook::new(0),
        Err(OrderBookError::InvalidTickSize(0))
    ));
}

#[test]
fn emptied_best_level_is_removed_and_next_becomes_best() {
    let mut book = OrderBook::new(1).unwrap();
    book.submit(limit(1, Side::Sell, 101, 1.0)).unwrap();
    book.submit(limit(2, Side::Sell, 102, 1.0)).unwrap();

    book.submit(market(10, Side::Buy, 1.0)).unwrap();

    assert_eq!(book.best_ask(), Some(Price::new(102)));
    assert_eq!(book.level_quantity(Side::Sell, Price::new(101)), None);
    let asks = book.depth(Side::Sell);
    assert_eq!(asks.len(), 1);
    assert_eq!(asks[0].price, Price::new(102));
}

#[test]
fn rejects_invalid_orders_and_duplicate_open_ids() {
    let mut book = OrderBook::new(1).unwrap();

    let invalid = book.submit(limit(1, Side::Buy, 100, 0.0)).unwrap_err();
    assert_eq!(invalid, OrderBookError::InvalidQuantity(0.0));

    book.submit(limit(1, Side::Buy, 100, 1.0)).unwrap();
    let duplicate = book.submit(limit(1, Side::Buy, 99, 1.0)).unwrap_err();
    assert_eq!(duplicate, OrderBookError::DuplicateOpenOrder(OrderId(1)));
}

#[test]
fn cancel_returns_remaining_quantity_after_partial_fill() {
    let mut book = OrderBook::new(1).unwrap();
    book.submit(limit(1, Side::Sell, 101, 10.0)).unwrap();
    book.submit(limit(2, Side::Buy, 101, 4.0)).unwrap();

    let cancelled = book.cancel(OrderId(1)).expect("remaining ask");
    assert_eq!(cancelled.quantity, 6.0);
    assert_eq!(cancelled.price, Some(Price::new(101)));
    assert!(book.is_empty());
}
