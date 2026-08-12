# Contract Verification Design

**Issue:** [#5986](https://github.com/kent8192/reinhardt-web/issues/5986)

**Depends on:** [#5985](https://github.com/kent8192/reinhardt-web/issues/5985)

**Follow-up:** [#5987](https://github.com/kent8192/reinhardt-web/issues/5987)

**Status:** Approved

**Date:** 2026-08-09

## Summary

Add a deterministic `manage verify` prototype that checks three Reinhardt
contract classes after Cargo has accepted the consumer project:

1. model state matches the state represented by migration files;
2. every registered endpoint explicitly declares its authentication posture;
3. merged settings satisfy required-path and type constraints.

The command consumes the in-process resolved contract state introduced by
#5985. It does not parse the JSON export and does not rebuild a second source
analysis pipeline.

## Goals

- Delegate Rust language and type checking to `cargo check`.
- Reuse the strict migration catalog and migration autodetector.
- Return every framework finding available from a resolved snapshot.
- Keep contract collection and validation fallible and non-panicking.
- Preserve SettingsBuilder coercion semantics.
- Make finding codes and ordering deterministic.
- Keep secret values out of findings and diagnostics.
- Leave the result types suitable for the machine-readable adapter in #5987.

## Non-Goals

- JSON or another machine-readable report format.
- Stable numeric process exit codes.
- Migration ordering or destructive-change policy warnings.
- Permission-definition and permission-reference matching.
- Analysis of handwritten HTTP routing, raw SQL, or other convention bypasses.
- Automatic source or migration repair.
- MCP integration.
- Detection or rebuilding of a stale, directly invoked prebuilt `manage`
  executable.

## Accepted Design Decisions

- #5985 provides a validation-ready in-process `ResolvedContractState` in
  addition to its export DTO.
- `cargo check` failure stops verification before contract collection.
- The Cargo phase receives the active feature selection used to build the
  management binary and never silently falls back to Cargo's default feature
  set.
- Settings type validation follows the same typed-coercion setting as
  `SettingsBuilder`.
- Domain validation stays in the owning crates; `reinhardt-commands` only
  orchestrates and aggregates.
- A database applied-state snapshot is optional. The verifier never opens a
  database connection itself.
- Contract-resolution failures are reduced to safe, stable summaries before
  they reach command output; underlying parser and deserializer diagnostics are
  not rendered.
- After contract resolution, an error in one domain check does not prevent the
  remaining domain checks from running.

## Prerequisite Contract from #5985

The JSON contract described by #5985 is not sufficient by itself for
verification. Migration identities do not reconstruct migration state, and a
settings `present` flag cannot validate a value's type.

#5985 must expose an internal aggregate with domain-native state equivalent to:

```rust
struct ResolvedContractState {
    schema: SchemaContractState,
    registered_endpoints: Vec<ResolvedEndpoint>,
    settings: SettingsContractState,
}

struct SchemaContractState {
    model_state: ProjectState,
    migration_state: ProjectState,
    known_migrations: Vec<MigrationKey>,
    applied_migrations: Option<BTreeSet<MigrationKey>>,
}

struct ResolvedEndpoint {
    handler_identity: String,
    method: String,
    resolved_path: String,
    metadata: EndpointMetadata,
}

struct SettingsContractState {
    root_schema: SettingsRootSchema,
    merged: IndexMap<String, serde_json::Value>,
    typed_coercion: bool,
}

struct CargoCheckContext {
    feature_selection: CargoFeatureSelection,
}
```

These names are design-level names; implementation may place the domain
structures in their owning crates while keeping the same information and
dependency direction.

`registered_endpoints` is not the raw `EndpointMetadata` inventory. The #5985
collector resolves `UrlPatternsRegistration` against the consumer's mounted
router and includes only metadata with an exposed method and path. Each
resolved entry carries the stable handler identity emitted by the route macro
and the final path after all mounts and prefixes. A decorated handler that is
linked but never mounted is omitted. If registration cannot be resolved,
contract resolution fails rather than producing an authentication finding from
an unexposed handler. Finding and endpoint correlation uses the stable handler
identity, not a method/path lookup that can be ambiguous after mounting.

`CargoCheckContext` is supplied by the generated management launcher and
records the feature selection of the Cargo invocation that built the binary
(default features, `--no-default-features`, named `--features`, or
`--all-features`). The verifier passes that selection to its Cargo phase. A
missing context is a verification execution error; it must not cause a plain
default-feature `cargo check`.

Contract-resolution errors use a redacted boundary type equivalent to:

```rust
struct ContractResolutionError {
    kind: ContractResolutionErrorKind,
    safe_target: Option<SafeContractTarget>,
}
```

The boundary type contains only a stable error kind and safe target metadata.
It does not store the underlying `SourceError`, TOML/JSON parser error,
deserializer error, source line, or rendered value. The resolver classifies and
discards those details before returning the error, so malformed settings cannot
place a secret literal in command, CI, or diagnostic output.

`SettingsContractState` contains secret material in memory because validation
must inspect the merged values. It must not derive or implement `Debug` or
`Serialize`. The #5985 exporter maps this internal state to redacted metadata
instead of serializing it directly.

The endpoint collection is sorted by method, path, module path, and function
name before it enters the aggregate. Final verification still applies its own
sort so correctness does not depend on `inventory` iteration order.

### Management Bootstrap

The generated management binary currently constructs typed settings before
command dispatch. That ordering would let a missing field or type mismatch fail
before `verify` can observe it.

#5985 must therefore make command selection precede eager typed-settings
validation. The generated binary passes a fallible settings resolver or
equivalent deferred provider to the command driver:

- source-loading failures remain contract-resolution errors;
- the provider preserves the merged map and resolved schema before required
  validation or typed deserialization;
- `contract` and `verify` consume that validation-ready state;
- other management commands request the typed settings value and retain their
  existing failure behavior.

The exact provider type belongs to #5985, but an implementation that calls
`get_settings()` before parsing `verify` does not satisfy this dependency.

## Architecture

`reinhardt-commands::verify` is a thin orchestration layer:

1. locate the consumer Cargo project and run `cargo check` with the active
   `CargoCheckContext`;
2. resolve `ResolvedContractState` through the #5985 collector;
3. pass each domain state to its owning validator;
4. retain both findings and per-check execution errors;
5. normalize all findings into `VerificationFinding`;
6. sort and render the complete human-readable result.

No validator reads Rust source. No validator parses the #5985 JSON output.

The supported freshness path is the normal
`cargo run --bin manage -- verify` invocation, where Cargo builds the current
management binary before the command starts. Self-reexecution and stale-binary
detection are outside this prototype.

### CLI and Feature Boundary

`verify` uses the same feature gate and resolved-state collector introduced for
#5985; it does not introduce a second contract feature. The command is added to
the built-in `Commands` enum and normal dispatch, but it is not added to
`requires_router()` or `requires_database()`. Compile-time inventories are
already linked into the consumer management binary, and the optional applied
snapshot is supplied rather than opened by the verifier.

The design adds no external dependency. The Cargo phase runs ordinary
`cargo check` with the feature selection from `CargoCheckContext`; it does not
invent a second feature-selection interface or silently use default features.

## Database Contract Validation

### Resolved Migration State

`MigrationCatalog` gains one reusable operation equivalent to
`resolved_project_state()`. It replays the catalog's resolved execution order
across all selected applications and dependencies.

The replay must:

- use the catalog's strict dependency, replacement, optional, and swappable
  resolution;
- skip `database_only` migrations;
- apply `state_only` operations to project state;
- preserve the existing opaque-schema marker;
- return `MigrationError` instead of panicking.

This operation becomes the single migration-file state source for both #5985
and #5986. The older `build_state_from_files()` path is not copied into the
verifier because it bypasses catalog resolution.

### Missing Migration Detection

The database validator compares `migration_state` to `model_state` with the
checked autodetector path. It uses the fallible migration-generation API so
rename ambiguity and invalid state remain execution errors.

Each generated migration operation becomes one
`schema.missing_migration` finding. The finding retains the application label,
the operation's existing migration-name fragment when available, and its
existing human description. This reuses migration semantics and presentation
without introducing another schema differ.

Autodetector policy warnings are not findings in this prototype. The operation
that represents the actual drift remains a finding; independent destructive or
policy advice stays out of scope.

If the reconstructed state contains opaque schema operations, the schema check
returns a check execution error rather than guessing. Authorization and settings
checks still run.

### Unapplied Migration Detection

When `applied_migrations` is `Some`, each known migration absent from the
applied set becomes one `schema.unapplied_migration` finding. When it is `None`,
the applied-state check is omitted without a warning or error.

The verifier uses the read-only applied snapshot supplied by #5985. It does not
create the migration recorder table and does not query migrations one at a
time.

## Authorization Contract Validation

`reinhardt-core` adds a non-panicking collector that accepts the resolved
registered-endpoint slice and returns every endpoint whose
`AuthProtection` is `None`.

Each violation retains only:

- HTTP method;
- route path;
- module path;
- function name.

The existing `validate_endpoint_security()` remains available as the
startup-compatible panic wrapper. It delegates classification to the collector
and preserves startup fail-fast behavior. `manage verify` calls only the
collector.

`Protected`, `Optional`, and explicitly `Public` endpoints are valid. Permission
contents and guard semantics are not inspected.

## Settings Contract Validation

### Resolved Root Schema

The settings composition macro exposes a runtime `SettingsRootSchema` containing
every composed section after composition-level policy overrides have been
applied. The schema retains the existing field path, Rust type name,
required/default policy, container shape, secret classification, and, for maps,
both key and value schemas.

The verifier must consume this resolved root schema rather than rebuilding
fragment policy rules in `reinhardt-commands`.

### Complete Validation

`reinhardt-conf` adds a pure traversal that returns all settings violations in
one pass:

- missing required fields at any nested path;
- node values that are not maps after typed coercion;
- sequence values that are not arrays after typed coercion;
- map values that are not maps after typed coercion;
- map keys that cannot deserialize as the declared key type;
- leaf values that cannot deserialize as the declared Rust type.

The traversal covers optional, sequence, and map contents recursively. At each
optional or container boundary, typed coercion runs before shape validation, so
a JSON string containing an array or map is normalized exactly as
`SettingsBuilder` would normalize it. An optional absent or null value is
valid.

Leaf schema metadata gains a type-check function generated for the concrete
field type and its field-level Serde attributes. Attributes such as
`deserialize_with`, `with`, and `skip_deserializing` are retained by the
generated checker with the same semantics as application deserialization; an
unsupported field-level attribute is rejected while generating the schema.
With typed coercion disabled it uses normal Serde JSON semantics. With typed
coercion enabled it uses the same typed-deserializer semantics as
`SettingsBuilder`. This avoids rejecting values that the application itself
accepts.

Map schema metadata likewise gains a key type and key-check function generated
for the concrete key type. Every object key is checked with the same coercion
mode before its value is traversed. An invalid key produces a
`settings.map_key_type_mismatch` finding; the finding identifies the map path,
expected key type, and JSON kind, but never includes the key literal.

### Redaction

A settings finding contains only:

- canonical settings path with every dynamic map entry represented by a
  wildcard segment (for example, `settings.backends.*.host`), never by the
  concrete key;
- expected Rust type or container shape;
- actual JSON kind: `null`, `boolean`, `number`, `string`, `sequence`, or
  `map`.

It never contains the value, a debug rendering of the value, or the underlying
Serde/coercion error text. This rule applies to all settings, not only fields
marked secret, so future classification mistakes cannot disclose a value.

## Aggregation and Determinism

`reinhardt-commands` wraps domain findings without erasing their typed data:

```rust
enum VerificationFinding {
    Schema(SchemaFinding),
    Authorization(EndpointSecurityViolation),
    Settings(SettingsViolation),
}

struct VerificationRun {
    findings: Vec<VerificationFinding>,
    check_errors: Vec<VerificationCheckError>,
}
```

The initial stable finding codes are:

| Class | Code | Target sort key |
|---|---|---|
| Schema | `schema.missing_migration` | app label, operation fragment, description |
| Schema | `schema.unapplied_migration` | app label, migration name |
| Authorization | `authorization.missing_declaration` | method, path, module path, function name |
| Settings | `settings.missing_required` | canonical path |
| Settings | `settings.type_mismatch` | canonical path, expected type, actual kind |
| Settings | `settings.map_key_type_mismatch` | canonical path, expected key type, actual kind |

Final ordering uses:

1. class rank: schema, authorization, settings;
2. finding code;
3. the target sort key above;
4. original deterministic operation ordinal only when otherwise identical.

Check execution errors use the same class rank followed by their stable error
kind and safe target. Collection order, hash-map order, and `inventory` order
never determine presentation order.

## Execution and Error Semantics

### Cargo Phase

The command runs ordinary `cargo check` from the resolved consumer project
root. Cargo owns compiler diagnostics and feature resolution. A non-zero exit,
spawn failure, or project-root resolution failure becomes a verification
execution error and prevents snapshot use.

The prototype does not add a process-runner abstraction. Focused command
planning remains a pure helper, and consumer integration tests exercise the
real Cargo process.

### Contract Phase

A #5985 contract-resolution failure stops domain validation because no coherent
snapshot exists. It is rendered only through the redacted
`ContractResolutionError` summary described above; the original source or
deserializer error is never formatted or retained at the command boundary.

After a snapshot exists, the three domain checks are attempted independently.
A domain error is appended to `check_errors`, and the other checks continue.
No verify path uses `expect`, `unwrap`, or panic for recoverable input.

### Command Result

- no findings and no check errors: print the clean summary and return success;
- findings present: print every finding, then return the existing generic
  command failure;
- check errors present: print the completed findings and the safe error
  summaries, then return the existing generic command failure.

#5986 does not promise distinct or stable numeric exit codes. #5987 will map
the typed outcome to a versioned report and explicit exit behavior.

## Testing

### reinhardt-db

- equal model and migration states produce no drift;
- several generated operations produce several ordered findings;
- known/applied sets report every unapplied migration;
- absent applied state omits only the applied-state check;
- replacement and `database_only` histories reconstruct the correct state;
- opaque state produces a schema check error without a panic.

### reinhardt-core

- protected, optional, and public endpoints pass;
- several `None` endpoints are all returned;
- an unmounted decorated endpoint is not reported;
- endpoint targets retain exact method, path, module, and function data;
- the startup wrapper retains fail-fast panic behavior.

### reinhardt-conf

- several missing required paths are returned together;
- several type mismatches are returned together;
- nested nodes, sequences, maps, and optional values are traversed;
- invalid map keys are checked against their declared key type;
- accepted typed coercions match `SettingsBuilder`;
- rejected coercions report only path, expected type, and actual kind;
- a distinctive secret literal cannot appear in any finding or error rendering,
  including malformed TOML source errors.

### reinhardt-commands

- a Cargo failure prevents contract collection;
- the active Cargo feature selection is preserved, and missing feature context
  does not fall back to default features;
- a contract-resolution failure prevents domain checks;
- contract-resolution output contains only the safe error kind and target;
- one domain check error does not suppress findings from the other domains;
- repeated runs over shuffled input produce identical finding order;
- clean, violation, and incomplete summaries remain distinct.

### Consumer Integration

Use two small consumer fixture applications:

- a clean application whose models, migrations, endpoints, and settings agree;
- a violating application containing model drift, multiple unprotected
  endpoints, and multiple settings violations at the same time.

Invoke the real management command so model and endpoint inventories are linked
as they are in an application. Assert the complete ordered result, not substring
presence. Keep applied-database coverage in the pure database tests so the
consumer fixtures do not require Docker.

## Documentation

Update:

- `reinhardt-commands` CLI and crate documentation with `manage verify` usage;
- `reinhardt-db` migration catalog documentation for resolved state replay;
- `reinhardt-core` endpoint security documentation for collector versus startup
  wrapper behavior;
- `reinhardt-conf` settings schema documentation for complete type validation.

The CLI documentation lists the three supported contract classes, optional
applied-state behavior, redaction guarantee, normal
`cargo run --bin manage -- verify` invocation, and all non-goals above.

## Alternatives Rejected

### Centralize Every Check in reinhardt-commands

This appears to reduce file count but duplicates migration replay, endpoint
classification, and settings coercion rules. Those copies would drift from the
framework behavior they claim to verify.

### Invoke and Parse Existing Commands

Combining `makemigrations --check`, startup validation, and settings loading
would retain panic and first-error behavior and would make command text an
unstable protocol.

### Build a Static Rust Source Analyzer

Source analysis cannot reproduce macro-expanded model and endpoint inventories,
resolved migration replacement graphs, or merged runtime settings. Cargo and
the resolved framework state already own those semantics.

## Delivery Boundary

#5986 may begin only after #5985 exposes the validation-ready internal state
described above. It may add the minimal domain APIs required to interpret that
state, but it must not redesign the exported JSON schema.

#5987 consumes `VerificationRun` and `VerificationFinding`. It must not move
domain validation into serialization or change the checks defined here.
