# reinhardt-query Security Policy

## System and Scope

This policy inherits the repository [Security Policy](../../SECURITY.md) and
the framework-crate [Security Policy](../SECURITY.md). Policies compose from
the repository root to this crate; this closest policy wins on conflict.

`reinhardt-query` constructs DML, DDL, and DCL for PostgreSQL, MySQL, SQLite,
and CockroachDB. Generated SQL, bound values, identifiers, role and privilege
statements, and function or procedure definitions are security-sensitive
outputs. SQL values and any runtime-supplied query structure are untrusted.

## Security Invariants

- Safe DML APIs bind values as backend-native parameters; they never interpolate
  values into SQL text.
- Builders quote every identifier with the selected backend's correct rules.
  Runtime identifiers are accepted only through a constrained, validated
  identifier representation; callers cannot supply arbitrary SQL fragments.
- Dynamic limits, offsets, orderings, directions, and columns use bound values
  where supported or a finite validated allowlist. They must not become raw SQL.
- DDL uses the same identifier-validation and backend-correct quoting guarantees
  as DML for databases, schemas, tables, columns, indexes, constraints, views,
  sequences, types, and related objects.
- GRANT, REVOKE, role, and user statements must validate and quote grantees,
  principals, roles, objects, privilege names, and backend-specific options.
  Callers must constrain or escape MySQL account option text; the safe builder
  does not make every option field safe for arbitrary untrusted input.
- PostgreSQL role-membership statements emit `RoleSpecification::RoleName`
  values as supplied rather than identifier-quoting them. Callers must
  validate role names and any MySQL `user@host` form before constructing
  `GRANT` or `REVOKE` statements.
- Function and procedure bodies, signature parameter types, and return types are
  explicit raw-code boundaries. They must not be assembled from untrusted
  fragments or silently treated as parameterized SQL; callers must validate
  signature types or treat them as trusted raw SQL.
- PostgreSQL custom-type fragments are also explicit raw-code boundaries:
  `TypeKind::Composite` attribute types, `Domain` base/default/constraint text,
  `Range` subtype and function names, and `ALTER TYPE` constraint/default text
  are emitted as supplied. Callers must validate these values or restrict them
  to trusted schema metadata.
- CockroachDB zone configuration fragments are also explicit raw-code
  boundaries: `ZoneConfig::add_constraint` and `add_lease_preference` accept
  arbitrary strings that are emitted inside single-quoted SQL literals.
  Callers must validate or escape these values before building an
  `ALTER DATABASE ... CONFIGURE ZONE` statement.
- Equivalent safe APIs preserve their injection, identifier, and privilege
  guarantees across every supported backend; an unsupported safe construction
  fails explicitly rather than falling back to unsafe syntax.
- Placeholder numbering and bound-value order remain exact through nested
  expressions, CTEs, unions, joins, and subqueries.

## Reportable Findings

Report injection, identifier or privilege escalation, altered placeholder/value
binding, or a backend-specific safe-API path that permits untrusted query
structure. Explicit raw SQL and explicit function or procedure bodies are out
of scope unless a supposedly safe API reaches them with attacker-controlled
input.
