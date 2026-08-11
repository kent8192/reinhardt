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
of its children.

## Database state

Without a database selector, an unavailable recorder is a non-fatal warning on
stderr and every migration has `applied: null`. An explicit `--database` or
`--database-url` makes recorder failure fatal. Diagnostics redact credentials
and sensitive-looking aliases; secrets never appear in the JSON document.

## HTTPS derivation and versioning

The schema URL is an HTTPS contract identifier and is derived from the published
schema, not from a local filesystem path. `v0.json` is immutable: compatible
clarifications keep the v0 identifier, while a breaking shape change publishes
the next version at a new URL. Consumers should validate the `$schema` and
`schema_version` constants before interpreting a document.
