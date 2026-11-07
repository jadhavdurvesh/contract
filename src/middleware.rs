//! CONTRACT middleware and request validation primitives.

use crate::{Request, Response};

pub trait Middleware: Send + Sync {
    /// Return Some(response) to short-circuit; None continues the pipeline.
    fn before(&self, request: &Request) -> Option<Response>;
    fn after(&self, _request: &Request, response: Response) -> Response { response }
}

pub struct Pipeline { layers: Vec<Box<dyn Middleware>> }
impl Pipeline {
    pub fn new() -> Self { Self { layers: Vec::new() } }
    pub fn push<M: Middleware + 'static>(&mut self, layer: M) { self.layers.push(Box::new(layer)); }

    pub fn execute<F>(&self, request: &Request, handler: F) -> Response
    where F: FnOnce(&Request) -> Response {
        let mut entered = 0usize;
        let mut short_circuit = None;
        for layer in &self.layers {
            entered += 1;
            if let Some(response) = layer.before(request) {
                short_circuit = Some(response);
                break;
            }
        }
        let mut response = short_circuit.unwrap_or_else(|| handler(request));
        for layer in self.layers[..entered].iter().rev() {
            response = layer.after(request, response);
        }
        response
    }
}
impl Default for Pipeline { fn default() -> Self { Self::new() } }

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_path_bytes: usize,
    pub max_body_bytes: usize,
    pub max_headers: usize,
}
impl Default for Limits {
    fn default() -> Self { Self { max_path_bytes: 2048, max_body_bytes: 1_048_576, max_headers: 64 } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationError { EmptyPath, PathTooLong, BodyTooLarge, TooManyHeaders }

pub fn validate_request(request: &Request, header_count: usize, limits: Limits) -> Result<(), ValidationError> {
    if request.path.is_empty() { return Err(ValidationError::EmptyPath); }
    if request.path.len() > limits.max_path_bytes { return Err(ValidationError::PathTooLong); }
    if request.body.len() > limits.max_body_bytes { return Err(ValidationError::BodyTooLarge); }
    if header_count > limits.max_headers { return Err(ValidationError::TooManyHeaders); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Method, Request};
    struct Marker;
    impl Middleware for Marker {
        fn before(&self, _: &Request) -> Option<Response> { None }
        fn after(&self, _: &Request, mut response: Response) -> Response {
            response.headers.insert("x-contract".into(), "processed".into()); response
        }
    }
    #[test]
    fn middleware_wraps_handler() {
        let mut p = Pipeline::new(); p.push(Marker);
        let req = Request::new(Method::Get, "/", Vec::new());
        let response = p.execute(&req, |_| Response::text(200, "ok"));
        assert_eq!(response.headers.get("x-contract").unwrap(), "processed");
    }
    struct Stop;
    impl Middleware for Stop {
        fn before(&self, _: &Request) -> Option<Response> { Some(Response::text(403, "blocked")) }
        fn after(&self, _: &Request, mut response: Response) -> Response {
            response.headers.insert("x-after-short-circuit".into(), "ran".into());
            response
        }
    }
    #[test]
    fn after_hooks_run_for_short_circuiting_layers() {
        let mut p = Pipeline::new(); p.push(Stop);
        let req = Request::new(Method::Get, "/", Vec::new());
        let response = p.execute(&req, |_| panic!("handler must not run"));
        assert_eq!(response.status, 403);
        assert_eq!(response.headers.get("x-after-short-circuit").map(String::as_str), Some("ran"));
    }
    #[test]
    fn request_limits_are_enforced() {
        let req = Request::new(Method::Post, "/x", vec![0; 5]);
        let limits = Limits { max_path_bytes: 10, max_body_bytes: 4, max_headers: 2 };
        assert_eq!(validate_request(&req, 1, limits), Err(ValidationError::BodyTooLarge));
    }
}
