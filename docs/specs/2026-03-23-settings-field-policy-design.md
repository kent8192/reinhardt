# Settings Field Policy Design

## Overview

Add field-level policy control (Required/Optional) to the `#[settings]` macro system.
Library authors declare default policies on fragment fields; users override policies
at composition time via inline block syntax.

## Decision Log

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Control layers | Two-layer (library defaults + user overrides) | Library sets safe defaults, users customize per-app |
| Policy types | Required / Optional only | Exclude adds complexity without clear benefit |
| Semantics | SettingsBuilder checks after all sources merged | Supports TOML + env vars + programmatic `.set()` |
| Library syntax | `default_policy` + `#[setting(...)]` exceptions | Minimizes boilerplate; most fields are Optional |
| User syntax | Inline block `core: CoreSettings { field: policy }` | Natural extension of existing nom parser syntax |
| Default value override | Not allowed at composition time | Preserves library author's safety defaults |
| Validation timing | `.build()` after all source merging | Enables multi-source value injection |

## Architecture

### Approach: Metadata Trait

Add `field_policies()` to `SettingsFragment` trait. Macros generate static policy
metadata from `#[setting(...)]` attributes. Composition macro generates override-merged
`resolved_*_policies()` methods. `SettingsBuilder.build()` checks required fields
before deserialization.

```
┌─────────────────────────────────────────────────────────────────┐
│ Library Author                                                  │
│                                                                 │
│  #[settings(fragment = true, section = "core",                  │
│             default_policy = "optional")]                       │
│  pub struct CoreSettings {                                      │
│      #[setting(required)]                                       │
│      pub secret_key: String,                                    │
│      #[setting(default = "true")]                               │
│      pub debug: bool,                                           │
│      pub allowed_hosts: Vec<String>,                            │
│  }                                                              │
│                          │                                      │
│                          ▼                                      │
│             SettingsFragment::field_policies()                   │
│             → &'static [FieldPolicy]                            │
└──────────────────────────┬──────────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────────┐
│ App Developer                                                   │
│                                                                 │
│  #[settings(                                                    │
│      core: CoreSettings { debug: required } |                   │
│      cors: CorsSettings                                         │
│  )]                                                             │
│  struct MyAppSettings {}                                        │
│                          │                                      │
│                          ▼                                      │
│             resolved_core_policies()                             │
│             = base policies + overrides merged                  │
└──────────────────────────┬──────────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────────┐
│ SettingsBuilder                                                 │
│                                                                 │
│  1. Merge sources (TOML, env, .set())                           │
│  2. validate_requirements(&merged)  ← required field check      │
│  3. Deserialize (all fields have serde defaults)                │
│  4. validate(profile)               ← fragment validation       │
└─────────────────────────────────────────────────────────────────┘
```

## Type Definitions

### Core Types (`reinhardt-conf`)

Types are defined in `reinhardt-conf` where `SettingsFragment` and `BuildError` live.

```rust
/// Field-level policy for settings fragments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldRequirement {
    /// The field MUST have a value after all sources are resolved.
    /// SettingsBuilder.build() returns an error if missing.
    Required,
    /// The field MAY be absent. Default value is used if not provided.
    Optional,
}

/// Metadata describing a single field's policy within a SettingsFragment.
#[derive(Debug, Clone)]
pub struct FieldPolicy {
    /// Field name (matches the struct field name, used for key lookup
    /// in the merged IndexMap<String, serde_json::Value>)
    pub name: &'static str,
    /// Whether the field is required or optional
    pub requirement: FieldRequirement,
    /// Whether a compile-time default value is defined via #[setting(default = "...")]
    /// or the type implements Default (for fields inheriting default_policy = "optional")
    pub has_default: bool,
}
```

### `has_default` Population Logic

| Condition | `has_default` |
|---|---|
| `#[setting(default = "expr")]` present | `true` |
| `#[setting(optional)]` or inherited optional (no `#[setting]`) | `true` (relies on `Default::default()`) |
| `#[setting(required)]` | `false` |
| No `#[setting]` + `default_policy = "required"` | `false` |

The macro generates `#[serde(default = "...")]` for fields where `has_default = true`:
- If `#[setting(default = "expr")]`: generates `#[serde(default = "generated_fn")]` where the fn returns the expression
- If inherited optional without explicit default: generates `#[serde(default)]` (uses `Default::default()`)

### Trait Extension

```rust
pub trait SettingsFragment:
    Clone + Debug + Serialize + DeserializeOwned + Send + Sync + 'static
{
    type Accessor: ?Sized;
    fn section() -> &'static str;

    /// Returns the default field policies defined by the library author.
    /// Generated by the #[settings(fragment = true)] macro.
    fn field_policies() -> &'static [FieldPolicy] {
        &[]  // backward-compatible default
    }

    fn validate(&self, _profile: &Profile) -> ValidationResult {
        Ok(())
    }
}
```

