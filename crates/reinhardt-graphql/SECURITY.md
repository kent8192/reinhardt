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

- Depth, complexity, field-count, document-size, and parsing-work limits apply
  before execution and account for aliases, fragments, repeated selections,
  variables, and nested operations so they cannot evade the effective cost.
- Every resolver and mutation enforces server-side authorization for its target
  object, tenant, and operation. Validation, schema visibility, and client-side
  query construction never replace that decision.
- DataLoader and resolver caches are isolated by request, authenticated user,
  and tenant. Cached values, keys, errors, and batching cannot disclose data
  across those boundaries.
- Subscription establishment and every event delivery enforce authorization.
  Broadcasts carry only data permitted to each recipient and remain protected
  when a subscriber's scope, tenant, or permissions differ.
- Request-scoped DI preserves the authenticated identity and tenant through
  resolver and subscription execution. GraphQL-over-gRPC preserves the same
  GraphQL and gRPC authorization, validation, isolation, and resource limits.

## Reportable Findings

Report alias or fragment limit bypass, unauthorized resolver/mutation access,
cross-user DataLoader leakage, unauthorized subscription or broadcast delivery,
DI identity loss, or weaker GraphQL-over-gRPC enforcement.
