use reqwest::blocking::{Client, RequestBuilder};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use super::strategy::StepArgs;
use super::trading::HistoryArgs;
use crate::network::mesh::Registration;
use crate::Error;

pub struct HttpAccessor {
    client: Client,
    url: String,
}

impl HttpAccessor {
    pub fn new(url: impl Into<String>) -> Self {
        Self::with_client(Client::new(), url)
    }

    pub fn with_client(client: Client, url: impl Into<String>) -> Self {
        Self {
            client,
            url: url.into().trim_end_matches('/').to_string(),
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn get<T, R>(&self, method: &str, args: &T) -> Result<R, Error>
    where
        T: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        self.send(self.client.get(self.endpoint(method)), args)
    }

    pub fn set<T, R>(&self, method: &str, args: &T) -> Result<R, Error>
    where
        T: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        self.send(self.client.put(self.endpoint(method)), args)
    }

    pub fn post<T, R>(&self, method: &str, args: &T) -> Result<R, Error>
    where
        T: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        self.send(self.client.post(self.endpoint(method)), args)
    }

    pub fn delete<R>(&self, method: &str) -> Result<R, Error>
    where
        R: DeserializeOwned,
    {
        self.receive(self.client.delete(self.endpoint(method)))
    }

    fn endpoint(&self, method: &str) -> String {
        format!("{}/{}", self.url, method.trim_start_matches('/'))
    }

    fn send<T, R>(&self, request: RequestBuilder, args: &T) -> Result<R, Error>
    where
        T: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        self.receive(request.json(args))
    }

    fn receive<R>(&self, request: RequestBuilder) -> Result<R, Error>
    where
        R: DeserializeOwned,
    {
        let response = request.send().map_err(|e| Error::Http(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            return Err(Error::Http(format!("{status}: {body}")));
        }

        response.json::<R>().map_err(|e| Error::Http(e.to_string()))
    }
}

pub fn http_on_step(accessor: &HttpAccessor, args: StepArgs) -> Result<Value, Error> {
    accessor.set("on_step", &args)
}

pub fn http_market_history(accessor: &HttpAccessor, args: HistoryArgs) -> Result<Value, Error> {
    accessor.get("market_history", &args)
}

pub fn http_portfolio(accessor: &HttpAccessor, account_id: String) -> Result<Value, Error> {
    accessor.get("portfolio", &account_id)
}

pub fn http_listen_async(accessor: &HttpAccessor, account_id: String) -> Result<Value, Error> {
    accessor.get("listen_async", &account_id)
}

pub fn http_events(accessor: &HttpAccessor, account_id: String) -> Result<Value, Error> {
    accessor.get("events", &account_id)
}

pub fn http_register(accessor: &HttpAccessor, registration: Registration) -> Result<Value, Error> {
    accessor.post("services", &registration)
}

pub fn http_unregister(accessor: &HttpAccessor, uuid: String) -> Result<Value, Error> {
    accessor.delete(&format!("services/{uuid}"))
}
