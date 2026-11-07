# CONTRACT roadmap

- [x] Transport-independent application core.
- [x] Static routes and named path parameters.
- [x] Distinguish 404 and 405.
- [x] Separate router with literal-route precedence and duplicate-route validation.
- [x] Middleware lifecycle primitive with short-circuit and reverse after-hooks.
- [x] Request path/body/header-count validation primitives.
- [x] Bounded HTTP/1.1 listener: one request per connection, Content-Length only.
- [x] CALLGRAPH UDP client using OCTET and DATAGRAM wire profiles.
- [x] Bounded retransmission and optional AES-256-GCM PSK interoperability with Go CALLGRAPH.
- [ ] Integrate Router and Middleware pipeline into the main Application dispatch path.
- [ ] Query/header abstractions and richer response/header types.
- [ ] HTTP parser conformance suite, fuzzing and malformed-request response mapping.
- [ ] Native TLS listener or documented TLS proxy integration.
- [ ] Structured errors, tracing, metrics and load benchmarks.

The HTTP listener is intentionally limited and should sit behind a hardened TLS reverse proxy for network-facing deployments.
