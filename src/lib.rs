//! CONTRACT: a small, transport-independent API application core.

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

type Handler = Box<dyn Fn(&Request) -> Response + Send + Sync + 'static>;
struct Route { method: Method, pattern: String, handler: Handler }

#[derive(Default)]
pub struct Application { routes: Vec<Route> }

impl Application {
    pub fn new() -> Self { Self::default() }

    pub fn route<F>(&mut self, method: Method, pattern: impl Into<String>, handler: F)
    where F: Fn(&Request) -> Response + Send + Sync + 'static {
        self.routes.push(Route { method, pattern: pattern.into(), handler: Box::new(handler) });
    }

    pub fn dispatch(&self, request: Request) -> Response {
        let mut path_matched = false;
        for route in &self.routes {
            if let Some(params) = match_path(&route.pattern, &request.path) {
                path_matched = true;
                if route.method == request.method {
                    let mut request = request;
                    request.path_params = params;
                    return (route.handler)(&request);
                }
            }
        }
        if path_matched { Response::text(405, "method not allowed") }
        else { Response::text(404, "not found") }
    }
}

fn match_path(pattern: &str, path: &str) -> Option<BTreeMap<String, String>> {
    let p: Vec<_> = pattern.trim_matches('/').split('/').collect();
    let a: Vec<_> = path.trim_matches('/').split('/').collect();
    if p.len() != a.len() { return None; }
    let mut params = BTreeMap::new();
    for (expected, actual) in p.iter().zip(a.iter()) {
        if let Some(name) = expected.strip_prefix(':') {
            if name.is_empty() { return None; }
            params.insert(name.to_string(), (*actual).to_string());
        } else if expected != actual { return None; }
    }
    Some(params)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dispatches_named_parameters() {
        let mut app = Application::new();
        app.route(Method::Get, "/robots/:id", |req| Response::text(200, req.param("id").unwrap()));
        let response = app.dispatch(Request::new(Method::Get, "/robots/FB-01", Vec::new()));
        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"FB-01");
    }
    #[test]
    fn distinguishes_missing_route_and_wrong_method() {
        let mut app = Application::new();
        app.route(Method::Get, "/health", |_| Response::text(200, "ok"));
        assert_eq!(app.dispatch(Request::new(Method::Post, "/health", Vec::new())).status, 405);
        assert_eq!(app.dispatch(Request::new(Method::Get, "/missing", Vec::new())).status, 404);
    }
}
