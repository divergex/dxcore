use polars::prelude::*;

use dxcore::trading::{
    BaseOrderEngine, BaseOrderEngineState, DailyView, OrderData, OrderEngine, OrderError,
    OrderPrice, OrderQuantity, OrderType, Side, Signal, Strategy, SyncExecutor, TickView,
};

use super::helpers;

/// Replays a fixed script of intents, one per step.
struct Script(Vec<Option<Signal>>);

impl Strategy for Script {
    type Input = (i32, DataFrame);
    type State = usize;
    type Output = Option<Signal>;

    fn on_step(
        &self,
        _step: &(i32, DataFrame),
        _history: &DataFrame,
        state: &mut usize,
    ) -> Option<Signal> {
        let output = self.0[*state].clone();
        *state += 1;
        output
    }
}

fn target(shares: f64) -> Option<Signal> {
    Some(Signal { symbol: None, shares })
}

fn step() -> (i32, DataFrame) {
    (19000, DataFrame::empty())
}

#[test]
fn target_signal_produces_delta_orders() {
    let df = helpers::ohlc_df();
    let view = DailyView::new("date");
    let script = Script(vec![
        target(100.0),
        target(40.0),  // sells 60 of the 100 held
        target(40.0),  // already there: nothing to do
    ]);

    let mut executor = SyncExecutor::new(BaseOrderEngine::new().with_symbol("AAPL"), script);
    let orders = executor.run(&df, view).unwrap();

    assert_eq!(orders.len(), 2, "unchanged target must not trade: {orders:?}");
    assert_eq!(orders[0], dxcore::trading::Order {
        date: Some(19000),
        symbol: "AAPL".into(),
        side: Side::Buy,
        quantity: OrderQuantity::Abs(100.0),
        order_type: OrderType::Market,
        price: OrderPrice::Mkt,
    });
    assert_eq!(orders[1].date, Some(19001));
    assert_eq!(orders[1].side, Side::Sell);
    assert_eq!(orders[1].quantity, OrderQuantity::Abs(60.0));
}

#[test]
fn targets_are_tracked_per_symbol() {
    let engine = BaseOrderEngine::new();
    let mut state = BaseOrderEngineState::default();
    let step = step();
    let signal = |symbol: &str, shares: f64| {
        Some(Signal { symbol: Some(symbol.into()), shares })
    };

    let buy_a = engine
        .transform(signal("A", 100.0), &step, None, &mut state)
        .unwrap();
    let buy_b = engine
        .transform(signal("B", 50.0), &step, None, &mut state)
        .unwrap();
    let hold_a = engine
        .transform(signal("A", 100.0), &step, None, &mut state)
        .unwrap();

    assert_eq!(buy_a.unwrap().quantity, OrderQuantity::Abs(100.0));
    assert_eq!(buy_b.unwrap().quantity, OrderQuantity::Abs(50.0));
    assert!(hold_a.is_none(), "A's target did not move");
    assert_eq!(state.positions["A"], 100.0);
    assert_eq!(state.positions["B"], 50.0);
}

#[test]
fn output_symbol_wins_over_engine_symbol() {
    let engine = BaseOrderEngine::new().with_symbol("AAPL");
    let mut state = BaseOrderEngineState::default();
    let step = step();

    let order = engine
        .transform(
            Some(Signal { symbol: Some("MSFT".into()), shares: 10.0 }),
            &step,
            None,
            &mut state,
        )
        .unwrap()
        .unwrap();

    assert_eq!(order.symbol, "MSFT");
    assert_eq!(state.positions["MSFT"], 10.0);
    assert!(!state.positions.contains_key("AAPL"));
}

#[test]
fn missing_symbol_is_rejected() {
    let engine = BaseOrderEngine::new();
    let mut state = BaseOrderEngineState::default();

    let err = engine
        .transform(target(10.0), &step(), None, &mut state)
        .unwrap_err();

    assert_eq!(err, OrderError::MissingSymbol);
    assert!(state.positions.is_empty());
}

#[test]
fn order_data_passes_through_as_signed_quantity() {
    let engine = BaseOrderEngine::new();
    let mut state = BaseOrderEngineState::default();

    let order = engine
        .transform(
            OrderData {
                symbol: Some("AAPL".into()),
                quantity: OrderQuantity::Abs(-250.0),
                order_type: OrderType::Limit,
                price: OrderPrice::Px(101.5),
            },
            &step(),
            Some(19000),
            &mut state,
        )
        .unwrap()
        .unwrap();

    assert_eq!(order.side, Side::Sell);
    assert_eq!(order.quantity, OrderQuantity::Abs(250.0));
    assert_eq!(order.order_type, OrderType::Limit);
    assert_eq!(order.price, OrderPrice::Px(101.5));
    assert_eq!(order.date, Some(19000));
    assert_eq!(state.positions["AAPL"], -250.0, "Abs size books onto the ledger");
}

#[test]
fn pct_order_stays_unresolved_and_untracked() {
    let engine = BaseOrderEngine::new();
    let mut state = BaseOrderEngineState::default();

    let order = engine
        .transform(
            OrderData {
                symbol: Some("AAPL".into()),
                quantity: OrderQuantity::Pct(0.5),
                order_type: OrderType::Market,
                price: OrderPrice::Mkt,
            },
            &step(),
            None,
            &mut state,
        )
        .unwrap()
        .unwrap();

    assert_eq!(order.side, Side::Buy);
    assert_eq!(order.quantity, OrderQuantity::Pct(0.5));
    assert!(state.positions.is_empty(), "a fraction is not a share count");
}

