# Reinhardt Settings System Documentation

## Overview

Reinhardt provides a flexible and powerful configuration management system. By
combining TOML-format configuration files with environment variables, it enables
consistent configuration management across all environments, from development to
production.

## Key Features

1. **TOML Format**: Easy-to-read and write configuration files
2. **Environment-Specific Settings**: Different configurations for local,
   staging, production, etc.
3. **Priority System**: Integrates settings from multiple sources with clear
   priority rules
4. **No Recompilation Required**: No need to rebuild Rust code when changing
   settings
5. **Secure**: Protects sensitive information with `.gitignore`
6. **Extensible**: Easy to add custom sources and configuration items

---

## Quick Start

### 1. Project Setup

Projects created with the `reinhardt-admin startproject` command already include
a `settings/` directory:

```
my-project/
├── settings/
│   ├── .gitignore         # Ignores *.toml, only commits *.example.toml
│   ├── base.example.toml  # Base configuration template
│   ├── local.example.toml # Local development template
│   ├── staging.example.toml
│   └── production.example.toml
└── src/
    └── config/
        └── settings.rs    # Settings loading logic
```

### 2. Creating Configuration Files

```bash
# Copy example files to create actual configuration files
cd my-project/settings
cp base.example.toml base.toml
cp local.example.toml local.toml
cp staging.example.toml staging.toml
cp production.example.toml production.toml
```

### 3. Editing Configuration Files

`settings/base.toml`:

```toml
debug = false
secret_key = "your-secret-key-here"

[database]
engine = "postgresql"
host = "localhost"
port = 5432
name = "mydb"
user = "postgres"
password = "change-this"
```

`settings/local.toml`:

```toml
debug = true
secret_key = "development-secret-key"

[database]
name = "mydb_dev"
password = "local-password"
```

### 4. Starting the Application

```bash
# Start in local environment (default)
cargo run --bin runserver

# Explicitly specify environment
REINHARDT_ENV=local cargo run --bin runserver
REINHARDT_ENV=staging cargo run --bin runserver
REINHARDT_ENV=production cargo run --bin runserver
```

---

## Settings Priority

Settings are merged based on the **priority value** of each source. Sources with
higher priority values override sources with lower priority values.

### Priority Values by Source Type

The table below lists sources in order of priority (highest to lowest):

| Source Type            | Priority Value | Description                                                               |
| ---------------------- | -------------- | ------------------------------------------------------------------------- |
| `EnvSource`            | 100            | **Highest priority** - Environment variables (override all other sources) |
| `DotEnvSource`         | 90             | .env file variables (override TOML and defaults)                          |
| `TomlFileSource`       | 50             | TOML configuration files (override defaults and low-priority env vars)    |
| `LowPriorityEnvSource` | 40             | Low-priority environment variables (overridden by TOML files)             |
| `DefaultSource`        | 0              | **Lowest priority** - Default values defined in code                      |

### Common Configuration Pattern

The documentation examples use this configuration pattern:

```rust
SettingsBuilder::new()
    .add_source(LowPriorityEnvSource::new().with_prefix("REINHARDT_"))
    .add_source(TomlFileSource::new("settings/base.toml"))
    .add_source(TomlFileSource::new("settings/local.toml"))
    .build()
```

**Priority order for this pattern (highest priority first):**

1. **Environment-Specific TOML Files** (`local.toml`, `staging.toml`,
   `production.toml`) - Priority 50
2. **Base TOML File** (`base.toml`) - Priority 50
3. **Low-Priority Environment Variables** (`REINHARDT_` prefix with
   `LowPriorityEnvSource`) - Priority 40
4. **Default Values** (defined in code) - Priority 0

**Note:** When multiple sources have the same priority (e.g., `base.toml` and
`local.toml`), sources added later override earlier ones.

### Environment Variable Priority Options

You can choose between two environment variable sources depending on your needs:

#### Option 1: Low-Priority Environment Variables (Recommended for Development)

```rust
.add_source(LowPriorityEnvSource::new().with_prefix("REINHARDT_"))
```

- Priority: 40 (lower than TOML files)
- TOML files override environment variables
- Useful for setting defaults that can be overridden by configuration files

