use std::{
    net::SocketAddr,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const OPERATION_ID: u32 = 0x4A31_1001;

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn rust_contract_client_interoperates_with_go_callgraph_server() {
    // Ordinary local cargo test remains self-contained. CI sets this variable
    // after checking out and compiling the CALLGRAPH interoperability fixture.
    let Some(binary) = std::env::var_os("CALLGRAPH_INTEROP_SERVER") else {
        eprintln!("skipping cross-language test: CALLGRAPH_INTEROP_SERVER is not set");
        return;
    };

    let address: SocketAddr = "127.0.0.1:19000".parse().unwrap();
    let child = Command::new(binary)
        .env("CALLGRAPH_LISTEN", address.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("start Go CALLGRAPH interoperability server");
    let _server = Server(child);

    let client = contract::rpc::RpcClient::new(
        address,
        contract::rpc::RpcConfig {
            timeout: Duration::from_millis(700),
            ..Default::default()
        },
    )
    .expect("valid RPC config");

    let started = Instant::now();
    loop {
        match client.call(OPERATION_ID, 1, 0, b"cross-language") {
            Ok(response) => {
                assert_eq!(response, b"go:cross-language");
                return;
            }
            Err(error) if started.elapsed() > Duration::from_secs(8) => {
                panic!("Go CALLGRAPH server did not interoperate: {error}");
            }
            Err(_) => {}
        }
        if let Ok(status) = _server.0.try_wait() {
            panic!("Go CALLGRAPH server exited early: {status:?}");
        }
        thread::sleep(Duration::from_millis(100));
    }
}
