use reqwest::blocking::{Client, RequestBuilder};
use serde::de::DeserializeOwned;
use serde::Serialize;

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

    fn endpoint(&self, method: &str) -> String {
        format!("{}/{}", self.url, method.trim_start_matches('/'))
    }

    fn send<T, R>(&self, request: RequestBuilder, args: &T) -> Result<R, Error>
    where
        T: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        let response = request
            .json(args)
            .send()
            .map_err(|e| Error::Http(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            return Err(Error::Http(format!("{status}: {body}")));
        }

        response.json::<R>().map_err(|e| Error::Http(e.to_string()))
    }
}
