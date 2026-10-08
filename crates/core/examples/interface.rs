mod utils;

use std::sync::Arc;

use polars::prelude::DataFrame;

use dxcore::interface::internal::{
    http_on_step, HttpAccessor, InterfaceFactory, StepArgs, StrategyInterface,
};
use dxcore::network::mesh::Protocol;
use dxcore::network::servers::HttpServer;
use dxcore::network::services::FunctionalService;
use dxcore::strategies::SmaCross;
use dxcore::trading::{Signal, Strategy, View};
use utils::{daily_view, generate_ohlc};

struct Runner {
    strategy: SmaCross,
    state: <SmaCross as Strategy>::State,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let df = generate_ohlc(30);
    let view = daily_view();

    let runner = Runner {
        strategy: SmaCross::new(10_000.0, 5, 10),
        state: Default::default(),
    };
    let service = FunctionalService::new("strategy", runner).with_set(
        "on_step",
        |runner: &mut Runner, args: StepArgs| {
            let (step, history) = args.into_parts();
            Ok(runner
                .strategy
                .on_step(&step, &history, &mut runner.state))
        },
    );

    let server = HttpServer::bind("127.0.0.1:0", Arc::new(service))?;
    let addr = server.addr();
    let handle = server.spawn();
    println!("serving the strategy at http://{addr}");

    let interface = InterfaceFactory::new("strategy")
        .accessor(Protocol::Http, HttpAccessor::new(format!("http://{addr}")))
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
