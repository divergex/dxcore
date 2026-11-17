use std::sync::Arc;

use polars::prelude::*;

use dxcore::core::Instrument;
use dxcore::interface::{
    AccountInterface, BacktestInterface, BacktestService, MarketInterface, OrderInterface,
    OrderStatus, Span, TradingProxy,
};
use dxcore::network::servers::HttpServer;
use dxcore::trading::{
    BaseOrderEngine, DailyView, Order, OrderPrice, OrderQuantity, OrderType, Side, Signal, Strategy,
    SyncExecutor,
};

fn frame() -> DataFrame {
    DataFrame::new(vec![
        Column::new(
            "date".into(),
            Series::new("date".into(), &[19000i32, 19001, 19002])
                .cast(&DataType::Date)
                .unwrap(),
        ),
        Column::new("symbol".into(), &["DEMO", "DEMO", "DEMO"]),
        Column::new("close".into(), &[100.0f64, 101.0, 102.0]),
    ])
    .unwrap()
}

struct BuyOnce;

impl Strategy for BuyOnce {
    type Input = (i32, DataFrame);
    type State = ();
    type Output = Option<Signal>;

    fn on_step(
        &self,
        _step: &(i32, DataFrame),
        _history: &DataFrame,
        _state: &mut (),
    ) -> Option<Signal> {
        Some(Signal {
            symbol: Some("DEMO".into()),
            shares: 1.0,
        })
    }
}

#[test]
fn served_backtest_runs_a_local_strategy() {
    let view = DailyView::new("date").with_col_map(vec![("close".into(), "price".into())]);
    let backtest = BacktestInterface::new(frame(), view);
    let server = HttpServer::bind(
        "127.0.0.1:0",
        Arc::new(BacktestService::new(backtest).service()),
    )
    .unwrap();
    let base = format!("http://{}", server.addr());
    let handle = server.spawn();

    let proxy = TradingProxy::connect(base).unwrap();
    assert_eq!(proxy.now().unwrap(), None);

    let maker = proxy
        .submit_order(Order {
            date: None,
            symbol: "DEMO".into(),
            side: Side::Sell,
            quantity: OrderQuantity::Abs(10.0),
            order_type: OrderType::Limit,
            price: OrderPrice::Px(100.0),
        })
        .unwrap();
    assert_eq!(maker.0, 1);

    let mut executor = SyncExecutor::new(BaseOrderEngine::new(), BuyOnce);
    let orders = executor
        .run(proxy.listen::<(i32, DataFrame)>(), &proxy)
        .unwrap();

    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].symbol, "DEMO");
    assert_eq!(orders[0].quantity, OrderQuantity::Abs(1.0));

    let records = proxy.orders().unwrap();
    assert_eq!(records.len(), 2);
    assert!(
        records
            .iter()
            .any(|record| record.status == OrderStatus::Filled)
    );
    assert!(!proxy.executions().unwrap().is_empty());
    assert_eq!(proxy.now().unwrap(), Some(19002));

    let instrument = Instrument {
        contract_id: 1,
        symbol: "DEMO".into(),
        security_type: "STK".into(),
        exchange: "SMART".into(),
        currency: "USD".into(),
    };
    let frame = proxy
        .market_history(&instrument, Span::new().days(1), Span::new().days(1))
        .unwrap();
    assert_eq!(frame.height(), 1);
    assert_eq!(frame.column("price").unwrap().f64().unwrap().get(0), Some(102.0));

    assert!(proxy.portfolio("U123").is_err());

    handle.stop().unwrap();
}
