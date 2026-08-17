//! # Reinhardt Configuration Framework
//!
//! Django-inspired settings management for Rust with secrets, encryption, and audit logging.
//!
//! This crate provides a comprehensive configuration management framework for Reinhardt applications,
//! inspired by Django's settings system with additional security features.
//!
//! ## Features
//!
//! - **Multiple configuration sources**: Files, environment variables, command-line arguments
//! - **Type-safe settings**: Strong type validation with custom validators
//! - **Secrets management**: Integration with HashiCorp Vault, AWS Secrets Manager, Azure Key Vault
//! - **Encryption**: Built-in encryption for sensitive settings
//! - **Dynamic backends**: Redis and database-backed dynamic settings
//! - **Secret rotation**: Automatic secret rotation support
//! - **Audit logging**: Track all setting changes
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use reinhardt_conf::settings::builder::SettingsBuilder;
//! use reinhardt_conf::settings::sources::TomlFileSource;
//! use std::path::PathBuf;
//!
//! // Load settings from a TOML configuration file
//! let settings = SettingsBuilder::new()
//!     .add_source(TomlFileSource::new(PathBuf::from("config.toml")))
//!     .build()
//!     .unwrap();
//!
//! let debug: bool = settings.get("debug").unwrap_or(false);
//! assert!(!debug);
//! ```
//!
//! ## Architecture
//!
//! Key modules in this crate:
//!
//! - [`settings`]: Core settings management with layered configuration and builder pattern
//!   - `builder`: `SettingsBuilder` for composing config from multiple sources
//!   - `sources`: Configuration source adapters (files, env vars, `.env` files)
//!   - `interpolation`: `${VAR}` expansion for TOML configuration files
//!   - `profile`: Environment profiles (Development, Staging, Production)
//!   - `dynamic`: Redis and database-backed dynamic settings
//!   - `secrets`: Secrets management integration (Vault, AWS, Azure)
//!   - `encryption`: AES-GCM encryption for sensitive settings values
//!   - `audit`: Change tracking and audit log for setting modifications
//!   - `hot_reload`: File system watcher for live settings reload
//!
//! ## Feature Flags
//!
//! | Feature | Default | Description |
//! |---------|---------|-------------|
//! | `settings` | enabled | Core settings management and builder |
//! | `async` | disabled | Async/await support via Tokio |
//! | `dynamic-redis` | disabled | Redis-backed dynamic settings |
//! | `dynamic-database` | disabled | Database-backed dynamic settings |
//! | `vault` | disabled | HashiCorp Vault secrets integration |
//! | `aws-secrets` | disabled | AWS Secrets Manager integration |
//! | `azure-keyvault` | disabled | Azure Key Vault integration |
//! | `secret-rotation` | disabled | Automatic secret rotation support |
//! | `encryption` | disabled | AES-GCM encryption for sensitive values |
//! | `hot-reload` | disabled | Live settings reload on file change |
//! | `caching` | disabled | In-memory settings caching with TTL |

#![warn(missing_docs)]
#![cfg_attr(not(feature = "settings"), allow(unused_imports))]

pub mod settings;

// Re-export commonly used types at the crate root for convenience
#[cfg(feature = "settings")]
pub use settings::{DatabaseConfig, MiddlewareConfig};
// `TemplateConfig` is deprecated in favor of the `TemplateSettings` fragment;
// keep the re-export discoverable during the compatibility window.
#[cfg(feature = "settings")]
#[allow(deprecated)]
pub use settings::TemplateConfig;

// Re-export third-party types used in macro-generated code.
// The `#[settings(...)]` composition macro generates `ComposedSettings` impls
// that reference these types; re-exporting here avoids requiring downstream
// crates to add direct dependencies on `indexmap` and `serde_json`.
#[doc(hidden)]
pub use indexmap;
#[doc(hidden)]
pub use serde_json;

// Re-export fragment system types at the crate root
#[cfg(feature = "settings")]
pub use settings::{
	ResolvedSettings, ResolvedSettingsField, SettingsResolutionMetadata, cache::CacheSettings,
	cache::HasCacheSettings, contacts::ContactSettings, contacts::HasContactSettings,
	core_settings::CoreSettings, core_settings::HasCoreSettings, cors::CorsSettings,
	cors::HasCorsSettings, email::EmailSettings, email::HasEmailSettings,
	fragment::HasCommonSettings, fragment::HasSettings, fragment::SettingsFragment,
	i18n::HasI18nSettings, i18n::I18nSettings, logging::HasLoggingSettings,
	logging::LoggingSettings, media::HasMediaSettings, media::MediaSettings,
	migrations::HasMigrationSettings, migrations::MigrationSettings, security::HasSecuritySettings,
	security::SecuritySettings, session::HasSessionSettings, session::SessionSettings,
	static_files::HasStaticSettings, static_files::StaticSettings,
	template_settings::HasTemplateSettings, template_settings::TemplateSettings,
};
