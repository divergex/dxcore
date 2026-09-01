# Getting started

dxcore is a Rust library for market data, portfolios, and trading logic. The workspace holds two crates. `crates/core` is the library itself. `crates/pybindings` exposes it to Python. This guide covers the `DataFrame` and the trading executor. The API reference documents every item in detail.

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

A strategy implements the `Strategy` trait. The executor feeds it one step at a time, in the order the view produces them. Here is a minimal example for a strategy, called `EchoClose`, where the strategy just records the close price of each step:

```rust
use dxcore::trading::{Strategy, SyncExecutor, TickView};
use polars::prelude::*;

struct EchoClose;

impl Strategy for EchoClose {
    type Input = DataFrame;
    type State = ();
    type Output = f64;
    type Frame = Vec<f64>;

    fn on_step(&self, step: &DataFrame, _history: &DataFrame, _state: &mut ()) -> f64 {
        step.column("close").unwrap().f64().unwrap().get(0).unwrap()
    }

    fn create_output(&self) -> Vec<f64> {
        Vec::new()
    }

    fn append_output(&self, frame: &mut Vec<f64>, output: f64, _step: &DataFrame) {
        frame.push(output);
    }
}

let df = dxcore::DataFrame::new(polars::prelude::DataFrame::new(vec![
    Column::new("close".into(), &[149.0f64, 150.0, 151.0]),
])
.unwrap());

let mut executor = SyncExecutor::new(EchoClose);
let closes = executor.run(&df, TickView::new("ts"));

assert_eq!(closes, vec![149.0, 150.0, 151.0]);
```

`DailyView` works the same way, but groups rows by date and emits `(date, frame)` pairs. The crate ships a ready-made `SmaCross` strategy behind the `strategies` feature. `crates/core/examples/strategy.rs` runs it end to end:

```bash
cargo run --example strategy --features strategies
```

## Where to go next

[`crate::trading`] holds the executor, views, and the `Strategy` trait. [`crate::interface`] covers market data sources, including the Interactive Brokers client. [`crate::network`] handles services and meshes. [`crate::core`] has instruments, portfolios, and the instrument store.