#### Option 2: High-Priority Environment Variables (Recommended for Production)

```rust
.add_source(EnvSource::new().with_prefix("REINHARDT_"))
```

- Priority: 100 (higher than TOML files)
- Environment variables override TOML files
- Useful for production deployments where environment variables should take
  precedence

### Example with LowPriorityEnvSource

```toml
# base.toml
debug = false
secret_key = "base-secret"

[database]
host = "localhost"
port = 5432
```

```toml
# local.toml
debug = true

[database]
host = "127.0.0.1"
```

```bash
# Environment variable (using LowPriorityEnvSource)
export REINHARDT_DATABASE_PORT=5433
```

**Result with LowPriorityEnvSource:**

- `debug = true` (local.toml overrides base.toml)
- `secret_key = "base-secret"` (not defined in local.toml, uses base.toml value)
- `database.host = "127.0.0.1"` (local.toml overrides base.toml)
- `database.port = 5432` (base.toml overrides environment variable because TOML
  has higher priority than LowPriorityEnvSource)

**Result if using EnvSource instead:**

- `debug = true` (local.toml value)
- `secret_key = "base-secret"` (base.toml value)
- `database.host = "127.0.0.1"` (local.toml value)
- `database.port = 5433` (environment variable overrides TOML because EnvSource
  has higher priority)

---

## Configuration File Structure

### Base Configuration (`base.toml`)

Common settings for all environments:

```toml
debug = false
secret_key = "CHANGE_THIS_IN_PRODUCTION"
allowed_hosts = ["localhost", "127.0.0.1"]

[database]
engine = "postgresql"
host = "localhost"
port = 5432
name = "mydb"
user = "postgres"
password = "CHANGE_THIS"
max_connections = 10
min_connections = 2

[static]
url = "/static/"
root = "static/"

[media]
url = "/media/"
root = "media/"

[logging]
level = "info"
format = "json"
```

### Local Development Environment (`local.toml`)

Settings for development:

```toml
debug = true
secret_key = "dev-secret-key-not-for-production"

[database]
name = "mydb_dev"
password = "local-dev-password"
max_connections = 5

[logging]
level = "debug"
format = "pretty"
```

### Staging Environment (`staging.toml`)

```toml
debug = false
secret_key = "staging-secret-key"
allowed_hosts = ["staging.example.com"]

[database]
host = "staging-db.example.com"
name = "mydb_staging"
password = "staging-db-password"
max_connections = 20
ssl_mode = "require"

[logging]
level = "info"
format = "json"
```

### Production Environment (`production.toml`)

```toml
debug = false
secret_key = "production-secret-key-from-secret-manager"
allowed_hosts = ["www.example.com", "api.example.com"]

[database]
host = "prod-db.example.com"
port = 5432
name = "mydb_production"
user = "app_user"
password = "production-db-password"
max_connections = 50
min_connections = 10
ssl_mode = "require"

[static]
url = "https://cdn.example.com/static/"

[media]
url = "https://cdn.example.com/media/"

[logging]
level = "warn"
format = "json"
```

---

## Configuration via Environment Variables

You can configure settings using environment variables. The priority of
environment variables depends on which source you use:

- **`LowPriorityEnvSource`** (Priority: 40): TOML files override environment
  variables
- **`EnvSource`** (Priority: 100): Environment variables override TOML files

The examples in this document use `LowPriorityEnvSource`, which means **TOML
files have higher priority** than environment variables.

### Naming Convention

Environment variable names follow this convention:

```
REINHARDT_<KEY_NAME>
REINHARDT_<SECTION_NAME>_<KEY_NAME>
```

### Examples

```bash
# Top-level settings
export REINHARDT_DEBUG=true
export REINHARDT_SECRET_KEY="env-secret-key"

# Settings within sections
export REINHARDT_DATABASE_HOST=localhost
export REINHARDT_DATABASE_PORT=5432
export REINHARDT_DATABASE_NAME=mydb
export REINHARDT_DATABASE_USER=postgres
export REINHARDT_DATABASE_PASSWORD=mypassword

# Nested settings
export REINHARDT_LOGGING_LEVEL=debug
export REINHARDT_STATIC_URL=/static/
```

### Choosing Between EnvSource and LowPriorityEnvSource

