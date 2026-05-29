use std::sync::mpsc;

use dxcore_rs::interface::ibkr::IbkrInterface;
use dxcore_rs::interface::MarketApi;
use dxcore_rs::Event;

fn main() {
    let _ = dotenvy::dotenv();

    let account_id = std::env::var("IB_ACCOUNT_ID").expect("IB_ACCOUNT_ID must be set");

    let (tx, rx) = mpsc::channel();
    let interface = IbkrInterface::new("127.0.0.1:7496".into(), 1);

    let handle = std::thread::spawn(move || {
        interface.listen(&account_id, tx)
    });

    for event in rx {
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

    if let Err(e) = handle.join().expect("worker panicked") {
        eprintln!("Worker error: {e}");
        std::process::exit(1);
    }
}
