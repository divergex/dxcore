use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, RwLock};

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    Get {
        attribute: String,
        args: Option<Value>,
    },
    Set {
        attribute: String,
        value: Value,
    },
    Post {
        attribute: String,
        value: Value,
    },
    Delete {
        attribute: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServiceError {
    UnknownAttribute(String),
    ReadOnly(String),
    WriteOnly(String),
    BadValue(String),
    Internal(String),
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServiceError::UnknownAttribute(name) => write!(f, "unknown attribute: {name}"),
            ServiceError::ReadOnly(name) => write!(f, "attribute is read-only: {name}"),
            ServiceError::WriteOnly(name) => write!(f, "attribute is write-only: {name}"),
            ServiceError::BadValue(msg) => write!(f, "bad value: {msg}"),
            ServiceError::Internal(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl std::error::Error for ServiceError {}

pub trait Service: Send + Sync {
    fn call(&self, request: Request) -> Result<Response, ServiceError>;

    fn name(&self) -> String {
        "service".into()
    }

    fn endpoints(&self) -> Vec<String> {
        Vec::new()
    }
}

pub(crate) type Getter<T> =
    Arc<dyn Fn(&T, Option<Value>) -> Result<Value, ServiceError> + Send + Sync>;

pub(crate) type Setter<T> = Arc<dyn Fn(&mut T, Value) -> Result<Value, ServiceError> + Send + Sync>;

pub(crate) struct Entry<T> {
    pub(crate) get: Option<Getter<T>>,
    pub(crate) set: Option<Setter<T>>,
}

pub(crate) fn call_entry<T: Send + Sync + 'static>(
    entries: &HashMap<String, Entry<T>>,
    instance: &RwLock<T>,
    request: Request,
) -> Result<Response, ServiceError> {
    match request {
        Request::Get { attribute, args } => {
            let entry = entries
                .get(&attribute)
                .ok_or_else(|| ServiceError::UnknownAttribute(attribute.clone()))?;
            let getter = entry
                .get
                .as_ref()
                .ok_or_else(|| ServiceError::ReadOnly(attribute))?;
            let instance = instance
                .read()
                .map_err(|_| ServiceError::Internal("instance lock poisoned".into()))?;
            Ok(Response {
                value: getter(&instance, args)?,
            })
        }
        Request::Set { attribute, value } => {
            let entry = entries
                .get(&attribute)
                .ok_or_else(|| ServiceError::UnknownAttribute(attribute.clone()))?;
            let setter = entry
                .set
                .as_ref()
                .ok_or_else(|| ServiceError::WriteOnly(attribute))?;
            let mut instance = instance
                .write()
                .map_err(|_| ServiceError::Internal("instance lock poisoned".into()))?;
            Ok(Response {
                value: setter(&mut instance, value)?,
            })
        }
        Request::Post { attribute, .. } | Request::Delete { attribute } => {
            Err(ServiceError::WriteOnly(attribute))
        }
    }
}
