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
  tokens according to their configured lifecycle.
- OAuth flows bind authorization responses to validated state, PKCE, and nonce
  values where applicable; redirect targets and callback parameters are not
  trusted merely because they arrive from the browser.
- MFA challenges, verification, and completion are bound to the authenticating
  user, login transaction, intended factor, and bounded lifetime; a response
  cannot complete another user's or another attempt's login.
- Remote-user authentication trusts proxy identity headers only from configured
  trusted immediate proxies. Forwarded headers from all other peers are ignored.
- Authentication errors, logs, responses, and telemetry do not disclose
  passwords, tokens, signing keys, MFA material, or account-enumeration detail.

## Reportable Findings

Report credential or authorization bypass, client-authoritative authorization,
session fixation, weak cookie or JWT validation, rotation or revocation gaps,
OAuth/MFA transaction confusion, spoofable proxy identity, or secret exposure.
Application-specific policies remain out of scope when callers intentionally
select an explicit unauthenticated endpoint.
