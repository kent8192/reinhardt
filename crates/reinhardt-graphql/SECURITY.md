# reinhardt-graphql Security Policy

## System and Scope

This policy inherits the repository [Security Policy](../../SECURITY.md) and
the framework-crate [Security Policy](../SECURITY.md). Policies compose from
the repository root to this crate; this closest policy wins on conflict.

`reinhardt-graphql` executes schemas, resolvers, mutations, DataLoaders,
subscriptions, broadcasts, and GraphQL-over-gRPC services. Documents,
variables, aliases, fragments, operation names, subscription inputs, and
resolver arguments are attacker-controlled.

## Security Invariants

- Protected GraphQL deployments must configure and apply depth, complexity,
  field-count, document-size, and parsing-work limits before execution on every
  transport path. The schema helpers do not automatically enforce every one of
  these controls; configured limits account for aliases, fragments, repeated
  selections, variables, and nested operations so they cannot evade the
  effective cost.
- Every resolver and mutation enforces server-side authorization for its target
  object, tenant, and operation. Validation, schema visibility, and client-side
  query construction never replace that decision.
- DataLoader and resolver caches are isolated by request, authenticated user,
  and tenant. Cached values, keys, errors, and batching cannot disclose data
  across those boundaries.
- Protected deployments must authorize subscription establishment and filter
  every event delivery for the recipient's scope, tenant, and permissions.
  `EventBroadcaster` and the public subscription resolvers do not perform
  recipient checks automatically.
- Request-scoped DI preserves the authenticated identity and tenant through
  resolver and subscription execution. Protected deployments must fork a
  request context for each GraphQL request; the schema-level
  `with_di_context` helper reuses one context and does not provide request
  isolation automatically. GraphQL-over-gRPC preserves the same GraphQL and
  gRPC authorization, validation, isolation, and resource limits only when
  each RPC rejects documents whose operation type does not match the RPC and
  applies the corresponding controls; the current service forwards documents
  to schema execution without enforcing that operation-type match.

## Reportable Findings

Report alias or fragment limit bypass, unauthorized resolver/mutation access,
cross-user DataLoader leakage, unauthorized subscription or broadcast delivery,
DI identity loss, or weaker GraphQL-over-gRPC enforcement.