#### Use `LowPriorityEnvSource` when:

- You want TOML files to override environment variables
- You're in development and want configuration files to take precedence
- You want environment variables as fallback defaults

```rust
SettingsBuilder::new()
    .add_source(LowPriorityEnvSource::new().with_prefix("REINHARDT_"))
    .add_source(TomlFileSource::new("settings/base.toml"))
    .add_source(TomlFileSource::new("settings/local.toml"))
    .build()
// Result: local.toml > base.toml > environment variables > defaults
```

#### Use `EnvSource` when:

- You want environment variables to override TOML files
- You're in production and want environment variables to take precedence
- You're deploying to cloud platforms (Heroku, AWS, etc.) that use environment
  variables

```rust
SettingsBuilder::new()
    .add_source(DefaultSource::new())
    .add_source(TomlFileSource::new("settings/base.toml"))
    .add_source(EnvSource::new().with_prefix("REINHARDT_"))
    .build()
// Result: environment variables > base.toml > defaults
```

### Managing Configuration with Environment Variables Only

If you want to manage configuration using only environment variables without
TOML files, use `EnvSource`:

```rust
use reinhardt_conf::settings::prelude::*;
use reinhardt_conf::settings::Settings;

pub fn get_settings() -> Settings {
    SettingsBuilder::new()
        .add_source(DefaultSource::new())
        .add_source(EnvSource::new().with_prefix("REINHARDT_"))
        .build()
        .expect("Failed to build settings")
        .into_typed()
        .expect("Failed to convert settings")
}
```

---

## Loading and Accessing Settings

### Basic Loading

`src/config/settings.rs`:

```rust
use reinhardt_conf::settings::prelude::*;
use reinhardt_conf::settings::Settings;
use std::env;
use std::path::PathBuf;

pub fn get_settings() -> Settings {
    let profile_str = env::var("REINHARDT_ENV").unwrap_or_else(|_| "local".to_string());
    let profile = Profile::parse(&profile_str);

    let settings_dir = PathBuf::from("settings");

    SettingsBuilder::new()
        .profile(profile)
        .add_source(LowPriorityEnvSource::new().with_prefix("REINHARDT_"))
        .add_source(TomlFileSource::new(settings_dir.join("base.toml")))
        .add_source(TomlFileSource::new(settings_dir.join(format!("{}.toml", profile_str))))
        .build()
        .expect("Failed to build settings")
        .into_typed()
        .expect("Failed to convert settings")
}
```

### Using in Application

```rust
use crate::config::settings::get_settings;

fn main() {
    let settings = get_settings();

    println!("Debug mode: {}", settings.debug);
    println!("Database host: {}", settings.database.host);
    println!("Database port: {}", settings.database.port);
}
```

---

## Security Best Practices

### 1. `.gitignore` Configuration

`settings/.gitignore`:

```gitignore
# Actual configuration files are not tracked by Git
*.toml

# Only commit example files
!*.example.toml
```

Project root `.gitignore`:

```gitignore
# Configuration files
settings/*.toml
!settings/*.example.toml

# Environment variable files
.env
.env.local
.env.*.local
```

### 2. Creating Example Files

```bash
# Create example files from actual configuration files
cd settings
cp base.toml base.example.toml
cp local.toml local.example.toml
cp staging.toml staging.example.toml
cp production.toml production.example.toml
```

**Important:** Remove sensitive information from `*.example.toml` files:

```toml
# ❌ Don't include production secrets
secret_key = "actual-production-secret-key"
password = "real-database-password"

# ✅ Use placeholders
secret_key = "CHANGE_THIS_IN_PRODUCTION"
password = "CHANGE_THIS"
```

### 3. Managing Sensitive Information

For production environments, it's recommended not to write sensitive information
directly in TOML files:

#### Option 1: Environment Variables (lower priority)

```bash
export REINHARDT_SECRET_KEY="actual-secret-from-vault"
export REINHARDT_DATABASE_PASSWORD="actual-db-password"
```

#### Option 2: Secret Management Systems

```rust
// TODO: Once the secrets module of reinhardt-conf is implemented,
// add implementation examples for loading secrets from
// AWS Secrets Manager, HashiCorp Vault, Azure Key Vault, etc.
```

