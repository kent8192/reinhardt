# reinhardt-di Security Policy

## System and Scope

This policy inherits the repository [Security Policy](../../SECURITY.md) and
the framework-crate [Security Policy](../SECURITY.md). Policies compose from
the repository root to this crate; this closest policy wins on conflict.

`reinhardt-di` resolves dependency graphs and caches request- and
application-scoped services. Injection contexts, providers, overrides, cache
keys, request extractors, and provider failures cross identity and request
isolation boundaries.

## Security Invariants

- Each request receives an isolated request scope. Request-scoped dependencies,
  values, cleanup state, and failures cannot be reused by another request,
  tenant, connection, or principal.
- User, authorization, session, credential, request, tenant, and other
  security-sensitive state is never globally cached. Singleton dependencies may
  hold only application-wide state that cannot identify or authorize a caller.
- Cache keys preserve both dependency type and scope identity. Distinct keyed
  providers, generic types, request contexts, and scopes cannot collide or
  receive a value created for another key.
- The immutable application root and the current request context remain
  distinct; request-derived values cannot mutate or replace root registrations
  or singleton state.
- `InjectionContext::clone` copies cached request-scoped entries and request
  context. It is suitable only for work within the same request scope; callers
  starting a new request must use `fork` or `fork_for_request` so cached
  values and request state cannot cross the request boundary.
- Authentication and authorization extractors fail closed when credentials,
  request context, middleware state, or dependencies are absent, malformed, or
  unresolved.
- Test overrides must be opt-in, scoped to test or explicitly configured
  development contexts, cleaned up reliably, and unavailable as an accidental
  production authorization bypass. The override APIs are not feature-gated;
  production applications must not expose or invoke them as runtime controls.
- Recursive provider graphs detect cycles and enforce a bounded resolution
  depth or work limit so untrusted dependency selection cannot exhaust stack,
  memory, or CPU.
- Network-facing dependency errors are sanitized: responses do not reveal
  implementation types, provider graphs, credentials, internal endpoints, or
  other sensitive state.

## Reportable Findings

Report cross-request or cross-principal cache leakage, cache-key collisions,
root-context replacement, fail-open auth extraction, production-reachable test
overrides, unbounded dependency resolution, or sensitive network error output.
