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
- Applications rendering attacker-controlled URL-bearing attributes must
  validate their context and permitted scheme before rendering; escaping alone
  does not validate a URL scheme. SSR and hydration preserve equivalent
  escaping, URL handling, and DOM semantics.
- Browser authentication and authorization state is non-authoritative. Server
  functions and server routes authenticate and authorize every protected
  operation, with target and tenant context, independently of client checks.
- Cookie-authenticated server-function mutations must verify CSRF tokens in the
  generated server path or surrounding middleware. The generated client adds a
  token header, but direct requests to `ServerFnEndpoint` are not verified
  automatically. Serialization to the browser excludes secrets, credentials,
  private server state, and data unauthorized for the current user. `SsrState`
  is a client-visible serialization boundary rather than a secret store;
  callers must place only browser-safe state in it.
- `ServerFnEndpoint` logs server-function error bodies and returns those bodies
  to the client. `ServerFnError::Server` and `ServerFnError::Application`
  messages must therefore be sanitized before they cross this boundary; the
  endpoint does not redact credentials, private endpoints, or implementation
  details automatically.
- Static resources, route-derived paths, and server-rendered assets remain
  confined to configured roots; route rendering cannot expose arbitrary files.
  `TemplateStaticConfig` and the static resolver concatenate caller-supplied
  asset names after trimming leading slashes, but do not reject `.` or `..`
  path segments. Callers must validate asset names before resolving them.

## Reportable Findings

Report safe-API XSS or unsafe URL construction, SSR/hydration protection
differences, client-authoritative access, CSRF or server-route authorization
bypass, secret-bearing serialization, static-resource escape, or implicit use
of raw HTML or unsafe DOM APIs.
