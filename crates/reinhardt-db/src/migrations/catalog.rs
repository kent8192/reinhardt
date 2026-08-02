//! Strict migration catalog loading and squash range resolution.

use super::{
	DependencyResolutionContext, Migration, MigrationError, MigrationGraph, MigrationKey,
	MigrationSource, Result,
};
use std::collections::{BTreeSet, HashMap, HashSet};

/// A validated snapshot of all migrations from a migration source.
pub struct MigrationCatalog {
	migrations: HashMap<MigrationKey, Migration>,
	graph: MigrationGraph,
	replacement_owners: HashMap<MigrationKey, MigrationKey>,
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
	pub(crate) available_migrations: Vec<MigrationKey>,
	pub(crate) replacement_owners: HashMap<MigrationKey, MigrationKey>,
}

impl SquashRange {
	pub(crate) fn normalize_dependency(
		&self,
		app_label: &str,
		migration_name: &str,
	) -> Result<MigrationKey> {
		let mut dependency = MigrationKey::new(app_label, migration_name);
		if self.available_migrations.is_empty() {
			return Ok(dependency);
		}
		if dependency.name == "__first__" {
			dependency = self
				.available_migrations
				.iter()
				.filter(|candidate| candidate.app_label == dependency.app_label)
				.min_by(|left, right| {
					MigrationCatalog::compare_migration_names(&left.name, &right.name)
				})
				.cloned()
				.ok_or_else(|| {
					MigrationError::DependencyError(format!(
						"Missing first migration for app {}",
						app_label
					))
				})?;
		}
		Ok(self
			.replacement_owners
			.get(&dependency)
			.cloned()
			.unwrap_or(dependency))
	}
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
		let context = DependencyResolutionContext::new()
			.with_apps(loaded.iter().map(|migration| migration.app_label.clone()));
		Self::from_loaded_with_context(loaded, &context)
	}

	/// Load and validate every migration exposed by a source with dependency settings.
	///
	/// The context resolves swappable dependencies and activates optional
	/// dependencies. Its installed applications are combined with the labels
	/// discovered from the source, so a local optional dependency is not silently
	/// omitted.
	pub async fn load_strict_with_context(
		source: &dyn MigrationSource,
		context: &DependencyResolutionContext,
	) -> Result<Self> {
		let mut loaded = source.all_migrations().await?;
		loaded.sort_by(|left, right| {
			left.app_label
				.cmp(&right.app_label)
				.then_with(|| left.name.cmp(&right.name))
		});
		let context = context
			.clone()
			.with_apps(loaded.iter().map(|migration| migration.app_label.clone()));
		Self::from_loaded_with_context(loaded, &context)
	}

	fn from_loaded_with_context(
		loaded: Vec<Migration>,
		context: &DependencyResolutionContext,
	) -> Result<Self> {
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
		let mut replacement_owner_candidates: HashMap<MigrationKey, Vec<MigrationKey>> =
			HashMap::new();
		for (key, migration) in &migrations {
			for (app, name) in &migration.replaces {
				replacement_owner_candidates
					.entry(MigrationKey::new(app, name))
					.or_default()
					.push(key.clone());
			}
		}
		let replacement_owners = replacement_owner_candidates
			.iter()
			.map(|(replaced, owners)| {
				Self::terminal_replacement_owner(replaced, owners, &replacement_owner_candidates)
					.map(|owner| (replaced.clone(), owner))
			})
			.collect::<Result<HashMap<_, _>>>()?;
		for key in &migration_keys {
			let migration = migrations
				.get(key)
				.expect("collected catalog key must have a migration");
			let mut graph_for_migration = MigrationGraph::new();
			graph_for_migration.add_migration_with_context(migration, context);
			let mut dependencies = graph_for_migration
				.get_dependencies(key)
				.map(<[MigrationKey]>::to_vec)
				.unwrap_or_default();
			dependencies.sort_by(Self::compare_keys);
			for dependency in dependencies {
				Self::resolve_graph_dependency(&dependency, &migrations, &replacement_owners, key)?;
			}
		}

		let mut graph = MigrationGraph::new();
		for key in &migration_keys {
			let migration = migrations
				.get(key)
				.expect("collected catalog key must have a migration");
			let mut graph_for_migration = MigrationGraph::new();
			graph_for_migration.add_migration_with_context(migration, context);
			let dependencies = graph_for_migration
				.get_dependencies(key)
				.unwrap_or_default()
				.iter()
				.map(|dependency| {
					Self::resolve_graph_dependency(
						dependency,
						&migrations,
						&replacement_owners,
						key,
					)
				})
				.collect::<Result<Vec<_>>>()?;
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

		Ok(Self {
			migrations,
			graph,
			replacement_owners,
		})
	}

	fn resolve_graph_dependency(
		dependency: &MigrationKey,
		migrations: &HashMap<MigrationKey, Migration>,
		replacement_owners: &HashMap<MigrationKey, MigrationKey>,
		dependent: &MigrationKey,
	) -> Result<MigrationKey> {
		let dependency = if dependency.name == "__first__" {
			migrations
				.keys()
				.filter(|candidate| candidate.app_label == dependency.app_label)
				.min_by(|left, right| Self::compare_migration_names(&left.name, &right.name))
				.cloned()
				.ok_or_else(|| {
					MigrationError::DependencyError(format!(
						"Missing first migration for app {} required by {}",
						dependency.app_label, dependent
					))
				})?
		} else {
			dependency.clone()
		};
		match replacement_owners.get(&dependency) {
			Some(owner) => Ok(owner.clone()),
			None if migrations.contains_key(&dependency) => Ok(dependency),
			_ => Err(MigrationError::DependencyError(format!(
				"Missing dependency {} required by {}",
				dependency, dependent
			))),
		}
	}

	fn terminal_replacement_owner(
		replaced: &MigrationKey,
		owners: &[MigrationKey],
		replacement_owner_candidates: &HashMap<MigrationKey, Vec<MigrationKey>>,
	) -> Result<MigrationKey> {
		let mut current = replaced.clone();
		let mut candidates = owners.to_vec();
		let mut visited = HashSet::new();
		loop {
			if !visited.insert(current.clone()) {
				return Err(MigrationError::CircularDependency {
					cycle: current.id(),
				});
			}
			let mut terminal: Vec<_> = candidates
				.iter()
				.filter(|candidate| !replacement_owner_candidates.contains_key(*candidate))
				.cloned()
				.collect();
			terminal.sort_by_key(MigrationKey::id);
			match terminal.as_slice() {
				[owner] => return Ok(owner.clone()),
				[] => {
					candidates.sort_by_key(MigrationKey::id);
					current = candidates
						.pop()
						.expect("replacement owner candidates are not empty");
					candidates = replacement_owner_candidates
						.get(&current)
						.cloned()
						.unwrap_or_default();
				}
				_ => {
					return Err(MigrationError::InvalidMigration(format!(
						"Replacement {} has multiple terminal owners",
						replaced
					)));
				}
			}
		}
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

		let external_dependencies: BTreeSet<(String, String)> = selected
			.iter()
			.flat_map(|key| self.graph.get_dependencies(key).unwrap_or_default())
			.filter(|dependency| !selected.contains(*dependency))
			.map(|dependency| (dependency.app_label.clone(), dependency.name.clone()))
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

		for selected_key in &selected {
			for child in self.graph.get_dependents(selected_key) {
				if !selected.contains(child) && !self.is_descendant_of(child, &end_key) {
					return Err(MigrationError::InvalidMigration(format!(
						"Cannot squash range: {} branches from selected migration {}",
						child, selected_key
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
			available_migrations: self.migrations.keys().cloned().collect(),
			replacement_owners: self.replacement_owners.clone(),
		})
	}

	fn compare_keys(left: &MigrationKey, right: &MigrationKey) -> std::cmp::Ordering {
		left.app_label
			.cmp(&right.app_label)
			.then_with(|| left.name.cmp(&right.name))
	}

	fn compare_migration_names(left: &str, right: &str) -> std::cmp::Ordering {
		let numeric_prefix = |name: &str| {
			name.split_once('_')
				.and_then(|(prefix, _)| prefix.parse::<u64>().ok())
		};
		match (numeric_prefix(left), numeric_prefix(right)) {
			(Some(left_prefix), Some(right_prefix)) => {
				left_prefix.cmp(&right_prefix).then_with(|| left.cmp(right))
			}
			(Some(_), None) => std::cmp::Ordering::Less,
			(None, Some(_)) => std::cmp::Ordering::Greater,
			(None, None) => left.cmp(right),
		}
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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn resolves_nested_replacement_dependencies_to_the_terminal_owner() {
		let original = Migration::new("0001_initial", "app");
		let mut older = Migration::new("0001_squashed_0002", "app");
		older.replaces = vec![("app".to_string(), "0001_initial".to_string())];
		let mut newer = Migration::new("0001_squashed_0003", "app");
		newer.replaces = vec![
			("app".to_string(), "0001_initial".to_string()),
			("app".to_string(), "0001_squashed_0002".to_string()),
		];
		let mut dependent = Migration::new("0004_dependent", "app");
		dependent.dependencies = vec![("app".to_string(), "0001_initial".to_string())];

		let context = DependencyResolutionContext::new().with_apps(["app".to_string()]);
		let catalog = MigrationCatalog::from_loaded_with_context(
			vec![original, older, newer, dependent],
			&context,
		)
		.expect("nested replacements should be valid");
		let dependent_key = MigrationKey::new("app", "0004_dependent");
		let dependencies = catalog
			.graph
			.get_dependencies(&dependent_key)
			.expect("dependent migration must be present");

		assert_eq!(
			dependencies,
			&[MigrationKey::new("app", "0001_squashed_0003")]
		);
	}
}
