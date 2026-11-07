# CONTRACT architecture

## Responsibility
CONTRACT owns application-level API semantics: route registration, request/response models, middleware and validation lifecycle (planned), and transport adapters. It does not own a binary wire format, datagram transport, or remote procedure semantics.

## Initial core
- Transport-independent `Application`, `Request`, and `Response` types.
- Deterministic route matching with named path parameters.
- Explicit 404 versus 405 behavior.
- Handler isolation through a small synchronous interface.

## Planned layers
1. Application core and routing.
2. Middleware and request lifecycle.
3. Schema/validation interfaces.
4. HTTP adapter and server lifecycle.
5. Observability, limits, and security hooks.

## Boundaries
OCTET may provide an optional body codec. DATAGRAM may provide a transport adapter for non-HTTP APIs. CALLGRAPH remains responsible for remote invocation semantics. These are optional adapters, not mandatory runtime dependencies.
