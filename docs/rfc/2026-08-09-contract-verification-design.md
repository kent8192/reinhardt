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
Issue #5985. It does not parse the JSON export and does not rebuild a second source
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

Issue #5985 must expose an internal aggregate with domain-native state
equivalent to:

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
    replacement_edges: Vec<(MigrationKey, MigrationKey)>,
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
    target: Option<String>,
    profile: CargoProfile,
    manifest_path: PathBuf,
    package: Option<String>,
    binary: Option<String>,
    config_overrides: CargoConfigReplay,
}
```

These names are design-level names; implementation may place the domain
structures in their owning crates while keeping the same information and
dependency direction.

`registered_endpoints` is not the raw `EndpointMetadata` inventory. The #5985
collector resolves a side-effect-free mounted-route topology supplied by the
route registration layer and includes only metadata with an exposed method and
path. Each resolved entry carries the stable handler identity emitted by the
route macro and the final path after all mounts and prefixes. A decorated
handler that is linked but never mounted is omitted. The collector must not
call `server_router_async()`, execute an async `#[routes]` factory, construct an
`InjectionContext`, or initialize a database merely to collect this metadata.
If a dynamic route cannot be represented by the side-effect-free topology,
contract resolution fails with a safe route-topology error rather than running
application services or producing an authentication finding from an unexposed
handler. Finding and endpoint correlation uses the stable handler identity,
not a method/path lookup that can be ambiguous after mounting.

`CargoCheckContext` is supplied by the generated management launcher and
records the manifest, package, and binary selection as well as the feature
selection of the Cargo invocation that built the binary (default features,
`--no-default-features`, named `--features`, or `--all-features`). It also
records the active target and profile (`dev`, `release`, or a named profile)
and whether every active Cargo configuration override can be replayed. The
verifier passes the recorded selections to its Cargo phase. A missing context
or an unreplayable override is a verification execution error; neither case
may cause a plain default-feature, host-target, development-profile
`cargo check`.

`CargoConfigReplay` contains the exact `--config` key/value or file overrides
and relevant build-flag overrides when they are available to the launcher. If
an active override cannot be captured without guessing, it is represented as
unsupported and verification stops before contract collection. This is a
fail-closed boundary: the verifier never claims to check the running artifact
after dropping a `build.rustflags`, `--cfg`, or equivalent Cargo override.

The nested Cargo check reuses the recorded manifest and passes `--package` and
`--bin` when they were selected, together with the recorded feature, target,
profile, and replayable configuration arguments. It therefore checks the
management target that produced the inventories rather than an unrelated
workspace default member.

`SchemaContractState::replacement_edges` retains every `replaces` edge from the
resolved migration catalog, including edges whose source is itself replaced.
Each edge is oriented as `(replacement, replaced)`. Starting with the applied
set, the validator repeatedly adds replaced ancestors of covered replacements
and a replacement whose entire replaced set is covered until no new key is
added. It compares known migrations with that fixed-point coverage set before
classifying unapplied migrations; it never infers replacement coverage from one
unordered map pass.

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

Issue #5985 must therefore make command selection precede eager typed-settings
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
Issue #5985; it does not introduce a second contract feature. The command is
added to
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

If the reconstructed state contains opaque schema operations, the drift
subcheck returns a check execution error rather than guessing. The verifier
still runs the unapplied-migration comparison from its independent applied
snapshot, even when that drift error is present. Authorization and settings
checks also run; one database subcheck error is reported alongside every
finding available from the other subchecks.

### Unapplied Migration Detection

When `applied_migrations` is `Some`, each known migration absent from the
applied set becomes one `schema.unapplied_migration` finding. When it is `None`,
the applied-state check is omitted without a warning or error.

The verifier uses the read-only applied snapshot supplied by #5985. It does not
create the migration recorder table and does not query migrations one at a
time.

Replacement resolution is transitive before this comparison. A migration
replaced by an intermediate squash is followed through that squash to the
terminal replacement, and the applied set is compared with the resulting
closure. The verifier therefore does not report an unapplied intermediate
migration when its terminal squash is the applied migration.

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
applied. The schema retains the generated field path, actual Serde input key,
Rust type name, required/default policy, accepted aliases, skipped-field
semantics, custom-deserializer boundary, container shape, secret
classification, and, for maps, both key and value schemas.

Serde naming is resolved into that schema rather than reconstructed from Rust
identifiers. At every nested struct boundary, `rename_all` and its
deserialization-specific form use the same case conversion as application
deserialization; an explicitly renamed field takes precedence. Unsupported or
ambiguous naming metadata is rejected while generating the schema rather than
silently using a Rust field name. The schema therefore uses `myField` when the
application deserializer expects `myField`, not the source identifier
`my_field`.

Struct-level `default` attributes are also retained. They make absent fields
valid only when the generated deserializer can actually construct the struct
without that field; they do not make an explicitly present `null` value valid.
The schema records this presence rule separately from each child's field
policy.

The schema also records whether a composed root section itself has a Serde
default. An omitted section is validated against that generated root-field
semantics even when every child field is optional; a section without a root
default remains required. Child optionality never implies root-section
optionality.

Type-only composition uses the same field key that the generated composed
struct deserializes. When a fragment such as `SchemaDatabaseSettings` declares
`section = "database"`, the composition generator emits the matching
`#[serde(rename = "database")]` on the generated `schema_database` field. If
that mapping cannot be emitted, composition is rejected; the root schema may
not validate `[database]` while typed deserialization expects
`schema_database`.

The verifier must consume this resolved root schema rather than rebuilding
fragment policy rules in `reinhardt-commands`.

