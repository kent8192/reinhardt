+++
title = "Application Contract"
description = "The immutable, deterministic JSON contract exported by Reinhardt applications."
weight = 70

[extra]
sidebar_weight = 70
+++

# Application Contract

Applications with the `commands-contract` feature can export a machine-readable
snapshot with:

```text
manage contract export --format json
```

The output is a pretty-printed JSON document with one trailing newline. Its
`$schema` and `$id` are the HTTPS resource
`https://reinhardt-web.dev/schemas/application-contract/v0.json`.

## Document sections

The document contains four arrays:

- `models` describes fields, constraints, indexes, and relationships.
- `migrations` preserves raw topological order, dependencies, replacements, and
  the optional applied-state overlay.
- `routes` lists mounted executable routes with uppercase methods, handler
  identities, authentication, and guards.
- `settings` lists resolved leaf settings with policy metadata and secret
  classification.

Every object is closed by the v0 schema. Optional producer values are present
as explicit `null` values rather than omitted properties. Arrays and nested
projections use canonical lexical ordering, while migration order remains the
raw dependency order. Settings paths escape literal dots, backslashes, and
asterisks; wildcard segments remain unescaped `*`.

Settings policy overrides apply to the leaf that declares the override. A
non-leaf override does not silently change the requirement or secret metadata
of its children. Route `handler` values are stable registration identifiers
(for example, `route:/health`, `view:/health`, or
`viewset:articles::list`), and settings `rust_type` values preserve the source
type expression rather than compiler-generated type-name formatting.

## Database state

Without a database selector, an unavailable recorder is a non-fatal warning on
stderr and every migration has `applied: null`. An explicit `--database` or
`--database-url` makes recorder failure fatal. Diagnostics redact credentials
and sensitive-looking aliases; secrets never appear in the JSON document.

## Human verification

The `contract` feature also provides a human-readable verification command:

```text
cargo run --bin manage -- verify
```

Verification first replays the consumer Cargo check captured by the generated
launcher. A spawn failure or non-zero Cargo status stops before contract
collection. After that phase, schema, authorization, and settings validators
run independently; a settings-source failure does not suppress authorization
findings. Launcher replay also fails closed when Cargo-exposed feature names are
ambiguous after normalization. The validators report stable finding codes:

- `schema.missing_migration` and `schema.unapplied_migration`;
- `authorization.missing_declaration`;
- `settings.missing_required`, `settings.type_mismatch`,
  `settings.map_key_type_mismatch`, and `settings.duplicate_input`.

Applied-migration coverage is optional; when no applied snapshot is available,
only that coverage check is omitted. Authorization checks materialize only
synchronous in-memory route registrations and reject asynchronous factories
and invalid route patterns without polling them; they do not install a router, initialize dependency
injection, or open a database. Settings checks use the same
typed-coercion mode as `SettingsBuilder`. Their findings retain canonical paths,
expected shapes, and JSON kinds, but never values, concrete dynamic map keys, or
parser/deserializer messages.

Verification is human-readable only and does not change the versioned JSON
export. The supported freshness path is `cargo run`; invoking an already-built
`manage` executable directly does not detect a stale binary.

## HTTPS derivation and versioning

The schema URL is an HTTPS contract identifier and is derived from the published
schema, not from a local filesystem path. `v0.json` is immutable: compatible
clarifications keep the v0 identifier, while a breaking shape change publishes
the next version at a new URL. Consumers should validate the `$schema` and
`schema_version` constants before interpreting a document.
