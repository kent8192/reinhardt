# reinhardt-http Security Policy

## System and Scope

This policy inherits the repository [Security Policy](../../SECURITY.md) and
the framework-crate [Security Policy](../SECURITY.md). Policies compose from
the repository root to this crate; this closest policy wins on conflict.

`reinhardt-http` provides request and response primitives, request metadata,
headers, cookies, uploads, chunked uploads, middleware execution, and typed
request extensions. Request lines, bodies, headers, cookies, proxy metadata,
paths, filenames, upload identifiers, and extension values supplied by callers
are attacker-controlled until their relevant boundary validates them.

## Security Invariants

- Proxy-derived scheme, address, host, and `Forwarded` metadata are trusted
  only when the immediate peer is a configured trusted proxy. Protected
  applications must configure that proxy to replace and sanitize forwarding
  headers; request helpers do not reject every duplicate or conflicting value
  before selecting one.
- Request metadata, including method, URI, query, headers, cookies, remote
  address, body, path parameters, and extensions, is untrusted input. It must
  not establish identity, authorization, tenancy, origin, or routing safety
  without the component that owns that decision validating it.
- Upload filenames and destinations must be confined to configured storage
  roots by the owning application or storage layer. Callers must perform
  decoding, normalization, traversal checks, symlink handling, and generated
  storage-name validation before passing a destination to a file operation.
- Applications exposing chunked or resumable uploads must bind upload IDs,
  chunks, completion, and cleanup to the creating principal, tenant, and
  configured storage scope. `ChunkedUploadManager` accepts a caller-supplied
  session ID and does not enforce this ownership context automatically.
- Protected applications must install configured limits for request bodies,
  multipart parts, upload sizes, field counts, and buffering work before
  allocation, decoding, decompression, or disk consumption. The HTTP
  primitives and `ChunkedUploadManager` do not enforce every cumulative limit
  automatically.
- Header, cookie, and redirect construction rejects control characters, line
  breaks, and invalid names or values so attacker-controlled data cannot inject
  response headers, cookie attributes, or response splitting.
- `ResponseCookies::add` and `SharedResponseCookies::add` accept a complete raw
  `Set-Cookie` string and do not validate cookie attributes or delimiters.
  Callers must use a structured serializer or otherwise validate every cookie
  name, value, and attribute before adding it.
- Request extensions are isolated to one request. Security-sensitive extension
  values are populated only by their owning validated middleware and are not a
  substitute for credential verification or authorization.
- Protected applications must map errors exposed through HTTP responses to
  safe client details. Some serialization failures preserve parser or custom
  deserializer text, so callers must retain the original detail only in
  server-side logs and avoid returning it to clients.

## Reportable Findings

Report trusted-proxy or Host confusion, request-metadata trust, upload
confinement or ownership escape, pre-limit resource exhaustion, header or
cookie injection, cross-request extension leakage, or sensitive HTTP error
content. Explicit application-defined raw response bodies remain in scope when
this crate's safe API turns attacker-controlled data into a privileged output.
