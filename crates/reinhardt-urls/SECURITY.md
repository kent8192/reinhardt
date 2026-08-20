# reinhardt-urls Security Policy

## System and Scope

This policy inherits the repository [Security Policy](../../SECURITY.md) and
the framework-crate [Security Policy](../SECURITY.md). Policies compose from
the repository root to this crate; this closest policy wins on conflict.

`reinhardt-urls` matches server and client routes, extracts path parameters,
builds route groups, applies routing middleware, reverses URLs, and produces
redirect destinations. Paths, percent-encoded segments, query strings, route
parameters, route registrations, and client routing state cross its boundary.

## Security Invariants

- Server matching, client matching, reverse routing, and redirects use
  security-equivalent path normalization and percent decoding. Invalid or
  ambiguous encodings fail rather than receiving a different interpretation at
  another stage or being decoded more than once.
- Slash normalization and dot-segment handling cannot turn a protected path
  into a different route, escape a mounted prefix, or bypass authorization and
  middleware attached to the intended route.
- Route registration and dispatch preserve deterministic, non-bypassable
  precedence. Conflicting routes fail explicitly, and fallback, wildcard, or
  less-specific routes cannot shadow a security-sensitive route after
  normalization.
- Global, router, group, and route middleware compose cumulatively in their
  configured order. Adding a route or group cannot discard inherited
  authentication, authorization, CSRF, origin, host, or other security
  middleware.
- Captured path and query parameters are attacker-controlled. Typed conversion
  and validation do not establish object ownership, tenant membership, or
  authorization; handlers enforce those checks on the resolved resource.
- Reverse routing encodes parameter values as path data and cannot generate
  traversal, delimiter, scheme, or authority injection. Redirect destinations
  are local paths or validated allowlisted destinations and are never accepted
  from an unvalidated route parameter or request value.
- Client-router state, browser history, route metadata, and client-side guards
  are presentation state only. Server-side routing and handlers retain the
  authoritative authentication, authorization, and tenant checks.

## Reportable Findings

Report normalization or decoding differentials, route-precedence bypasses,
lost inherited middleware, parameter-driven authorization bypass, unsafe URL
generation or redirects, and client metadata treated as authoritative.
