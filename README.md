# CONTRACT

A transport-independent API application framework and request-dispatch core.

## CALLGRAPH integration

CONTRACT includes a synchronous UDP RPC client in `contract::rpc`. It sends the shared wire stack:

`CALLGRAPH frame → OCTET frame → DATAGRAM envelope → UDP/IP`

A CONTRACT route can capture an `RpcClient` and forward an API request body to a registered CALLGRAPH operation. See [examples/rpc_gateway.rs](examples/rpc_gateway.rs).

```rust
let rpc = RpcClient::new("127.0.0.1:9000".parse()?, RpcConfig::default())?;
app.route(Method::Post, "/v1/robots/:id/command", move |request| {
    match rpc.call(OPERATION_ID, 1, 0, &request.body) {
        Ok(body) => Response::new(200, body),
        Err(_) => Response::text(502, "upstream RPC failure"),
    }
});
```

The example demonstrates application dispatch, not a complete HTTP listener. A dedicated HTTP adapter remains a separate component and must enforce request-line, header, body, connection, and timeout limits before constructing a CONTRACT request.

## Scope

- Method and path dispatch with named path parameters.
- Middleware pipeline and request validation.
- Bounded synchronous CALLGRAPH RPC client.
- No dependency on a third-party web framework.

## Safety and limitations

The UDP RPC client is a prototype transport adapter. Correlation/session values are not authentication credentials. The current wire stack does not provide authenticated encryption, reliable delivery, congestion control, or production-grade peer identity. Do not use it for emergency-stop or hard real-time actuator paths.

## Related repositories

- [OCTET](https://github.com/jadhavdurvesh/octet) — binary serialization profile.
- [DATAGRAM](https://github.com/jadhavdurvesh/datagram) — UDP transport primitives.
- [CALLGRAPH](https://github.com/jadhavdurvesh/callgraph) — remote operation runtime.
