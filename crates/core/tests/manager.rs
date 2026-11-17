use std::sync::Arc;

use reqwest::blocking::Client;
use serde_json::{json, Value};

use dxcore::network::manager::ServiceManager;
use dxcore::network::mesh::MeshService;
use dxcore::network::servers::{HttpServer, ServerHandle};
use dxcore::network::services::{FunctionalService, Service};

fn spawn_server(service: Arc<dyn Service>) -> (String, ServerHandle) {
    let server = HttpServer::bind("127.0.0.1:0", service).unwrap();
    let addr = server.addr();
    let handle = server.spawn();
    (format!("http://{addr}"), handle)
}

fn client() -> Client {
    Client::new()
}

struct MarketData {}

#[test]
fn manager_binds_starts_registers_and_stops() {
    let service = FunctionalService::new("market-data", MarketData {})
        .with_get("quotes", |_: &MarketData, symbol: String| Ok(format!("{symbol}:42.5")));

    let mut manager = ServiceManager::<HttpServer>::new();
    manager.add("market-data", Arc::new(service)).unwrap();
    manager.bind("market-data", "127.0.0.1:0").unwrap();
    manager.start();

    let base = format!("http://{}", manager.addr("market-data").unwrap());
    let (mesh_base, _mesh) = spawn_server(Arc::new(MeshService::new()));
    let c = client();

    // The service serves before registration.
    let resp = c
        .get(format!("{base}/quotes"))
        .body(r#""AAPL""#)
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.json::<Value>().unwrap(), json!("AAPL:42.5"));

    // Register to the mesh, then discover it.
    manager.register_to_mesh(&mesh_base).unwrap();
    let discover: Value = c
        .get(format!("{mesh_base}/discover"))
        .send()
        .unwrap()
        .json()
        .unwrap();
    let entries = discover.as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["name"], json!("market-data"));
    assert_eq!(entries[0]["url"], json!(base));

    // Stopping everything unregisters from the mesh.
    manager.stop_all().unwrap();
    let discover: Value = c
        .get(format!("{mesh_base}/discover"))
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert!(discover.as_array().unwrap().is_empty());
}
