mod account;
mod market;
mod order;

use std::sync::{Arc, Mutex};

use ibapi::client::blocking::Client;

use crate::{Error, Event};

pub struct IbkrInterface {
    host: String,
    client_id: i32,
    events: Arc<Mutex<Vec<Event>>>,
}

impl IbkrInterface {
    pub fn new(host: String, client_id: i32) -> Self {
        Self {
            host,
            client_id,
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn connect(&self) -> Result<Client, Error> {
        Client::connect(&self.host, self.client_id).map_err(|e| Error::Connection(e.to_string()))
    }

    fn record(&self, event: Event) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }
}