#[test]
fn abs_orders_move_the_position_a_later_target_diffs_against() {
    let engine = BaseOrderEngine::new().with_symbol("AAPL");
    let mut state = BaseOrderEngineState::default();
    let step = step();

    engine
        .transform(
            OrderData {
                symbol: None,
                quantity: OrderQuantity::Abs(100.0),
                order_type: OrderType::Market,
                price: OrderPrice::Mkt,
            },
            &step,
            None,
            &mut state,
        )
        .unwrap();

    let next = engine.transform(target(100.0), &step, None, &mut state).unwrap();
    assert!(next.is_none(), "engine already holds the target");
}

#[test]
fn incoherent_price_specs_are_rejected() {
    let engine = BaseOrderEngine::new().with_symbol("AAPL");
    let mut state = BaseOrderEngineState::default();

    let market_with_price = engine.transform(
        OrderData {
            symbol: None,
            quantity: OrderQuantity::Abs(1.0),
            order_type: OrderType::Market,
            price: OrderPrice::Px(100.0),
        },
        &step(),
        None,
        &mut state,
    );
    assert_eq!(
        market_with_price.unwrap_err(),
        OrderError::IncoherentPrice {
            order_type: OrderType::Market,
            price: OrderPrice::Px(100.0)
        }
    );

    let limit_without_price = engine.transform(
        OrderData {
            symbol: None,
            quantity: OrderQuantity::Abs(1.0),
            order_type: OrderType::Limit,
            price: OrderPrice::Mkt,
        },
        &step(),
        None,
        &mut state,
    );
    assert_eq!(
        limit_without_price.unwrap_err(),
        OrderError::IncoherentPrice {
            order_type: OrderType::Limit,
            price: OrderPrice::Mkt
        }
    );
}

#[test]
fn stop_order_needs_a_positive_finite_price() {
    let engine = BaseOrderEngine::new().with_symbol("AAPL");
    let mut state = BaseOrderEngineState::default();

    let err = engine
        .transform(
            OrderData {
                symbol: None,
                quantity: OrderQuantity::Abs(1.0),
                order_type: OrderType::StopLoss,
                price: OrderPrice::Px(-1.0),
            },
            &step(),
            None,
            &mut state,
        )
        .unwrap_err();

    assert_eq!(err, OrderError::InvalidPrice(-1.0));
}

#[test]
fn non_finite_quantities_are_rejected() {
    let engine = BaseOrderEngine::new().with_symbol("AAPL");
    let mut state = BaseOrderEngineState::default();

    let err = engine
        .transform(
            OrderData {
                symbol: None,
                quantity: OrderQuantity::Abs(f64::NAN),
                order_type: OrderType::Market,
                price: OrderPrice::Mkt,
            },
            &step(),
            None,
            &mut state,
        )
        .unwrap_err();

    assert!(matches!(err, OrderError::InvalidQuantity(v) if v.is_nan()));

    let err = engine
        .transform(target(f64::INFINITY), &step(), None, &mut state)
        .unwrap_err();
    assert_eq!(err, OrderError::InvalidQuantity(f64::INFINITY));
}

#[test]
fn zero_sized_intents_produce_no_order() {
    let engine = BaseOrderEngine::new().with_symbol("AAPL");
    let mut state = BaseOrderEngineState::default();
    let step = step();

    let zero_order = engine.transform(
        OrderData {
            symbol: None,
            quantity: OrderQuantity::Abs(0.0),
            order_type: OrderType::Market,
            price: OrderPrice::Mkt,
        },
        &step,
        None,
        &mut state,
    );
    assert_eq!(zero_order, Ok(None));

    let flat = engine.transform(target(0.0), &step, None, &mut state);
    assert_eq!(flat, Ok(None));
}

#[test]
fn tick_view_steps_carry_no_ordinal() {
    struct BuyOnce;

    impl Strategy for BuyOnce {
        type Input = DataFrame;
        type State = usize;
        type Output = Option<Signal>;

        fn on_step(
            &self,
            _step: &DataFrame,
            _history: &DataFrame,
            state: &mut usize,
        ) -> Option<Signal> {
            let output = (*state == 0).then_some(Signal { symbol: None, shares: 10.0 });
            *state += 1;
            output
        }
    }

    let df = helpers::ohlc_df();

    let mut executor = SyncExecutor::new(BaseOrderEngine::new().with_symbol("AAPL"), BuyOnce);
    let orders = executor.run(&df, TickView::new("date")).unwrap();

    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].date, None);
}

#[test]
fn engine_errors_stop_the_backtest() {
    struct BadOrder;

    impl Strategy for BadOrder {
        type Input = DataFrame;
        type State = ();
        type Output = OrderData;

        fn on_step(&self, _step: &DataFrame, _history: &DataFrame, _state: &mut ()) -> OrderData {
            OrderData {
                symbol: Some("AAPL".into()),
                quantity: OrderQuantity::Abs(1.0),
                order_type: OrderType::Limit,
                price: OrderPrice::Mkt,
            }
        }
    }

    let df = helpers::ohlc_df();
    let mut executor = SyncExecutor::new(BaseOrderEngine::new(), BadOrder);
    let err = executor.run(&df, TickView::new("date")).unwrap_err();

    assert_eq!(
        err,
        OrderError::IncoherentPrice {
            order_type: OrderType::Limit,
            price: OrderPrice::Mkt
        }
    );
}
