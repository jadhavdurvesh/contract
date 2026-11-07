use contract::{rpc::{RpcClient,RpcConfig},Application,Method,Request,Response};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Start the CALLGRAPH UDP server separately and register the same operation ID.
    const OPERATION_ID: u32 = 0x4A31_1001;
    let rpc = RpcClient::new("127.0.0.1:9000".parse()?, RpcConfig::default())?;
    let mut api = Application::new();
    api.route(Method::Post, "/v1/robots/:id/command", move |request| {
        match rpc.call(OPERATION_ID, 1, 0, &request.body) {
            Ok(body) => Response::new(200, body),
            Err(_) => Response::text(502, "upstream RPC failure"),
        }
    });

    // This demonstrates CONTRACT dispatch. Attach an HTTP listener adapter to
    // translate real HTTP requests into Request and serialize Response.
    let request = Request::new(Method::Post, "/v1/robots/FB-01/command", b"status".to_vec());
    let response = api.dispatch(request);
    println!("API dispatch status: {}", response.status);
    Ok(())
}
