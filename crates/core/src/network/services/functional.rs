use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use super::base::{call_entry, Entry, Getter, Request, Response, Service, ServiceError, Setter};
use crate::serialization::{from_value, to_value};

pub struct FunctionalService<T> {
    name: String,
    instance: Arc<RwLock<T>>,
    entries: HashMap<String, Entry<T>>,
}

impl<T: Send + Sync + 'static> FunctionalService<T> {
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

    pub fn with_get<Args, Ret>(
        mut self,
        name: &str,
        method: impl Fn(&T, Args) -> Result<Ret, ServiceError> + Send + Sync + 'static,
    ) -> Self
    where
        Args: DeserializeOwned + Send + Sync + 'static,
        Ret: Serialize + Send + Sync + 'static,
    {
        let erased: Getter<T> = Arc::new(move |t: &T, args: Option<Value>| {
            let args =
                args.ok_or_else(|| ServiceError::BadValue("method requires arguments".into()))?;
            let args: Args = from_value(args).map_err(|e| ServiceError::BadValue(e.to_string()))?;
            let ret = method(t, args)?;
            to_value(&ret).map_err(|e| ServiceError::BadValue(e.to_string()))
        });
        self.entries.insert(
            name.to_string(),
            Entry {
                get: Some(erased),
                set: None,
            },
        );
        self
    }

    pub fn with_set<Args, Ret>(
        mut self,
        name: &str,
        method: impl Fn(&mut T, Args) -> Result<Ret, ServiceError> + Send + Sync + 'static,
    ) -> Self
    where
        Args: DeserializeOwned + Send + Sync + 'static,
        Ret: Serialize + Send + Sync + 'static,
    {
        let erased: Setter<T> = Arc::new(move |t: &mut T, args: Value| {
            let args: Args = from_value(args).map_err(|e| ServiceError::BadValue(e.to_string()))?;
            let ret = method(t, args)?;
            to_value(&ret).map_err(|e| ServiceError::BadValue(e.to_string()))
        });
        self.entries.insert(
            name.to_string(),
            Entry {
                get: None,
                set: Some(erased),
            },
        );
        self
    }
}

impl<T: Send + Sync + 'static> Service for FunctionalService<T> {
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
