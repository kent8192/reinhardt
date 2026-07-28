# Reinhardt Configuration Framework

Django-inspired settings management for Rust with secrets, encryption, and audit logging.

## Overview

The `reinhardt-conf` crate provides a comprehensive configuration management framework for Reinhardt applications, inspired by Django's settings system with additional security features.

## Features

- **Multiple configuration sources**: Files, environment variables, command-line arguments
- **Type-safe settings**: Strong type validation with custom validators
- **Secrets management**: Integration with HashiCorp Vault, AWS Secrets Manager, Azure Key Vault
- **Encryption**: Built-in encryption for sensitive settings
- **Dynamic backends**: Redis and database-backed dynamic settings
- **Secret rotation**: Automatic secret rotation support
- **Audit logging**: Track all setting changes

## Modules

This crate provides the following modules:

- **`` `settings` ``**: Core settings management functionality

## Installation

Add `reinhardt` to your `Cargo.toml`:

<!-- reinhardt-version-sync:3 -->
```toml
[dependencies]
reinhardt = { version = "0.3.3", features = ["conf"] }

# Or use a preset:
# reinhardt = { version = "0.3.3", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.3.3", features = ["full"] }      # All features
```

Then import configuration features:

```rust
use reinhardt::conf::settings::{SettingsBuilder, Settings};
use reinhardt::conf::settings::sources::ConfigSource;
```

**Note:** Configuration features are included in the `standard` and `full` feature presets.

### Optional Features

Enable specific features based on your needs:

<!-- reinhardt-version-sync:3 -->
```toml
# With async support
reinhardt = { version = "0.3.3", features = ["conf", "async"] }

# With encryption
reinhardt = { version = "0.3.3", features = ["conf", "encryption"] }

# With Vault integration
reinhardt = { version = "0.3.3", features = ["conf", "vault"] }
```

Available features:

- `settings` (default): Core settings functionality
- `async`: Asynchronous settings operations
- `dynamic-redis`: Redis-backed dynamic settings
- `dynamic-database`: Database-backed dynamic settings
- `vault`: HashiCorp Vault integration
- `aws-secrets`: AWS Secrets Manager integration
- `azure-keyvault`: Azure Key Vault integration
- `secret-rotation`: Automatic secret rotation
- `encryption`: Built-in encryption for sensitive settings

## Usage

```rust
use reinhardt::conf::settings::SettingsBuilder;
use reinhardt::conf::settings::sources::ConfigSource;

// Basic usage
let settings = SettingsBuilder::new()
    .add_source(ConfigSource::File("config.toml"))
    .add_source(ConfigSource::Environment)
    .build()?;

// Access settings
let database_url = settings.get::<String>("DATABASE_URL")?;
```

## Configuration Sources

### TOML Interpolation

Opt-in `${VAR}` substitution for TOML string values.

| Token              | Behavior                                           |
|--------------------|----------------------------------------------------|
| `${VAR}`           | required — fails if `VAR` is unset OR empty        |
| `${VAR:-default}`  | substitutes `default` if `VAR` is unset OR empty   |
| `${VAR:-}`         | explicit empty fallback (special case of `:-`)     |
| `${VAR:?message}`  | fails with `message` if `VAR` is unset OR empty    |
| `$$`               | escape — emits a literal `$`                       |

Variable names follow POSIX conventions: `[A-Za-z_][A-Za-z0-9_]*`.

Interpolation is **enabled by default** since `0.1.0-rc.27`. Call
`.without_interpolation()` if you need raw `${...}` strings to survive
the load (for example, when the TOML is itself a template that
downstream code expands).

```rust,ignore
use reinhardt_conf::settings::builder::SettingsBuilder;
use reinhardt_conf::settings::sources::TomlFileSource;

let settings = SettingsBuilder::new()
    // Interpolation is on by default — no builder method required.
    .add_source(TomlFileSource::new("settings/local.toml"))
    .build()?;

// Opt out when literal `${...}` must survive:
// .add_source(TomlFileSource::new("settings/template.toml").without_interpolation())
```

Example TOML:

