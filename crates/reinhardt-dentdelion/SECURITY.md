# reinhardt-dentdelion Security Policy

## System and Scope

This policy inherits the repository [Security Policy](../../SECURITY.md) and
the framework-crate [Security Policy](../SECURITY.md). Policies compose from
the repository root to this crate; this closest policy wins on conflict.

`reinhardt-dentdelion` installs, loads, registers, and runs static and dynamic
plugins. Installed manifests, WASM modules, JavaScript/TypeScript code, plugin
metadata, registry data, host-call arguments, and plugin events may be
malicious. A plugin's requested capabilities do not establish trust.

## Security Invariants

- Every plugin privilege is an explicit, identity-bound capability grant. A
  missing, malformed, disabled, or unrecognized capability lookup fails closed;
  manifests, metadata, and another plugin's registration cannot grant it.
- Network host calls validate scheme, host, resolved addresses, ports, and
  redirect targets against the configured policy before every connection.
  Database host calls use only configured, capability-authorized connections.
  Alternate URL forms, DNS changes, redirects, proxies, IP literals, and other
  alternate-host paths cannot bypass either check.
- WASM execution has enforced memory, fuel, and wall-clock limits. Host calls
  are metered and bounded so a plugin cannot turn a small invocation into
  unbounded network, database, CPU, memory, or event work.
- Database access is available only through its granted host interface and
  intended SQL boundary. Plugins cannot acquire raw connections, credentials,
  or a broader database identity, and untrusted values remain parameterized or
  use validated query structure.
- Installation and loading confine manifest, module, cache, and extracted paths
  to their configured roots after normalization and symlink resolution. Plugin
  identifiers, dependency names, versions, and manifests are parsed and
  validated before they select files, registries, capabilities, or dependencies.
- JavaScript and TypeScript plugins have no ambient filesystem, process,
  network, database, secret, or host-object access. Their host interaction is
  capability-mediated and subject to the same resource limits as WASM plugins.
- Plugin SSR treats plugin output as untrusted rendered content: it preserves
  the caller's output-encoding and response-security rules and cannot gain
  ambient server privileges through rendering context or serialization.
- Registry entries, lifecycle state, and event delivery are isolated by plugin
  identity. A plugin cannot replace another plugin's registration, observe or
  forge private events, or recursively fan out events without bounded,
  capability-authorized dispatch.

## Reportable Findings

Report capability escalation or fail-open lookup, alternate-host bypass,
resource-limit bypass, SQL or credential boundary escape, unsafe install/load
path or manifest handling, ambient JavaScript/TypeScript authority, unsafe SSR,
or cross-plugin registry and event isolation failures.
