# reinhardt-storages Security Policy

## System and Scope

This policy inherits the repository [Security Policy](../../SECURITY.md) and
the framework-crate [Security Policy](../SECURITY.md). Policies compose from
the repository root to this crate; this closest policy wins on conflict.

`reinhardt-storages` maps application object names to local files and cloud
providers, creates signed URLs, and handles provider responses. Object names,
paths, metadata, redirect locations, and provider errors are untrusted until
validated for the selected backend and configured storage scope.

## Security Invariants

- Local storage must confine every read, write, delete, listing, and URL
  operation to its configured root after decoding, separator normalization,
  and canonical resolution. Before each write, callers must reject a final
  component symlink or use no-follow/root-relative semantics, and validate
  every existing parent before creating missing directories; `LocalStorage::save`
  currently calls `create_dir_all` before canonicalizing the parent, so an
  untrusted parent symlink can create directories outside the configured root
  even when the later file check rejects the write. Parent-directory validation
  alone does not protect an existing destination symlink. This guarantee also
  assumes the root and its entries are not modified by an untrusted concurrent
  process: `LocalStorage::open` checks the canonical path and then reads by
  pathname, without an atomic no-follow open. Shared hostile filesystems require
  root-relative/no-follow primitives or an equivalent trusted-filesystem
  boundary.
- Provider object names use one provider-safe canonical representation before
  authorization, storage, and comparison. Ambiguous encodings, separators,
  prefixes, and normalization forms cannot cause an object to be authorized as
  one key and operated on as another.
- Signed URLs bind a single authorized operation and canonical object to a
  bounded expiry. Protected applications must validate or cap caller-selected
  expiries before invoking provider URL helpers; `AzureStorage::url` does not
  impose every application maximum and an out-of-range duration can overflow
  timestamp arithmetic. `GcsStorage::url` in custom-endpoint mode does not use
  the signer; its `X-Goog-Expires` query parameter is not an access-control
  mechanism. Treat that mode as an emulator/development boundary, or reject
  signed-URL requests there until the endpoint provides compatible signing.
  They cannot be replayed for another object, operation, bucket, account, or
  unbounded lifetime.
- Signing uses an unambiguous canonical request that includes the selected
  method, object, host, path, relevant query parameters, expiry, and signed
  headers. Parsing or serialization differences cannot alter a signed request.
- Provider credentials, signed URL signatures, tokens, and private endpoints
  must be redacted from errors, logs, `Debug` output, and telemetry by the
  provider integration. Transport errors may include a request URL, so
  callers must sanitize provider errors before exposing or logging them.
- Redirects and provider-supplied locations use only validated configured hosts
  and safe schemes. The Azure and GCS integrations currently use default
  clients without a redirect policy or per-hop host validation, so callers must
  disable redirects or validate every `Location` against the configured
  provider origin before sending credentials or object content.

## Reportable Findings

Report local-root escape, provider-key canonicalization confusion, reusable or
overbroad signed URLs, ambiguous signing, credential disclosure, or redirect
host bypass.
