# reinhardt-websockets Security Policy

## System and Scope

This policy inherits the repository [Security Policy](../../SECURITY.md) and
the framework-crate [Security Policy](../SECURITY.md). Policies compose from
the repository root to this crate; this closest policy wins on conflict.

`reinhardt-websockets` upgrades HTTP connections and handles persistent
connections, frames, messages, rooms, actions, broadcasts, and distributed
channels. Handshake metadata and all received frames are attacker-controlled.

## Security Invariants

- The handshake applies an explicit origin and authentication policy. Cookie or
  session authentication is validated with protections equivalent to the HTTP
  operation it upgrades and is never inferred from client-supplied identity.
- A validated principal and tenant are bound immutably to each connection.
  Client messages cannot select another identity, room, or authorization scope.
- Joining rooms, sending actions, and receiving broadcasts require authorization
  for the relevant resource and tenant. Broadcast delivery is filtered per
  recipient so one subscriber cannot receive another's data.
- Frame and message size, nesting, count, decompression output, and processing
  work are limited before allocation or decompression. Streaming and queues use
  bounded backpressure.
- Rate limits derive their key from validated connection identity or trusted
  peer metadata, not spoofable headers or message fields. Distributed channels
  preserve the same authenticated identity, authorization, isolation, limits,
  and rate-limit semantics as local delivery.

## Reportable Findings

Report cross-origin or handshake-auth bypass, connection identity confusion,
unauthorized room/action/broadcast access, pre-limit resource exhaustion,
spoofable rate limits, or weaker distributed-channel enforcement.
