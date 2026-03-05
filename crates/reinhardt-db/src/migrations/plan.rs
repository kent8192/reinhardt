//! Migration execution plan

use super::{Migration, Result};

/// Transaction mode for migration execution
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::plan::TransactionMode;
///
/// let mode = TransactionMode::PerMigration;
/// assert_eq!(mode.name(), "Per Migration");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransactionMode {
	/// Each migration in its own transaction
	PerMigration,
	/// All migrations in a single transaction
	All,
	/// No transactions (migrations run without transaction protection)
	None,
	/// Respect individual migration atomic flags
	#[default]
	RespectMigrationFlags,
}

impl TransactionMode {
	/// Get mode name
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::plan::TransactionMode;
	///
	/// assert_eq!(TransactionMode::PerMigration.name(), "Per Migration");
	/// assert_eq!(TransactionMode::All.name(), "All in One");
	/// ```
	pub fn name(&self) -> &str {
		match self {
			TransactionMode::PerMigration => "Per Migration",
			TransactionMode::All => "All in One",
			TransactionMode::None => "No Transactions",
			TransactionMode::RespectMigrationFlags => "Respect Migration Flags",
		}
	}
}

/// Migration execution plan
#[derive(Debug, Clone)]
pub struct MigrationPlan {
	pub migrations: Vec<Migration>,
	/// Transaction mode for execution
	pub transaction_mode: TransactionMode,
	/// Whether to continue on error
	pub continue_on_error: bool,
}

impl MigrationPlan {
	/// Create a new empty migration plan
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::MigrationPlan;
	///
	/// let plan = MigrationPlan::new();
	/// assert_eq!(plan.migrations.len(), 0);
	/// ```
	pub fn new() -> Self {
		Self {
			migrations: Vec::new(),
			transaction_mode: TransactionMode::default(),
			continue_on_error: false,
		}
	}

	/// Set transaction mode
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{MigrationPlan, plan::TransactionMode};
	///
	/// let plan = MigrationPlan::new()
	///     .with_transaction_mode(TransactionMode::All);
	/// assert_eq!(plan.transaction_mode, TransactionMode::All);
	/// ```
	pub fn with_transaction_mode(mut self, mode: TransactionMode) -> Self {
		self.transaction_mode = mode;
		self
	}

	/// Set whether to continue on error
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::MigrationPlan;
	///
	/// let plan = MigrationPlan::new()
	///     .continue_on_error(true);
	/// assert!(plan.continue_on_error);
	/// ```
	pub fn continue_on_error(mut self, continue_on_error: bool) -> Self {
		self.continue_on_error = continue_on_error;
		self
	}
	/// Add a migration to this plan
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{Migration, MigrationPlan};
	///
	/// let migration = Migration::new("0001_initial", "myapp");
	/// let plan = MigrationPlan::new().with_migration(migration);
	///
	/// assert_eq!(plan.migrations.len(), 1);
	/// ```
	pub fn with_migration(mut self, migration: Migration) -> Self {
		self.migrations.push(migration);
		self
	}
	/// Sort migrations by dependencies (topological sort)
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{Migration, MigrationPlan};
	///
	/// let migration1 = Migration::new("0001_initial", "myapp");
	/// let migration2 = Migration::new("0002_add_field", "myapp")
	///     .add_dependency("myapp", "0001_initial");
	///
	/// let mut plan = MigrationPlan::new()
	///     .with_migration(migration2.clone())
	///     .with_migration(migration1.clone());
	///
	/// plan.sort().unwrap();
	///
	/// // After sorting, 0001 should come before 0002
	/// assert_eq!(plan.migrations[0].name, "0001_initial");
	/// assert_eq!(plan.migrations[1].name, "0002_add_field");
	/// ```
	pub fn sort(&mut self) -> Result<()> {
		// Simple topological sort
		let mut sorted = Vec::new();
		let mut remaining: Vec<_> = self.migrations.drain(..).collect();

		while !remaining.is_empty() {
			let mut found_any = false;

			let mut i = 0;
			while i < remaining.len() {
				let migration = &remaining[i];
				let all_deps_met = migration.dependencies.iter().all(|(app, name)| {
					sorted
						.iter()
						.any(|m: &Migration| m.app_label == *app && m.name == *name)
				});

				if all_deps_met {
					sorted.push(remaining.remove(i));
					found_any = true;
				} else {
					i += 1;
				}
			}

			if !found_any && !remaining.is_empty() {
				return Err(super::MigrationError::DependencyError(
					"Circular dependency detected".to_string(),
				));
			}
		}

		self.migrations = sorted;
		Ok(())
	}
}

impl Default for MigrationPlan {
	fn default() -> Self {
		Self::new()
	}
}
