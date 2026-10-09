use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, MutexGuard};

use crate::network::mesh::Protocol;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodKind {
    Get,
    Set,
    Post,
    Delete,
}

#[derive(Debug, Clone)]
pub struct MethodSpec {
    kind: MethodKind,
    protocols: Vec<Protocol>,
}

impl MethodSpec {
    pub fn new(kind: MethodKind, protocols: &[Protocol]) -> Self {
        Self {
            kind,
            protocols: protocols.to_vec(),
        }
    }

    pub fn kind(&self) -> MethodKind {
        self.kind
    }

    pub fn protocols(&self) -> &[Protocol] {
        &self.protocols
    }

    pub fn accepts(&self, protocol: Protocol) -> bool {
        self.protocols.contains(&protocol)
    }
}

#[derive(Debug, Clone)]
pub struct ServiceSpec {
    pub name: String,
    methods: HashMap<String, MethodSpec>,
}

impl ServiceSpec {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            methods: HashMap::new(),
        }
    }

    pub fn with_get(mut self, method: &str, protocols: &[Protocol]) -> Self {
        self.methods
            .insert(method.into(), MethodSpec::new(MethodKind::Get, protocols));
        self
    }

    pub fn with_set(mut self, method: &str, protocols: &[Protocol]) -> Self {
        self.methods
            .insert(method.into(), MethodSpec::new(MethodKind::Set, protocols));
        self
    }

    pub fn with_post(mut self, method: &str, protocols: &[Protocol]) -> Self {
        self.methods
            .insert(method.into(), MethodSpec::new(MethodKind::Post, protocols));
        self
    }

    pub fn with_delete(mut self, method: &str, protocols: &[Protocol]) -> Self {
        self.methods
            .insert(method.into(), MethodSpec::new(MethodKind::Delete, protocols));
        self
    }

    pub fn method(&self, name: &str) -> Option<&MethodSpec> {
        self.methods.get(name)
    }

    pub fn methods(&self) -> impl Iterator<Item = (&str, &MethodSpec)> {
        self.methods.iter().map(|(name, spec)| (name.as_str(), spec))
    }
}

#[derive(Default)]
pub struct Registry {
    services: HashMap<String, ServiceSpec>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, spec: ServiceSpec) {
        self.services.insert(spec.name.clone(), spec);
    }

    pub fn get(&self, service: &str) -> Option<&ServiceSpec> {
        self.services.get(service)
    }
}

static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(|| {
    let mut registry = Registry::new();
    register_defaults(&mut registry);
    Mutex::new(registry)
});

pub fn registry_guard() -> MutexGuard<'static, Registry> {
    REGISTRY.lock().expect("registry lock not poisoned")
}

fn register_defaults(registry: &mut Registry) {
    registry.register(ServiceSpec::new("strategy").with_set("on_step", &[Protocol::Http]));
    registry.register(
        ServiceSpec::new("trading")
            .with_get("market_history", &[Protocol::Http])
            .with_get("portfolio", &[Protocol::Http])
            .with_get("listen_async", &[Protocol::Http])
            .with_get("events", &[Protocol::Http]),
    );
    registry.register(
        ServiceSpec::new("mesh")
            .with_post("services", &[Protocol::Http])
            .with_delete("unregister", &[Protocol::Http]),
    );
}