### Composed Settings Trait

```rust
pub trait ComposedSettings: Sized + DeserializeOwned {
    /// Check that all required fields have values in the merged source.
    /// The merged data is an IndexMap<String, serde_json::Value> from MergedSettings.
    fn validate_requirements(merged: &IndexMap<String, serde_json::Value>) -> Result<(), BuildError>;

    /// Run all fragment validate() methods.
    fn validate(&self, profile: &Profile) -> ValidationResult;
}
```

### Error Type Extension

New variant added to existing `BuildError` in `reinhardt-conf`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    // Existing variants...

    /// A required field was not provided by any source.
    #[error(
        "missing required field `{field}` in section `[{section}]`. \
         Provide it via TOML, environment variable, or .set()"
    )]
    MissingRequiredField {
        section: &'static str,
        field: &'static str,
    },
}
```

## Library-Side Macro Syntax

### Fragment Definition

```rust
#[settings(fragment = true, section = "core", default_policy = "optional")]
pub struct CoreSettings {
    #[setting(required)]
    pub secret_key: String,

    #[setting(default = "true")]
    pub debug: bool,

    #[setting(default = "vec![]")]
    pub allowed_hosts: Vec<String>,

    // No #[setting] → inherits default_policy = optional
    pub middleware: Vec<String>,
}
```

### Attribute Interpretation Rules

| Fragment Attribute | Field Attribute | Result |
|---|---|---|
| `default_policy = "optional"` | None | Optional, `Default::default()` |
| `default_policy = "optional"` | `#[setting(required)]` | Required |
| `default_policy = "optional"` | `#[setting(default = "expr")]` | Optional, default = expr |
| `default_policy = "required"` | None | Required |
| `default_policy = "required"` | `#[setting(optional)]` | Optional, `Default::default()` |
| `default_policy = "required"` | `#[setting(optional, default = "expr")]` | Optional, default = expr |
| **Omitted** (`default_policy` absent) | None | **Optional** (backward compatible) |

### Compile-Time Errors

| Condition | Error Message |
|---|---|
| Unknown attribute key | `unknown setting attribute: 'X'. Expected: 'required', 'optional', 'default'` |
| `required` + `default` | `'required' and 'default' are mutually exclusive on field 'X'` |
| Invalid `default_policy` | `invalid default_policy: 'X'. Expected: 'required', 'optional'` |

## User-Side Composition Syntax

### Override Syntax

```rust
#[settings(
    core: CoreSettings { debug: required, allowed_hosts: required } |
    cors: CorsSettings |
    session: SessionSettings { cookie_secure: required }
)]
struct MyAppSettings {}
```

No overrides = existing syntax, fully backward compatible.

### Parser Extension (`settings_parser.rs`)

```rust
pub enum FragmentEntry {
    Include {
        key: String,
        type_name: String,
        overrides: Vec<FieldOverride>,  // added
    },
    // Exclude variant removed (was already deprecated and errored at compile time)
}

pub struct FieldOverride {
    pub field_name: String,
    pub policy: FieldRequirement,
}
```

Grammar extension:

```
fragment_entry  = key ":" type_name [ "{" field_overrides "}" ]
field_overrides = field_override ("," field_override)* [","]
field_override  = ident ":" policy
policy          = "required" | "optional"
```

### Compile-Time Field Validation

The macro validates override field names against the target type's actual fields
at expansion time. Non-existent fields produce `compile_error!` with a
Levenshtein distance-based "did you mean?" suggestion (threshold: distance <= 3
or distance <= half the field name length, whichever is smaller).

### Override Constraint Rules

