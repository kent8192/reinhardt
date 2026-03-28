//! SQL query builder module.
//!
//! This module re-exports [`reinhardt_query`] for building type-safe SQL queries
//! targeting PostgreSQL, MySQL, and SQLite.
//!
//! # Availability
//!
//! Requires `database` feature.
//!
//! # Examples
//!
//! ```rust,no_run
//! # #[cfg(feature = "database")]
//! use reinhardt::query::prelude::*;
//!
//! # #[cfg(feature = "database")]
//! # {
//! let mut stmt = Query::select();
//! stmt.column(ColumnRef::asterisk())
//!     .from("users")
//!     .and_where(Expr::col("active").eq(true));
//!
//! let builder = PostgresQueryBuilder::new();
//! let (sql, values) = builder.build_select(&stmt);
//! # }
//! ```

#[cfg(feature = "database")]
pub use reinhardt_query::*;
