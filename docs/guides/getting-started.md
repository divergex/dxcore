# Getting started

dxcore is a Rust library for market data, portfolios, and trading logic. The workspace holds two crates. `crates/core` is the library itself. `crates/pybindings` exposes it to Python. This guide covers the `DataFrame`, the order engine, and the trading executors. The API reference documents every item in detail.

## The DataFrame

dxcore exposes data as `dxcore::DataFrame`, a wrapper around a polars `DataFrame`. It derefs to the inner frame, so the full polars column and row API stays available.

```rust
use dxcore::DataFrame;
use polars::prelude::*;

let df = DataFrame::new(polars::prelude::DataFrame::new(vec![
    Column::new("date".into(), &[20240528i32, 20240529]),
    Column::new("close".into(), &[149.0f64, 150.0]),
])
.unwrap());

assert_eq!(df.height(), 2);
assert_eq!(df.column("close").unwrap().f64().unwrap().get(1), Some(150.0));
```

`into_inner()` returns the polars frame when you need it back.

## Run a strategy

A strategy decides *what* it wants and nothing else. It declares its output type — `Signal`, `OrderData`, `Option` of either, or a type an engine of yours understands — and returns it from `on_step`:

```rust
use dxcore::trading::{Signal, Strategy};
use polars::prelude::*;

/// Buys 10 shares on the first step, then holds.
struct BuyOnce;

impl Strategy for BuyOnce {
    type Input = DataFrame;
    type State = bool;
    type Output = Option<Signal>;

    fn on_step(&self, _step: &DataFrame, _history: &DataFrame, state: &mut bool) -> Option<Signal> {
        if *state {
            return None;
        }
        *state = true;
        Some(Signal { symbol: None, shares: 10.0 })
    }
}

let mut state = false;
let step = DataFrame::empty();
assert!(BuyOnce.on_step(&step, &step, &mut state).is_some());
assert!(BuyOnce.on_step(&step, &step, &mut state).is_none());
```

A `Signal` is a target, not an order: it says how much of the instrument the strategy wants to hold once the step is done. Turning that into something executable is the engine's job, so the same strategy runs through different engines without changing a line:

```rust
use dxcore::trading::{
    BaseOrderEngine, OrderQuantity, Side, Signal, Strategy, SyncExecutor, TickView,
};
use polars::prelude::*;

struct BuyOnce;

impl Strategy for BuyOnce {
    type Input = DataFrame;
    type State = bool;
    type Output = Option<Signal>;

    fn on_step(&self, _step: &DataFrame, _history: &DataFrame, state: &mut bool) -> Option<Signal> {
        if *state {
            return None;
        }
        *state = true;
        Some(Signal { symbol: None, shares: 10.0 })
    }
}

let df = dxcore::DataFrame::new(polars::prelude::DataFrame::new(vec![
    Column::new("close".into(), &[149.0f64, 150.0, 151.0]),
])
.unwrap());

let engine = BaseOrderEngine::new().with_symbol("AAPL");
let mut executor = SyncExecutor::new(engine, BuyOnce);
let orders = executor.run_static(&df, TickView::new("ts")).unwrap();

assert_eq!(orders.len(), 1);
assert_eq!(orders[0].symbol, "AAPL");
assert_eq!(orders[0].side, Side::Buy);
assert_eq!(orders[0].quantity, OrderQuantity::Abs(10.0));
```

The executor ends up with orders and nothing else: steps that imply no order leave the frame untouched, so the second and third steps above contribute nothing.

## Writing orders directly

`Signal` covers "get me to this position". When a strategy wants to state the instruction itself — size, order type, price — it emits `OrderData` instead, and the engine passes it through after validating it:

```rust
use dxcore::trading::{
    BaseOrderEngine, BaseOrderEngineState, OrderData, OrderEngine, OrderPrice, OrderQuantity,
    OrderType, Side,
};
use polars::prelude::*;

let engine = BaseOrderEngine::new();
let mut state = BaseOrderEngineState::default();
let step = (19000i32, DataFrame::empty());

let order = engine
    .transform(
        OrderData {
            symbol: Some("AAPL".into()),
            quantity: OrderQuantity::Abs(-50.0), // signed: negative sells
            order_type: OrderType::Limit,
            price: OrderPrice::Px(151.0),
        },
        &step,
        Some(19000),
        &mut state,
    )
    .unwrap()
    .unwrap();

assert_eq!(order.side, Side::Sell);
assert_eq!(order.quantity, OrderQuantity::Abs(50.0));
assert_eq!(order.order_type, OrderType::Limit);
```

