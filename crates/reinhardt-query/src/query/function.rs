//! Function DDL statement builders
//!
//! This module provides builders for function-related DDL statements:
//!
//! - [`CreateFunctionStatement`]: CREATE FUNCTION statement
//! - [`AlterFunctionStatement`]: ALTER FUNCTION statement
//! - [`DropFunctionStatement`]: DROP FUNCTION statement
//!
//! ## Backend Support
//!
//! | Backend | CREATE | ALTER | DROP |
//! |---------|--------|-------|------|
//! | PostgreSQL | ✅ | ✅ | ✅ |
//! | MySQL | ✅ | ✅ | ✅ |
//! | SQLite | ❌ (panics) | ❌ (panics) | ❌ (panics) |
//! | CockroachDB | ✅ | ✅ | ✅ |
//!
//! ## PostgreSQL vs MySQL Differences
//!
//! - **Language**: PostgreSQL supports PL/pgSQL, MySQL has its own syntax
//! - **Dollar quotes**: PostgreSQL uses `$$` for function body, MySQL uses BEGIN/END
//! - **Parameter placeholders**: PostgreSQL uses `$1, $2`, MySQL uses parameters directly

pub mod alter_function;
pub mod create_function;
pub mod drop_function;

pub use alter_function::AlterFunctionStatement;
pub use create_function::CreateFunctionStatement;
pub use drop_function::DropFunctionStatement;
