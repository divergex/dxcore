mod utils;

use polars::prelude::DataFrame;

use dxcore::interface::internal::{http_on_step, HttpAccessor, InterfaceFactory, StrategyInterface};
use dxcore::network::Protocol;
use dxcore::strategies::SmaCross;
use dxcore::trading::{Signal, Strategy, View};
use utils::{daily_view, generate_ohlc, serve};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let df = generate_ohlc(30);
    let view = daily_view();

    let (base, handle) = serve(SmaCross::new(10_000.0, 5, 10))?;
    println!("serving the strategy at {base}");

    let interface = InterfaceFactory::new("strategy")
        .accessor(Protocol::Http, HttpAccessor::new(base))
        .set("on_step", Protocol::Http, http_on_step)
        .build()?;
    let strategy = StrategyInterface::<Option<Signal>>::new(interface);

    let mut history = DataFrame::empty();
    let mut signals = Vec::new();
    for step in view.steps(&df) {
        if let Some(signal) = strategy.on_step(&step, &history, &mut ()) {
            signals.push((step.0, signal));
        }
        view.append(&mut history, &step);
    }

    handle.stop()?;
    if let Some(error) = strategy.take_error() {
        return Err(error.into());
    }

    println!("=== Signals ===\n{signals:#?}");

    Ok(())
}
