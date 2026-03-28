//! Reinhardt Storages Test Suite
//!
//! Comprehensive tests for the reinhardt-storages crate covering:
//! - S3 backend (with LocalStack/MinIO via TestContainers)
//! - Local filesystem backend
//! - Configuration parsing
//! - Factory pattern
//! - Error handling

mod fixtures;
mod utils;

// Comprehensive test modules
mod config_tests;
mod factory_tests;
mod local_tests;
mod s3_tests;

// Legacy basic tests (kept for compatibility)
mod local_storage;
