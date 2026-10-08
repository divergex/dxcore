use std::sync::Arc;

use serde::Deserialize;

use dxcore::core::Instrument;
use dxcore::interface::external::ibkr::IbkrInterface;
use dxcore::interface::{AccountInterface, MarketInterface, Span};
use dxcore::network::servers::HttpServer;
use dxcore::network::services::{ClassService, ServiceError};

#[derive(Deserialize)]
struct HistoryArgs {
    instrument: Instrument,
    bar_size: Span,
    duration: Span,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    let port = std::env::args().nth(1).unwrap_or_else(|| "8080".into());
    let host = std::env::var("IB_HOST").unwrap_or_else(|_| "127.0.0.1:4002".into());

    let service = ClassService::new("ibkr", IbkrInterface::new(host, 1))
        .with_get("portfolio", |c: &IbkrInterface, account_id: String| {
            c.portfolio(&account_id)
                .map_err(|e| ServiceError::Internal(e.to_string()))
        })
        .with_get("listen_async", |c: &IbkrInterface, account_id: String| {
            c.listen_async(&account_id)
                .map(|_| true)
                .map_err(|e| ServiceError::Internal(e.to_string()))
        })
        .with_get("events", |c: &IbkrInterface, account_id: String| {
            c.events(&account_id)
                .map_err(|e| ServiceError::Internal(e.to_string()))
        })
        .with_get("market_history", |c: &IbkrInterface, args: HistoryArgs| {
            c.market_history(&args.instrument, args.bar_size, args.duration)
                .map(dxcore::DataFrame::new)
                .map_err(|e| ServiceError::Internal(e.to_string()))
        });

    let server = HttpServer::bind(format!("127.0.0.1:{port}"), Arc::new(service))?;
    println!("serving IBKR at http://{}", server.addr());
    println!("  GET /portfolio      body: \"<account id>\"");
    println!("  GET /listen_async   body: \"<account id>\"");
    println!("  GET /events         body: \"<account id>\"");
    println!(
        "  GET /market_history body: {{\"instrument\":{{...}},\"bar_size\":\"1 day\",\"duration\":\"30 days\"}}"
    );
    server.serve()?;

    Ok(())
}
