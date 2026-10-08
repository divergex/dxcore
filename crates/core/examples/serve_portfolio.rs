use std::sync::Arc;

use dxcore::attribute;
use dxcore::core::Portfolio;
use dxcore::network::servers::HttpServer;
use dxcore::network::services::{Attribute, AttributeService, ServiceError};

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
