use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use super::registry::{registry_guard, MethodKind};
use crate::network::mesh::Protocol;
use crate::Error;

struct Handler {
    invoke: Box<dyn Fn(Value) -> Result<Value, Error> + Send + Sync>,
}

type Accessors = HashMap<Protocol, Arc<dyn Any + Send + Sync>>;

type Binder = Box<dyn FnOnce(&Accessors) -> Result<Handler, Error> + Send + Sync>;

pub struct InterfaceFactory {
    service: String,
    accessors: Accessors,
    methods: Vec<(String, Protocol, MethodKind, Binder)>,
}

impl InterfaceFactory {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
            accessors: HashMap::new(),
            methods: Vec::new(),
        }
    }

    pub fn accessor<A: Send + Sync + 'static>(mut self, protocol: Protocol, accessor: A) -> Self {
        self.accessors.insert(protocol, Arc::new(accessor));
        self
    }

    pub fn get<A, Args, Ret, F>(self, method: &str, protocol: Protocol, handler: F) -> Self
    where
        A: Send + Sync + 'static,
        Args: DeserializeOwned + 'static,
        Ret: Serialize + 'static,
        F: Fn(&A, Args) -> Result<Ret, Error> + Send + Sync + 'static,
    {
        self.register(method, protocol, MethodKind::Get, handler)
    }

    pub fn set<A, Args, Ret, F>(self, method: &str, protocol: Protocol, handler: F) -> Self
    where
        A: Send + Sync + 'static,
        Args: DeserializeOwned + 'static,
        Ret: Serialize + 'static,
        F: Fn(&A, Args) -> Result<Ret, Error> + Send + Sync + 'static,
    {
        self.register(method, protocol, MethodKind::Set, handler)
    }

    fn register<A, Args, Ret, F>(
        mut self,
        method: &str,
        protocol: Protocol,
        kind: MethodKind,
        handler: F,
    ) -> Self
    where
        A: Send + Sync + 'static,
        Args: DeserializeOwned + 'static,
        Ret: Serialize + 'static,
        F: Fn(&A, Args) -> Result<Ret, Error> + Send + Sync + 'static,
    {
        let method = method.to_string();
        let label = method.clone();
        let binder: Binder = Box::new(move |accessors: &Accessors| {
            let accessor = accessors.get(&protocol).cloned().ok_or_else(|| {
                Error::Interface(format!("{label} uses {protocol:?}, which has no accessor"))
            })?;
            let label = label.clone();
            Ok(Handler {
                invoke: Box::new(move |value: Value| {
                    let args: Args = serde_json::from_value(value)
                        .map_err(|e| Error::Interface(format!("{label}: {e}")))?;
                    let accessor = accessor.downcast_ref::<A>().ok_or_else(|| {
                        Error::Interface(format!(
                            "{label}: {protocol:?} accessor has the wrong type"
                        ))
                    })?;
                    let ret = handler(accessor, args)?;
                    serde_json::to_value(ret)
                        .map_err(|e| Error::Interface(format!("{label}: {e}")))
                }),
            })
        });
        self.methods.push((method, protocol, kind, binder));
        self
    }

    pub fn build(self) -> Result<Interface, Error> {
        let Self {
            service,
            accessors,
            methods,
        } = self;

        let spec = registry_guard()
            .get(&service)
            .cloned()
            .ok_or_else(|| Error::Interface(format!("service {service} is not registered")))?;

        let mut handlers = HashMap::with_capacity(methods.len());
        for (method, protocol, kind, binder) in methods {
            let declared = spec.method(&method).ok_or_else(|| {
                Error::Interface(format!("{service} has no method {method}"))
            })?;
            if declared.kind() != kind {
                return Err(Error::Interface(format!(
                    "{service}.{method} is a {:?} method, registered as a {kind:?}",
                    declared.kind()
                )));
            }
            if !declared.accepts(protocol) {
                return Err(Error::Interface(format!(
                    "{service}.{method} does not accept {protocol:?}"
                )));
            }
            handlers.insert(method, binder(&accessors)?);
        }

        Ok(Interface { service, handlers })
    }
}

pub struct Interface {
    service: String,
    handlers: HashMap<String, Handler>,
}

impl std::fmt::Debug for Interface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Interface")
            .field("service", &self.service)
            .field("methods", &self.handlers.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Interface {
    pub fn service(&self) -> &str {
        &self.service
    }

    pub fn call(&self, method: &str, args: Value) -> Result<Value, Error> {
        let handler = self.handlers.get(method).ok_or_else(|| {
            Error::Interface(format!("{} has no handler for {method}", self.service))
        })?;
        (handler.invoke)(args)
    }
}
