
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use reqwest::blocking::Client;
use serde_json::{json, Value};

use dxcore::network::mesh::{MeshService, Protocol, Registration};
use dxcore::network::servers::{HttpServer, ServerHandle};
use dxcore::network::services::{
    FunctionalService, Request, Response, Service, ServiceError,
};

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
fn mesh_registers_locally_and_serves() {
    let mesh = MeshService::new();

    let market_data = FunctionalService::new("market-data", MarketData {})
        .with_get("quotes", |_: &MarketData, symbol: String| {
            Ok(format!("{symbol}:42.5"))
        });
    let uuid = mesh
        .register(Arc::new(market_data), "http://127.0.0.1:9001", Protocol::Http)
        .unwrap();

    let regs = mesh.registrations().unwrap();
    assert_eq!(regs[&uuid].name, "market-data");
    assert_eq!(regs[&uuid].url, "http://127.0.0.1:9001");

    let (base, handle) = spawn_server(Arc::new(mesh));
    let c = client();

    let resp = c.get(format!("{base}/health")).send().unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().unwrap(), r#"{"status":"ok"}"#);

    let resp = c
        .get(format!("{base}/services/{uuid}/endpoints"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.json::<Value>().unwrap(), json!(["quotes"]));

    let resp = c
        .get(format!("{base}/services/{uuid}/endpoints?protocol=HTTP"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.json::<Value>().unwrap(), json!(["quotes"]));

    let resp = c
        .get(format!("{base}/services/{uuid}/endpoints/quotes"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().unwrap();
    assert_eq!(body["protocols"], json!(["http"]));

    let resp = c
        .get(format!("{base}/services/nope/endpoints"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 404);
    let resp = c
        .get(format!("{base}/services/{uuid}/endpoints/nope"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 404);

    let resp = c.get(format!("{base}/discover")).send().unwrap();
    assert_eq!(resp.status(), 200);
    let list: Value = resp.json().unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["uuid"], uuid);
    assert_eq!(list[0]["name"], "market-data");
    assert_eq!(list[0]["url"], "http://127.0.0.1:9001");
    assert_eq!(list[0]["protocols"], json!(["http"]));

    // Discover filtered by protocol, case-insensitive.
    let resp = c.get(format!("{base}/discover?protocol=HTTP")).send().unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.json::<Value>().unwrap().as_array().unwrap().len(), 1);

    // Unknown query parameters are ignored, in any order.
    let resp = c
        .get(format!("{base}/discover?protocol=HTTP&verbose=true"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.json::<Value>().unwrap().as_array().unwrap().len(), 1);

    let resp = c
        .get(format!("{base}/discover?verbose=true&protocol=HTTP"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.json::<Value>().unwrap().as_array().unwrap().len(), 1);

    let resp = c.get(format!("{base}/discover?protocol=ws")).send().unwrap();
    assert_eq!(resp.status(), 400);

    handle.stop().unwrap();
}

#[test]
fn mesh_registers_over_http() {
    let (base, handle) = spawn_server(Arc::new(MeshService::new()));
    let c = client();

    let body = json!({
        "name": "analytics",
        "url": "http://127.0.0.1:9002",
        "protocols": ["http"],
        "endpoints": {
            "returns": {
                "protocols": ["http"],
                "description": "Annual returns for a symbol"
            }
        }
    });
    let resp = c
        .post(format!("{base}/services"))
        .body(body.to_string())
        .header("Content-Type", "application/json")
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
    let uuid = resp.json::<Value>().unwrap()["uuid"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = c.get(format!("{base}/discover")).send().unwrap();
    let list: Value = resp.json().unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["name"], "analytics");

    let resp = c
        .get(format!("{base}/services/{uuid}/endpoints/returns"))
        .send()
        .unwrap();
    let body: Value = resp.json().unwrap();
    assert_eq!(body["description"], "Annual returns for a symbol");
    assert_eq!(body["protocols"], json!(["http"]));

    let body = json!({
        "name": "bare",
        "url": "http://127.0.0.1:9003",
        "protocols": ["http"],
        "endpoints": {
            "status": {}
        }
    });
    let resp = c
        .post(format!("{base}/services"))
        .body(body.to_string())
        .header("Content-Type", "application/json")
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
    let uuid = resp.json::<Value>().unwrap()["uuid"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = c
        .get(format!("{base}/services/{uuid}/endpoints?protocol=HTTP"))
        .send()
        .unwrap();
    assert_eq!(resp.json::<Value>().unwrap(), json!([]));

    let resp = c
        .get(format!("{base}/services/{uuid}/endpoints/status?protocol=HTTP"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 404);

    let resp = c
        .get(format!("{base}/services/{uuid}/endpoints/status"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);

    let bad = json!({ "name": "x", "url": "http://x", "protocols": ["ws"] });
    let resp = c
        .post(format!("{base}/services"))
        .body(bad.to_string())
        .header("Content-Type", "application/json")
        .send()
        .unwrap();
    assert_eq!(resp.status(), 400);

    let resp = c
        .post(format!("{base}/health"))
        .body("{}")
        .header("Content-Type", "application/json")
        .send()
        .unwrap();
    assert_eq!(resp.status(), 405);
    let resp = c
        .put(format!("{base}/discover"))
        .body("{}")
        .header("Content-Type", "application/json")
        .send()
        .unwrap();
    assert_eq!(resp.status(), 405);

    handle.stop().unwrap();
}

#[test]
fn mesh_removes_over_http() {
    let (base, handle) = spawn_server(Arc::new(MeshService::new()));
    let c = client();

    let body = json!({
        "name": "analytics",
        "url": "http://127.0.0.1:9002",
        "protocols": ["http"],
        "endpoints": { "returns": { "protocols": ["http"] } }
    });
    let resp = c
        .post(format!("{base}/services"))
        .body(body.to_string())
        .header("Content-Type", "application/json")
        .send()
        .unwrap();
    let uuid = resp.json::<Value>().unwrap()["uuid"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = c.delete(format!("{base}/services/{uuid}")).send().unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.json::<Value>().unwrap(), json!({ "uuid": uuid }));

    let resp = c.get(format!("{base}/discover")).send().unwrap();
    assert_eq!(resp.json::<Value>().unwrap(), json!([]));

    let resp = c.delete(format!("{base}/services/{uuid}")).send().unwrap();
    assert_eq!(resp.status(), 404);

    let resp = c.delete(format!("{base}/returns")).send().unwrap();
    assert_eq!(resp.status(), 404);

    handle.stop().unwrap();
}

#[test]
fn registration_builder_registers_and_unregisters() {
    let (base, handle) = spawn_server(Arc::new(MeshService::new()));
    let c = client();

    let market_data = Arc::new(
        FunctionalService::new("market-data", MarketData {})
            .with_get("quotes", |_: &MarketData, symbol: String| {
                Ok(format!("{symbol}:42.5"))
            }),
    );

    let builder = Registration::builder(Arc::clone(&market_data), "http://127.0.0.1:9001");
    let payload = builder.build();
    assert_eq!(payload.name, "market-data");
    assert_eq!(payload.url, "http://127.0.0.1:9001");
    assert_eq!(payload.protocols, vec![Protocol::Http]);
    assert_eq!(
        payload
            .endpoints
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["quotes"]
    );

    let registered = builder.mesh(base.clone()).register().unwrap();
    assert!(!registered.uuid().is_empty());

    let resp = c.get(format!("{base}/discover")).send().unwrap();
    let list: Value = resp.json().unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["name"], "market-data");
    assert_eq!(list[0]["url"], "http://127.0.0.1:9001");
    assert_eq!(list[0]["protocols"], json!(["http"]));
    assert_eq!(list[0]["uuid"], registered.uuid());

    registered.unregister().unwrap();

    let resp = c.get(format!("{base}/discover")).send().unwrap();
    assert_eq!(resp.json::<Value>().unwrap(), json!([]));

    handle.stop().unwrap();
}

struct Counting {
    mesh: MeshService,
    deletes: AtomicUsize,
}

impl Counting {
    fn new() -> Self {
        Self {
            mesh: MeshService::new(),
            deletes: AtomicUsize::new(0),
        }
    }

    fn deletes(&self) -> usize {
        self.deletes.load(Ordering::SeqCst)
    }
}

impl Service for Counting {
    fn call(&self, request: Request) -> Result<Response, ServiceError> {
        if matches!(request, Request::Delete { .. }) {
            self.deletes.fetch_add(1, Ordering::SeqCst);
        }
        self.mesh.call(request)
    }

    fn name(&self) -> String {
        self.mesh.name()
    }

    fn endpoints(&self) -> Vec<String> {
        self.mesh.endpoints()
    }
}

fn market_data() -> Arc<FunctionalService<MarketData>> {
    Arc::new(
        FunctionalService::new("market-data", MarketData {})
            .with_get("quotes", |_: &MarketData, symbol: String| {
                Ok(format!("{symbol}:42.5"))
            }),
    )
}

#[test]
fn dropping_the_handle_unregisters() {
    let (base, handle) = spawn_server(Arc::new(MeshService::new()));
    let c = client();

    let registered = Registration::builder(market_data(), "http://127.0.0.1:9001")
        .mesh(base.clone())
        .register()
        .unwrap();

    let resp = c.get(format!("{base}/discover")).send().unwrap();
    assert_eq!(resp.json::<Value>().unwrap().as_array().unwrap().len(), 1);

    drop(registered);

    let resp = c.get(format!("{base}/discover")).send().unwrap();
    assert_eq!(resp.json::<Value>().unwrap(), json!([]));

    handle.stop().unwrap();
}

#[test]
fn explicit_unregister_deletes_once() {
    let counting = Arc::new(Counting::new());
    let (base, handle) = spawn_server(Arc::clone(&counting) as Arc<dyn Service>);
    let c = client();

    let registered = Registration::builder(market_data(), "http://127.0.0.1:9001")
        .mesh(base.clone())
        .register()
        .unwrap();

    registered.unregister().unwrap();

    // The drop that follows the explicit call must not repeat the delete.
    assert_eq!(counting.deletes(), 1);

    let resp = c.get(format!("{base}/discover")).send().unwrap();
    assert_eq!(resp.json::<Value>().unwrap(), json!([]));

    handle.stop().unwrap();
}
