# reinhardt-conf Security Policy

## System and Scope

This policy inherits the repository [Security Policy](../../SECURITY.md) and
the framework-crate [Security Policy](../SECURITY.md). Policies compose from
the repository root to this crate; this closest policy wins on conflict.

`reinhardt-conf` resolves typed configuration, environment and backend values,
interpolation, encrypted values, audits, dynamic settings, hot reload, and
secret rotation. Configuration inputs and dynamic backends cross a trust
boundary; secret material must not become diagnostic output.

## Security Invariants

- Secrets, credentials, keys, tokens, and connection URLs are redacted from
  logs, errors, `Debug` output, audits, serialization, and equivalent
  diagnostics. Redaction covers backend failures and nested configuration
  values as well as direct secret fields.
- Interpolation accepts only defined source and reference forms, detects cycles,
  and bounds recursion depth, expanded size, and work before resolution. Missing
  or malformed references fail safely rather than leaking values or selecting a
  permissive fallback.
- Encryption uses authenticated encryption with algorithm-appropriate, unique
  nonces and securely generated, scoped keys. Keys, plaintext, nonce material,
  and authentication failures are never exposed; decryption rejects tampering
  before a value is used.
- Secret and audit backends redact their credentials and connection details on
  every error path, including initialization, refresh, audit persistence, and
  retries.
- Hot reload constructs and validates a complete candidate configuration before
  one atomic swap. A failed reload leaves the active configuration unchanged;
  readers cannot observe a partial, mixed, or unvalidated configuration.
- Rotation accepts only authenticated replacement material and bounds the
  lifetime of stale credentials or privileges. Revoked, expired, or failed
  replacements cannot remain valid indefinitely through caches or reloads.
- Dynamic-backend integrations document their integrity, authentication,
  authorization, availability, and freshness assumptions. Implementations do
  not silently treat an unavailable or unauthenticated backend as trusted local
  configuration.

## Reportable Findings

Report secret disclosure, unsafe interpolation or unbounded expansion,
unauthenticated encryption or nonce/key misuse, backend credential exposure,
partial hot reload, indefinitely stale privilege after rotation, or an
undocumented dynamic-backend trust assumption that permits unsafe configuration.