```toml
[database]
host = "${REINHARDT_DB_HOST:-localhost}"
port = "${REINHARDT_DB_PORT:-5432}"

[secrets]
db_password = "${DB_PASSWORD:?Set DB_PASSWORD via direnv or 1Password CLI}"
```

Secret fields backed by `SecretString` also accept explicit source maps:

```toml
[database.default]
engine = "postgresql"
host = "localhost"
port = 5432
name = "app"
user = "app"
password = { env = "DATABASE_PASSWORD" }

[database.replica]
engine = "postgresql"
host = "replica.internal"
port = 5432
name = "app"
user = "readonly"
password = { file = "/run/secrets/db-replica-password" }
```

Use `{ secret = "literal" }` for inline values, `{ env = "NAME" }` for process
environment variables, and `{ file = "path" }` for file-backed secrets. File
sources trim trailing CR/LF so Docker/Kubernetes-style secret files work without
leaking the final newline into connection strings.

#### Behavior Notes

- **Strict empty handling**: an empty environment-variable value is treated identically
  to "unset". This catches typos like `export REINHARDT_DB_HOST=` early. To allow an
  explicit empty fallback, write `${VAR:-}`.
- **Single-pass**: resolved values are not re-expanded, so `${OUTER}` whose value
  happens to contain `${INNER}` resolves to literally `${INNER}`.
- **String-only scope**: only `toml::Value::String` is rewritten, but every string
  in the TOML tree is scanned — strings inside nested tables and arrays are
  interpolated as well. Numeric, boolean, and datetime values pass through
  untouched. To inject a typed override use `HighPriorityEnvSource`
  (priority 60 — beats interpolated TOML at priority 50).
- **Composition**: interpolation runs at `TomlFileSource::load()` time. The resolved
  value participates in the normal source-priority merge; later sources at higher
  priority still override.
