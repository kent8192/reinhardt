//! File cache and metadata management
//!
//! This module provides in-memory caching of file metadata for fast lookup
//! during request handling.

pub mod etag;
pub mod file_cache;

pub use etag::generate_etag;
pub use file_cache::{CompressedVariants, FileCache, FileMetadata};
