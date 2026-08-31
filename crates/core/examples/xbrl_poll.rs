//! Polls XBRL filings every 60 seconds.
//!
//! Run with: `cargo run --example xbrl_poll -- AAPL MSFT`

use std::collections::HashSet;
use std::time::Duration;

use dxlib::interface::external::xbrl;
use dxlib::interface::stream::poll;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() {
    let queries: Vec<String> = {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if args.is_empty() {
            vec!["ASML".to_string()]
        } else {
            args
        }
    };

    let display = queries.join(", ");
    println!("polling XBRL filings for [{display}] every 60s...\n");

    let mut stream = poll(Duration::from_secs(60), false, {
        let queries = queries.clone();
        move || {
            let queries = queries.clone();
            async move {
                tokio::task::spawn_blocking(move || {
                    let qs: Vec<&str> = queries.iter().map(|s| s.as_str()).collect();
                    xbrl::query_filings(&qs, 20)
                })
                .await
                .unwrap()
            }
        }
    });

    let mut seen = HashSet::new();

    while let Some(result) = stream.next().await {
        match result {
            Ok(filings) => {
                let new: Vec<_> = filings
                    .into_iter()
                    .filter(|f| seen.insert(f.view_url.clone()))
                    .collect();
                if new.is_empty() {
                    println!("no new filings");
                } else {
                    println!("{} new filing(s)", new.len());
                    for f in &new {
                        println!("  {} | {} | {}", f.entity_name, f.period_end, f.date_added);
                    }
                }
            }
            Err(e) => eprintln!("error: {e}"),
        }
        println!();
    }
}
