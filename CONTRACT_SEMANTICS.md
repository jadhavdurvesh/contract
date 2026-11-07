# CONTRACT route and middleware semantics — draft 0.2

## Route matching
- Literal path segments have precedence over named segments.
- A named segment is written `:name`; names must be non-empty and unique within a route.
- Route registration must reject duplicate method/pattern pairs.
- A path matching at least one route but not its method returns 405; otherwise 404.
- Matching is segment-based. Percent-decoding belongs to the HTTP adapter and must occur exactly once before dispatch.

## Middleware
- Request middleware runs in registration order.
- A middleware may short-circuit with a response.
- Response middleware runs in reverse registration order.
- Middleware must be Send + Sync and must not retain request references beyond the call.

## Request limits
The application core provides configurable maximum path bytes, body bytes and header count validation. The HTTP adapter must enforce limits while reading, before unbounded allocation.

## Validation
Validation is deterministic and must return explicit error categories. Application-specific schema validation is an extension point; CONTRACT does not prescribe a schema language in this draft.
