# Security Policy

## System and Scope

Reinhardt is a Rust web framework and workspace of framework crates, macros,
runtime services, generated code, and documented production integrations. This
policy defines the repository-wide security contract inherited by nested
components. Security policies compose from the repository root to each nested
component; the closest policy wins on conflict. A downstream application
remains responsible for its deployment, configuration, content, credentials,
and use of explicitly unsafe interfaces.

## Protected Assets

- Application identities, sessions, credentials, tokens, cookies, and secrets.
- Tenant-scoped data, authorization decisions, and database integrity.
- Filesystem and object-storage data outside an application's configured scope.
- Browser-originated data, rendered content, and same-origin privileges.
- Availability and predictable resource use of production services.
- Plugin host integrity and the capabilities granted to each plugin.

## Threat Model and Trust Boundaries

Treat HTTP fields, tokens, cookies, uploads, filters, routes, GraphQL, gRPC,
and WebSocket inputs, server-function arguments, attacker-writable database
fields, and loadable plugin code as attacker-controlled. Public framework APIs
can therefore receive untrusted values even when an application does not expose
an HTTP endpoint directly.

Application configuration, explicit raw interfaces, and developer tooling are
trust boundaries rather than automatically safe inputs. Proxy-provided identity
is trusted only after the immediate peer has been verified as a configured
proxy; forwarded headers from any other peer are attacker-controlled.

## Global Security Invariants

- Authentication establishes identity only with validated credentials, and
  authorization is enforced for every protected action using the relevant
  resource and tenant context.
- Request handling preserves isolation: one request, tenant, or principal must
  not read, modify, or act as another through shared state or identifier
  selection.
- Safe database APIs construct SQL with parameter binding and validated query
  structure; attacker-controlled values must not become executable SQL.
- Filesystem and storage APIs confine operations to configured roots, buckets,
  prefixes, and permissions, preventing traversal, symlink escape, and
  cross-tenant object access.
- Parsers and decoders bound untrusted input size, nesting, work, and resource
  consumption before processing it.
- Rate-limit state is bounded only when callers constrain key cardinality and
  run cleanup. The default per-route strategy creates a bucket and request
  history entry for each distinct URI path; applications exposing attacker-
  controlled paths must use bounded route keys and periodic eviction.
- Browser-facing APIs preserve contextual output encoding, origin protections,
  and safe cookie and redirect handling so untrusted content cannot gain
  browser privileges. APIs such as `HtmlHighlighter` that return raw HTML with
  inserted `<mark>` tags are explicit unsafe boundaries; callers must escape
  source text before rendering or restrict input to trusted text.
- Errors, logs, diagnostics, and telemetry redact secrets and credentials.
- `ResponseCookies` debug output contains complete raw `Set-Cookie` strings,
  which may include session credentials. Callers must not format or log
  response-cookie containers across a secret-bearing diagnostic boundary
  without redaction.
- WebSocket configuration is also a secret boundary: Redis URLs, passwords,
  tokens, and connection options must be redacted before errors, logs,
  diagnostics, telemetry, or client-visible responses are produced.
- HTTP, GraphQL, gRPC, WebSocket, and server-function transports enforce
  equivalent authentication, authorization, validation, and isolation for the
  same operation.
- Plugin code receives only its explicitly granted capabilities and cannot
  obtain host, filesystem, network, secret, or other plugin privileges by
  default.

## Reportable Findings

Report a finding when a realistic downstream application can violate a global
security invariant without intentionally entering an explicit unsafe boundary.
Severity is assessed from reachability, prerequisites, and impact, not from the
vulnerability class alone.

Examples include authentication or authorization bypass, cross-request or
cross-tenant access, injection through safe APIs, confinement escape,
credential exposure, browser privilege compromise, plugin capability escape,
and remotely triggerable resource exhaustion.

## Out of Scope

The following are out of scope unless they cross a production boundary:

- Test-only code and fixtures.
- Explicit raw SQL, raw HTML, arbitrary-code, and equivalent APIs used with
  their documented trust assumptions.
- Trusted developer tooling and local development environments.
- Dependency advisories that are unreachable in the relevant production
  configuration.

## Known Limitations and Audit Status

No comprehensive independent product security audit has occurred. The
[0.3.0 dependency-advisory review](docs/security-audit-0.3.0.md) records a
separate dependency-advisory review; it is not a comprehensive product audit.

## Supported Releases

Reinhardt follows the lifecycle in
[`instructions/STABILITY_POLICY.md`](instructions/STABILITY_POLICY.md).
<!-- reinhardt-version-sync -->
Security fixes target the current supported release, `0.4.0-alpha.8`, and the current
development line as appropriate.

## Reporting a Vulnerability

Report vulnerabilities privately through [GitHub Security
Advisories](https://github.com/kent8192/reinhardt-web/security/advisories), the
preferred reporting channel. Do not create a public issue or disclose details
publicly before a fix is available.

Include affected versions, prerequisites, a minimal reproduction, impact, and
any proposed mitigation.
