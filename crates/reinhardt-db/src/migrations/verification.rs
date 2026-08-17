//! Migration schema contract verification.

use super::{MigrationAutodetector, MigrationKey, MigrationOperation, ProjectState};
use std::collections::{BTreeMap, BTreeSet};

/// Inputs required to verify schema migration contracts.
#[derive(Clone)]
pub struct SchemaContractState {
	/// Current model metadata state.
	pub model_state: ProjectState,
	/// State reconstructed from the resolved migration history.
	pub migration_state: ProjectState,
	/// Every migration known to the catalog.
	pub known_migrations: Vec<MigrationKey>,
	/// Migrations recorded as applied, when the recorder is available.
	pub applied_migrations: Option<BTreeSet<MigrationKey>>,
	/// `(replacement, replaced)` migration relationships.
	pub replacement_edges: Vec<(MigrationKey, MigrationKey)>,
}

/// A schema contract violation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchemaFinding {
	/// A model change has no corresponding migration operation.
	MissingMigration {
		/// Application that owns the generated migration.
		app_label: String,
		/// Stable migration-name fragment for the operation.
		name_fragment: String,
		/// Human-readable operation description.
		description: String,
	},
	/// A known migration is not covered by the applied migration history.
	UnappliedMigration {
		/// The unapplied migration.
		migration: MigrationKey,
	},
}

/// A schema check that could not be completed safely.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchemaCheckError {
	/// Migration state includes operations that cannot be represented in `ProjectState`.
	OpaqueMigrationState,
	/// Autodetection could not safely infer a migration.
	Autodetector {
		/// Application implicated by the ambiguity, when available.
		app_label: Option<String>,
	},
}

/// Schema contract verification output.
pub struct SchemaVerification {
	/// Deterministic contract violations.
	pub findings: Vec<SchemaFinding>,
	/// Checks that could not be safely evaluated.
	pub check_errors: Vec<SchemaCheckError>,
}

/// Verify migration drift and applied migration coverage.
pub fn verify_schema_contract(state: &SchemaContractState) -> SchemaVerification {
	let mut findings = Vec::new();
	let mut check_errors = Vec::new();

	if state.migration_state.has_opaque_schema_operations {
		check_errors.push(SchemaCheckError::OpaqueMigrationState);
	} else {
		match MigrationAutodetector::new(state.migration_state.clone(), state.model_state.clone())
			.try_generate_migrations()
		{
			Ok(migrations) => {
				for migration in migrations {
					for operation in migration.operations {
						findings.push(SchemaFinding::MissingMigration {
							app_label: migration.app_label.clone(),
							name_fragment: operation
								.migration_name_fragment()
								.unwrap_or_else(|| "auto".to_string()),
							description: operation.describe(),
						});
					}
				}
			}
			Err(_) => check_errors.push(SchemaCheckError::Autodetector { app_label: None }),
		}
	}

	if let Some(applied) = &state.applied_migrations {
		let covered = replacement_coverage(applied, &state.replacement_edges);
		let mut known = state.known_migrations.clone();
		known.sort();
		known.dedup();
		findings.extend(
			known
				.into_iter()
				.filter(|migration| !covered.contains(migration))
				.map(|migration| SchemaFinding::UnappliedMigration { migration }),
		);
	}

	SchemaVerification {
		findings,
		check_errors,
	}
}

fn replacement_coverage(
	applied: &BTreeSet<MigrationKey>,
	edges: &[(MigrationKey, MigrationKey)],
) -> BTreeSet<MigrationKey> {
	let mut covered = applied.clone();
	let mut replacements = BTreeMap::<MigrationKey, Vec<MigrationKey>>::new();
	for (replacement, replaced) in edges {
		replacements
			.entry(replacement.clone())
			.or_default()
			.push(replaced.clone());
	}

	loop {
		let before = covered.len();
		for (replacement, replaced) in edges {
			if covered.contains(replacement) {
				covered.insert(replaced.clone());
			}
		}
		for (replacement, replaced) in &replacements {
			if replaced.iter().all(|migration| covered.contains(migration)) {
				covered.insert(replacement.clone());
			}
		}
		if covered.len() == before {
			return covered;
		}
	}
}
