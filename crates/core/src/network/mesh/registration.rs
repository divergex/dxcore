use std::sync::Arc;
use std::time::Duration;

use reqwest::blocking::Client;

use crate::interface::internal::{
    http_register, http_unregister, HttpAccessor, InterfaceFactory, MeshInterface,
};
use crate::network::mesh::{Endpoint, Protocol, Registration};
use crate::network::services::Service;
use crate::Error;

// Drop unregisters synchronously, so an unreachable mesh must not hang the
// process waiting to exit.
const MESH_TIMEOUT: Duration = Duration::from_secs(5);

impl Registration {
    /// The registration a served service announces: its name, url, protocol and
    /// the endpoints it exposes.
    pub fn of(service: &Arc<dyn Service>, url: impl Into<String>, protocol: Protocol) -> Self {
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
        Self {
            name: service.name(),
            url: url.into(),
            protocols: vec![protocol],
            endpoints,
        }
    }

    /// Translates a served service into a registration and posts it to a mesh.
    pub fn builder<S>(service: Arc<S>, url: impl Into<String>) -> RegistrationBuilder
    where
        S: Service + 'static,
    {
        RegistrationBuilder::new(service, url)
    }
}

/// Builds a [`Registration`] from a served service and registers it with a mesh.
pub struct RegistrationBuilder {
    service: Arc<dyn Service>,
    url: String,
    mesh: Option<String>,
}

impl RegistrationBuilder {
    pub fn new<S>(service: Arc<S>, url: impl Into<String>) -> Self
    where
        S: Service + 'static,
    {
        Self {
            service,
            url: url.into(),
            mesh: None,
        }
    }

    /// The mesh to register with.
    pub fn mesh(mut self, url: impl Into<String>) -> Self {
        self.mesh = Some(url.into());
        self
    }

    /// The payload as it will be sent.
    pub fn build(&self) -> Registration {
        Registration::of(&self.service, self.url.clone(), Protocol::Http)
    }

    /// Registers with the configured mesh, returning a handle that can remove
    /// the registration again.
    pub fn register(self) -> Result<RegistrationHandle, Error> {
        let mesh_url = self
            .mesh
            .clone()
            .ok_or_else(|| Error::Interface("no mesh url configured".into()))?;
        let client = Client::builder()
            .timeout(MESH_TIMEOUT)
            .build()
            .map_err(|e| Error::Http(e.to_string()))?;
        let interface = InterfaceFactory::new("mesh")
            .accessor(Protocol::Http, HttpAccessor::with_client(client, mesh_url))
            .post("services", Protocol::Http, http_register)
            .delete("unregister", Protocol::Http, http_unregister)
            .build()?;
        let mesh = MeshInterface::new(interface);
        let uuid = mesh.register(&self.build())?;
        Ok(RegistrationHandle {
            uuid,
            registered: true,
            mesh,
        })
    }
}

/// A registration the mesh accepted.
///
/// Dropping the handle removes the registration, so a host that goes away
/// without calling [`unregister`](Self::unregister) leaves no entry behind. Drop
/// reports failures on stderr, since it cannot return them.
pub struct RegistrationHandle {
    uuid: String,
    registered: bool,
    mesh: MeshInterface,
}

impl RegistrationHandle {
    pub fn uuid(&self) -> &str {
        &self.uuid
    }

    /// Removes the registration from the mesh.
    pub fn unregister(mut self) -> Result<(), Error> {
        self.registered = false;
        self.mesh.unregister(&self.uuid)
    }
}

impl Drop for RegistrationHandle {
    fn drop(&mut self) {
        if !self.registered {
            return;
        }
        // Swallow the error: a mesh that is gone must not abort an unwinding
        // process through a panic in drop.
        if let Err(error) = self.mesh.unregister(&self.uuid) {
            eprintln!("failed to unregister {}: {error}", self.uuid);
        }
    }
}
