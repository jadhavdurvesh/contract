//! Bounded HTTP/1.1 listener for CONTRACT.
//! Deliberately supports one request per connection and Content-Length bodies only.
//! Chunked transfer coding, upgrades, pipelining and keep-alive are rejected/closed.
use crate::{middleware::{validate_request, Limits}, Application, Method, Request, Response};
use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{Arc, atomic::{AtomicUsize, Ordering}},
    thread,
    time::Duration,
};

#[derive(Clone, Copy, Debug)]
pub struct HttpConfig {
    pub max_request_line_bytes: usize,
    pub max_header_bytes: usize,
    pub max_headers: usize,
    pub max_body_bytes: usize,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
    pub max_connections: usize,
}
impl Default for HttpConfig {
    fn default() -> Self {
        Self { max_request_line_bytes: 8_192, max_header_bytes: 32_768, max_headers: 64,
            max_body_bytes: 1_048_576, read_timeout: Duration::from_secs(5),
            write_timeout: Duration::from_secs(5), max_connections: 128 }
    }
}
#[derive(Debug)]
pub enum HttpError { Io(io::Error), InvalidConfig, MalformedRequest, RequestTooLarge, UnsupportedTransferEncoding }
impl From<io::Error> for HttpError { fn from(e: io::Error) -> Self { Self::Io(e) } }

pub struct HttpServer { listener: TcpListener, app: Arc<Application>, config: HttpConfig, active: Arc<AtomicUsize> }
impl HttpServer {
    pub fn bind(address: impl std::net::ToSocketAddrs, app: Arc<Application>, config: HttpConfig) -> Result<Self,HttpError> {
        if config.max_request_line_bytes == 0 || config.max_header_bytes < config.max_request_line_bytes
            || config.max_headers == 0 || config.max_body_bytes == 0 || config.read_timeout.is_zero()
            || config.write_timeout.is_zero() || config.max_connections == 0 { return Err(HttpError::InvalidConfig); }
        let listener = TcpListener::bind(address)?;
        listener.set_nonblocking(true)?;
        Ok(Self { listener, app, config, active: Arc::new(AtomicUsize::new(0)) })
    }
    pub fn local_addr(&self) -> Result<SocketAddr,HttpError> { Ok(self.listener.local_addr()?) }
    /// Runs until the supplied shutdown flag becomes true. Polling is bounded to 100 ms.
    pub fn serve(&self, shutdown: &std::sync::atomic::AtomicBool) -> Result<(),HttpError> {
        while !shutdown.load(Ordering::Acquire) {
            match self.listener.accept() {
                Ok((stream,_)) => {
                    let previous = self.active.fetch_add(1, Ordering::AcqRel);
                    if previous >= self.config.max_connections {
                        self.active.fetch_sub(1, Ordering::AcqRel);
                        let _ = write_response(stream, Response::text(503,"server busy"), self.config.write_timeout);
                        continue;
                    }
                    let app=Arc::clone(&self.app); let active=Arc::clone(&self.active); let cfg=self.config;
                    thread::spawn(move || {
                        let _guard=ActiveGuard(active);
                        let _=handle_connection(stream,&app,cfg);
                    });
                }
                Err(e) if e.kind()==io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(20)),
                Err(e) => return Err(HttpError::Io(e)),
            }
        }
        Ok(())
    }
}
struct ActiveGuard(Arc<AtomicUsize>);
impl Drop for ActiveGuard { fn drop(&mut self){ self.0.fetch_sub(1,Ordering::AcqRel); } }

