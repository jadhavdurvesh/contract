//! CONTRACT: transport-independent API application core.
pub mod middleware;
pub mod router;
pub mod rpc;
use std::collections::BTreeMap;
#[derive(Clone,Debug,PartialEq,Eq)]pub enum Method{Get,Post,Put,Patch,Delete}
#[derive(Clone,Debug,PartialEq,Eq)]pub struct Request{pub method:Method,pub path:String,pub body:Vec<u8>,pub path_params:BTreeMap<String,String>}
impl Request{pub fn new(method:Method,path:impl Into<String>,body:impl Into<Vec<u8>>)->Self{Self{method,path:path.into(),body:body.into(),path_params:BTreeMap::new()}}pub fn param(&self,n:&str)->Option<&str>{self.path_params.get(n).map(String::as_str)}}
#[derive(Clone,Debug,PartialEq,Eq)]pub struct Response{pub status:u16,pub headers:BTreeMap<String,String>,pub body:Vec<u8>}
impl Response{pub fn new(status:u16,body:impl Into<Vec<u8>>)->Self{Self{status,headers:BTreeMap::new(),body:body.into()}}pub fn text(status:u16,body:impl Into<String>)->Self{let mut r=Self::new(status,body.into().into_bytes());r.headers.insert("content-type".into(),"text/plain; charset=utf-8".into());r}}
type Handler=Box<dyn Fn(&Request)->Response+Send+Sync+'static>;struct Route{method:Method,pattern:String,handler:Handler}
#[derive(Default)]pub struct Application{routes:Vec<Route>}
impl Application{pub fn new()->Self{Self::default()}pub fn route<F>(&mut self,m:Method,p:impl Into<String>,h:F)where F:Fn(&Request)->Response+Send+Sync+'static{self.routes.push(Route{method:m,pattern:p.into(),handler:Box::new(h)});}pub fn dispatch(&self,req:Request)->Response{let mut path_match=false;for r in &self.routes{if let Some(params)=match_path(&r.pattern,&req.path){path_match=true;if r.method==req.method{let mut q=req;q.path_params=params;return(r.handler)(&q)}}}if path_match{Response::text(405,"method not allowed")}else{Response::text(404,"not found")}}}
fn match_path(p:&str,path:&str)->Option<BTreeMap<String,String>>{let expected:Vec<_>=p.trim_matches('/').split('/').collect();let actual:Vec<_>=path.trim_matches('/').split('/').collect();if expected.len()!=actual.len(){return None}let mut out=BTreeMap::new();for(e,a)in expected.iter().zip(actual.iter()){if let Some(n)=e.strip_prefix(':'){if n.is_empty(){return None}out.insert(n.to_owned(),(*a).to_owned());}else if e!=a{return None}}Some(out)}
#[cfg(test)]mod tests{use super::*;#[test]fn dispatches_params(){let mut a=Application::new();a.route(Method::Get,"/robots/:id",|r|Response::text(200,r.param("id").unwrap()));let x=a.dispatch(Request::new(Method::Get,"/robots/FB-01",Vec::new()));assert_eq!(x.body,b"FB-01");}#[test]fn distinguishes_404_405(){let mut a=Application::new();a.route(Method::Get,"/health",|_|Response::text(200,"ok"));assert_eq!(a.dispatch(Request::new(Method::Post,"/health",Vec::new())).status,405);assert_eq!(a.dispatch(Request::new(Method::Get,"/missing",Vec::new())).status,404);}}
