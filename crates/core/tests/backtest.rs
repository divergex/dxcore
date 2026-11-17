use std::sync::{Arc, Mutex};

use dxcore::Error;
use dxcore::core::Instrument;
use dxcore::interface::{
    AccountInterface, BacktestInterface, MarketInterface, OrderId, OrderInterface, OrderStatus,
    Span, StepObserver,
};
use dxcore::trading::{
    BaseOrderEngine, DailyView, Order, OrderPrice, OrderQuantity, OrderType, PanelView, Side,
    Signal, Strategy, SyncExecutor, TickView, View,
};
use polars::prelude::*;

fn instrument(symbol: &str) -> Instrument {
    Instrument {
        contract_id: 1,
        symbol: symbol.into(),
        security_type: "STK".into(),
        exchange: "SMART".into(),
        currency: "USD".into(),
    }
}

fn frame() -> DataFrame {
    DataFrame::new(vec![
        Column::new("timestamp".into(), &[100i64, 200]),
        Column::new("symbol".into(), &["AAPL", "AAPL"]),
        Column::new("bid".into(), &[99.0f64, 101.0]),
        Column::new("ask".into(), &[101.0f64, 102.0]),
        Column::new("close".into(), &[100.0f64, 101.5]),
    ])
    .unwrap()
}

fn mid_frame() -> DataFrame {
    DataFrame::new(vec![
        Column::new("timestamp".into(), &[100i64]),
        Column::new("symbol".into(), &["AAPL"]),
        Column::new("mid".into(), &[100.0f64]),
    ])
    .unwrap()
}

fn levels_frame() -> DataFrame {
    DataFrame::new(vec![
        Column::new("timestamp".into(), &[100i64]),
        Column::new("symbol".into(), &["AAPL"]),
        Column::new("bid".into(), &[99.0f64]),
        Column::new("ask".into(), &[101.0f64]),
        Column::new("bid_2".into(), &[98.0f64]),
        Column::new("ask_2".into(), &[102.0f64]),
    ])
    .unwrap()
}

fn date_frame() -> DataFrame {
    DataFrame::new(vec![
        Column::new(
            "date".into(),
            Series::new("date".into(), &[19000i32, 19001])
                .cast(&DataType::Date)
                .unwrap(),
        ),
        Column::new("symbol".into(), &["AAPL", "AAPL"]),
        Column::new("bid".into(), &[99.0f64, 100.0]),
        Column::new("ask".into(), &[101.0f64, 102.0]),
    ])
    .unwrap()
}

fn backtest() -> BacktestInterface<TickView> {
    BacktestInterface::new(frame(), TickView::new("timestamp"))
}

fn market_order(side: Side, quantity: f64) -> Order {
    Order {
        date: None,
        symbol: "AAPL".into(),
        side,
        quantity: OrderQuantity::Abs(quantity),
        order_type: OrderType::Market,
        price: OrderPrice::Mkt,
    }
}

fn limit_order(side: Side, price: f64, quantity: f64) -> Order {
    Order {
        date: None,
        symbol: "AAPL".into(),
        side,
        quantity: OrderQuantity::Abs(quantity),
        order_type: OrderType::Limit,
        price: OrderPrice::Px(price),
    }
}

fn history<V: View>(backtest: &BacktestInterface<V>, symbol: &str) -> DataFrame {
    backtest
        .market_history(
            &instrument(symbol),
            Span::new().days(1),
            Span::new().days(5),
        )
        .unwrap()
}

#[test]
fn steps_advance_through_view_entries() {
    let backtest = backtest();

    assert_eq!(backtest.now(), None);
    assert_eq!(history(&backtest, "AAPL").height(), 0);

    assert!(backtest.step().unwrap());
    assert_eq!(backtest.now(), Some(100));
    assert_eq!(history(&backtest, "AAPL").height(), 1);
    assert_eq!(
        history(&backtest, "AAPL")
            .column("close")
            .unwrap()
            .f64()
            .unwrap()
            .get(0),
        Some(100.0)
    );

    assert!(backtest.step().unwrap());
    assert_eq!(backtest.now(), Some(200));
    assert_eq!(history(&backtest, "AAPL").height(), 1);
    assert_eq!(
        history(&backtest, "AAPL")
            .column("close")
            .unwrap()
            .f64()
            .unwrap()
            .get(0),
        Some(101.5)
    );

    assert!(!backtest.step().unwrap());
}

#[test]
fn quote_returns_requested_bid_ask_levels() {
    let backtest = backtest();
    backtest.step().unwrap();

    let quote = backtest
        .quote(&instrument("AAPL"), 1)
        .unwrap()
        .expect("quote");
    assert_eq!(quote.timestamp, 100);
    assert_eq!(quote.levels.len(), 1);
    assert_eq!(quote.levels[0].bid, Some(99.0));
    assert_eq!(quote.levels[0].ask, Some(101.0));

    assert!(backtest.quote(&instrument("AAPL"), 2).unwrap().is_none());
}

