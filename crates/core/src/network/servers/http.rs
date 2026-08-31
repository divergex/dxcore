
use std::net::ToSocketAddrs;
use std::sync::Arc;

use tiny_http::{Header, Method, Response, Server, StatusCode};

use crate::network::services::{Request, Response as ServiceResponse, Service, ServiceError};

pub struct HttpServer {
    server: Arc<Server>,
    service: Arc<dyn Service>,
}

impl HttpServer {
    /// Bind a server to `addr` serving `service`.
    pub fn bind(
        addr: impl ToSocketAddrs,
        service: Arc<dyn Service>,
    ) -> Result<Self, ServiceError> {
        let server = Server::http(addr).map_err(|e| ServiceError::Internal(e.to_string()))?;
        Ok(Self {
            server: Arc::new(server),
            service,
        })
    }

    pub fn addr(&self) -> std::net::SocketAddr {
        self.server
            .server_addr()
            .to_ip()
            .expect("http server always binds an IP socket")
    }

    /// Serve incoming requests on the current thread until
    /// [`ServerHandle::stop`] unblocks the listener.
    pub fn serve(self) -> Result<(), ServiceError> {
        for mut request in self.server.incoming_requests() {
            let response = dispatch(self.service.as_ref(), &mut request);
            request
                .respond(response)
                .map_err(|e| ServiceError::Internal(e.to_string()))?;
        }
        Ok(())
    }

    /// Serve on a background thread. The returned handle can stop the server.
    pub fn spawn(self) -> ServerHandle {
        let server = Arc::clone(&self.server);
        let service = Arc::clone(&self.service);
        let handle = std::thread::spawn(move || {
            for mut request in server.incoming_requests() {
                let response = dispatch(service.as_ref(), &mut request);
                request
                    .respond(response)
                    .map_err(|e| ServiceError::Internal(e.to_string()))?;
            }
            Ok(())
        });
        ServerHandle {
            handle,
            server: self.server,
        }
    }
}

pub struct ServerHandle {
    handle: std::thread::JoinHandle<Result<(), ServiceError>>,
    server: Arc<Server>,
}

impl ServerHandle {
    pub fn stop(self) -> Result<(), ServiceError> {
        self.server.unblock();
        self.handle
            .join()
            .map_err(|_| ServiceError::Internal("server thread panicked".into()))?
    }

    pub fn addr(&self) -> std::net::SocketAddr {
        self.server
            .server_addr()
            .to_ip()
            .expect("http server always binds an IP socket")
    }
}

fn dispatch(
    service: &dyn Service,
    request: &mut tiny_http::Request,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let attribute = request.url().trim_start_matches('/').to_string();

    let service_request = match request.method() {
        Method::Get => match parse_optional_json_body(request) {
            Ok(args) => Request::Get { attribute, args },
            Err(response) => return response,
        },
        Method::Put => match parse_json_body(request) {
            Ok(value) => Request::Set { attribute, value },
            Err(response) => return response,
        },
        _ => return json_error(StatusCode(405), "method not allowed".into()),
    };

    match service.call(service_request) {
        Ok(ServiceResponse { value }) => json_response(value),
        Err(err) => json_error(status_for(&err), err.to_string()),
    }
}

/// Parse the request body as JSON, or `None` when the client sent no body.
fn parse_optional_json_body(
    request: &mut tiny_http::Request,
) -> Result<Option<serde_json::Value>, Response<std::io::Cursor<Vec<u8>>>> {
    let mut body = Vec::new();
    request
        .as_reader()
        .read_to_end(&mut body)
        .map_err(|e| json_error(StatusCode(400), e.to_string()))?;
    if body.is_empty() {
        return Ok(None);
    }
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|e| json_error(StatusCode(400), format!("invalid JSON body: {e}")))
}

fn parse_json_body(
    request: &mut tiny_http::Request,
) -> Result<serde_json::Value, Response<std::io::Cursor<Vec<u8>>>> {
    let mut body = Vec::new();
    request
        .as_reader()
        .read_to_end(&mut body)
        .map_err(|e| json_error(StatusCode(400), e.to_string()))?;
    serde_json::from_slice(&body)
        .map_err(|e| json_error(StatusCode(400), format!("invalid JSON body: {e}")))
}

fn status_for(err: &ServiceError) -> StatusCode {
    match err {
        ServiceError::UnknownAttribute(_) => StatusCode(404),
        ServiceError::ReadOnly(_) | ServiceError::WriteOnly(_) => StatusCode(405),
        ServiceError::BadValue(_) => StatusCode(400),
        ServiceError::Internal(_) => StatusCode(500),
    }
}

fn json_response(data: serde_json::Value) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(data.to_string())
        .with_status_code(StatusCode(200))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
}

fn json_error(status: StatusCode, message: String) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(serde_json::json!({ "error": message }).to_string())
        .with_status_code(status)
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
}
