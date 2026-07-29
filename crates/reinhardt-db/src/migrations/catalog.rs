//! Strict migration catalog loading and squash range resolution.

use super::{Migration, MigrationError, MigrationGraph, MigrationKey, MigrationSource, Result};
use std::collections::{BTreeSet, HashMap, HashSet};

/// A validated snapshot of all migrations from a migration source.
pub struct MigrationCatalog {
	migrations: HashMap<MigrationKey, Migration>,
	graph: MigrationGraph,
}

impl std::fmt::Debug for MigrationCatalog {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("MigrationCatalog")
			.field("migration_count", &self.migrations.len())
			.finish_non_exhaustive()
	}
}

/// A continuous migration range selected for squashing.
#[derive(Debug)]
pub struct SquashRange {
	/// Migrations in topological order.
	pub migrations: Vec<Migration>,
	/// Dependencies that cross into the selected range.
	pub external_dependencies: Vec<(String, String)>,
}

impl MigrationCatalog {
	/// Load and validate every migration exposed by a source.
	pub async fn load_strict(source: &dyn MigrationSource) -> Result<Self> {
		let loaded = source.all_migrations().await?;
		let mut migrations = HashMap::with_capacity(loaded.len());

		for migration in loaded {
			let key = MigrationKey::new(&migration.app_label, &migration.name);
			if migrations.insert(key.clone(), migration).is_some() {
				return Err(MigrationError::InvalidMigration(format!(
					"Duplicate migration: {}",
					key
				)));
			}
		}

		for (key, migration) in &migrations {
			for (dependency_app, dependency_name) in &migration.dependencies {
				let dependency = MigrationKey::new(dependency_app, dependency_name);
				if !migrations.contains_key(&dependency) {
					return Err(MigrationError::DependencyError(format!(
						"Missing dependency {} required by {}",
						dependency, key
					)));
				}
			}
		}

		let mut graph = MigrationGraph::new();
		for (key, migration) in &migrations {
			let dependencies = migration
				.dependencies
				.iter()
				.map(|(app, name)| MigrationKey::new(app, name))
				.collect();
			let replaces = migration
				.replaces
				.iter()
				.map(|(app, name)| MigrationKey::new(app, name))
				.collect();
			graph.add_migration_with_replaces(key.clone(), dependencies, replaces);
		}

		let mut cycle_nodes: Vec<String> = graph
			.detect_all_cycles()
			.into_iter()
			.flatten()
			.map(|key| key.id())
			.collect();
		cycle_nodes.sort();
		cycle_nodes.dedup();
		if !cycle_nodes.is_empty() {
			return Err(MigrationError::CircularDependency {
				cycle: cycle_nodes.join(", "),
			});
		}
		graph.topological_sort()?;

		Ok(Self { migrations, graph })
	}

	/// Resolve an exact migration name or an unambiguous name prefix.
	pub fn resolve_unique_prefix(&self, app: &str, prefix: &str) -> Result<MigrationKey> {
		let exact = MigrationKey::new(app, prefix);
		if self.migrations.contains_key(&exact) {
			return Ok(exact);
		}

		let app_exists = self
			.migrations
			.keys()
			.any(|key| key.app_label.as_str() == app);
		if !app_exists {
			return Err(MigrationError::NotFound(format!("app {}", app)));
		}

		let mut candidates: Vec<&str> = self
			.migrations
			.keys()
			.filter(|key| key.app_label == app && key.name.starts_with(prefix))
			.map(|key| key.name.as_str())
			.collect();
		candidates.sort_unstable();

		match candidates.as_slice() {
			[] => Err(MigrationError::NotFound(format!("{}.{}", app, prefix))),
			[name] => Ok(MigrationKey::new(app, *name)),
			_ => Err(MigrationError::InvalidMigration(format!(
				"Ambiguous migration prefix '{}' for app '{}'; candidates: {}",
				prefix,
				app,
				candidates.join(", ")
			))),
		}
	}

	/// Resolve a continuous, single-app ancestor range.
	pub fn squash_range(&self, app: &str, start: Option<&str>, end: &str) -> Result<SquashRange> {
		let end_key = self.resolve_unique_prefix(app, end)?;
		let start_key = start
			.map(|prefix| self.resolve_unique_prefix(app, prefix))
			.transpose()?;
		let mut selected = HashSet::new();
		let mut current = end_key.clone();

		loop {
			selected.insert(current.clone());
			if start_key.as_ref() == Some(&current) {
				break;
			}

			let mut app_parents: Vec<MigrationKey> = self
				.graph
				.get_dependencies(&current)
				.unwrap_or_default()
				.iter()
				.filter(|dependency| dependency.app_label == app)
				.cloned()
				.collect();
			app_parents.sort_by(|left, right| left.name.cmp(&right.name));

			match app_parents.as_slice() {
				[] if start_key.is_some() => {
					let start_key = start_key.as_ref().expect("checked above");
					return Err(MigrationError::InvalidMigration(format!(
						"{} is not an ancestor of {}",
						start_key, end_key
					)));
				}
				[] => break,
				[parent] => current = parent.clone(),
				_ => {
					let parents = app_parents
						.iter()
						.map(|parent| parent.name.as_str())
						.collect::<Vec<_>>()
						.join(", ");
					return Err(MigrationError::InvalidMigration(format!(
						"Ambiguous migration ancestry for {}; parents: {}",
						current, parents
					)));
				}
			}
		}

		let ordered_keys = self.graph.topological_sort()?;
		let migrations: Vec<Migration> = ordered_keys
			.into_iter()
			.filter(|key| selected.contains(key))
			.map(|key| {
				self.migrations
					.get(&key)
					.expect("catalog and graph keys must match")
					.clone()
			})
			.collect();

		let external_dependencies: BTreeSet<(String, String)> = migrations
			.iter()
			.flat_map(|migration| migration.dependencies.iter())
			.filter(|(dependency_app, dependency_name)| {
				!selected.contains(&MigrationKey::new(dependency_app, dependency_name))
			})
			.cloned()
			.collect();

		Ok(SquashRange {
			migrations,
			external_dependencies: external_dependencies.into_iter().collect(),
		})
	}
}
