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
  helpers are opt-in and do not enforce limits automatically. The current
  `DepthLimitedDecoder` recursively scans the complete wire payload before
  comparing the measured depth with `max_depth`, so applications must impose a
  separate wire-size/work bound or wrap it with a bounded pre-scan.
- Service-wide validation applies to generated, unary, and streaming handlers;
  schema validation does not replace server-side authorization on the target
  resource and tenant.
- Protected applications must install bounded buffering, backpressure,
  cancellation, timeout, and concurrency controls on every streaming handler.
  `GrpcServerConfig` stores timeout and concurrency values but does not compose
  the required layers or stream policy automatically.
- Protected applications must install an authentication interceptor that
  validates metadata and constructs request-scoped DI state. The optional `di`
  feature consumes application-provided context; it does not authenticate
  metadata itself. Caller-controlled metadata or injected values cannot
  establish a different principal, tenant, or authorization context.
- Client-visible statuses sanitize implementation, dependency, and internal
  error detail. Production `ErrorSanitizer` logs currently retain the original
  message for connection, service, and internal errors, so those messages must
  be treated as sensitive diagnostic data and protected or redacted separately.
  GraphQL-over-gRPC preserves the authentication, authorization, validation,
  isolation, and limit guarantees of native gRPC and GraphQL operations only
  when the application applies equivalent controls to both RPC methods. The
  current `GraphQLGrpcService` forwards query and mutation documents directly
  to schema execution without enforcing that the query RPC receives only query
  operations or that the mutation RPC receives only mutation operations;
  method-specific controls must therefore be enforced by the application.

## Reportable Findings

Report pre-limit protobuf exhaustion, handler or stream validation gaps,
authorization or DI identity confusion, unbounded streaming, sensitive error
detail, or weaker GraphQL-over-gRPC protections.