fn handle_connection(mut stream:TcpStream, app:&Application, cfg:HttpConfig)->Result<(),HttpError>{
    stream.set_read_timeout(Some(cfg.read_timeout))?;
    stream.set_write_timeout(Some(cfg.write_timeout))?;
    let mut bytes=Vec::with_capacity(4096);
    let mut chunk=[0u8;1024];
    let header_end;
    loop {
        if bytes.len()>=cfg.max_header_bytes { return Err(HttpError::RequestTooLarge); }
        let n=stream.read(&mut chunk)?;
        if n==0 { return Err(HttpError::MalformedRequest); }
        bytes.extend_from_slice(&chunk[..n]);
        if let Some(pos)=find_header_end(&bytes) { header_end=pos; break; }
        if bytes.len()>cfg.max_header_bytes { return Err(HttpError::RequestTooLarge); }
    }
    let head=&bytes[..header_end];
    let text=std::str::from_utf8(head).map_err(|_|HttpError::MalformedRequest)?;
    let mut lines=text.split("\r\n");
    let request_line=lines.next().ok_or(HttpError::MalformedRequest)?;
    if request_line.len()>cfg.max_request_line_bytes { return Err(HttpError::RequestTooLarge); }
    let mut parts=request_line.split(' ');
    let method=parse_method(parts.next().ok_or(HttpError::MalformedRequest)?).ok_or(HttpError::MalformedRequest)?;
    let target=parts.next().ok_or(HttpError::MalformedRequest)?;
    let version=parts.next().ok_or(HttpError::MalformedRequest)?;
    if parts.next().is_some() || !(version=="HTTP/1.1"||version=="HTTP/1.0")
        || !target.starts_with('/') || target.contains('#') { return Err(HttpError::MalformedRequest); }
    let path=target.split('?').next().unwrap_or("/");
    let mut content_length=None; let mut count=0usize; let mut host_seen=false;
    for line in lines {
        if line.is_empty(){continue}
        count+=1; if count>cfg.max_headers{return Err(HttpError::RequestTooLarge)}
        let (name,value)=line.split_once(':').ok_or(HttpError::MalformedRequest)?;
        if name.is_empty() || name.bytes().any(|b| !b.is_ascii_alphanumeric() && b!=b'-') { return Err(HttpError::MalformedRequest); }
        let value=value.trim();
        if value.bytes().any(|b| b==0 || b==b'\r' || b==b'\n') { return Err(HttpError::MalformedRequest); }
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(HttpError::UnsupportedTransferEncoding);
        }
        if name.eq_ignore_ascii_case("content-length") {
            if content_length.is_some(){return Err(HttpError::MalformedRequest)}
            content_length=Some(value.parse::<usize>().map_err(|_|HttpError::MalformedRequest)?);
        }
        if name.eq_ignore_ascii_case("host"){host_seen=true;}
    }
    if version=="HTTP/1.1" && !host_seen { return Err(HttpError::MalformedRequest); }
    let body_len=content_length.unwrap_or(0);
    if body_len>cfg.max_body_bytes { return Err(HttpError::RequestTooLarge); }
    let body_start=header_end+4;
    while bytes.len()-body_start<body_len {
        let n=stream.read(&mut chunk)?;
        if n==0 {return Err(HttpError::MalformedRequest)}
        bytes.extend_from_slice(&chunk[..n]);
        if bytes.len()-body_start>body_len { return Err(HttpError::MalformedRequest); }
    }
    let body=bytes[body_start..body_start+body_len].to_vec();
    let request=Request::new(method,path.to_owned(),body);
    if validate_request(&request,count,Limits{max_path_bytes:cfg.max_request_line_bytes,max_body_bytes:cfg.max_body_bytes,max_headers:cfg.max_headers}).is_err(){
        return write_response(stream,Response::text(400,"invalid request"),cfg.write_timeout).map_err(HttpError::Io);
    }
    let response=app.dispatch(request);
    write_response(stream,response,cfg.write_timeout).map_err(HttpError::Io)
}
fn find_header_end(b:&[u8])->Option<usize>{b.windows(4).position(|w|w==b"\r\n\r\n")}
fn parse_method(s:&str)->Option<Method>{match s{"GET"=>Some(Method::Get),"POST"=>Some(Method::Post),"PUT"=>Some(Method::Put),"PATCH"=>Some(Method::Patch),"DELETE"=>Some(Method::Delete),_=>None}}
fn write_response(mut stream:TcpStream,mut response:Response,timeout:Duration)->io::Result<()> {
    stream.set_write_timeout(Some(timeout))?;
    if !(100..=599).contains(&response.status){response.status=500;response.body=b"invalid response status".to_vec();}
    response.headers.retain(|k,v| !k.eq_ignore_ascii_case("content-length")&&!k.eq_ignore_ascii_case("connection")&&!k.eq_ignore_ascii_case("transfer-encoding")&&!k.bytes().any(|b|b<=32||b==127)&&!v.contains(['\r','\n']));
    response.headers.insert("content-length".into(),response.body.len().to_string());
    response.headers.insert("connection".into(),"close".into());
    let reason=match response.status{200=>"OK",201=>"Created",204=>"No Content",400=>"Bad Request",404=>"Not Found",405=>"Method Not Allowed",413=>"Payload Too Large",500=>"Internal Server Error",502=>"Bad Gateway",503=>"Service Unavailable",_=>"Response"};
    write!(stream,"HTTP/1.1 {} {}\r\n",response.status,reason)?;
    for(k,v)in response.headers{write!(stream,"{}: {}\r\n",k,v)?;}
    stream.write_all(b"\r\n")?;stream.write_all(&response.body)?;stream.flush()
}

#[cfg(test)]
mod tests {
 use super::*;
 #[test] fn parses_supported_methods(){assert!(parse_method("GET").is_some());assert!(parse_method("TRACE").is_none());}
 #[test] fn finds_header_terminator(){assert_eq!(find_header_end(b"GET / HTTP/1.1\r\n\r\n"),Some(14));}
 #[test] fn server_dispatches_http_request(){
  let mut app=Application::default();
  app.route(Method::Get,"/health",|_|Response::text(200,"ok"));
  let app=Arc::new(app);let server=HttpServer::bind("127.0.0.1:0",app,HttpConfig::default()).unwrap();
  let addr=server.local_addr().unwrap();
  let worker=thread::spawn(move||{let mut stream=TcpStream::connect(addr).unwrap();stream.write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();let mut out=String::new();stream.read_to_string(&mut out).unwrap();out});
  let (stream,_)=server.listener.accept().unwrap();handle_connection(stream,&server.app,server.config).unwrap();
  assert!(worker.join().unwrap().contains("200 OK"));
 }
}
