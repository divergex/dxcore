//! Serves a data source over HTTP through the network service layer.
//!
//! Run with: `cargo run --example serve_data_source -- [port]`
//!
//! Each data-source query is registered on an [`AttributeService`] as an
//! immutable method: the argument arrives as JSON in the request body, is
//! deserialized with `from_value`, and the return value is serialized back
//! with `to_value`. Mutable methods are registered with `with_set` instead.
//!
//! - `curl -X GET http://127.0.0.1:8080/profile -d '"AAPL"'`
//! - `curl -X GET http://127.0.0.1:8080/balance_sheet -d '"AAPL"'`
//! - `curl -X GET http://127.0.0.1:8080/income_statement -d '"AAPL"'`

use std::sync::Arc;

use dxlib::interface::external::fmp::FmpClient;
use dxlib::network::servers::HttpServer;
use dxlib::network::services::{AttributeService, ServiceError};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    let port = std::env::args().nth(1).unwrap_or_else(|| "8080".into());

    let service = AttributeService::new("fmp", FmpClient::from_env()?)
        .with_get("profile", |c: &FmpClient, symbol: String| {
            c.profile(&symbol).map_err(|e| ServiceError::Internal(e.to_string()))
        })
        .with_get("balance_sheet", |c: &FmpClient, symbol: String| {
            c.balance_sheet(&symbol).map_err(|e| ServiceError::Internal(e.to_string()))
        })
        .with_get("income_statement", |c: &FmpClient, symbol: String| {
            c.income_statement(&symbol).map_err(|e| ServiceError::Internal(e.to_string()))
        });

    let server = HttpServer::bind(format!("127.0.0.1:{port}"), Arc::new(service))?;
    println!("serving FMP data source at http://{}", server.addr());
    println!("  GET /profile | /balance_sheet | /income_statement, body: \"<symbol>\"");
    server.serve()?;

    Ok(())
}
