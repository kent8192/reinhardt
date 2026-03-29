//! Type-safe field lookup system with compile-time validation
//!
//! This module provides a field lookup API with full type safety enforced at compile time.
//!
//! # Examples
//!
//! ```rust,ignore
//! use reinhardt_db::orm::prelude::*;
//! # use reinhardt_core::types::DateTime;
//!
//! #[model(app_label = "users", table_name = "users")]
//! #[derive(QueryFields)]
//! struct User {
//!     id: i64,
//!     email: String,
//!     age: i32,
//!     created_at: DateTime,
//! }
//!
//! // Type-safe queries
//! QuerySet::<User>::new()
//!     .filter(User::email().lower().contains("example.com"))
//!     .filter(User::age().gte(18))
//!     .filter(User::created_at().year().eq(2025));
//!
//! // Compile errors for invalid operations
//! // User::age().contains(18);  // ERROR: contains() only available for String
//! // User::email().year();       // ERROR: year() only available for DateTime
//! ```

pub mod aggregate;
pub mod comparison;
pub mod compiler;
mod field;
mod lookup;
mod traits;

pub use compiler::QueryFieldCompiler;
pub use field::Field;
pub use lookup::{Lookup, LookupType, LookupValue};
pub use traits::{Comparable, Date, DateTime, DateTimeType, NumericType, StringType};

/// Helper type for building GROUP BY clauses with type-safe field selection
///
/// This type collects field paths from type-safe field selectors and converts
/// them to SQL GROUP BY clause.
///
/// # Examples
///
/// ```no_run
/// QuerySet::<User>::new()
///     .group_by(|f| {
///         GroupByFields::new()
///             .add(&f.user_id)
///             .add(&f.category)
///     })
#[derive(Debug, Clone)]
pub struct GroupByFields {
	paths: Vec<String>,
}

impl GroupByFields {
	/// Create a new empty GROUP BY fields collection
	pub fn new() -> Self {
		Self { paths: Vec::new() }
	}

	/// Add a field to the GROUP BY clause
	///
	/// This method accepts any `Field<M, T>` and extracts its path.
	///
	/// Builder pattern method - returns Self for chaining, not implementing std::ops::Add
	#[allow(clippy::should_implement_trait)]
	pub fn add<M: super::Model, T>(mut self, field: &Field<M, T>) -> Self {
		self.paths.push(field.path().join("."));
		self
	}

	/// Build the final list of field paths for SQL generation
	pub(crate) fn build(self) -> Vec<String> {
		self.paths
	}
}

impl Default for GroupByFields {
	fn default() -> Self {
		Self::new()
	}
}
