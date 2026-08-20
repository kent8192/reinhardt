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
  only when the immediate peer is a configured trusted proxy. Parsing rejects
  ambiguous, malformed, duplicate, or otherwise conflicting Host and forwarded
  values rather than selecting an attacker-preferred interpretation.
- Request metadata, including method, URI, query, headers, cookies, remote
  address, body, path parameters, and extensions, is untrusted input. It must
  not establish identity, authorization, tenancy, origin, or routing safety
  without the component that owns that decision validating it.
- Upload filenames and destinations are confined to configured storage roots.
  Decoding, normalization, traversal checks, symlink handling, and generated
  storage names cannot permit writes or reads outside the authorized root.
- Applications exposing chunked or resumable uploads must bind upload IDs,
  chunks, completion, and cleanup to the creating principal, tenant, and
  configured storage scope. `ChunkedUploadManager` accepts a caller-supplied
  session ID and does not enforce this ownership context automatically.
- Request bodies, multipart parts, upload sizes, field counts, and buffering
  work have configured limits enforced before unbounded allocation, decoding,
  decompression, or disk consumption.
- Header, cookie, and redirect construction rejects control characters, line
  breaks, and invalid names or values so attacker-controlled data cannot inject
  response headers, cookie attributes, or response splitting.
- Request extensions are isolated to one request. Security-sensitive extension
  values are populated only by their owning validated middleware and are not a
  substitute for credential verification or authorization.
- Errors exposed through HTTP responses are safe for the client and do not
  disclose credentials, filesystem paths, proxy topology, internal headers,
  parser details, or upload state belonging to another caller.

## Reportable Findings

Report trusted-proxy or Host confusion, request-metadata trust, upload
confinement or ownership escape, pre-limit resource exhaustion, header or
cookie injection, cross-request extension leakage, or sensitive HTTP error
content. Explicit application-defined raw response bodies remain in scope when
this crate's safe API turns attacker-controlled data into a privileged output.
