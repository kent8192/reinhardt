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
//! - **Named storage registry**: file fields can resolve validated named
//!   backends with scoped process-wide activation
//! - **Collision-safe uploads**: portable normalized keys use atomic exclusive
//!   creation and bounded random suffix retries
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
//! ## Storage settings contract
//!
//! The original `[storage]` entry is the `default` storage alias. It remains
//! valid when named entries are added. Each named entry has its own backend and
//! URL expiry; an omitted `url_expiry_secs` uses the 3,600-second default. The
//! named map is deliberately one level deep, so a named backend cannot contain
//! another `named` map.
//!
//! ```toml
//! [storage]
//! backend = "local"
//! url_expiry_secs = 3600
//!
//! [storage.local]
//! base_path = "media"
//!
//! [storage.named.private_uploads]
//! backend = "local"
//! url_expiry_secs = 900
//!
//! [storage.named.private_uploads.local]
//! base_path = "private-media"
//! ```
//!
//! A file-field alias must resolve to a backend that advertises atomic
//! exclusive creation (`StorageCapabilities::exclusive_create`). The upload
//! service calls `StorageBackend::save_if_absent`; `save` remains the explicit
//! overwrite operation. The local, S3, GCS, and Azure implementations provide
//! this capability. Application startup should validate every alias before
//! activating the registry, so a missing alias or non-exclusive backend fails
//! before an upload can begin.
//!
//! Select one provider feature for an application build. For example, a local
//! deployment can use `default-features = false, features = ["local"]`, while
//! an S3 deployment can use `default-features = false, features = ["s3"]`.
//! The `all` feature is intended for compatibility or provider-matrix tests,
//! not as an application default. A settings entry still selects exactly one
//! backend through its `backend` value.
//!
//! ## Storage-backed model fields (Phase A)
//!
//! `reinhardt-db` exposes the opt-in typed `FileField` value and a generated
//! model descriptor. The application facade initializes this crate's registry
//! before model operations and holds the returned
//! `ActiveStorageRegistryGuard` for the lifetime of the application. The
//! descriptor eagerly stores the upload and returns a typed value; later
//! `Model::save` persists the logical path.
//!
//! Phase A intentionally does not provide replacement or delete cleanup. If
//! the eager object write succeeds and the subsequent database save fails, the
//! object is an orphan and must be repaired by application or operational
//! tooling. Replacement/delete lifecycle cleanup and `ImageField` validation
//! are Phase B work. Multipart parsing, form binding, and admin integration
//! are Phase C work; none of those integrations are implied by this API.
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
pub mod file_naming;
pub mod registry;
pub mod settings;
pub mod upload;

pub use backend::{StorageBackend, StorageCapabilities};
#[allow(deprecated)] // Re-export keeps the compatibility API discoverable during the 0.2 line.
pub use config::{BackendType, StorageConfig};
pub use error::{FileStorageError, Result, StorageError};
pub use factory::{create_storage, create_storage_from_settings};
pub use file_naming::{
	normalize_client_filename, validate_logical_key, validate_storage_alias,
	validate_upload_template,
};
pub use registry::{
	ActiveStorageRegistryGuard, StorageEntry, StorageRegistry, active_storage_registry,
};
#[cfg(feature = "azure")]
pub use settings::AzureStorageSettings;
#[cfg(feature = "gcs")]
pub use settings::GcsStorageSettings;
#[cfg(feature = "local")]
pub use settings::LocalStorageSettings;
pub use settings::NamedStorageSettings;
#[cfg(feature = "s3")]
pub use settings::S3StorageSettings;
pub use settings::StorageSettings;
pub use upload::{StoredFile, UploadPolicy, store_uploaded_file};