| Rule | Description |
|---|---|
| Optional → Required | Allowed (stricter) |
| Required → Optional | Allowed (user's responsibility) |
| Default value change | Prohibited |
| Non-existent field | Compile error |
| Unknown policy | Compile error |
| Duplicate field override | Compile error |

## SettingsBuilder Integration

### Data Model

The existing `SettingsBuilder` merges all sources into `MergedSettings`, which wraps
`Arc<IndexMap<String, serde_json::Value>>`. The composition macro generates
`#[serde(flatten)]` on all fragment fields, so the merged data is a **flat key-value
map** — field names are top-level keys, not nested under section names.

### Flat Key Lookup Strategy

Since `#[serde(flatten)]` flattens all fragment fields into the top-level namespace:
- `CoreSettings.secret_key` → key `"secret_key"` in the merged map
- `CorsSettings.allow_origins` → key `"allow_origins"` in the merged map

The `validate_requirements` method looks up field names directly as top-level keys
in the `IndexMap`. The `section()` value is used only for error messages (to tell
the user which fragment the missing field belongs to), not for key lookup.

**Note:** If two fragments define fields with the same name, `#[serde(flatten)]`
already causes a conflict at deserialization time. This is an existing limitation
unrelated to field policies.

### Build Flow

1. Merge all sources (TOML, env, programmatic) into `MergedSettings` (`IndexMap<String, serde_json::Value>`)
2. Call `ComposedSettings::validate_requirements(&merged.data)` — check required fields in flat map
3. Deserialize via `MergedSettings::try_into::<T>()` (all fields have `#[serde(default)]`)
4. Call `validate(profile)` — run fragment-level validation

### Builder Method Extension

A new typed build method is added alongside the existing `build()`:

```rust
impl SettingsBuilder {
    // Existing: returns MergedSettings (untyped)
    pub fn build(self) -> Result<MergedSettings, BuildError> { ... }

    // New: returns typed composed settings with required field validation
    pub fn build_composed<T: ComposedSettings>(self) -> Result<T, BuildError> {
        let merged = self.build()?;
        T::validate_requirements(merged.as_map())?;
        let settings: T = merged.into_typed()
            .map_err(BuildError::from)?;  // GetError -> BuildError conversion needed
        // Fragment-level validation is called by the caller via settings.validate(profile)
        Ok(settings)
    }
}
```

### Serde Coordination

All fields get `#[serde(default)]` or `#[serde(default = "...")]` regardless of
their Required/Optional policy. The required check happens BEFORE deserialization
via `validate_requirements()`, operating on the flat `IndexMap<String, serde_json::Value>`.

This separation ensures:
- Multi-source merging works correctly (TOML + env + `.set()`)
- Required fields are validated against the merged result, not individual sources
- Serde never fails on missing fields (all have defaults)

## Test Strategy

### Compile-Time Tests (trybuild)

| Test | File |
|---|---|
| Non-existent field in override | `tests/ui/settings_unknown_field.rs` |
| Unknown policy keyword | `tests/ui/settings_unknown_policy.rs` |
| `required` + `default` conflict | `tests/ui/settings_required_with_default.rs` |
| Duplicate field override | `tests/ui/settings_duplicate_override.rs` |

### Runtime Tests (rstest)

| Test | Description |
|---|---|
| Required field missing | `.build()` returns `MissingRequiredField` error |
| Required field via `.set()` | Programmatic source satisfies required check |
| Optional uses default | Absent optional field gets default value |
| Override optional→required | User override enforces required on previously optional field |

### Test Placement

| Category | Location | Framework |
|---|---|---|
| Compile errors | `crates/reinhardt-core/macros/tests/ui/` | trybuild |
| FieldPolicy unit tests | `crates/reinhardt-conf/` | rstest |
| SettingsBuilder checks | `crates/reinhardt-conf/` | rstest |
| Composition + override integration | `tests/` (integration crate) | rstest |

## Error Messages

All errors follow the pattern: **problem + resolution**.

| Error | Message |
|---|---|
| Required field missing | ``missing required field `X` in section `[Y]`. Provide it via TOML, environment variable, or .set()`` |
| Non-existent field | ``field `X` does not exist in `Y` `` + ``did you mean `Z`?`` |
| Unknown policy | ``unknown policy `X`. Expected: `required`, `optional` `` |
| Required + default | ``'required' and 'default' are mutually exclusive on field `X` `` |
| Duplicate override | ``duplicate override for field `X` in `Y` `` |

## Files to Modify

| File | Change |
|---|---|
| `crates/reinhardt-conf/src/settings/policy.rs` (new) | `FieldRequirement`, `FieldPolicy` types |
| `crates/reinhardt-conf/src/settings/fragment.rs` | `SettingsFragment::field_policies()` default method |
| `crates/reinhardt-conf/src/settings/composed.rs` (new) | `ComposedSettings` trait |
| `crates/reinhardt-conf/src/settings/builder.rs` | `BuildError::MissingRequiredField` variant, `build_composed()` method |
| `crates/reinhardt-core/macros/src/settings_fragment.rs` | Parse `#[setting(...)]`, generate `field_policies()` |
| `crates/reinhardt-core/macros/src/settings_compose.rs` | Parse override blocks, generate `resolved_*_policies()`, `ComposedSettings` impl |
| `crates/reinhardt-core/macros/src/settings_parser.rs` | Extend nom parser for `{ field: policy }` syntax |
| System fragment definitions | Add `#[setting(required)]` / `#[setting(default = "...")]` to `CoreSettings`, `CorsSettings`, etc. |

## Backward Compatibility

- `field_policies()` has default impl `&[]` — existing fragments work unchanged
- `default_policy` omitted defaults to `"optional"` — existing fragments are all-optional
- Composition without `{ }` blocks works exactly as before
- `ComposedSettings` trait is generated for ALL compositions going forward (not conditional on override presence), ensuring a single consistent code path
