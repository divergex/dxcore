//! End-to-end tests: services registered to servers, exercised over HTTP.

use std::sync::Arc;

use reqwest::blocking::Client;
use serde_json::Value;

use dxcore::attribute;
use dxcore::core::Portfolio;
use dxcore::network::servers::{HttpServer, ServerHandle};
use dxcore::network::services::{Attribute, AttributeService, Service, ServiceError};

/// Bind a server on an ephemeral port, spawn it, return base URL + handle.
fn spawn_server(service: Arc<dyn Service>) -> (String, ServerHandle) {
    let server = HttpServer::bind("127.0.0.1:0", service).unwrap();
    let addr = server.addr();
    let handle = server.spawn();
    (format!("http://{addr}"), handle)
}

fn client() -> Client {
    Client::new()
}

struct Counter {
    value: i64,
}

#[test]
fn http_server_serves_get_set() {
    let counter = Counter { value: 0 };
    let service = AttributeService::new("counter", counter)
        .with_attribute(attribute!("value", &counter.value));
    let (base, handle) = spawn_server(Arc::new(service));

    let c = client();

    let resp = c.get(format!("{base}/value")).send().unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().unwrap(), "0");

    let resp = c
        .put(format!("{base}/value"))
        .body("41")
        .header("Content-Type", "application/json")
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);

    let resp = c.get(format!("{base}/value")).send().unwrap();
    assert_eq!(resp.text().unwrap(), "41");

    // Unknown attribute → 404, not a panic.
    let resp = c.get(format!("{base}/nope")).send().unwrap();
    assert_eq!(resp.status(), 404);

    // POST is not accepted by attribute services -> 405.
    let resp = c
        .post(format!("{base}/value"))
        .body("null")
        .header("Content-Type", "application/json")
        .send()
        .unwrap();
    assert_eq!(resp.status(), 405);

    // Bad JSON body on PUT → 400.
    let resp = c
        .put(format!("{base}/value"))
        .body("not json")
        .header("Content-Type", "application/json")
        .send()
        .unwrap();
    assert_eq!(resp.status(), 400);

    handle.stop().unwrap();
}

#[test]
fn attributes_control_exposure() {
    let mut portfolio = Portfolio::default();
    portfolio.upsert_metric("NetLiquidation".into(), "100000".into(), "USD".into());

    // `metrics` is a real field: served get+set via the attribute! macro.
    // `net_liquidation` is a projection: custom getter, so read-only.
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
    let (base, handle) = spawn_server(Arc::new(service));
    let c = client();

    // Read the whole metrics map.
    let resp = c.get(format!("{base}/metrics")).send().unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().unwrap();
    assert_eq!(body["NetLiquidation"]["value"], "100000");

    // Read a single projected attribute.
    let resp = c.get(format!("{base}/net_liquidation")).send().unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().unwrap();
    assert_eq!(body["value"], "100000");

    // Write to the read-write attribute, then read it back.
    let resp = c
        .put(format!("{base}/metrics"))
        .body(
            serde_json::json!({
                "NetLiquidation": {
                    "key": "NetLiquidation",
                    "value": "105000",
                    "currency": "USD",
                }
            })
            .to_string(),
        )
        .header("Content-Type", "application/json")
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);

    let resp = c.get(format!("{base}/net_liquidation")).send().unwrap();
    let body: Value = resp.json().unwrap();
    assert_eq!(body["value"], "105000");

    // Getter-only attribute rejects writes.
    let resp = c
        .put(format!("{base}/net_liquidation"))
        .body("null")
        .header("Content-Type", "application/json")
        .send()
        .unwrap();
    assert_eq!(resp.status(), 405);

    handle.stop().unwrap();
}

struct Ledger {
    balance: i64,
}

#[test]
fn services_register_methods() {
    let service = AttributeService::new("ledger", Ledger { balance: 100 })
        .with_get("lookup", |l: &Ledger, account: String| {
            Ok(format!("{account}:{}", l.balance))
        })
        .with_set("add", |l: &mut Ledger, (account, amount): (String, i64)| {
            l.balance += amount;
            Ok(format!("{account}:{}", l.balance))
        });
    let (base, handle) = spawn_server(Arc::new(service));
    let c = client();

    // Immutable method: args deserialized from the GET body, response serialized.
    let resp = c
        .get(format!("{base}/lookup"))
        .body("\"checking\"")
        .header("Content-Type", "application/json")
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().unwrap(), "\"checking:100\"");

    // Mutable method: tuple args deserialized, state mutated, result returned.
    let resp = c
        .put(format!("{base}/add"))
        .body("[\"checking\", 5]")
        .header("Content-Type", "application/json")
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().unwrap(), "\"checking:105\"");

    // The mutation is visible to the next read.
    let resp = c
        .get(format!("{base}/lookup"))
        .body("\"checking\"")
        .header("Content-Type", "application/json")
        .send()
        .unwrap();
    assert_eq!(resp.text().unwrap(), "\"checking:105\"");

    // Missing args on an immutable method -> 400.
    let resp = c.get(format!("{base}/lookup")).send().unwrap();
    assert_eq!(resp.status(), 400);

    // Args of the wrong shape -> 400.
    let resp = c
        .get(format!("{base}/lookup"))
        .body("42")
        .header("Content-Type", "application/json")
        .send()
        .unwrap();
    assert_eq!(resp.status(), 400);

    // Immutable methods reject writes, mutable methods reject reads.
    let resp = c
        .put(format!("{base}/lookup"))
        .body("null")
        .header("Content-Type", "application/json")
        .send()
        .unwrap();
    assert_eq!(resp.status(), 405);

    let resp = c.get(format!("{base}/add")).send().unwrap();
    assert_eq!(resp.status(), 405);

    handle.stop().unwrap();
}
