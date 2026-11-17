mod utils;

use std::sync::Arc;

use polars::prelude::DataFrame;

use dxcore::interface::{BacktestInterface, BacktestService, OrderInterface, TradingProxy};
use dxcore::network::servers::HttpServer;
use dxcore::trading::{
    BaseOrderEngine, Order, OrderPrice, OrderQuantity, OrderType, Side, Signal, Strategy,
    SyncExecutor,
};

use utils::{daily_view, generate_ohlc};

struct BuyAndHold;

impl Strategy for BuyAndHold {
    type Input = (i32, DataFrame);
    type State = bool;
    type Output = Option<Signal>;

    fn on_step(
        &self,
        _step: &(i32, DataFrame),
        _history: &DataFrame,
        state: &mut bool,
    ) -> Option<Signal> {
        if *state {
            return None;
        }
        *state = true;
        Some(Signal {
            symbol: Some("DEMO".into()),
            shares: 10.0,
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backtest = BacktestInterface::new(generate_ohlc(30), daily_view());
    let server = HttpServer::bind(
        "127.0.0.1:0",
        Arc::new(BacktestService::new(backtest).service()),
    )?;
    let addr = server.addr();
    let handle = server.spawn();
    println!("serving the backtest at http://{addr}");

    let proxy = TradingProxy::connect(format!("http://{addr}"))?;

    proxy.submit_order(Order {
        date: None,
        symbol: "DEMO".into(),
        side: Side::Sell,
        quantity: OrderQuantity::Abs(10.0),
        order_type: OrderType::Limit,
        price: OrderPrice::Px(100.0),
    })?;

    let mut executor = SyncExecutor::new(BaseOrderEngine::new(), BuyAndHold);
    let orders = executor.run(proxy.listen::<(i32, DataFrame)>(), &proxy)?;

    println!("\nsent orders:\n{orders:#?}");
    println!("\norder records:\n{:#?}", proxy.orders()?);
    println!("\nexecutions:\n{:#?}", proxy.executions()?);

    handle.stop()?;
    Ok(())
}
