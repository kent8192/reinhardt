//! Storage backend implementations.

#[cfg(feature = "s3")]
pub mod s3;

#[cfg(feature = "gcs")]
pub mod gcs;

#[cfg(feature = "azure")]
pub mod azure;

#[cfg(feature = "local")]
pub mod local;
