# CONTRACT

**A research-oriented API application framework.**

CONTRACT is an independently implemented API framework focused on explicit request/response semantics, predictable routing, composable middleware, bounded resource use, and a small inspectable core.

## Design boundaries

- Own the application model, routing, middleware lifecycle, request validation hooks, and response construction.
- Keep the HTTP server adapter separate from application semantics.
- Keep data serialization delegated to OCTET through a future adapter; CONTRACT must not silently define a competing wire format.
- Keep network transport delegated to DATAGRAM where applicable; ordinary HTTP deployment may use a separately documented TCP/TLS adapter.
- Keep remote procedure semantics delegated to CALLGRAPH.

## Initial implementation

- Language: Rust.
- Initial scope: synchronous application core and HTTP/1.1 server adapter.
- Initial milestone: route registration, path matching, request/response model, bounded parsing, and deterministic tests.

## Status

Early development. Not production-ready. No security or performance guarantees are made until tested and documented.

## Development

```sh
cargo fmt --all
cargo test
```

## Related technologies

- [OCTET](https://github.com/jadhavdurvesh/octet) — data encoding and serialization.
- [DATAGRAM](https://github.com/jadhavdurvesh/datagram) — datagram networking.
- [CALLGRAPH](https://github.com/jadhavdurvesh/callgraph) — remote procedure execution.