- **Typed coercion** (since 0.1.0-rc.27, default ON): a resolved string value whose
  destination Rust type is non-`String` is coerced into the target at deserialize
  time. Supported destinations:

  | Target                          | Source string format                              |
  |---------------------------------|---------------------------------------------------|
  | `bool`, integers, floats, `char`| `FromStr`-parseable text                          |
  | enum unit variant               | variant name (matched via serde's normal rules)   |
  | `Option<T>`                     | empty string -> `None`, otherwise recurse into `T`|
  | `Vec<T>`                        | JSON array literal: `"[1, 2, 3]"`                 |
  | `HashMap<K, V>` / `BTreeMap`    | JSON object literal: `"{\"a\": 1}"`               |
  | `Vec<u8>`                       | base64 (STANDARD)                                 |

  Coercion failures abort `SettingsBuilder::build_composed()` with
  `BuildError::Coercion`, naming the TOML key path, target type, original value,
  and parser cause.

  Disable with `SettingsBuilder::with_typed_coercion(false)` to fall back to the
  legacy serde-json passthrough.

- **Nested struct from a single string is rejected** with
  `CoercionError::UnsupportedShape`. Use per-field interpolation instead:

  ```toml
  [endpoint]
  host = "${HOST:-localhost}"
  port = "${PORT:-5432}"
  ```

## Typed Settings Schemas

Composed settings expose typed schema references through `ProjectSettings::schema()`.
Embedded settings nodes are addressable with normal field access:

```rust,ignore
let password = ProjectSettings::schema().database.default.password;
assert_eq!(password.path().to_string(), "database.default.db-password");
```

The path is derived from the root composition key, the embedded field key, and
serde rename attributes. For example, `#[settings(database: DatabaseSettings)]`,
`DatabaseSettings { default: DatabaseConfig }`, and
`#[serde(rename = "db-password")] password` produce
`database.default.db-password`. Type-only composition still uses the fragment's
section hint for the root path.

Schema generation peels semantically agnostic wrappers before building nested
references: `Option<T>`, `Vec<T>`, `HashMap<String, T>`,
`BTreeMap<String, T>`, `IndexMap<String, T>`, and `Box<T>`. Optional refs expose
`.some()`, sequence refs expose `.any()`, map refs expose `.any()` and
`.entry(key)`, and boxed values are transparent.

Use `#[setting(node)]` to force a field to be treated as an embedded settings
node and `#[setting(leaf)]` to force leaf behavior. The default inference is
conservative: types ending in `Config` may infer node behavior, while types
ending in `Settings` should be annotated explicitly unless they are built-in
fragments already annotated by the crate.

Required validation descends into embedded nodes after the direct section field
exists. A missing nested required leaf reports
`BuildError::MissingRequiredPath` with the full schema path. A missing direct
required field on the section still reports `BuildError::MissingRequiredField`.

### Embedded-Only Settings Nodes

Use `#[settings(fragment = true)]` without a `section = "..."` argument for a
settings struct that should participate in schema metadata and validation below
a root fragment, but should not become a top-level TOML section by itself.

```rust,ignore
#[settings(fragment = true, section = "database")]
pub struct DatabaseSettings {
    pub default: DatabaseConfig,
    pub replica: Option<DatabaseConfig>,
}

#[settings(fragment = true, default_policy = "required")]
pub struct DatabaseConfig {
    pub engine: DatabaseEngine,
    pub host: String,
    pub port: u16,
    pub name: String,
    pub user: String,
    pub password: SecretString,
}
```

The corresponding TOML still nests the embedded node under the root fragment:

```toml
[database.default]
engine = "postgresql"
host = "localhost"
port = 5432
name = "app"
user = "app"
password = { env = "DATABASE_PASSWORD" }
```

A root fragment with `section = "..."` implements `SettingsFragment` and can be
used in composed project settings. An embedded-only node implements
`SettingsNode` for recursive schema and validation support, but does not
implement `SettingsFragment`, expose `section()`, or register as a root
composition section.

## Field Status

The `Settings` struct contains fields that are either actively consumed by the framework or reserved for future implementation.

### Active Fields

These fields are actively consumed by the framework and affect runtime behavior:

| Field | Description |
|-------|-------------|
| `base_dir` | Base directory of the project |
| `secret_key` | Secret key for cryptographic signing |
| `debug` | Debug mode toggle |
| `allowed_hosts` | List of allowed host/domain names |
| `installed_apps` | List of installed applications |
| `middleware` | List of middleware classes |
| `root_urlconf` | Root URL configuration module |
| `databases` | Database configurations |
| `static_url` | Static files URL prefix |
| `static_root` | Static files root directory |
| `staticfiles_dirs` | Additional static files directories |
| `media_url` | Media files URL prefix |
| `media_root` | Media files root directory |
| `secure_proxy_ssl_header` | Proxy SSL header configuration |
| `secure_ssl_redirect` | HTTPS redirect toggle |
| `secure_hsts_seconds` | HSTS max-age header value |
| `secure_hsts_include_subdomains` | HSTS subdomain inclusion |
| `secure_hsts_preload` | HSTS preload directive |
| `session_cookie_secure` | Secure session cookie toggle |
| `csrf_cookie_secure` | Secure CSRF cookie toggle |
| `append_slash` | Trailing slash auto-append toggle |
| `admins` | Administrator contact list |
| `managers` | Manager contact list |

### Reserved for Future Implementation

These fields exist for Django settings compatibility but are **not yet consumed** by the framework. Setting these values currently has no effect on framework behavior:

| Field | Description | Planned Feature |
|-------|-------------|-----------------|
| `language_code` | Language code (default: `"en-us"`) | i18n support |
| `time_zone` | Time zone (default: `"UTC"`) | Timezone-aware datetime handling |
| `use_i18n` | Enable i18n (default: `true`) | i18n support |
| `use_tz` | Enable timezone-aware datetimes (default: `true`) | Timezone-aware datetime handling |
| `templates` | Template engine configurations | Template engine integration |
| `default_auto_field` | Default auto field type for models | Auto field configuration |

## Module Organization

`` `reinhardt-conf` `` is organized into the following modules:

- `` `settings` `` - Core settings management (builder, validation, encryption)

### Using Modules

```rust
use reinhardt::conf::settings::{SettingsBuilder, SettingsConfig};
```

## License

Licensed under the BSD 3-Clause License.

## Contributing

Contributions are welcome! Please see the main [CONTRIBUTING.md](../../CONTRIBUTING.md) for guidelines.
