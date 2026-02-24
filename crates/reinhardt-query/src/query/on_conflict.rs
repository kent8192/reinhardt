//! ON CONFLICT clause for INSERT statements.
//!
//! This module provides the [`OnConflict`] builder for constructing
//! ON CONFLICT clauses used in upsert operations.

use crate::types::{DynIden, IntoIden};

/// Target for ON CONFLICT clause.
#[derive(Debug, Clone)]
pub enum OnConflictTarget {
	/// Single column target
	Column(DynIden),
	/// Multiple columns target
	Columns(Vec<DynIden>),
}

/// Action for ON CONFLICT clause.
#[derive(Debug, Clone)]
pub enum OnConflictAction {
	/// DO NOTHING - skip the conflicting row
	DoNothing,
	/// DO UPDATE SET - update specified columns
	DoUpdate(Vec<DynIden>),
}

/// ON CONFLICT clause builder for INSERT statements.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::query::OnConflict;
///
/// // INSERT ... ON CONFLICT (id) DO NOTHING
/// let on_conflict = OnConflict::column("id").do_nothing();
///
/// // INSERT ... ON CONFLICT (id) DO UPDATE SET name, email
/// let on_conflict = OnConflict::column("id")
///     .update_columns(["name", "email"]);
/// ```
#[derive(Debug, Clone)]
pub struct OnConflict {
	pub(crate) target: OnConflictTarget,
	pub(crate) action: OnConflictAction,
}

impl OnConflict {
	/// Create an ON CONFLICT clause targeting a single column.
	pub fn column<C: IntoIden>(col: C) -> Self {
		Self {
			target: OnConflictTarget::Column(col.into_iden()),
			action: OnConflictAction::DoNothing,
		}
	}

	/// Create an ON CONFLICT clause targeting multiple columns.
	pub fn columns<I, C>(cols: I) -> Self
	where
		I: IntoIterator<Item = C>,
		C: IntoIden,
	{
		Self {
			target: OnConflictTarget::Columns(cols.into_iter().map(|c| c.into_iden()).collect()),
			action: OnConflictAction::DoNothing,
		}
	}

	/// Set the action to DO NOTHING.
	#[must_use]
	pub fn do_nothing(mut self) -> Self {
		self.action = OnConflictAction::DoNothing;
		self
	}

	/// Set the action to DO UPDATE SET with specified columns.
	#[must_use]
	pub fn update_columns<I, C>(mut self, cols: I) -> Self
	where
		I: IntoIterator<Item = C>,
		C: IntoIden,
	{
		self.action = OnConflictAction::DoUpdate(cols.into_iter().map(|c| c.into_iden()).collect());
		self
	}

	/// Consume and return self (for compatibility with reinhardt-query API pattern).
	#[must_use]
	pub fn to_owned(self) -> Self {
		self
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_on_conflict_column_do_nothing() {
		// Arrange & Act
		let on_conflict = OnConflict::column("id").do_nothing();

		// Assert
		assert!(matches!(on_conflict.target, OnConflictTarget::Column(_)));
		assert!(matches!(on_conflict.action, OnConflictAction::DoNothing));
	}

	#[test]
	fn test_on_conflict_column_update_columns() {
		// Arrange & Act
		let on_conflict = OnConflict::column("id").update_columns(["name", "email"]);

		// Assert
		assert!(matches!(on_conflict.target, OnConflictTarget::Column(_)));
		match &on_conflict.action {
			OnConflictAction::DoUpdate(cols) => {
				assert_eq!(cols.len(), 2);
			}
			_ => panic!("Expected DoUpdate action"),
		}
	}

	#[test]
	fn test_on_conflict_columns_do_nothing() {
		// Arrange & Act
		let on_conflict = OnConflict::columns(["id", "tenant_id"]).do_nothing();

		// Assert
		match &on_conflict.target {
			OnConflictTarget::Columns(cols) => {
				assert_eq!(cols.len(), 2);
			}
			_ => panic!("Expected Columns target"),
		}
		assert!(matches!(on_conflict.action, OnConflictAction::DoNothing));
	}

	#[test]
	fn test_on_conflict_columns_update_columns() {
		// Arrange & Act
		let on_conflict =
			OnConflict::columns(["id", "tenant_id"]).update_columns(["name", "email", "age"]);

		// Assert
		match &on_conflict.target {
			OnConflictTarget::Columns(cols) => {
				assert_eq!(cols.len(), 2);
			}
			_ => panic!("Expected Columns target"),
		}
		match &on_conflict.action {
			OnConflictAction::DoUpdate(cols) => {
				assert_eq!(cols.len(), 3);
			}
			_ => panic!("Expected DoUpdate action"),
		}
	}

	#[test]
	fn test_on_conflict_to_owned() {
		// Arrange & Act
		let on_conflict = OnConflict::column("id").do_nothing().to_owned();

		// Assert
		assert!(matches!(on_conflict.target, OnConflictTarget::Column(_)));
		assert!(matches!(on_conflict.action, OnConflictAction::DoNothing));
	}

	#[test]
	fn test_on_conflict_default_action_is_do_nothing() {
		// Arrange & Act
		let on_conflict = OnConflict::column("id");

		// Assert
		assert!(matches!(on_conflict.action, OnConflictAction::DoNothing));
	}
}