#[test]
fn midprice_dataset_returns_one_synthetic_level() {
    let backtest = BacktestInterface::new(mid_frame(), TickView::new("timestamp"));
    backtest.step().unwrap();

    let quote = backtest
        .quote(&instrument("AAPL"), 1)
        .unwrap()
        .expect("quote");
    assert_eq!(quote.levels.len(), 1);
    assert_eq!(quote.levels[0].bid, Some(100.0));
    assert_eq!(quote.levels[0].ask, Some(100.0));

    assert!(backtest.quote(&instrument("AAPL"), 2).unwrap().is_none());
}

#[test]
fn numbered_columns_supply_deeper_levels() {
    let backtest = BacktestInterface::new(levels_frame(), TickView::new("timestamp"));
    backtest.step().unwrap();

    let quote = backtest
        .quote(&instrument("AAPL"), 2)
        .unwrap()
        .expect("two levels");
    assert_eq!(quote.levels[0].bid, Some(99.0));
    assert_eq!(quote.levels[1].bid, Some(98.0));
    assert_eq!(quote.levels[1].ask, Some(102.0));

    assert!(backtest.quote(&instrument("AAPL"), 3).unwrap().is_none());
}

#[test]
fn market_order_matches_resting_ask_and_notifies() {
    let backtest = backtest();
    backtest.step().unwrap();
    backtest
        .submit_order(limit_order(Side::Sell, 101.0, 10.0))
        .unwrap();

    let updates = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&updates);
    backtest
        .on_order_update(Box::new(move |update| {
            sink.lock().unwrap().push(update.clone());
        }))
        .unwrap();

    let id = backtest
        .submit_order(market_order(Side::Buy, 10.0))
        .unwrap();
    assert_eq!(id, OrderId(2));

    let executions = backtest.executions().unwrap();
    let taker: Vec<_> = executions
        .iter()
        .filter(|execution| execution.order_id == id)
        .collect();
    assert_eq!(taker.len(), 1);
    assert_eq!(taker[0].timestamp, 100);
    assert_eq!(taker[0].price, 101.0);
    assert_eq!(taker[0].quantity, 10.0);

    let orders = backtest.orders().unwrap();
    let order = orders
        .iter()
        .find(|record| record.id == id)
        .expect("taker record");
    assert_eq!(order.status, OrderStatus::Filled);
    assert_eq!(order.filled_quantity, 10.0);

    let updates = updates.lock().unwrap();
    assert!(updates.iter().any(|update| update.order_id == id));
}

#[test]
fn sell_order_matches_resting_bid() {
    let backtest = backtest();
    backtest.step().unwrap();
    backtest
        .submit_order(limit_order(Side::Buy, 99.0, 3.0))
        .unwrap();

    let id = backtest
        .submit_order(market_order(Side::Sell, 3.0))
        .unwrap();

    let executions = backtest.executions().unwrap();
    let taker = executions
        .iter()
        .find(|execution| execution.order_id == id)
        .expect("taker execution");
    assert_eq!(taker.price, 99.0);
}

#[test]
fn market_order_without_liquidity_is_rejected() {
    let backtest = backtest();
    backtest.step().unwrap();

    let id = backtest.submit_order(market_order(Side::Buy, 2.0)).unwrap();

    let orders = backtest.orders().unwrap();
    assert!(matches!(
        orders.iter().find(|record| record.id == id).unwrap().status,
        OrderStatus::Rejected(_)
    ));
    assert!(backtest.executions().unwrap().is_empty());
}

#[test]
fn limit_orders_rest_pending() {
    let backtest = backtest();
    backtest.step().unwrap();

    backtest
        .submit_order(limit_order(Side::Buy, 100.0, 1.0))
        .unwrap();

    assert!(backtest.executions().unwrap().is_empty());
    assert_eq!(backtest.orders().unwrap()[0].status, OrderStatus::Pending);
}

#[test]
fn maker_record_accumulates_partial_fills() {
    let backtest = backtest();
    backtest.step().unwrap();

    let buy_id = backtest
        .submit_order(limit_order(Side::Buy, 101.0, 10.0))
        .unwrap();
    assert_eq!(backtest.orders().unwrap()[0].status, OrderStatus::Pending);

    backtest
        .submit_order(limit_order(Side::Sell, 101.0, 4.0))
        .unwrap();
    let buy = |backtest: &BacktestInterface<TickView>| {
        backtest
            .orders()
            .unwrap()
            .into_iter()
            .find(|record| record.id == buy_id)
            .unwrap()
    };
    assert_eq!(buy(&backtest).status, OrderStatus::PartiallyFilled);
    assert_eq!(buy(&backtest).filled_quantity, 4.0);

    backtest
        .submit_order(limit_order(Side::Sell, 101.0, 6.0))
        .unwrap();
    assert_eq!(buy(&backtest).status, OrderStatus::Filled);
    assert_eq!(buy(&backtest).filled_quantity, 10.0);
}

