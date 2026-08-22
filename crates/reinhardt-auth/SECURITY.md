# reinhardt-auth Security Policy

## System and Scope

This policy inherits the repository [Security Policy](../../SECURITY.md) and
the framework-crate [Security Policy](../SECURITY.md). Policies compose from
the repository root to this crate; this closest policy wins on conflict.

`reinhardt-auth` establishes and authorizes application identities through
credentials, sessions, cookies, tokens, OAuth, MFA, permission checks, and
middleware. Credentials, bearer tokens, cookies, authorization headers, OAuth
responses, proxy headers, and client state are attacker-controlled until their
specific validation completes.

## Security Invariants

- Credential parsing and verification fail closed. Missing, malformed, expired,
  revoked, inactive, or unverifiable credentials never establish identity.
- Authorization is enforced server-side for every protected action with the
  target object, model, tenant, and operation context. Client `AuthState` is
  display state only and is never authoritative for identity or permissions.
- Missing authentication middleware, request auth state, or required dependency
  is denial, not anonymous success or a permissive fallback.
- `SessionAuthentication` treats a missing `_auth_user_is_active` session flag as
  active and does not perform an authoritative user lookup. Deployments that
  disable accounts after session creation must validate active status before
  accepting the session.
- Object-level and model-level permission checks have equivalent enforcement;
  collection, detail, mutation, and alternate transport paths cannot bypass one
  another.
- Authentication regenerates session identifiers at login and privilege change,
  invalidates replaced sessions, and prevents session fixation. Session cookies
  use Secure, HttpOnly, appropriate SameSite, scoped path/domain, and expiry
  semantics for their deployment.
- JWT verification pins permitted algorithms and keys, validates signatures and
  expiry before claims are used, and validates issuer, audience, and time claims
  when configured. Key rotation preserves verification only for approved keys;
  logout, revocation, password changes, and credential compromise invalidate
  tokens according to their configured lifecycle. `JwtAuthMiddleware` and the
  `JwtAuth` REST backend do not independently recheck account status or
  revocation state after token issuance; deployments requiring immediate
  disable or revocation must perform an authoritative account lookup.
- OAuth flows must bind authorization responses to validated state, PKCE, nonce,
  and the initiating browser session where applicable; the state stores do not
  automatically establish a browser binding.
- Protected MFA integrations must bind challenges, verification, and completion
  to the authenticating user, login transaction, intended factor, and bounded
  lifetime. `MFAAuthentication` can operate as a standalone TOTP backend and
  does not supply a first-factor transaction by itself.
- Deployments using remote-user authentication must accept proxy identity
  headers only behind a configured trusted immediate proxy and strip or reject
  them from all other peers. The remote-user backend consumes the configured
  header and does not inspect the peer trust boundary itself.
- Basic authentication skips Argon2 verification when the username is absent,
  and `ProviderConfig` derives `Debug` while storing `client_secret` in a
  public `String`. Callers handling untrusted authentication traffic must use a
  dummy-hash/timing-equalized failure path and redact provider configuration
  before logging or formatting it.
- `LoginCredentials`, `CreateUserData`, `OAuthToken`, `TokenResponse`, and
  `TokenRotationRecord` are public credential-bearing types whose derived
  `Debug` or `Serialize` implementations may expose passwords or live tokens.
  Callers must treat these values as secrets and redact or avoid formatting
  them before logging, diagnostics, telemetry, or error responses.
- Authentication errors, logs, responses, and telemetry do not disclose
  passwords, tokens, signing keys, MFA material, or account-enumeration detail.

## Reportable Findings

Report credential or authorization bypass, client-authoritative authorization,
session fixation, weak cookie or JWT validation, rotation or revocation gaps,
OAuth/MFA transaction confusion, spoofable proxy identity, or secret exposure.
Application-specific policies remain out of scope when callers intentionally
select an explicit unauthenticated endpoint.
