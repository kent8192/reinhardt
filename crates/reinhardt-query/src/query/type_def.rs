//! Custom type DDL statement builders (PostgreSQL, CockroachDB)
//!
//! This module provides builders for custom type-related DDL statements:
//!
//! - [`CreateTypeStatement`]: CREATE TYPE statement
//! - [`AlterTypeStatement`]: ALTER TYPE statement
//! - [`DropTypeStatement`]: DROP TYPE statement
//!
//! ## Backend Support
//!
//! | Backend | CREATE | ALTER | DROP |
//! |---------|--------|-------|------|
//! | PostgreSQL | ✅ | ✅ | ✅ |
//! | MySQL | ❌ (panics) | ❌ (panics) | ❌ (panics) |
//! | SQLite | ❌ (panics) | ❌ (panics) | ❌ (panics) |
//! | CockroachDB | ✅ | ✅ | ✅ |
//!
//! **Note**: Custom types are only supported by PostgreSQL and CockroachDB.
//! MySQL and SQLite will panic with a helpful error message.
//!
//! ## Supported Type Kinds
//!
//! - **ENUM**: Enumerated types with a list of values
//! - **COMPOSITE**: Composite types with named fields
//! - **DOMAIN**: Domain types with constraints on base types
//! - **RANGE**: Range types (PostgreSQL only)

pub mod alter_type;
pub mod create_type;
pub mod drop_type;

pub use alter_type::AlterTypeStatement;
pub use create_type::CreateTypeStatement;
pub use drop_type::DropTypeStatement;