Unsupported struct-level Serde behavior is rejected during schema generation.
This includes `deny_unknown_fields`, `try_from`, `from`, `into`, and
`transparent` unless their exact deserialization semantics are represented in
the root schema. The supported exceptions are the naming and default rules
described above. The verifier must not silently approximate a container whose
Serde implementation replaces field-wise deserialization or changes unknown-
field behavior.

Recursive container shape is resolved only when the macro can see the concrete
`Option`, sequence, map, or node shape. An ordinary type alias that hides a
container and cannot be expanded by the macro is rejected during schema
generation; it is never downgraded to a leaf schema. A later explicit shape
annotation may extend this boundary, but the prototype does not claim
recursive validation for an unexpanded alias.

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

Composition-level optional overrides must agree with actual Serde behavior. An
override may make a field optional only when the generated deserializer also
provides a default for an absent field. An override on a required fragment
field without such a default is rejected (or the generator must emit the
matching default); changing only `FieldRequirement` or `has_default` is not
valid. The verifier never treats an optional override as a substitute for a
missing Serde default.

The resolved schema records the complete set of accepted input keys for every
field: its canonical Serde key and any aliases. Presence validation inspects
that set before selecting a value: zero keys is missing only when the resolved
Serde policy requires it, exactly one key is deserialized, and more than one
key emits one redacted duplicate-input finding without choosing a value.
`skip_deserializing` fields are absent by design. This matches Serde's
duplicate-field rejection and keeps aliases from creating false missing-field
findings.

Schema metadata gains a type-check function generated for the concrete field
or container type and its Serde attributes. Attributes such as
`deserialize_with`, `with`, and `skip_deserializing` are retained by the
generated checker with the same semantics as application deserialization. A
custom deserializer attached to a sequence, map, optional, or nested-struct
field runs against the whole field value before generic container shape
validation; after it succeeds, traversal stops at that field. Generic
recursive traversal is used only when no whole-field deserializer replaces
the field's representation. A skipped-deserialization field is absent from
input presence validation, while `alias` names are accepted as input names but
never emitted as canonical schema paths. An unsupported attribute is rejected
while generating the schema.
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

| Class | Code |
| --- | --- |
| Schema | `schema.missing_migration` |
| Schema | `schema.unapplied_migration` |
| Authorization | `authorization.missing_declaration` |
| Settings | `settings.missing_required` |
| Settings | `settings.type_mismatch` |
| Settings | `settings.map_key_type_mismatch` |
| Settings | `settings.duplicate_input` |

Target sort keys:

- `schema.missing_migration`: app label, operation fragment, description;
- `schema.unapplied_migration`: app label, migration name;
- `authorization.missing_declaration`: method, path, module path, function
  name;
- `settings.missing_required`: canonical path;
- `settings.type_mismatch`: canonical path, expected type, actual kind;
- `settings.map_key_type_mismatch`: canonical path, expected key type, actual
  kind;
- `settings.duplicate_input`: canonical path.

`settings.duplicate_input` is emitted at most once for each canonical path, so
the canonical path is its stable target sort key. The finding contains no
concrete setting value or map key.

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

The concrete check command is assembled from `CargoCheckContext`: it passes
`--manifest-path`, the selected `--package` and `--bin`, the recorded feature
flags, `--target`, `--profile`, and every replayable `--config` override. An
unsupported override becomes a safe execution error before spawning Cargo.
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

Issue #5986 does not promise distinct or stable numeric exit codes. The
follow-up #5987 will map
the typed outcome to a versioned report and explicit exit behavior.

## Testing

### reinhardt-db

- equal model and migration states produce no drift;
- several generated operations produce several ordered findings;
- known/applied sets report every unapplied migration;
- absent applied state omits only the applied-state check;
- replacement and `database_only` histories reconstruct the correct state;
- opaque state produces a schema check error without a panic;
- an opaque drift error does not suppress independent unapplied-migration
  findings;
- nested replacement edges reach a fixed point before unapplied classification.

### reinhardt-core

- protected, optional, and public endpoints pass;
- several `None` endpoints are all returned;
- an unmounted decorated endpoint is not reported;
- endpoint targets retain exact method, path, module, and function data;
- an async injected route factory is not executed during side-effect-free
  verification metadata collection;
- the startup wrapper retains fail-fast panic behavior.

### reinhardt-conf

- several missing required paths are returned together;
- several type mismatches are returned together;
- nested nodes, sequences, maps, and optional values are traversed;
- invalid map keys are checked against their declared key type;
- canonical and alias keys supplied together produce one stable
  `settings.duplicate_input` finding;
- struct-level rename and default semantics match the generated Serde keys and
  presence rules;
- unsupported struct-level behavior and unexpanded container aliases are
  rejected during schema generation;
- custom deserializers on non-leaf fields run before generic traversal;
- composition-level optional overrides cannot weaken a required Serde field;
- omitted composed root sections are checked against generated root-field
  default semantics;
- accepted typed coercions match `SettingsBuilder`;
- rejected coercions report only path, expected type, and actual kind;
- a distinctive secret literal cannot appear in any finding or error rendering,
  including malformed TOML source errors.

### reinhardt-commands

- a Cargo failure prevents contract collection;
- the active Cargo feature selection is preserved, and missing feature context
  does not fall back to default features;
- target, profile, manifest, package, binary, and replayable configuration
  overrides are preserved; unsupported overrides fail closed;
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

Issue #5986 may begin only after #5985 exposes the validation-ready internal state
described above. It may add the minimal domain APIs required to interpret that
state, but it must not redesign the exported JSON schema.

Issue #5987 consumes `VerificationRun` and `VerificationFinding`. It must not move
domain validation into serialization or change the checks defined here.
