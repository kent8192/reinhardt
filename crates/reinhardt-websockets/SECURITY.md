# reinhardt-websockets Security Policy

## System and Scope

This policy inherits the repository [Security Policy](../../SECURITY.md) and
the framework-crate [Security Policy](../SECURITY.md). Policies compose from
the repository root to this crate; this closest policy wins on conflict.

`reinhardt-websockets` upgrades HTTP connections and handles persistent
connections, frames, messages, rooms, actions, broadcasts, and distributed
channels. Handshake metadata and all received frames are attacker-controlled.

## Security Invariants

- Protected deployments must install an explicit origin and authentication
  policy in every handshake entry point. `OriginValidationMiddleware` is an
  opt-in connection middleware; cookie or session authentication is never
  inferred from client-supplied identity.
- Applications must bind a validated principal and tenant immutably to each
  connection before exposing protected room APIs; the public room APIs do not
  infer this context from a client ID or raw connection.
- Applications must authorize joining rooms, sending actions, and receiving
  broadcasts for the relevant resource and tenant. Broadcast delivery must be
  filtered per recipient so one subscriber cannot receive another's data; the
  room broadcast primitive does not apply these checks automatically.
- Frame and message size, nesting, count, decompression output, and processing
  work must be limited before allocation or decompression. Applications must
  bound outbound queues and define an overflow or disconnect policy; the current
  connection and in-memory channel primitives do not provide backpressure
  automatically.
- Rate limits derive their key from validated connection identity or trusted
  peer metadata, not spoofable headers or message fields. The built-in
  `RateLimitMiddleware` keys message buckets by `WebSocketConnection::id()`;
  unless the application assigns a stable, validated principal-derived ID,
  this is a per-connection limit and reconnects start a fresh bucket. Protected
  deployments must treat that behavior as per-connection or provide a
  rebinding-safe identity-key strategy. Distributed channels preserve the same
  authenticated identity, authorization, isolation, limits, and rate-limit
  semantics as local delivery.

## Reportable Findings

Report cross-origin or handshake-auth bypass, connection identity confusion,
unauthorized room/action/broadcast access, pre-limit resource exhaustion,
spoofable rate limits, or weaker distributed-channel enforcement.
