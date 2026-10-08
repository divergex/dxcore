use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use super::base::{call_entry, Entry, Getter, Request, Response, Service, ServiceError, Setter};
use crate::serialization::{from_value, to_value};

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
/// use dxcore::attribute;
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

pub struct AttributeService<T> {
    name: String,
    instance: Arc<RwLock<T>>,
    entries: HashMap<String, Entry<T>>,
}

impl<T: Send + Sync + 'static> AttributeService<T> {
    pub fn new(name: impl Into<String>, instance: T) -> Self {
        Self::from_shared(name.into(), Arc::new(RwLock::new(instance)))
    }

    pub(crate) fn from_shared(name: String, instance: Arc<RwLock<T>>) -> Self {
        Self {
            name,
            instance,
            entries: HashMap::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn instance(&self) -> &Arc<RwLock<T>> {
        &self.instance
    }

    pub fn with_attribute<A>(mut self, (name, attribute): (&str, Attribute<T, A>)) -> Self
    where
        A: Serialize + DeserializeOwned + Send + Sync + 'static,
    {
        let entry = Entry {
            get: attribute.getter.map(|getter| {
                let erased: Getter<T> = Arc::new(move |t: &T, _args: Option<Value>| {
                    let value = getter(t)?;
                    to_value(&value).map_err(|e| ServiceError::BadValue(e.to_string()))
                });
                erased
            }),
            set: attribute.setter.map(|setter| {
                let erased: Setter<T> = Arc::new(move |t: &mut T, v: Value| {
                    let value: A =
                        from_value(v).map_err(|e| ServiceError::BadValue(e.to_string()))?;
                    setter(t, value)?;
                    Ok(Value::Null)
                });
                erased
            }),
        };
        self.entries.insert(name.to_string(), entry);
        self
    }
}

impl<T: Send + Sync + 'static> Service for AttributeService<T> {
    fn call(&self, request: Request) -> Result<Response, ServiceError> {
        call_entry(&self.entries, &self.instance, request)
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn endpoints(&self) -> Vec<String> {
        self.entries.keys().map(|name| format!("/{name}")).collect()
    }
}
