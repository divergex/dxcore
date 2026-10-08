mod utils;

use dxcore::strategies::SmaCross;
use dxcore::trading::{BaseOrderEngine, SyncExecutor};
use utils::{daily_view, generate_ohlc};

fn main() {
    let df = generate_ohlc(30);
    println!("=== OHLC Data (source columns: 'close') ===\n{:?}\n", df.head(Some(5)));

    let engine = BaseOrderEngine::new();
    let strategy = SmaCross::new(10_000.0, 5, 10);
    let mut executor = SyncExecutor::new(engine, strategy);

    let orders = executor.run(&df, daily_view()).expect("backtest failed");

    println!("=== Orders ===\n{orders:#?}");
}
