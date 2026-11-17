mod utils;

use std::env;
use std::time::Duration;

use dxcore::network::servers::HttpServer;
use dxcore::network::ServiceManager;
use dxcore::strategies::SmaCross;

use utils::strategy_service;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Address this strategy binds to, and the mesh it announces itself to.
    let bind_addr = env::var("STRATEGY_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let mesh_url = env::var("MESH_URL").unwrap_or_else(|_| "http://127.0.0.1:8081".into());

    let service = strategy_service(SmaCross::new(10_000.0, 5, 10));

    let mut manager = ServiceManager::<HttpServer>::new();
    manager.add("strategy", service)?;
    manager.bind("strategy", bind_addr.as_str())?;
    manager.start();
    manager.register_to_mesh(&mesh_url)?;

    println!(
        "strategy serving at http://{}",
        manager.addr("strategy").expect("bound")
    );
    println!("registered with mesh at {mesh_url}");
    println!("press ctrl-c to stop");

    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}
