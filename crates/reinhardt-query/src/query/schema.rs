//! Schema DDL statement builders
//!
//! This module provides builders for schema management operations:
//! - CREATE SCHEMA
//! - ALTER SCHEMA
//! - DROP SCHEMA

mod alter_schema;
mod create_schema;
mod drop_schema;

pub use alter_schema::{AlterSchemaOperation, AlterSchemaStatement};
pub use create_schema::CreateSchemaStatement;
pub use drop_schema::DropSchemaStatement;
