# reinhardt-middleware Security Policy

## System and Scope

This policy inherits the repository [Security Policy](../../SECURITY.md) and
the framework-crate [Security Policy](../SECURITY.md). Policies compose from
the repository root to this crate; this closest policy wins on conflict.

`reinhardt-middleware` composes authentication, session, CSRF, origin, host,
CORS, remote-user, rate-limit, compression, cache, redirect, and response
header controls. Requests, proxy headers, cookies, origins, credentials,
request IDs, cache selectors, and compression inputs are attacker-controlled
unless their owning control validates them.

## Security Invariants

- Middleware ordering preserves security dependencies: trusted proxy and host
  interpretation precede controls that use them; authentication and session
  population precede authorization; and CSRF, origin, and authorization checks
  run before state-changing handlers. Reordering or short-circuiting cannot
  create a permissive path.
- CSRF tokens are unpredictable, bounded in lifetime, and bound to the
  authenticated session or equivalent request context. The middleware's
  timeless HMAC format and configuration flags do not enforce token age by
  themselves; protected deployments must wire timestamp validation or another
  expiry mechanism. Deployments that disable referer checks or allow
  cross-site cookie delivery must require a separately submitted token from a
  header or request field and use the cookie only as the independent expected
  value; `extract_token` can otherwise fall back to the automatically attached
  CSRF cookie. `CsrfMiddleware` derives its HMAC input from the first String
  extension it interprets as a session identifier; an authentication layer
  that stores a predictable bare user ID there makes the token predictable.
  Protected deployments must provide a typed, unpredictable per-session value
  or treat this token only as defense in depth. Origin and referer checks use
  validated origins and do not accept cross-site state changes based on a
  token or header supplied by an attacker; exempt paths bypass those checks as
  documented below.
- CSRF exemptions for an exact path or its segment-delimited subtree return
  before origin, referer, and token validation. Such exemptions are complete
  CSRF-boundary bypasses and require separate authentication and origin or
  equivalent request-integrity controls for every exempt handler.
- Applications using credentialed CORS must configure explicit allowed origins,
  methods, and headers. `CorsConfig` does not reject `allow_origins = ["*"]`
  with credentials, so callers must not use that combination or treat its
  reflected origin as protected.
- Host validation, HTTPS redirects, origin guards, and remote-user identity use
  one validated request interpretation. Remote-user headers are accepted only
  from configured trusted immediate proxies, never merely because a forwarded
  header claims a trusted source.
- `LoginRequiredMiddleware` exempts the configured login URL using a prefix
  check when the URL has no trailing slash. Applications must configure a
  segment-delimited login URL, normally with a trailing slash, or otherwise
  ensure that sibling paths such as `/login-admin` cannot be treated as the
  login endpoint.
- Sessions, cookies, and session stores isolate principals, tenants, and
  requests. A session identifier or cached session state cannot be confused,
  reused, or exposed across callers, and failure paths do not retain a prior
  caller's authentication state.
- Authentication middleware establishes the authoritative request auth state
  before any consumer reads it. Absent, malformed, or failed authentication is
  denial or an explicit anonymous state, not stale, spoofed, or permissive
  authenticated state.
- Caches that can affect authorization or responses must key and invalidate by
  the complete security context, including principal, tenant, authorization
  scope, relevant credentials, and response variation. `CacheMiddleware` does
  not infer that context; applications must skip private responses by default
  or provide a principal/tenant-aware strategy instead of using `UrlOnly` for
  authenticated endpoints.
- Rate-limit identities derive from authenticated principals or validated
  network identity. Client-controlled headers, request IDs, and arbitrary
  forwarded addresses cannot select another caller's bucket or evade limits.
- Compression bounds input and output resources, rejects compression abuse,
  and preserves response integrity. `GZipMiddleware` and `BrotliMiddleware`
  buffer and compress the complete response; applications must bound response
  bodies or provide separate CPU, output-size, and timeout controls because
  these middleware do not enforce those limits automatically. Security headers,
  cookies, and other required response controls apply equally to success,
  error, redirect, short-circuit, cached, and compressed responses.

## Reportable Findings

Report a security-control ordering bypass, context-confused CSRF or session,
credentialed CORS overreach, spoofed proxy identity, auth-state timing error,
security-context cache leakage, spoofable throttling identity, compression
resource exhaustion, or missing security headers on alternate responses.
