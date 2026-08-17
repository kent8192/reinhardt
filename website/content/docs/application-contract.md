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

## Verification protocol

The `contract` feature provides human-readable verification by default and a
version 1 JSON report for automation:

```text
cargo run --bin manage -- verify
cargo run --bin manage -- verify --format json
```

The clean report is:

```json
{
  "schema_version": 1,
  "status": "passed",
  "violations": []
}
```

| Result | Exit status |
| --- | ---: |
| `passed` | 0 |
| `failed` | 1 |
| `error` | 2 |

JSON stdout contains only one report document. Cargo and operational
diagnostics use stderr. All current violations have severity `error`; settings
values and concrete dynamic keys are absent. `location` is currently `null`
because the verifier does not retain source positions. Human-readable output
remains the default.

Each violation has `code`, `class`, `severity`, `target`, `location`,
`evidence`, and `suggested_fix`. The seven stable codes are
`schema.missing_migration`, `schema.unapplied_migration`,
`authorization.missing_declaration`, `settings.missing_required`,
`settings.type_mismatch`, `settings.map_key_type_mismatch`, and
`settings.duplicate_input`. Canonical ordering is inherited from
`VerificationRun`.

Targets have these shapes:

```text
model_change: app_label, name_fragment
migration: app_label, migration_name
endpoint: method, path, module_path, function_name
setting: canonical wildcarded path
```

Use this loop to let an agent repair contract violations:

```bash
cargo run --bin manage -- verify --format json > /tmp/reinhardt-verify.json
status=$?
case "$status" in
  0) echo "contract verified" ;;
  1) jq -r '.violations[] | [.code, .target.kind, .suggested_fix] | @tsv' /tmp/reinhardt-verify.json ;;
  2) echo "verification could not complete" >&2 ;;
esac
rm -f /tmp/reinhardt-verify.json
```

An agent repairs source only for exit 1, reruns the command, and stops at
`passed`. Exit 2 requires repairing the execution environment or configuration
before findings can be trusted. The supported freshness path is `cargo run`;
invoking an already-built `manage` executable directly does not detect a stale
binary.

## HTTPS derivation and versioning

The schema URL is an HTTPS contract identifier and is derived from the published
schema, not from a local filesystem path. `v0.json` is immutable: compatible
clarifications keep the v0 identifier, while a breaking shape change publishes
the next version at a new URL. Consumers should validate the `$schema` and
`schema_version` constants before interpreting a document.
