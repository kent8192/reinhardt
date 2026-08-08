//! # reinhardt-storages
//!
//! Cloud storage backend abstraction for the Reinhardt framework.
//!
//! This crate provides a unified interface for interacting with multiple cloud storage
//! providers (Amazon S3, Google Cloud Storage, Azure Blob Storage) and local file system.
//!
//! ## Features
//!
//! - **Unified API**: Single `StorageBackend` trait for all storage providers
//! - **Settings-first configuration**: `StorageSettings` composes with the
//!   Reinhardt `#[settings]` macro
//! - **Async I/O**: All operations are asynchronous using Tokio
//! - **Feature Flags**: Enable only the backends you need
//! - **Temporary URLs**: Generate S3 presigned URLs, GCS V4 signed URLs, and
//!   Azure SAS URLs for secure file sharing
//! - **Provider boundary**: S3 uses `reinhardt-providers` for minimal HTTP and
//!   SigV4 support instead of depending on the full AWS SDK
//!
//! ## Example
//!
//! ```rust,no_run
//! use reinhardt_storages::{StorageSettings, create_storage_from_settings};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let settings: StorageSettings = toml::from_str(r#"
//! backend = "local"
//!
//! [local]
//! base_path = "media"
//! "#)?;
//!
//!     let storage = create_storage_from_settings(&settings).await?;
//!     storage.save("example.txt", b"Hello, world!").await?;
//!     let content = storage.open("example.txt").await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Compatibility
//!
//! `StorageConfig` and provider-specific `XxxConfig` structs are deprecated.
//! Use `StorageSettings` with `create_storage_from_settings()` for new code.

#![warn(missing_docs)]

pub mod backend;
pub mod backends;
pub mod config;
pub mod error;
pub mod factory;
pub mod settings;

pub use backend::{StorageBackend, StorageCapabilities};
#[allow(deprecated)] // Re-export keeps the compatibility API discoverable during the 0.2 line.
pub use config::{BackendType, StorageConfig};
pub use error::{Result, StorageError};
pub use factory::{create_storage, create_storage_from_settings};
#[cfg(feature = "azure")]
pub use settings::AzureStorageSettings;
#[cfg(feature = "gcs")]
pub use settings::GcsStorageSettings;
#[cfg(feature = "local")]
pub use settings::LocalStorageSettings;
#[cfg(feature = "s3")]
pub use settings::S3StorageSettings;
pub use settings::StorageSettings;
