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

- Secrets, credentials, keys, tokens, and connection URLs must be redacted from
  logs, errors, `Debug` output, audits, serialization, and equivalent
  diagnostics. Configuration types must use a redacting secret type or custom
  implementations; `CoreSettings` stores `secret_key` as a plain `String`, so
  callers must not use its derived diagnostics or serialization for secrets.
  `VaultConfig` likewise derives `Debug` while storing its authentication token
  as a public `String`; callers must treat it as secret-bearing input and must
  not log or serialize the derived diagnostic without redaction.
  `EmailSettings` derives diagnostics and serialization over its plaintext SMTP
  password, `CacheSettings` exposes a potentially credential-bearing Redis URL
  through `location`, and `DatabaseUrl` serializes its password and original
  URL; callers must treat these fields as secret-bearing inputs and redact them
  before diagnostics or serialization.
- `VaultSecretProvider` propagates the underlying `reqwest` transport error;
  that error may contain the Vault request URL, including a private host or
  path. Callers must sanitize the error before logging or returning it across a
  boundary where the address is sensitive.
- Interpolation accepts only defined source and reference forms and detects
  cycles. Protected applications must bound recursion depth, substitution
  count, expanded size, and work before resolution; `Interpolator` does not
  impose every output or work limit automatically. Missing or malformed
  references fail safely rather than selecting a permissive fallback.
- Encryption uses authenticated encryption with securely generated, scoped
  keys and algorithm-appropriate nonces that are unique per key and never
  reused. Keys and plaintext are never exposed; authentication failures do not
  disclose sensitive material. Decryption rejects tampering before a value is
  used.
- Secret and audit backends redact their credentials and connection details on
  every error path, including initialization, refresh, audit persistence, and
  retries.
- Hot-reload consumers must construct and validate a complete candidate
  configuration before one atomic swap. `DynamicSettings::watch_file` is
  notification-only unless the caller installs that candidate-build, validate,
  and swap callback; it does not replace active settings by itself.
- Rotation accepts only authenticated replacement material and bounds the
  lifetime of stale credentials or privileges. `SecretRotation::rotate` and
  `force_rotate` currently record timestamps and audit entries only: they do
  not replace or revoke credentials or authenticate the `rotated_by` value.
  Callers must wrap them with the actual authenticated replacement and
  revocation workflow. Revoked, expired, or failed replacements cannot remain
  valid indefinitely through caches or reloads.
- Dynamic-backend integrations document their integrity, authentication,
  authorization, availability, and freshness assumptions. Implementations do
  not silently treat an unavailable or unauthenticated backend as trusted local
  configuration.

## Reportable Findings

Report secret disclosure, unsafe interpolation or unbounded expansion,
unauthenticated encryption or nonce/key misuse, backend credential exposure,
partial hot reload, indefinitely stale privilege after rotation, or an
undocumented dynamic-backend trust assumption that permits unsafe configuration.
