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

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", features = ["conf"] }

# Or use a preset:
# reinhardt = { version = "0.1.0-alpha.1", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.1.0-alpha.1", features = ["full"] }      # All features
```

Then import configuration features:

```rust
use reinhardt::conf::settings::{SettingsBuilder, Settings};
use reinhardt::conf::settings::sources::ConfigSource;
```

**Note:** Configuration features are included in the `standard` and `full` feature presets.

### Optional Features

Enable specific features based on your needs:

```toml
# With async support
reinhardt = { version = "0.1.0-alpha.1", features = ["conf", "conf-settings-async"] }

# With encryption
reinhardt = { version = "0.1.0-alpha.1", features = ["conf", "conf-settings-encryption"] }

# With Vault integration
reinhardt = { version = "0.1.0-alpha.1", features = ["conf", "conf-settings-vault"] }
```

Available features:

- `conf-settings` (default): Core settings functionality
- `conf-settings-async`: Asynchronous settings operations
- `conf-settings-dynamic-redis`: Redis-backed dynamic settings
- `conf-settings-dynamic-database`: Database-backed dynamic settings
- `conf-settings-vault`: HashiCorp Vault integration
- `conf-settings-aws-secrets`: AWS Secrets Manager integration
- `conf-settings-azure-keyvault`: Azure Key Vault integration
- `conf-settings-secret-rotation`: Automatic secret rotation
- `conf-settings-encryption`: Built-in encryption for sensitive settings

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
