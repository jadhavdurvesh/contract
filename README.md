# CONTRACT

CONTRACT is a transport-independent API application core with a bounded HTTP/1.1 listener and a CALLGRAPH RPC client.

## HTTP server

`HttpServer` supports one request per TCP connection, HTTP/1.0 and HTTP/1.1, the common API methods, bounded request/header/body sizes, read/write timeouts, and a bounded active-connection count.

The first listener deliberately rejects transfer-encoding and does not support chunked bodies, keep-alive, upgrades, or pipelining. Place TLS termination at a trusted reverse proxy until a native TLS listener is implemented.

## CALLGRAPH RPC

The RPC client uses:

`CALLGRAPH frame → OCTET frame → DATAGRAM envelope → UDP/IP`

It supports bounded retransmission and optional AES-256-GCM protection. Supply a 32-byte `pre_shared_key` in `RpcConfig` and configure the Go CALLGRAPH server with the same key. Keep keys in a secret manager; never commit them.

## Example

See [examples/rpc_gateway.rs](examples/rpc_gateway.rs) and the real Rust-to-Go encrypted interoperability test in [tests/callgraph_interop.rs](tests/callgraph_interop.rs).

## Safety and limitations

The HTTP listener is an initial bounded implementation, not a complete hardened internet-facing server. Use a reverse proxy for TLS, request filtering and rate limits. The UDP PSK mode has no key exchange or forward secrecy. The transport is unsuitable for hard real-time actuator control.