---

## Adding Custom Settings

### 1. Extending Settings Structures

`src/config/settings.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomSettings {
    pub api_timeout: u64,
    pub max_retries: u32,
    pub features: FeatureFlags,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    pub enable_graphql: bool,
    pub enable_websockets: bool,
}
```

### 2. Adding to TOML Files

`settings/base.toml`:

```toml
[custom]
api_timeout = 30
max_retries = 3

[custom.features]
enable_graphql = false
enable_websockets = false
```

`settings/local.toml`:

```toml
[custom]
api_timeout = 60

[custom.features]
enable_graphql = true
enable_websockets = true
```

---

## Advanced Usage

### Using Multiple File Sources

```rust
pub fn get_settings() -> Settings {
    SettingsBuilder::new()
        .add_source(LowPriorityEnvSource::new().with_prefix("REINHARDT_"))
        .add_source(TomlFileSource::new("settings/base.toml"))
        .add_source(TomlFileSource::new("settings/database.toml"))
        .add_source(TomlFileSource::new("settings/cache.toml"))
        .add_source(TomlFileSource::new("settings/local.toml"))
        .build()
        .expect("Failed to build settings")
        .into_typed()
        .expect("Failed to convert settings")
}
```

### Using JSON Format

```rust
use reinhardt_conf::settings::sources::JsonFileSource;

pub fn get_settings() -> Settings {
    SettingsBuilder::new()
        .add_source(JsonFileSource::new("settings/base.json"))
        .add_source(JsonFileSource::new("settings/local.json"))
        .build()
        .expect("Failed to build settings")
        .into_typed()
        .expect("Failed to convert settings")
}
```

### Implementing Custom Sources

```rust
use reinhardt_conf::settings::sources::{ConfigSource, SourceError};
use indexmap::IndexMap;
use serde_json::Value;

struct RemoteConfigSource {
    url: String,
}

impl ConfigSource for RemoteConfigSource {
    fn load(&self) -> Result<IndexMap<String, Value>, SourceError> {
        // Implementation to fetch configuration from remote server
        // Example: HTTP request to fetch JSON configuration
        todo!("Implement remote config loading - fetch from {}", self.url)
    }

    fn priority(&self) -> u8 {
        // Custom priority between TOML files and high-priority env vars
        // 0 = lowest, 100 = highest
        // 75 = higher than TOML (50) but lower than EnvSource (100)
        75
    }

    fn description(&self) -> String {
        format!("Remote configuration from: {}", self.url)
    }
}
```

**Usage:**

```rust
pub fn get_settings() -> Settings {
    SettingsBuilder::new()
        .add_source(DefaultSource::new())
        .add_source(TomlFileSource::new("settings/base.toml"))
        .add_source(RemoteConfigSource { url: "https://config.example.com/api/settings".to_string() })
        .build()
        .expect("Failed to build settings")
        .into_typed()
        .expect("Failed to convert settings")
}
// Priority order: RemoteConfigSource (75) > TomlFileSource (50) > DefaultSource (0)
```

---

## Troubleshooting

### Issue 1: Configuration File Not Found

**Error:**

```
Failed to build settings: Source error in 'TOML file: settings/base.toml'
```

**Cause:**

- `settings/` directory doesn't exist
- Required TOML files haven't been created

**Solution:**

```bash
ls settings/
# Verify base.toml, local.toml, etc. exist

# If they don't exist, copy from example files
cp settings/base.example.toml settings/base.toml
```

### Issue 2: TOML Syntax Error

**Error:**

```
Failed to build settings: Parse error
```

**Cause:**

- Incorrect TOML syntax
- Invalid quotes
- Duplicate section names

**Solution:**

```bash
# Use TOML validation tool
cargo install toml-cli
toml get settings/base.toml .

# Use online validation tool
# https://www.toml-lint.com/
```

### Issue 3: Type Conversion Error

**Error:**

```
Failed to deserialize key 'debug': invalid type
```

**Cause:**

- Setting value type doesn't match expected type

**Solution:**

```toml
# ✅ Correct types
debug = true          # Boolean
port = 5432           # Integer
timeout = 30.5        # Float
name = "mydb"         # String

# ❌ Wrong types
debug = "true"        # String (Boolean expected)
port = "5432"         # String (Integer expected)
```

