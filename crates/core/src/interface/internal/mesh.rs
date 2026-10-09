use std::sync::Arc;

use serde::Deserialize;

use super::Interface;
use crate::network::mesh::Registration;
use crate::Error;

/// The local side of a served mesh: registering services and removing them
/// again, over whatever protocol the [`Interface`] speaks.
pub struct MeshInterface {
    interface: Arc<Interface>,
}

impl MeshInterface {
    pub fn new(interface: Interface) -> Self {
        Self::from_shared(Arc::new(interface))
    }

    pub fn from_shared(interface: Arc<Interface>) -> Self {
        Self { interface }
    }

    /// Returns the uuid the mesh assigned to the registration.
    pub fn register(&self, registration: &Registration) -> Result<String, Error> {
        let args =
            serde_json::to_value(registration).map_err(|e| Error::Interface(e.to_string()))?;
        let value = self.interface.call("services", args)?;
        let registered: Registered =
            serde_json::from_value(value).map_err(|e| Error::Interface(e.to_string()))?;
        Ok(registered.uuid)
    }

    pub fn unregister(&self, uuid: &str) -> Result<(), Error> {
        let args = serde_json::to_value(uuid).map_err(|e| Error::Interface(e.to_string()))?;
        self.interface.call("unregister", args)?;
        Ok(())
    }
}

#[derive(Deserialize)]
struct Registered {
    uuid: String,
}
