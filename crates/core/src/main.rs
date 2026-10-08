mod cli;

#[cfg(feature = "ibkr")]
use dxcore::interface::external::ibkr::IbkrInterface;
#[cfg(feature = "ibkr")]
use dxcore::interface::AccountInterface;
#[cfg(feature = "ibkr")]
use dxcore::Event;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "mesh" => {
                cli::mesh::run(&args[2..]);
                return;
            }
            #[cfg(feature = "strategies")]
            "strategies" => {
                run_strategies(&args[2..]);
                return;
            }
            _ => {
                eprintln!("unknown subcommand: {}", args[1]);
                print_usage();
                return;
            }
        }
    }

    #[cfg(feature = "ibkr")]
    run_ibkr();
}

fn print_usage() {
    eprint!("usage: dxcore");
    eprint!(" [mesh <start|stop>]");
    #[cfg(feature = "strategies")]
    eprint!(" [strategies <name> ...]");
    eprintln!();
}

#[cfg(feature = "strategies")]
fn run_strategies(args: &[String]) {
    use dxcore::strategies::SmaCross;
    use dxcore::trading::{BaseOrderEngine, DailyView, SyncExecutor};

    if args.is_empty() {
        println!("available strategies:");
        println!("  sma-cross  SMA crossover (short=5, long=10, cash=10000)");
        return;
    }

    match args[0].as_str() {
        "sma-cross" => {
            let df = generate_ohlc(30);
            println!("=== OHLC Data ===\n{:?}\n", df.head(Some(5)));

            let view = DailyView::new("date").with_col_map(vec![("close".into(), "price".into())]);

            let engine = BaseOrderEngine::new();
            let strategy = SmaCross::new(10_000.0, 5, 10);
            let mut executor = SyncExecutor::new(engine, strategy);
            let orders = executor.run(&df, view).expect("backtest failed");

            println!("=== Orders ===\n{orders:#?}");
        }
        name => {
            eprintln!("unknown strategy: {name}");
            eprintln!("available: sma-cross");
        }
    }
}

#[cfg(feature = "strategies")]
fn generate_ohlc(n_days: usize) -> polars::prelude::DataFrame {
    use polars::prelude::*;

    let prices: Vec<f64> = (0..n_days)
        .map(|i| {
            let t = i as f64;
            let trend = 100.0 + t * 0.3;
            let wave = 5.0 * (t * 0.4).sin() - 3.0 * (t * 0.15).cos();
            let dip = -15.0 * (-((t - 15.0).powi(2) / 30.0)).exp();
            ((trend + wave + dip) * 100.0).round() / 100.0
        })
        .collect();

    let base_date = 19000i32;
    let dates: Vec<i32> = (0..n_days).map(|i| base_date + i as i32).collect();
    let opens: Vec<f64> = prices.clone();
    let closes: Vec<f64> = prices
        .iter()
        .enumerate()
        .map(|(i, p)| if i < n_days - 1 { prices[i + 1] } else { *p })
        .collect();
    let highs: Vec<f64> = opens
        .iter()
        .zip(closes.iter())
        .map(|(o, c)| o.max(*c) + 0.5)
        .collect();
    let lows: Vec<f64> = opens
        .iter()
        .zip(closes.iter())
        .map(|(o, c)| o.min(*c) - 0.5)
        .collect();
    let volumes: Vec<f64> = (0..n_days).map(|_| 10_000.0).collect();

    let date_col = Column::new(
        "date".into(),
        Series::new("date".into(), dates)
            .cast(&DataType::Date)
            .unwrap(),
    );
    let symbol_col = Column::new("symbol".into(), vec!["DEMO"; n_days]);

    DataFrame::new(vec![
        date_col,
        symbol_col,
        Column::new("open".into(), opens),
        Column::new("high".into(), highs),
        Column::new("low".into(), lows),
        Column::new("close".into(), closes),
        Column::new("volume".into(), volumes),
    ])
    .unwrap()
}

#[cfg(feature = "ibkr")]
fn run_ibkr() {
    let _ = dotenvy::dotenv();

    let account_id = std::env::var("IB_ACCOUNT_ID").expect("IB_ACCOUNT_ID must be set");

    let interface = IbkrInterface::new("127.0.0.1:7496".into(), 1);

    if let Err(e) = interface.listen_async(&account_id) {
        eprintln!("listen failed: {e}");
        std::process::exit(1);
    }

    loop {
        let events = match interface.events(&account_id) {
            Ok(events) => events,
            Err(e) => {
                eprintln!("events failed: {e}");
                std::process::exit(1);
            }
        };
        let disconnected = events
            .iter()
            .any(|event| matches!(event, Event::Disconnected(_)));

        for event in events {
            match event {
                Event::Connected => println!("Connected to IB."),
                Event::Disconnected(e) => eprintln!("Disconnected: {e}"),
                Event::AccountValue(av) => {
                    println!("Account: {} = {} {}", av.key, av.value, av.currency);
                }
                Event::Position(pv) => {
                    println!(
                    "{} {} | pos={:.0} | mkt_price={:.2} | mkt_value={:.2} | avg_cost={:.2} | unreal={:.2} | real={:.2}",
                    pv.contract.symbol,
                    pv.contract.security_type,
                    pv.position,
                    pv.market_price,
                    pv.market_value,
                    pv.average_cost,
                    pv.unrealized_pnl,
                    pv.realized_pnl,
                );
                }
                Event::UpdateTime(ts) => println!("Update time: {ts}"),
                Event::HistoricalBars { contract_id, bars } => {
                    println!("\n--- Historical bars for contract {contract_id} ---");
                    for bar in &bars {
                        println!(
                            "  {date} | O:{open:.2} H:{high:.2} L:{low:.2} C:{close:.2} V:{vol:.0}",
                            date = bar.date,
                            open = bar.open,
                            high = bar.high,
                            low = bar.low,
                            close = bar.close,
                            vol = bar.volume,
                        );
                    }
                }
                Event::HistoricalError { contract_id, error } => {
                    eprintln!("  Historical error for contract {contract_id}: {error}");
                }
            }
        }

        if disconnected {
            return;
        }

        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}