`BaseOrderEngine` resolves what a strategy leaves implicit and rejects what cannot be traded as stated:

- the instrument comes from the output, else from `BaseOrderEngine::with_symbol`;
- `Signal` targets are diffed against a per-symbol position ledger, so they are idempotent, while an `OrderData` size is a delta — `Abs` sizes move that ledger, `Pct` sizes are passed through unresolved because turning a fraction into shares needs account equity;
- a market order must carry `OrderPrice::Mkt`, a limit or stop order a finite positive `OrderPrice::Px`.

Anything it cannot accept comes back as an `OrderError` (missing instrument, non-finite size, incoherent type/price pair) rather than a silently dropped order.

## Views and the shipped strategies

`DailyView` works the same way, but groups rows by date and emits `(date, frame)` pairs; `PanelView` slices by date *and* symbol. The crate ships a ready-made `SmaCross` strategy behind the `strategies` feature. `crates/core/examples/strategy.rs` runs it end to end:

```bash
cargo run --example strategy --features strategies
```

## Backtesting

`BacktestInterface` takes a `View` (and an optional tick size via `with_tick_size`). The view owns the dataset's timestamp and column layout; the interface stores the view's steps, advances through them with the `step` helper, and exposes them through the `listen` iterator as `BacktestStep` values containing the view input, current frame, and timestamp. Market data is available through `market_history` and `quote`. Orders are matched by `trading::OrderBook` and tracked through `OrderInterface`; `on_order_update` registers a callback. A midprice-only dataset yields one synthetic quote level with `bid == ask`.

`QuoteSchema` controls how bid/ask, numbered book levels, or a single midprice column are read from the current view step.

## Serving a backtest

A backtest can stay local or become a service without changing the client code: `BacktestService` hosts the interface and `TradingProxy` implements the same traits against the served one. Naming follows the role: `Interface` is the local object, `Service` hosts it, and `Proxy` is the local stand-in that forwards to a service. The same scheme applies to any trading object, so a proxy can connect to a backtest, a broker adapter, or another served interface.

```rust,no_run
use std::sync::Arc;

use dxcore::interface::{BacktestInterface, BacktestService, OrderInterface, TradingProxy};
use dxcore::network::servers::HttpServer;
use dxcore::trading::{
    DailyView, Order, OrderPrice, OrderQuantity, OrderType, Side,
};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let history = polars::prelude::DataFrame::empty();
let backtest = BacktestInterface::new(history, DailyView::new("date"));

let server = HttpServer::bind(
    "127.0.0.1:0",
    Arc::new(BacktestService::new(backtest).service()),
)?;
let addr = server.addr();
let handle = server.spawn();

let proxy = TradingProxy::connect(format!("http://{addr}"))?;
let order = Order {
    date: None,
    symbol: "DEMO".into(),
    side: Side::Buy,
    quantity: OrderQuantity::Abs(1.0),
    order_type: OrderType::Market,
    price: OrderPrice::Mkt,
};
proxy.submit_order(order)?; // POST /submit_order creates an order
let step = proxy.step::<(i32, polars::prelude::DataFrame)>()?; // PUT /step advances
# let _ = step;
# handle.stop()?;
# Ok(())
# }
```

The service exposes `market_history`, `quote`, `submit_order` (`POST`), `orders`, `executions`, and the backtest clock (`PUT step`, `GET now`). It deliberately does not serve `portfolio`: account state stays in the account service and is read-only from a trading interface. Each hosted backtest owns one cursor, so a run takes exactly one client.

`listen` yields only the current view entry (`BacktestStep { input, frame, timestamp }`), never the accumulated history; executors build history client-side by appending each step frame.

## Where to go next

[`crate::trading`] holds the executors, views, the `Strategy` trait, the order types and the `OrderEngine` trait. [`crate::interface`] covers market data sources, including the Interactive Brokers client. [`crate::network`] handles services and meshes. [`crate::core`] has instruments, portfolios, and the instrument store.
