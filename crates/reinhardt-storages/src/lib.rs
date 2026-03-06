//! # reinhardt-storages
//!
//! Cloud storage backend abstraction for Reinhardt framework.
//!
//! This crate provides a unified interface for interacting with multiple cloud storage
//! providers (Amazon S3, Google Cloud Storage, Azure Blob Storage) and local file system.
//!
//! ## Features
//!
//! - **Unified API**: Single `` `StorageBackend` `` trait for all storage providers
//! - **Async I/O**: All operations are asynchronous using Tokio
//! - **Feature Flags**: Enable only the backends you need
//! - **Presigned URLs**: Generate temporary access URLs for secure file sharing
//!
//! ## Example
//!
//! ```rust,no_run
//! use reinhardt_storages::{StorageBackend, create_storage, StorageConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Load configuration from environment
//!     let config = StorageConfig::from_env()?;
//!
//!     // Create storage backend
//!     let storage = create_storage(config).await?;
//!
//!     // Save a file
//!     let data = b"Hello, world!";
//!     storage.save("example.txt", data).await?;
//!
//!     // Read a file
//!     let content = storage.open("example.txt").await?;
//!
//!     Ok(())
//! }
//! ```

pub mod backend;
pub mod backends;
pub mod config;
pub mod error;
pub mod factory;

pub use backend::StorageBackend;
pub use config::StorageConfig;
pub use error::{Result, StorageError};
pub use factory::create_storage;
