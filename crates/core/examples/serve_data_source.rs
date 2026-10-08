use std::sync::Arc;

use dxcore::interface::external::fmp::FmpClient;
use dxcore::network::servers::HttpServer;
use dxcore::network::services::{ClassService, ServiceError};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    let port = std::env::args().nth(1).unwrap_or_else(|| "8080".into());

    let service = ClassService::new("fmp", FmpClient::from_env()?)
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
