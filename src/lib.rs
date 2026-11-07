//! CONTRACT: transport-independent API application core.
pub mod http;
pub mod middleware;
pub mod router;
pub mod rpc;

use crate::{middleware::{Middleware, Pipeline}, router::{RouteError, Router}};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Method { Get, Post, Put, Patch, Delete }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub method: Method,
    pub path: String,
    pub body: Vec<u8>,
    pub path_params: BTreeMap<String, String>,
}
impl Request {
    pub fn new(method: Method, path: impl Into<String>, body: impl Into<Vec<u8>>) -> Self {
        Self { method, path: path.into(), body: body.into(), path_params: BTreeMap::new() }
    }
    pub fn param(&self, name: &str) -> Option<&str> {
        self.path_params.get(name).map(String::as_str)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}
impl Response {
    pub fn new(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self { status, headers: BTreeMap::new(), body: body.into() }
    }
    pub fn text(status: u16, body: impl Into<String>) -> Self {
        let mut response = Self::new(status, body.into().into_bytes());
        response.headers.insert("content-type".into(), "text/plain; charset=utf-8".into());
        response
    }
}

/// Application composes route matching and middleware into one dispatch path.
#[derive(Default)]
pub struct Application {
    router: Router,
    middleware: Pipeline,
}
impl Application {
    pub fn new() -> Self { Self::default() }

    pub fn route<F>(&mut self, method: Method, pattern: impl Into<String>, handler: F) -> Result<(), RouteError>
    where F: Fn(&Request) -> Response + Send + Sync + 'static {
        self.router.add(method, pattern, handler)
    }

    pub fn add_middleware<M: Middleware + 'static>(&mut self, middleware: M) {
        self.middleware.push(middleware);
    }

    pub fn dispatch(&self, request: Request) -> Response {
        let routed_request = request.clone();
        self.middleware.execute(&request, |_| self.router.dispatch(routed_request))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct HeaderMarker;
    impl Middleware for HeaderMarker {
        fn before(&self, _: &Request) -> Option<Response> { None }
        fn after(&self, _: &Request, mut response: Response) -> Response {
            response.headers.insert("x-contract-pipeline".into(), "active".into());
            response
        }
    }

    #[test]
    fn application_integrates_router_precedence_and_middleware() {
        let mut app = Application::new();
        app.add_middleware(HeaderMarker);
        app.route(Method::Get, "/robots/:id", |_| Response::text(200, "parameter")).unwrap();
        app.route(Method::Get, "/robots/status", |_| Response::text(200, "literal")).unwrap();
        let response = app.dispatch(Request::new(Method::Get, "/robots/status", Vec::new()));
        assert_eq!(response.body, b"literal");
        assert_eq!(response.headers.get("x-contract-pipeline").map(String::as_str), Some("active"));
    }

    #[test]
    fn application_rejects_duplicate_routes() {
        let mut app = Application::new();
        app.route(Method::Get, "/health", |_| Response::text(200, "ok")).unwrap();
        assert_eq!(app.route(Method::Get, "/health", |_| Response::text(200, "again")), Err(RouteError::DuplicateRoute));
    }

    #[test]
    fn application_distinguishes_not_found_and_method_not_allowed() {
        let mut app = Application::new();
        app.route(Method::Get, "/health", |_| Response::text(200, "ok")).unwrap();
        assert_eq!(app.dispatch(Request::new(Method::Post, "/health", Vec::new())).status, 405);
        assert_eq!(app.dispatch(Request::new(Method::Get, "/missing", Vec::new())).status, 404);
    }
}
