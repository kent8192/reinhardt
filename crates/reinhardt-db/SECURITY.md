# reinhardt-db Security Policy

## System and Scope

This policy inherits the repository [Security Policy](../../SECURITY.md) and
the framework-crate [Security Policy](../SECURITY.md). Policies compose from
the repository root to this crate; this closest policy wins on conflict.

`reinhardt-db` provides ORM, schema migration, connection-pool, transaction,
and database-routing interfaces. Model data, request-derived filters, relation
keys, runtime metadata, connection settings, and migration state may cross a
database trust boundary.

## Security Invariants

- ORM reads and writes parameterize values. Safe filters, relations,
  annotations, aggregations, lookups, and orderings preserve query structure
  and do not accept arbitrary SQL fragments. PostgreSQL aggregate constructors
  that accept expression, separator, or ordering strings must receive validated
  identifiers/values or be treated as an explicit raw-SQL boundary; callers
  must not pass request-derived strings directly.
- Runtime model and schema metadata, including table, column, relation, index,
  and database-routing names, is validated and safely quoted before use.
- Migration and schema operations apply the same backend-correct identifier
  quoting and validation as runtime queries; generated migration state cannot
  turn metadata into executable SQL.
- Applications must use redacting URL/configuration types or custom diagnostic
  and serialization implementations for connection URLs, credentials,
  passwords, tokens, and private endpoints. The public raw URL/configuration
  wrappers do not redact derived `Debug` or serialized output automatically.
- Pools isolate transaction state between borrowers. Applications using roles,
  tenant/session variables, temporary settings, or other session state must
  configure backend-specific reset/discard behavior or keep that state inside a
  caller-owned transaction; returning a connection does not reset arbitrary
  session state automatically.
- Errors roll back or discard failed transactional work safely, and migrations
  serialize conflicting schema changes with a reliable lock that is released on
  completion or failure.
- PostgreSQL advisory migration locks are session-scoped. Lock acquisition and
  the protected migration work must use the same dedicated connection or
  caller-owned transaction; separate pool checkouts do not preserve the lock.
- Raw SQL is an explicit documented trust boundary. Safe ORM and migration APIs
  must not route untrusted input into it, and raw-SQL errors must remain
  secret-safe.

## Reportable Findings

Report injection through a safe database API, cross-request transaction or
session leakage, unsafe schema identifiers, migration-lock bypass, or exposed
credentials. Explicit raw SQL is out of scope unless a safe API introduces
untrusted input into that boundary.
