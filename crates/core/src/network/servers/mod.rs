pub mod http;

use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;

use crate::network::services::{Service, ServiceError};

pub use http::{HttpServer, ServerHandle};

/// A server implementation a [`ServiceManager`](crate::network::ServiceManager)
/// can bind, start, and stop.
pub trait Server: Sized {
    /// The handle a spawned server returns.
    type Handle;

    /// Bind a server to `addr` serving `service`.
    fn bind(addr: impl ToSocketAddrs, service: Arc<dyn Service>) -> Result<Self, ServiceError>;

    /// The actual bound address.
    fn addr(&self) -> SocketAddr;

    /// Serve on a background thread, returning a handle that can stop it.
    fn spawn(self) -> Self::Handle;

    /// Stop a spawned server.
    fn stop(handle: Self::Handle) -> Result<(), ServiceError>;
}

impl Server for HttpServer {
    type Handle = ServerHandle;

    fn bind(addr: impl ToSocketAddrs, service: Arc<dyn Service>) -> Result<Self, ServiceError> {
        HttpServer::bind(addr, service)
    }

    fn addr(&self) -> SocketAddr {
        HttpServer::addr(self)
    }

    fn spawn(self) -> Self::Handle {
        HttpServer::spawn(self)
    }

    fn stop(handle: Self::Handle) -> Result<(), ServiceError> {
        handle.stop()
    }
}
