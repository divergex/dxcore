mod utils;

use dxcore::interface::internal::{http_on_step, HttpAccessor, InterfaceFactory, StrategyInterface};
use dxcore::network::mesh::Protocol;
use dxcore::strategies::SmaCross;
use dxcore::trading::{BaseOrderEngine, Signal, SyncExecutor};
use utils::{daily_view, generate_ohlc, serve};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let df = generate_ohlc(30);

    let (base, handle) = serve(SmaCross::new(10_000.0, 5, 10))?;
    println!("serving the strategy at {base}");

    let interface = InterfaceFactory::new("strategy")
        .accessor(Protocol::Http, HttpAccessor::new(base))
        .set("on_step", Protocol::Http, http_on_step)
        .build()?;

    let strategy = StrategyInterface::<Option<Signal>>::new(interface);
    let mut executor = SyncExecutor::new(BaseOrderEngine::new(), strategy);
    let orders = executor.run(&df, daily_view())?;

    handle.stop()?;
    if let Some(error) = executor.strategy.take_error() {
        return Err(error.into());
    }

    println!("=== Orders ===\n{orders:#?}");

    Ok(())
}
