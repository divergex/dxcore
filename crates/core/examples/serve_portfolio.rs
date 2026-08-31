//! Serves a `Portfolio` over HTTP via the generic attribute service.
//!
//! Run with: `cargo run --example serve_portfolio -- [port]`
//!
//! The service exposes only the declared attributes: `metrics` is a real
//! field (get+set via the `attribute!` macro), `net_liquidation` is a
//! projection with a custom getter (read-only).
//!
//! Then:
//! - `curl http://127.0.0.1:8080/metrics` → read the metrics map
//! - `curl -X PUT http://127.0.0.1:8080/metrics -d '{"NetLiquidation":{"key":"NetLiquidation","value":"100000","currency":"USD"}}'`
//! - `curl http://127.0.0.1:8080/net_liquidation` → read one metric
//! - `curl -X PUT http://127.0.0.1:8080/net_liquidation -d 'null'` → 405 (read-only)

use std::sync::Arc;

use dxlib::attribute;
use dxlib::core::Portfolio;
use dxlib::network::servers::HttpServer;
use dxlib::network::services::{Attribute, AttributeService, ServiceError};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = std::env::args().nth(1).unwrap_or_else(|| "8080".into());

    let portfolio = Portfolio::default();

    let service = AttributeService::new("portfolio", portfolio)
        .with_attribute(attribute!("metrics", &portfolio.metrics))
        .with_attribute((
            "net_liquidation",
            Attribute::getter(|p: &Portfolio| {
                p.metrics
                    .get("NetLiquidation")
                    .cloned()
                    .ok_or_else(|| ServiceError::UnknownAttribute("NetLiquidation".into()))
            }),
        ));

    let server = HttpServer::bind(format!("127.0.0.1:{port}"), Arc::new(service))?;
    println!("serving portfolio at http://{}", server.addr());
    server.serve()?;

    Ok(())
}
