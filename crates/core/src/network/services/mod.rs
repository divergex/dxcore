use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, RwLock};

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::serialization::{from_value, to_value};

#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    Get { attribute: String },
    Set { attribute: String, value: Value },
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
}

/// Accessors for one attribute of `T` whose value is of type `A`.
///
/// `A` is the typed value; the service serializes/deserializes it to the
/// wire format, so the accessors work in `A`, never in JSON. For attributes
/// that are real fields of `T`, use the [`attribute!`] macro instead of
/// writing these by hand.
pub struct Attribute<T, A> {
    getter: Option<Arc<dyn Fn(&T) -> Result<A, ServiceError> + Send + Sync>>,
    setter: Option<Arc<dyn Fn(&mut T, A) -> Result<(), ServiceError> + Send + Sync>>,
}

impl<T: 'static, A: Send + Sync + 'static> Attribute<T, A> {
    pub fn getter(get: fn(&T) -> Result<A, ServiceError>) -> Self {
        Self {
            getter: Some(Arc::new(get)),
            setter: None,
        }
    }

    pub fn setter(set: fn(&mut T, A) -> Result<(), ServiceError>) -> Self {
        Self {
            getter: None,
            setter: Some(Arc::new(set)),
        }
    }

    pub fn read_write(
        get: fn(&T) -> Result<A, ServiceError>,
        set: fn(&mut T, A) -> Result<(), ServiceError>,
    ) -> Self {
        Self {
            getter: Some(Arc::new(get)),
            setter: Some(Arc::new(set)),
        }
    }
}

/// Builds an [`Attribute`] over a real field of `T`, paired with its wire
/// name — a `(name, Attribute)` tuple ready for
/// [`AttributeService::with_attribute`]. `attribute!` reads the field
/// reference syntactically and generates the getter/setter; `get` limits
/// the attribute to getter-only (e.g. an id that must not be editable).
///
/// ```ignore
/// use dxlib::attribute;
///
/// // get + set:
/// attribute!("metrics", &portfolio.metrics);
/// // getter only:
/// attribute!("id", &portfolio.id, get);
/// ```
#[macro_export]
macro_rules! attribute {
    ($name:expr, &$port:ident.$field:ident) => {
        (
            $name,
            $crate::network::services::Attribute::read_write(
                move |obj: &_| Ok(obj.$field.clone()),
                move |obj: &mut _, value: _| {
                    obj.$field = value;
                    Ok(())
                },
            ),
        )
    };
    ($name:expr, &$port:ident.$field:ident, get) => {
        (
            $name,
            $crate::network::services::Attribute::getter(move |obj: &_| Ok(obj.$field.clone())),
        )
    };
}

struct ErasedAttribute<T> {
    getter: Option<Arc<dyn Fn(&T) -> Result<Value, ServiceError> + Send + Sync>>,
    setter: Option<Arc<dyn Fn(&mut T, Value) -> Result<(), ServiceError> + Send + Sync>>,
}

pub struct AttributeService<T> {
    name: String,
    instance: Arc<RwLock<T>>,
    attributes: HashMap<String, ErasedAttribute<T>>,
}

impl<T: Send + Sync + 'static> AttributeService<T> {
    pub fn new(name: impl Into<String>, instance: T) -> Self {
        Self {
            name: name.into(),
            instance: Arc::new(RwLock::new(instance)),
            attributes: HashMap::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn instance(&self) -> &RwLock<T> {
        &self.instance
    }

    pub fn with_attribute<A>(
        mut self,
        (name, attribute): (&str, Attribute<T, A>),
    ) -> Self
    where
        A: Serialize + DeserializeOwned + Send + Sync + 'static,
    {
        let erased = ErasedAttribute {
            getter: attribute.getter.map(|getter| {
                let erased: Arc<dyn Fn(&T) -> Result<Value, ServiceError> + Send + Sync> =
                    Arc::new(move |t: &T| {
                        let value = getter(t)?;
                        to_value(&value).map_err(|e| ServiceError::BadValue(e.to_string()))
                    });
                erased
            }),
            setter: attribute.setter.map(|setter| {
                let erased: Arc<dyn Fn(&mut T, Value) -> Result<(), ServiceError> + Send + Sync> =
                    Arc::new(move |t: &mut T, v: Value| {
                        let value: A = from_value(v)
                            .map_err(|e| ServiceError::BadValue(e.to_string()))?;
                        setter(t, value)
                    });
                erased
            }),
        };
        self.attributes.insert(name.to_string(), erased);
        self
    }
}

impl<T: Send + Sync + 'static> Service for AttributeService<T> {
    fn call(&self, request: Request) -> Result<Response, ServiceError> {
        match request {
            Request::Get { attribute } => {
                let attr = self
                    .attributes
                    .get(&attribute)
                    .ok_or_else(|| ServiceError::UnknownAttribute(attribute.clone()))?;
                let getter = attr
                    .getter
                    .as_ref()
                    .ok_or_else(|| ServiceError::ReadOnly(attribute))?;
                let instance = self
                    .instance
                    .read()
                    .map_err(|_| ServiceError::Internal("instance lock poisoned".into()))?;
                let value = getter(&instance)?;
                Ok(Response { value })
            }
            Request::Set { attribute, value } => {
                let attr = self
                    .attributes
                    .get(&attribute)
                    .ok_or_else(|| ServiceError::UnknownAttribute(attribute.clone()))?;
                let setter = attr
                    .setter
                    .as_ref()
                    .ok_or_else(|| ServiceError::WriteOnly(attribute))?;
                let mut instance = self
                    .instance
                    .write()
                    .map_err(|_| ServiceError::Internal("instance lock poisoned".into()))?;
                setter(&mut instance, value)?;
                Ok(Response { value: Value::Null })
            }
        }
    }
}
