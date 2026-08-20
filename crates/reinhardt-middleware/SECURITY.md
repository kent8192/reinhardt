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
  authenticated session or equivalent request context. Origin and referer
  checks use validated origins and do not accept cross-site state changes based
  on a token, path exemption, or header supplied by an attacker.
- Applications using credentialed CORS must configure explicit allowed origins,
  methods, and headers. `CorsConfig` does not reject `allow_origins = ["*"]`
  with credentials, so callers must not use that combination or treat its
  reflected origin as protected.
- Host validation, HTTPS redirects, origin guards, and remote-user identity use
  one validated request interpretation. Remote-user headers are accepted only
  from configured trusted immediate proxies, never merely because a forwarded
  header claims a trusted source.
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
  and preserves response integrity. Security headers, cookies, and other
  required response controls apply equally to success, error, redirect,
  short-circuit, cached, and compressed responses.

## Reportable Findings

Report a security-control ordering bypass, context-confused CSRF or session,
credentialed CORS overreach, spoofed proxy identity, auth-state timing error,
security-context cache leakage, spoofable throttling identity, compression
resource exhaustion, or missing security headers on alternate responses.