### Issue 4: Missing Required Field

**Error:**

```
Failed to convert to Settings: missing field 'secret_key'
```

**Cause:**

- Required field not defined in TOML file

**Solution:**

```toml
# Add required field to base.toml
secret_key = "your-secret-key-here"
```

### Issue 5: Environment Variables Not Working

**Cause:**

- TOML files have higher priority than environment variables
- Environment variable name is incorrect

**Solution:**

```bash
# 1. Remove the setting from TOML file (if you want environment variable to take priority)

# 2. Verify environment variable name
echo $REINHARDT_DATABASE_HOST

# 3. Use correct prefix
export REINHARDT_DATABASE_HOST=localhost  # ✅ Correct
export DATABASE_HOST=localhost            # ❌ No prefix
```

---

## Practical Examples

### Example 1: Multi-Tenant Configuration

```toml
# base.toml
[[tenants]]
name = "tenant1"
database = "tenant1_db"
schema = "public"

[[tenants]]
name = "tenant2"
database = "tenant2_db"
schema = "public"
```

```rust
#[derive(Debug, Deserialize)]
pub struct TenantConfig {
    pub name: String,
    pub database: String,
    pub schema: String,
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub tenants: Vec<TenantConfig>,
}
```

### Example 2: Feature Flag Management

**Note:** This example shows runtime feature flags (application settings). For
compile-time feature flags (Cargo features), see the
[Feature Flags Guide](FEATURE_FLAGS.md).

```toml
# base.toml
[features]
enable_graphql = false
enable_websockets = false
enable_admin_panel = true
enable_api_versioning = true

# local.toml (all features enabled during development)
[features]
enable_graphql = true
enable_websockets = true
```

### Example 3: External Service Configuration

```toml
# base.toml
[services.redis]
host = "localhost"
port = 6379
db = 0

[services.rabbitmq]
host = "localhost"
port = 5672
vhost = "/"

[services.elasticsearch]
hosts = ["http://localhost:9200"]
```

---

## Performance and Best Practices

### 1. Initialize Settings Only Once

```rust
use once_cell::sync::Lazy;

static SETTINGS: Lazy<Settings> = Lazy::new(|| {
    get_settings()
});

fn main() {
    // Settings are loaded only once, even if accessed multiple times
    println!("{}", SETTINGS.debug);
    println!("{}", SETTINGS.database.host);
}
```

### 2. Settings Validation

```rust
pub fn get_settings() -> Settings {
    let settings = SettingsBuilder::new()
        // ... add sources ...
        .build()
        .expect("Failed to build settings")
        .into_typed()
        .expect("Failed to convert settings");

    // Validate settings
    validate_settings(&settings).expect("Invalid settings");

    settings
}

fn validate_settings(settings: &Settings) -> Result<(), String> {
    if settings.secret_key == "CHANGE_THIS_IN_PRODUCTION" {
        return Err("Secret key not set!".to_string());
    }

    if settings.database.max_connections < settings.database.min_connections {
        return Err("max_connections must be >= min_connections".to_string());
    }

    Ok(())
}
```

### 3. Type-Safe Settings Access

```rust
// ❌ Access via string key (not type-safe)
let debug = settings.get("debug").unwrap();

// ✅ Access via struct field (type-safe)
let debug = settings.debug;
```

---

## Summary

Reinhardt's settings system is:

- ✅ **Flexible**: Supports multiple sources including TOML, JSON, and
  environment variables
- ✅ **Secure**: Protects sensitive information with `.gitignore`, type-safe
  access
- ✅ **Efficient**: No recompilation required, settings loaded only once
- ✅ **Environment-Aware**: Separates configuration for local, staging,
  production, etc.
- ✅ **Extensible**: Easy to add custom sources and configuration items

## Next Steps

- [Getting Started Guide](GETTING_STARTED.md) - Basic usage of Reinhardt
- [Example Projects](../examples/) - Real project examples
- [reinhardt-conf API Documentation](https://docs.rs/reinhardt-conf) -
  Detailed API specification

---

**Questions or feedback:** https://github.com/kent8192/reinhardt-web/issues
