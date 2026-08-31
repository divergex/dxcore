use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, RwLock};

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::serialization::{from_value, to_value};

#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// Read an attribute, or call an immutable method. `args` holds the
    /// method arguments as JSON (a single value, or an array for multiple);
    /// `None` for plain attribute reads.
    Get {
        attribute: String,
        args: Option<Value>,
    },
    /// Write an attribute, or call a mutable method. `value` is the new
    /// attribute value, or the method arguments as JSON.
    Set { attribute: String, value: Value },
    /// Create a resource, e.g. `POST /services` on a mesh. `value` is the
    /// request body as JSON.
    Post { attribute: String, value: Value },
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

    /// Name this service registers under in a mesh.
    fn name(&self) -> String {
        "service".into()
    }

    /// Endpoint paths this service serves, as `/path` strings. Empty for
    /// services that address requests dynamically.
    fn endpoints(&self) -> Vec<String> {
        Vec::new()
    }
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

struct ErasedEntry<T> {
    get: Option<Arc<dyn Fn(&T, Option<Value>) -> Result<Value, ServiceError> + Send + Sync>>,
    set: Option<Arc<dyn Fn(&mut T, Value) -> Result<Value, ServiceError> + Send + Sync>>,
}

pub struct AttributeService<T> {
    name: String,
    instance: Arc<RwLock<T>>,
    entries: HashMap<String, ErasedEntry<T>>,
}

impl<T: Send + Sync + 'static> AttributeService<T> {
    pub fn new(name: impl Into<String>, instance: T) -> Self {
        Self {
            name: name.into(),
            instance: Arc::new(RwLock::new(instance)),
            entries: HashMap::new(),
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
        let entry = ErasedEntry {
            get: attribute.getter.map(|getter| {
                let erased: Arc<dyn Fn(&T, Option<Value>) -> Result<Value, ServiceError> + Send + Sync> =
                    Arc::new(move |t: &T, _args: Option<Value>| {
                        let value = getter(t)?;
                        to_value(&value).map_err(|e| ServiceError::BadValue(e.to_string()))
                    });
                erased
            }),
            set: attribute.setter.map(|setter| {
                let erased: Arc<dyn Fn(&mut T, Value) -> Result<Value, ServiceError> + Send + Sync> =
                    Arc::new(move |t: &mut T, v: Value| {
                        let value: A = from_value(v)
                            .map_err(|e| ServiceError::BadValue(e.to_string()))?;
                        setter(t, value)?;
                        Ok(Value::Null)
                    });
                erased
            }),
        };
        self.entries.insert(name.to_string(), entry);
        self
    }

    /// Register an immutable method: `GET`-only, arguments are deserialized
    /// from the request with `from_value` and the return value is serialized
    /// with `to_value`. `Args` is a single value for one parameter, or a
    /// tuple for several.
    pub fn with_get<Args, Ret>(
        mut self,
        name: &str,
        method: impl Fn(&T, Args) -> Result<Ret, ServiceError> + Send + Sync + 'static,
    ) -> Self
    where
        Args: DeserializeOwned + Send + Sync + 'static,
        Ret: Serialize + Send + Sync + 'static,
    {
        let erased: Arc<dyn Fn(&T, Option<Value>) -> Result<Value, ServiceError> + Send + Sync> =
            Arc::new(move |t: &T, args: Option<Value>| {
                let args = args
                    .ok_or_else(|| ServiceError::BadValue("method requires arguments".into()))?;
                let args: Args = from_value(args)
                    .map_err(|e| ServiceError::BadValue(e.to_string()))?;
                let ret = method(t, args)?;
                to_value(&ret).map_err(|e| ServiceError::BadValue(e.to_string()))
            });
        self.entries.insert(
            name.to_string(),
            ErasedEntry {
                get: Some(erased),
                set: None,
            },
        );
        self
    }

    /// Register a mutable method: `PUT`-only, arguments are deserialized from
    /// the request with `from_value` and the return value is serialized with
    /// `to_value`. `Args` is a single value for one parameter, or a tuple
    /// for several.
    pub fn with_set<Args, Ret>(
        mut self,
        name: &str,
        method: impl Fn(&mut T, Args) -> Result<Ret, ServiceError> + Send + Sync + 'static,
    ) -> Self
    where
        Args: DeserializeOwned + Send + Sync + 'static,
        Ret: Serialize + Send + Sync + 'static,
    {
        let erased: Arc<dyn Fn(&mut T, Value) -> Result<Value, ServiceError> + Send + Sync> =
            Arc::new(move |t: &mut T, args: Value| {
                let args: Args = from_value(args)
                    .map_err(|e| ServiceError::BadValue(e.to_string()))?;
                let ret = method(t, args)?;
                to_value(&ret).map_err(|e| ServiceError::BadValue(e.to_string()))
            });
        self.entries.insert(
            name.to_string(),
            ErasedEntry {
                get: None,
                set: Some(erased),
            },
        );
        self
    }
}

impl<T: Send + Sync + 'static> Service for AttributeService<T> {
    fn call(&self, request: Request) -> Result<Response, ServiceError> {
        match request {
            Request::Get { attribute, args } => {
                let entry = self
                    .entries
                    .get(&attribute)
                    .ok_or_else(|| ServiceError::UnknownAttribute(attribute.clone()))?;
                let getter = entry
                    .get
                    .as_ref()
                    .ok_or_else(|| ServiceError::ReadOnly(attribute))?;
                let instance = self
                    .instance
                    .read()
                    .map_err(|_| ServiceError::Internal("instance lock poisoned".into()))?;
                let value = getter(&instance, args)?;
                Ok(Response { value })
            }
            Request::Set { attribute, value } => {
                let entry = self
                    .entries
                    .get(&attribute)
                    .ok_or_else(|| ServiceError::UnknownAttribute(attribute.clone()))?;
                let setter = entry
                    .set
                    .as_ref()
                    .ok_or_else(|| ServiceError::WriteOnly(attribute))?;
                let mut instance = self
                    .instance
                    .write()
                    .map_err(|_| ServiceError::Internal("instance lock poisoned".into()))?;
                let value = setter(&mut instance, value)?;
                Ok(Response { value })
            }
            Request::Post { attribute, .. } => Err(ServiceError::WriteOnly(attribute)),
        }
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn endpoints(&self) -> Vec<String> {
        self.entries.keys().map(|name| format!("/{name}")).collect()
    }
}
