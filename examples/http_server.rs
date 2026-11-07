use contract::{http::{HttpConfig, HttpServer}, Application, Method, Response};
use std::{sync::{Arc, atomic::AtomicBool}, error::Error};

fn main() -> Result<(), Box<dyn Error>> {
    let mut app = Application::new();
    app.route(Method::Get, "/health", |_| Response::text(200, "ok"))?;
    app.route(Method::Get, "/robots/:id", |request| {
        Response::text(200, format!("robot={}", request.param("id").unwrap_or("unknown")))
    })?;

    let server = HttpServer::bind("127.0.0.1:8080", Arc::new(app), HttpConfig::default())?;
    eprintln!("CONTRACT listening on http://{}", server.local_addr()?);
    let shutdown = AtomicBool::new(false);
    server.serve(&shutdown)?;
    Ok(())
}
