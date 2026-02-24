//! Procedure DDL statement builders
//!
//! This module provides builders for procedure-related DDL statements:
//!
//! - [`CreateProcedureStatement`]: CREATE PROCEDURE statement
//! - [`AlterProcedureStatement`]: ALTER PROCEDURE statement
//! - [`DropProcedureStatement`]: DROP PROCEDURE statement
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
//! - **Dollar quotes**: PostgreSQL uses `$$` for procedure body, MySQL uses BEGIN/END
//! - **Parameter placeholders**: PostgreSQL uses `$1, $2`, MySQL uses parameters directly

pub mod alter_procedure;
pub mod create_procedure;
pub mod drop_procedure;

pub use alter_procedure::AlterProcedureStatement;
pub use create_procedure::CreateProcedureStatement;
pub use drop_procedure::DropProcedureStatement;
