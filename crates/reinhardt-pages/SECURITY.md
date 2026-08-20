# reinhardt-pages Security Policy

## System and Scope

This policy inherits the repository [Security Policy](../../SECURITY.md) and
the framework-crate [Security Policy](../SECURITY.md). Policies compose from
the repository root to this crate; this closest policy wins on conflict.

`reinhardt-pages` renders SSR and hydrated WASM pages, server functions,
browser routes, serialized state, and static resources. Browser input, DOM
state, route parameters, serialized values, and hydration data are untrusted.

## Security Invariants

- Text and attribute output is escaped by default. Raw HTML and unsafe
  portal/DOM interfaces remain explicit trust boundaries; safe APIs must not
  reach them with attacker-controlled content.
- URL-bearing attributes validate their context and permitted scheme before
  rendering. SSR and hydration preserve equivalent escaping, URL handling, and
  DOM semantics.
- Browser authentication and authorization state is non-authoritative. Server
  functions and server routes authenticate and authorize every protected
  operation, with target and tenant context, independently of client checks.
- Cookie-authenticated server-function mutations preserve CSRF protections.
  Serialization to the browser excludes secrets, credentials, private server
  state, and data unauthorized for the current user.
- Static resources, route-derived paths, and server-rendered assets remain
  confined to configured roots; route rendering cannot expose arbitrary files.

## Reportable Findings

Report safe-API XSS or unsafe URL construction, SSR/hydration protection
differences, client-authoritative access, CSRF or server-route authorization
bypass, secret-bearing serialization, static-resource escape, or implicit use
of raw HTML or unsafe DOM APIs.
