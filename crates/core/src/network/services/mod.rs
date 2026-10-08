pub mod attribute;
mod base;
pub mod functional;

use std::sync::{Arc, RwLock};

use serde::de::DeserializeOwned;
use serde::Serialize;

pub use attribute::{Attribute, AttributeService};
pub use base::{Request, Response, Service, ServiceError};
pub use functional::FunctionalService;

/// Serves one instance's attributes and functions; attribute names take
/// precedence when both register the same name.
pub struct ClassService<T> {
    name: String,
    attributes: AttributeService<T>,
    functions: FunctionalService<T>,
}

impl<T: Send + Sync + 'static> ClassService<T> {
    pub fn new(name: impl Into<String>, instance: T) -> Self {
        let name = name.into();
        let instance = Arc::new(RwLock::new(instance));
        Self {
            attributes: AttributeService::from_shared(name.clone(), Arc::clone(&instance)),
            functions: FunctionalService::from_shared(name.clone(), instance),
            name,
        }
    }

    pub fn with_attribute<A>(mut self, attribute: (&str, Attribute<T, A>)) -> Self
    where
        A: Serialize + DeserializeOwned + Send + Sync + 'static,
    {
        self.attributes = self.attributes.with_attribute(attribute);
        self
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
        self.functions = self.functions.with_get(name, method);
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
        self.functions = self.functions.with_set(name, method);
        self
    }
}

impl<T: Send + Sync + 'static> Service for ClassService<T> {
    fn call(&self, request: Request) -> Result<Response, ServiceError> {
        match self.attributes.call(request.clone()) {
            Err(ServiceError::UnknownAttribute(_)) => self.functions.call(request),
            result => result,
        }
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn endpoints(&self) -> Vec<String> {
        let mut endpoints = self.attributes.endpoints();
        endpoints.extend(self.functions.endpoints());
        endpoints
    }
}
