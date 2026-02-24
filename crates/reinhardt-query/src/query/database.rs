//! Database DDL statement builders
//!
//! This module provides builders for database management operations:
//! - CREATE DATABASE
//! - ALTER DATABASE
//! - DROP DATABASE
//! - ATTACH DATABASE (SQLite)
//! - DETACH DATABASE (SQLite)

mod alter_database;
mod attach_database;
mod create_database;
mod detach_database;
mod drop_database;

pub use alter_database::AlterDatabaseStatement;
pub use attach_database::AttachDatabaseStatement;
pub use create_database::CreateDatabaseStatement;
pub use detach_database::DetachDatabaseStatement;
pub use drop_database::DropDatabaseStatement;
