//! - `GET /health`
//! - `GET /discover?protocol=<name>` registered services
//! - `GET /services/{uuid}/endpoints?protocol=<name>` endpoint names
//! - `GET /services/{uuid}/endpoints/{name}?protocol=<name>` endpoint record
//! - `POST /services`  register a service (body: [`Registration`])

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::network::services::{Request, Response, Service, ServiceError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Http,
}

impl Protocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Protocol::Http => "http",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "http" => Some(Protocol::Http),
            _ => None,
        }
    }
}

impl Serialize for Protocol {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Protocol {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        Self::parse(&name)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown protocol: {name}")))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    #[serde(default)]
    pub protocols: Vec<Protocol>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registration {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub protocols: Vec<Protocol>,
    #[serde(default)]
    pub endpoints: HashMap<String, Endpoint>,
}

pub struct MeshService {
    services: RwLock<HashMap<String, Registration>>,
}

impl MeshService {
    pub fn new() -> Self {
        Self {
            services: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(
        &self,
        service: Arc<dyn Service>,
        url: &str,
        protocol: Protocol,
    ) -> Result<String, ServiceError> {
        let endpoints = service
            .endpoints()
            .into_iter()
            .map(|path| {
                let name = path.trim_start_matches('/').to_string();
                (
                    name,
                    Endpoint {
                        protocols: vec![protocol],
                        description: None,
                    },
                )
            })
            .collect();
        let registration = Registration {
            name: service.name(),
            url: url.to_string(),
            protocols: vec![protocol],
            endpoints,
        };
        self.insert(registration)
    }

    fn insert(&self, registration: Registration) -> Result<String, ServiceError> {
        let uuid = Uuid::new_v4().to_string();
        let mut services = self
            .services
            .write()
            .map_err(|_| ServiceError::Internal("mesh lock poisoned".into()))?;
        services.insert(uuid.clone(), registration);
        Ok(uuid)
    }

    /// All registered services, keyed by uuid.
    pub fn registrations(&self) -> Result<HashMap<String, Registration>, ServiceError> {
        let services = self
            .services
            .read()
            .map_err(|_| ServiceError::Internal("mesh lock poisoned".into()))?;
        Ok(services.clone())
    }

    fn with_registry<T>(
        &self,
        f: impl FnOnce(&HashMap<String, Registration>) -> Result<T, ServiceError>,
    ) -> Result<T, ServiceError> {
        let services = self
            .services
            .read()
            .map_err(|_| ServiceError::Internal("mesh lock poisoned".into()))?;
        f(&services)
    }

    fn with_service<T>(
        &self,
        uuid: &str,
        f: impl FnOnce(&Registration) -> Result<T, ServiceError>,
    ) -> Result<T, ServiceError> {
        self.with_registry(|services| {
            let registration = services
                .get(uuid)
                .ok_or_else(|| ServiceError::UnknownAttribute(format!("service {uuid}")))?;
            f(registration)
        })
    }

    fn route_get(&self, attribute: &str) -> Result<Response, ServiceError> {
        let (path, query) = attribute.split_once('?').unwrap_or((attribute, ""));
        let protocol = query_protocol(query)?;
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        match segments.as_slice() {
            ["health"] => Ok(Response {
                value: json!({ "status": "ok" }),
            }),
            ["discover"] => self.discover(protocol),
            ["services", uuid, "endpoints"] => self.endpoint_names(uuid, protocol),
            ["services", uuid, "endpoints", name] => self.endpoint_record(uuid, name, protocol),
            _ => Err(ServiceError::UnknownAttribute(attribute.to_string())),
        }
    }

    fn route_post(&self, attribute: &str, value: Value) -> Result<Response, ServiceError> {
        if attribute != "services" {
            return Err(ServiceError::WriteOnly(
                "mesh only accepts POST /services".into(),
            ));
        }
        let registration: Registration = serde_json::from_value(value)
            .map_err(|e| ServiceError::BadValue(e.to_string()))?;
        let uuid = self.insert(registration)?;
        Ok(Response {
            value: json!({ "uuid": uuid }),
        })
    }

    fn discover(&self, protocol: Option<Protocol>) -> Result<Response, ServiceError> {
        self.with_registry(|services| {
            let list: Vec<Value> = services
                .iter()
                .filter(|(_, r)| protocol.map_or(true, |p| r.protocols.contains(&p)))
                .map(|(uuid, r)| {
                    json!({
                        "uuid": uuid,
                        "name": r.name,
                        "url": r.url,
                        "protocols": r.protocols,
                    })
                })
                .collect();
            Ok(Response {
                value: Value::Array(list),
            })
        })
    }

    fn endpoint_names(
        &self,
        uuid: &str,
        protocol: Option<Protocol>,
    ) -> Result<Response, ServiceError> {
        self.with_service(uuid, |registration| {
            let names: Vec<Value> = registration
                .endpoints
                .iter()
                .filter(|(_, e)| protocol.map_or(true, |p| e.protocols.contains(&p)))
                .map(|(name, _)| Value::String(name.clone()))
                .collect();
            Ok(Response {
                value: Value::Array(names),
            })
        })
    }

    fn endpoint_record(
        &self,
        uuid: &str,
        name: &str,
        protocol: Option<Protocol>,
    ) -> Result<Response, ServiceError> {
        self.with_service(uuid, |registration| {
            let endpoint = registration
                .endpoints
                .get(name)
                .ok_or_else(|| ServiceError::UnknownAttribute(format!("endpoint {name}")))?;
            if let Some(p) = protocol {
                if !endpoint.protocols.contains(&p) {
                    return Err(ServiceError::UnknownAttribute(format!(
                        "endpoint {name} on protocol {}",
                        p.as_str()
                    )));
                }
            }
            Ok(Response {
                value: serde_json::to_value(endpoint)
                    .map_err(|e| ServiceError::Internal(e.to_string()))?,
            })
        })
    }
}

impl Default for MeshService {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for MeshService {
    fn call(&self, request: Request) -> Result<Response, ServiceError> {
        match request {
            Request::Get { attribute, args: _ } => self.route_get(&attribute),
            Request::Post { attribute, value } => self.route_post(&attribute, value),
            Request::Set { .. } => Err(ServiceError::WriteOnly(
                "mesh only accepts POST /services".into(),
            )),
        }
    }
}

fn query_protocol(query: &str) -> Result<Option<Protocol>, ServiceError> {
    let mut protocol = None;
    for pair in query.split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        if key == "protocol" {
            protocol = Some(
                Protocol::parse(value)
                    .ok_or_else(|| ServiceError::BadValue(format!("unknown protocol: {value}")))?,
            );
        }
    }
    Ok(protocol)
}
