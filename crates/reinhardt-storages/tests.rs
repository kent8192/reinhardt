//! Reinhardt Storages Test Suite
//!
//! Comprehensive tests for the reinhardt-storages crate covering:
//! - S3 backend (with wiremock mock S3 server)
//! - Local filesystem backend
//! - Configuration parsing
//! - Factory pattern
//! - Error handling

mod fixtures;
mod utils;

// Comprehensive test modules
mod config_tests;
mod backend_contract_tests;
mod factory_tests;
#[cfg(feature = "gcs")]
mod gcs_tests;
mod local_tests;
mod registry_tests;
mod s3_tests;
mod settings_tests;
#[cfg(feature = "azure")]
mod azure_tests;

// Legacy basic tests (kept for compatibility)
mod local_storage;
