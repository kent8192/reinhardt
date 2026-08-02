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
		let mut loaded = source.all_migrations().await?;
		loaded.sort_by(|left, right| {
			left.app_label
				.cmp(&right.app_label)
				.then_with(|| left.name.cmp(&right.name))
		});
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

		let mut migration_keys: Vec<MigrationKey> = migrations.keys().cloned().collect();
		migration_keys.sort_by(Self::compare_keys);
		let mut replacement_owners: HashMap<MigrationKey, Vec<MigrationKey>> = HashMap::new();
		for (key, migration) in &migrations {
			for (app, name) in &migration.replaces {
				replacement_owners
					.entry(MigrationKey::new(app, name))
					.or_default()
					.push(key.clone());
			}
		}
		for key in &migration_keys {
			let migration = migrations
				.get(key)
				.expect("collected catalog key must have a migration");
			let mut dependencies = migration.dependencies.clone();
			dependencies.sort();
			for (dependency_app, dependency_name) in dependencies {
				let dependency = MigrationKey::new(dependency_app, dependency_name);
				let replacement_count = replacement_owners.get(&dependency).map_or(0, Vec::len);
				if !migrations.contains_key(&dependency) && replacement_count != 1 {
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
		let safe_pre_start_ancestors = start_key
			.as_ref()
			.map(|key| self.same_app_ancestors_before_start(key, app))
			.unwrap_or_default();
		let mut selected = HashSet::new();
		let mut current = end_key.clone();
		let mut external_ancestry_paths = Vec::new();

		loop {
			selected.insert(current.clone());
			external_ancestry_paths.extend(
				self.external_ancestry_paths(&current, app)?
					.into_iter()
					.map(|path| (current.clone(), path)),
			);
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
			let direct_parents = app_parents
				.iter()
				.filter(|candidate| {
					!app_parents
						.iter()
						.any(|other| other != *candidate && self.depends_on(other, candidate))
				})
				.cloned()
				.collect();
			app_parents = direct_parents;

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

		for (key, migration) in &self.migrations {
			if !selected.contains(key)
				&& migration
					.replaces
					.iter()
					.any(|(replacement_app, replacement_name)| {
						selected.contains(&MigrationKey::new(replacement_app, replacement_name))
					}) {
				return Err(MigrationError::InvalidMigration(format!(
					"Cannot squash range: {} already replaces a selected migration",
					key
				)));
			}
		}

		for selected_key in &selected {
			for child in self.graph.get_dependents(selected_key) {
				if child.app_label == app
					&& !selected.contains(child)
					&& !self.is_descendant_of(child, &end_key)
				{
					return Err(MigrationError::InvalidMigration(format!(
						"Cannot squash range: {} branches from selected migration {}",
						child, selected_key
					)));
				}
			}
		}

		for (dependency_app, dependency_name) in &external_dependencies {
			let dependency = MigrationKey::new(dependency_app, dependency_name);
			if let Some(selected_ancestor) =
				self.first_reachable_selected_dependency(&dependency, &selected)
			{
				return Err(MigrationError::InvalidMigration(format!(
					"Cannot squash range: external dependency {} depends on selected migration {}",
					dependency, selected_ancestor
				)));
			}
		}

		for migration in &migrations {
			let conditional_dependencies = migration
				.optional_dependencies
				.iter()
				.map(|dependency| {
					MigrationKey::new(&dependency.app_label, &dependency.migration_name)
				})
				.chain(migration.swappable_dependencies.iter().map(|dependency| {
					MigrationKey::new(&dependency.default_app, &dependency.migration_name)
				}));
			for dependency in conditional_dependencies {
				if !selected.contains(&dependency)
					&& self.migrations.contains_key(&dependency)
					&& let Some(selected_ancestor) =
						self.first_reachable_selected_dependency(&dependency, &selected)
				{
					return Err(MigrationError::InvalidMigration(format!(
						"Cannot squash range: conditional dependency {} of {} depends on selected migration {}",
						dependency, migration.app_label, selected_ancestor
					)));
				}
			}
		}

		external_ancestry_paths.sort_by(|(left_origin, left_path), (right_origin, right_path)| {
			Self::compare_keys(left_origin, right_origin).then_with(|| {
				left_path
					.iter()
					.map(MigrationKey::id)
					.cmp(right_path.iter().map(MigrationKey::id))
			})
		});
		let unsafe_external_path = external_ancestry_paths.iter().find(|(_, path)| {
			path.last()
				.is_none_or(|ancestor| !safe_pre_start_ancestors.contains(ancestor))
		});
		if let Some((origin, path)) = unsafe_external_path {
			let rendered_path = path
				.iter()
				.map(MigrationKey::id)
				.collect::<Vec<_>>()
				.join(" -> ");
			return Err(MigrationError::InvalidMigration(format!(
				"Migration ancestry for {} crosses external-app nodes: {}",
				origin, rendered_path
			)));
		}

		Ok(SquashRange {
			migrations,
			external_dependencies: external_dependencies.into_iter().collect(),
		})
	}

	fn compare_keys(left: &MigrationKey, right: &MigrationKey) -> std::cmp::Ordering {
		left.app_label
			.cmp(&right.app_label)
			.then_with(|| left.name.cmp(&right.name))
	}

	fn external_ancestry_paths(
		&self,
		origin: &MigrationKey,
		app: &str,
	) -> Result<Vec<Vec<MigrationKey>>> {
		let mut paths = Vec::new();
		let mut dependencies = self
			.graph
			.get_dependencies(origin)
			.unwrap_or_default()
			.iter()
			.filter(|dependency| dependency.app_label != app)
			.cloned()
			.collect::<Vec<_>>();
		dependencies.sort_by(Self::compare_keys);

		for dependency in dependencies {
			let mut stack = vec![(dependency.clone(), vec![dependency])];
			let mut visited = HashSet::new();
			while let Some((current, path)) = stack.pop() {
				if !visited.insert(current.clone()) {
					continue;
				}
				if current.app_label == app {
					paths.push(path);
					continue;
				}
				let mut parents = self
					.graph
					.get_dependencies(&current)
					.unwrap_or_default()
					.to_vec();
				parents.sort_by(Self::compare_keys);
				for parent in parents.into_iter().rev() {
					let mut next_path = path.clone();
					next_path.push(parent.clone());
					stack.push((parent, next_path));
				}
			}
		}

		Ok(paths)
	}

	fn first_reachable_selected_dependency(
		&self,
		start: &MigrationKey,
		selected: &HashSet<MigrationKey>,
	) -> Option<MigrationKey> {
		let mut stack = vec![start.clone()];
		let mut visited = HashSet::new();
		let mut reachable_selected = Vec::new();

		while let Some(current) = stack.pop() {
			if !visited.insert(current.clone()) {
				continue;
			}
			if selected.contains(&current) {
				reachable_selected.push(current.clone());
			}
			let mut dependencies = self
				.graph
				.get_dependencies(&current)
				.unwrap_or_default()
				.to_vec();
			dependencies.sort_by(Self::compare_keys);
			stack.extend(dependencies.into_iter().rev());
		}

		reachable_selected.sort_by(Self::compare_keys);
		reachable_selected.into_iter().next()
	}

	fn depends_on(&self, start: &MigrationKey, target: &MigrationKey) -> bool {
		let mut stack = vec![start.clone()];
		let mut visited = HashSet::new();
		while let Some(current) = stack.pop() {
			if !visited.insert(current.clone()) {
				continue;
			}
			if &current == target {
				return true;
			}
			stack.extend(
				self.graph
					.get_dependencies(&current)
					.unwrap_or_default()
					.iter()
					.cloned(),
			);
		}
		false
	}

	fn is_descendant_of(&self, start: &MigrationKey, ancestor: &MigrationKey) -> bool {
		let mut stack = vec![ancestor.clone()];
		let mut visited = HashSet::new();
		while let Some(current) = stack.pop() {
			if !visited.insert(current.clone()) {
				continue;
			}
			if &current == start {
				return true;
			}
			stack.extend(self.graph.get_dependents(&current).into_iter().cloned());
		}
		false
	}

	fn same_app_ancestors_before_start(
		&self,
		start: &MigrationKey,
		app: &str,
	) -> HashSet<MigrationKey> {
		let mut stack = self
			.graph
			.get_dependencies(start)
			.unwrap_or_default()
			.iter()
			.filter(|dependency| dependency.app_label == app)
			.cloned()
			.collect::<Vec<_>>();
		stack.sort_by(Self::compare_keys);
		stack.reverse();

		let mut visited = HashSet::new();
		let mut same_app_ancestors = HashSet::new();
		while let Some(current) = stack.pop() {
			if !visited.insert(current.clone()) {
				continue;
			}
			if current.app_label == app {
				same_app_ancestors.insert(current.clone());
			}
			let mut dependencies = self
				.graph
				.get_dependencies(&current)
				.unwrap_or_default()
				.to_vec();
			dependencies.sort_by(Self::compare_keys);
			stack.extend(dependencies.into_iter().rev());
		}
		same_app_ancestors
	}
}
