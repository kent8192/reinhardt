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
//! use reinhardt_conf::Settings;
//!
//! // Create settings with defaults and override specific fields
//! let mut settings = Settings::default();
//! settings.secret_key = "my-secret-key".to_string();
//! settings.debug = false;
//! settings.allowed_hosts = vec!["example.com".to_string()];
//!
//! assert!(!settings.debug);
//! assert_eq!(settings.allowed_hosts[0], "example.com");
//! ```
//!
//! ## Architecture
//!
//! Key modules in this crate:
//!
//! - [`settings`]: Core settings management with layered configuration and builder pattern
//!   - `builder`: `SettingsBuilder` for composing config from multiple sources
//!   - `sources`: Configuration source adapters (files, env vars, `.env` files)
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

#![cfg_attr(not(feature = "settings"), allow(unused_imports))]

pub mod settings;

// Re-export commonly used types at the crate root for convenience
#[cfg(feature = "settings")]
pub use settings::{DatabaseConfig, MiddlewareConfig, Settings, TemplateConfig};