#[test]
fn date_column_is_supported_as_timestamp() {
    let backtest = BacktestInterface::new(date_frame(), DailyView::new("date"));

    assert!(backtest.step().unwrap());
    assert_eq!(backtest.now(), Some(19000));

    let quote = backtest
        .quote(&instrument("AAPL"), 1)
        .unwrap()
        .expect("quote");
    assert_eq!(quote.timestamp, 19000);
    assert_eq!(quote.levels[0].bid, Some(99.0));
    assert_eq!(quote.levels[0].ask, Some(101.0));
}

#[test]
fn panel_view_scopes_each_step_to_one_instrument() {
    let df = DataFrame::new(vec![
        Column::new(
            "date".into(),
            Series::new("date".into(), &[19000i32, 19000, 19001, 19001])
                .cast(&DataType::Date)
                .unwrap(),
        ),
        Column::new("symbol".into(), &["AAPL", "MSFT", "AAPL", "MSFT"]),
        Column::new("bid".into(), &[99.0f64, 199.0, 100.0, 200.0]),
        Column::new("ask".into(), &[101.0f64, 201.0, 102.0, 202.0]),
    ])
    .unwrap();

    let backtest = BacktestInterface::new(df, PanelView::new("date", "symbol"));

    assert!(backtest.step().unwrap());
    assert_eq!(backtest.now(), Some(19000));
    let quote = backtest
        .quote(&instrument("AAPL"), 1)
        .unwrap()
        .expect("quote");
    assert_eq!(quote.levels[0].bid, Some(99.0));

    assert!(backtest.step().unwrap());
    let quote = backtest
        .quote(&instrument("MSFT"), 1)
        .unwrap()
        .expect("quote");
    assert_eq!(quote.levels[0].bid, Some(199.0));
}

#[test]
fn listen_yields_view_steps_with_frame_and_timestamp() {
    let backtest = backtest();

    let steps = backtest
        .listen(None)
        .collect::<Result<Vec<_>, Error>>()
        .unwrap();

    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].timestamp, Some(100));
    assert_eq!(
        steps[0]
            .frame
            .column("close")
            .unwrap()
            .f64()
            .unwrap()
            .get(0),
        Some(100.0)
    );
    assert_eq!(steps[1].timestamp, Some(200));
    assert_eq!(
        steps[1]
            .frame
            .column("close")
            .unwrap()
            .f64()
            .unwrap()
            .get(0),
        Some(101.5)
    );
    assert_eq!(backtest.now(), Some(200));
}

#[test]
fn listen_observer_can_submit_orders() {
    let backtest = backtest();
    backtest
        .submit_order(limit_order(Side::Sell, 101.0, 2.0))
        .unwrap();
    let observer: StepObserver<'_, TickView> = Box::new(|backtest, _step| {
        if backtest.now() == Some(100) {
            backtest.submit_order(market_order(Side::Buy, 2.0))?;
        }
        Ok(())
    });

    let _ = backtest
        .listen(Some(observer))
        .collect::<Result<Vec<_>, Error>>()
        .unwrap();

    let executions = backtest.executions().unwrap();
    assert!(
        executions
            .iter()
            .any(|execution| execution.order_id == OrderId(2) && execution.price == 101.0)
    );
}

struct BuyOnce;

impl Strategy for BuyOnce {
    type Input = DataFrame;
    type State = bool;
    type Output = Option<Signal>;

    fn on_step(
        &self,
        _step: &DataFrame,
        _history: &DataFrame,
        bought: &mut bool,
    ) -> Option<Signal> {
        if *bought {
            return None;
        }
        *bought = true;
        Some(Signal {
            symbol: None,
            shares: 10.0,
        })
    }
}

#[test]
fn executor_consumes_listen_generator() {
    let backtest = backtest();
    backtest.step().unwrap();
    backtest
        .submit_order(limit_order(Side::Sell, 101.0, 10.0))
        .unwrap();

    let engine = BaseOrderEngine::new().with_symbol("AAPL");
    let mut executor = SyncExecutor::new(engine, BuyOnce);

    let orders = executor.run(backtest.listen(None), &backtest).unwrap();

    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].symbol, "AAPL");
    assert_eq!(orders[0].quantity, OrderQuantity::Abs(10.0));

    let executions = backtest.executions().unwrap();
    assert!(
        executions
            .iter()
            .any(|execution| execution.order_id == OrderId(2) && execution.price == 101.0)
    );
}

#[test]
fn listen_yields_only_the_current_view_entry() {
    let backtest = backtest();
    let mut steps = 0;

    for step in backtest.listen(None) {
        let step = step.unwrap();
        assert_eq!(step.frame.height(), 1);
        assert_eq!(step.input.height(), 1);
        steps += 1;
    }

    assert_eq!(steps, 2);
}

#[test]
fn account_starts_empty_and_has_no_events() {
    let backtest = backtest();

    let portfolio = backtest.portfolio("U123").unwrap();
    assert!(portfolio.metrics.is_empty());
    assert_eq!(portfolio.holding_count(), 0);

    AccountInterface::listen_async(&backtest, "U123").unwrap();
    assert!(backtest.events("U123").unwrap().is_empty());
}
