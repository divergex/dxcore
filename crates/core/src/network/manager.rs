use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;

use crate::network::mesh::{RegistrationBuilder, RegistrationHandle};
use crate::network::servers::{HttpServer, Server};
use crate::network::services::{Service, ServiceError};

/// Owns the serving lifecycle of a set of [`Service`]s: bind servers, start and
/// stop them together or individually, and announce the running set to a mesh.
///
/// The mesh is external: [`ServiceManager::register_to_mesh`] takes its URL and
/// posts one registration per running service. Stopping a service removes it
/// from the mesh again.
///
/// `S` is the server implementation, defaulting to [`HttpServer`].
pub struct ServiceManager<S: Server = HttpServer> {
    services: HashMap<String, Managed<S>>,
}

struct Managed<S: Server> {
    service: Arc<dyn Service>,
    addr: Option<SocketAddr>,
    server: Option<S>,
    handle: Option<S::Handle>,
    registration: Option<RegistrationHandle>,
}

impl<S: Server> ServiceManager<S> {
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
        }
    }

    /// Register `service` under `name`. Fails if `name` is already taken.
    pub fn add(
        &mut self,
        name: impl Into<String>,
        service: Arc<dyn Service>,
    ) -> Result<(), ServiceError> {
        let name = name.into();
        if self.services.contains_key(&name) {
            return Err(ServiceError::BadValue(format!("service {name} already added")));
        }
        self.services.insert(
            name,
            Managed {
                service,
                addr: None,
                server: None,
                handle: None,
                registration: None,
            },
        );
        Ok(())
    }

    /// Bind a server for `name` at `addr`. Fails if `name` is unknown, already
    /// bound, or the bind itself fails.
    pub fn bind(&mut self, name: &str, addr: impl ToSocketAddrs) -> Result<(), ServiceError> {
        let managed = self
            .services
            .get_mut(name)
            .ok_or_else(|| ServiceError::UnknownAttribute(name.to_string()))?;
        if managed.server.is_some() {
            return Err(ServiceError::BadValue(format!("service {name} already bound")));
        }
        let server = S::bind(addr, Arc::clone(&managed.service))?;
        managed.addr = Some(server.addr());
        managed.server = Some(server);
        Ok(())
    }

    /// Spawn a background thread for every bound-but-not-started server.
    pub fn start(&mut self) {
        for managed in self.services.values_mut() {
            if managed.handle.is_some() || managed.server.is_none() {
                continue;
            }
            let server = managed.server.take().expect("server present");
            managed.handle = Some(S::spawn(server));
        }
    }

    /// The bound address of `name`, if a server has been bound.
    pub fn addr(&self, name: &str) -> Option<SocketAddr> {
        self.services.get(name).and_then(|m| m.addr)
    }

    /// Announce every running server to the mesh at `mesh_url`. Already
    /// registered services are left alone; if one registration fails, those
    /// made by this call are rolled back.
    pub fn register_to_mesh(&mut self, mesh_url: &str) -> Result<(), ServiceError> {
        let mut fresh: Vec<(String, RegistrationHandle)> = Vec::new();
        for (name, managed) in &mut self.services {
            if managed.registration.is_some() || managed.handle.is_none() {
                continue;
            }
            let url = format!("http://{}", managed.addr.expect("started server has an address"));
            let handle = RegistrationBuilder::new(Arc::clone(&managed.service), url)
                .mesh(mesh_url)
                .register()
                .map_err(|e| ServiceError::Internal(e.to_string()))?;
            fresh.push((name.clone(), handle));
        }
        for (name, handle) in fresh {
            self.services
                .get_mut(&name)
                .expect("service present")
                .registration = Some(handle);
        }
        Ok(())
    }

    /// Stop one server and remove it from the mesh.
    pub fn stop(&mut self, name: &str) -> Result<(), ServiceError> {
        let managed = self
            .services
            .get_mut(name)
            .ok_or_else(|| ServiceError::UnknownAttribute(name.to_string()))?;

        if let Some(registration) = managed.registration.take() {
            registration
                .unregister()
                .map_err(|e| ServiceError::Internal(e.to_string()))?;
        }
        if let Some(handle) = managed.handle.take() {
            S::stop(handle)?;
        }
        managed.server = None;
        managed.addr = None;
        Ok(())
    }

    /// Stop and unregister every service, continuing past individual failures.
    pub fn stop_all(&mut self) -> Result<(), ServiceError> {
        let names: Vec<String> = self.services.keys().cloned().collect();
        let mut first: Option<ServiceError> = None;
        for name in names {
            if let Err(e) = self.stop(&name) {
                if first.is_none() {
                    first = Some(e);
                }
            }
        }
        match first {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

impl<S: Server> Default for ServiceManager<S> {
    fn default() -> Self {
        Self::new()
    }
}
