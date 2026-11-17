use dxcore::core::Instrument;
use dxcore::interface::{BacktestInterface, MarketInterface, OrderInterface};
use dxcore::trading::{
    BaseOrderEngine, Order, OrderPrice, OrderQuantity, OrderType, Side, Signal, Strategy,
    SyncExecutor, TickView,
};
use polars::prelude::*;

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = DataFrame::new(vec![
        Column::new("timestamp".into(), &[100i64, 200, 300]),
        Column::new("symbol".into(), &["AAPL"; 3]),
        Column::new("bid".into(), &[99.0f64, 100.0, 102.0]),
        Column::new("ask".into(), &[101.0f64, 102.0, 104.0]),
        Column::new("close".into(), &[100.0f64, 101.0, 103.0]),
    ])?;

    let instrument = Instrument {
        contract_id: 1,
        symbol: "AAPL".into(),
        security_type: "STK".into(),
        exchange: "SMART".into(),
        currency: "USD".into(),
    };

    let backtest = BacktestInterface::new(data, TickView::new("timestamp"));

    // Seed the matching engine with the liquidity a market buy will cross.
    backtest.submit_order(Order {
        date: None,
        symbol: "AAPL".into(),
        side: Side::Sell,
        quantity: OrderQuantity::Abs(10.0),
        order_type: OrderType::Limit,
        price: OrderPrice::Px(101.0),
    })?;

    let engine = BaseOrderEngine::new().with_symbol("AAPL");
    let mut executor = SyncExecutor::new(engine, BuyOnce);

    let orders = executor.run(backtest.listen(None), &backtest)?;
    let executions = backtest.executions()?;

    println!("orders: {orders:#?}");
    println!("executions: {executions:#?}");

    let quote = backtest.quote(&instrument, 1)?.expect("final quote");
    println!("final timestamp: {:?}", backtest.now());
    println!("final quote: {quote:?}");

    Ok(())
}
