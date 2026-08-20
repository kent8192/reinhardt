# reinhardt-grpc Security Policy

## System and Scope

This policy inherits the repository [Security Policy](../../SECURITY.md) and
the framework-crate [Security Policy](../SECURITY.md). Policies compose from
the repository root to this crate; this closest policy wins on conflict.

`reinhardt-grpc` provides protobuf decoding, service handlers, unary and
streaming RPCs, metadata authentication, dependency injection, and
GraphQL-over-gRPC adapters. Protobuf bytes, metadata, stream timing, and
generated-service inputs are attacker-controlled.

## Security Invariants

- Applications must apply `MessageSizeLimiter` and `DepthLimitedDecoder` to
  every exposed service so incoming message size, decoded size, nesting depth,
  and parsing work are bounded before allocation or recursive decoding. These
  helpers are opt-in and do not enforce limits automatically; the depth scan
  must abort at the configured limit rather than recursively traversing an
  attacker-controlled payload before reporting the violation.
- Service-wide validation applies to generated, unary, and streaming handlers;
  schema validation does not replace server-side authorization on the target
  resource and tenant.
- Streams enforce bounded buffering, backpressure, cancellation, timeout, and
  concurrency behavior so a slow or malicious peer cannot consume unbounded
  memory, tasks, or connections.
- Protected applications must install an authentication interceptor that
  validates metadata and constructs request-scoped DI state. The optional `di`
  feature consumes application-provided context; it does not authenticate
  metadata itself. Caller-controlled metadata or injected values cannot
  establish a different principal, tenant, or authorization context.
- Client-visible statuses and logs sanitize implementation, dependency, and
  internal error detail. GraphQL-over-gRPC preserves the authentication,
  authorization, validation, isolation, and limit guarantees of native gRPC
  and GraphQL operations.

## Reportable Findings

Report pre-limit protobuf exhaustion, handler or stream validation gaps,
authorization or DI identity confusion, unbounded streaming, sensitive error
detail, or weaker GraphQL-over-gRPC protections.
