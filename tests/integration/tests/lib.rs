//! Reinhardt Integration Tests
//!
//! Integration tests for the Reinhardt framework functionality.

#[path = "fixtures/mongodb_fixtures.rs"]
pub mod mongodb_fixtures;

#[path = "document/crud_operations.rs"]
pub mod document_crud;

#[path = "field/indexes.rs"]
pub mod field_indexes;

#[path = "field/validation.rs"]
pub mod field_validation;

#[path = "integration/mongodb_backend.rs"]
pub mod mongodb_backend;
