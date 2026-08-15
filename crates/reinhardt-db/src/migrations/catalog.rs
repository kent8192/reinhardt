//! Strict migration catalog loading and squash range resolution.

use super::{
	DatabaseMigrationRecorder, DependencyResolutionContext, Migration, MigrationError,
	MigrationGraph, MigrationKey, MigrationSource, ProjectState, Result,
};
use chrono::{DateTime, Utc};
use std::collections::{BTreeSet, HashMap, HashSet};

/// A migration identity paired with the time it was applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedMigration {
	/// The migration identity.
	pub key: MigrationKey,
	/// The UTC timestamp recorded after applying the migration.
	pub applied_at: DateTime<Utc>,
}

/// An immutable view of ordered migrations and their applied state.
#[derive(Debug, Clone)]
pub struct MigrationSnapshot {
	/// Selected migrations in topological order.
	pub ordered: Vec<Migration>,
	/// Applied timestamps keyed by migration identity.
	pub applied: HashMap<MigrationKey, DateTime<Utc>>,
}

/// A validated snapshot of all migrations from a migration source.
pub struct MigrationCatalog {
	migrations: HashMap<MigrationKey, Migration>,
	graph: MigrationGraph,
	raw_graph: MigrationGraph,
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
	/// Create a migration range for direct squashing.
	///
	/// Ranges created this way do not include catalog-only dependency
	/// normalization metadata. Use [`MigrationCatalog::squash_range`] when
	/// resolving replacement migrations or `__first__` dependencies.
	pub fn new(migrations: Vec<Migration>, external_dependencies: Vec<(String, String)>) -> Self {
		Self {
			migrations,
			external_dependencies,
			available_migrations: Vec::new(),
			replacement_owners: HashMap::new(),
		}
	}

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
				let replaced = MigrationKey::new(app, name);
				if replaced == *key {
					return Err(MigrationError::InvalidMigration(format!(
						"Migration {} cannot replace itself",
						key
					)));
				}
				replacement_owner_candidates
					.entry(replaced)
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
		let mut raw_graph = MigrationGraph::new();
		for key in &migration_keys {
			let migration = migrations
				.get(key)
				.expect("collected catalog key must have a migration");
			let mut graph_for_migration = MigrationGraph::new();
			graph_for_migration.add_migration_with_context(migration, context);
			let raw_dependencies = graph_for_migration
				.get_dependencies(key)
				.unwrap_or_default()
				.iter()
				.map(|dependency| {
					let dependency = if dependency.name == "__first__" {
						migrations
							.keys()
							.filter(|candidate| candidate.app_label == dependency.app_label)
							.min_by(|left, right| {
								Self::compare_migration_names(&left.name, &right.name)
							})
							.cloned()
							.ok_or_else(|| {
								MigrationError::DependencyError(format!(
									"Missing first migration for app {} required by {}",
									dependency.app_label, key
								))
							})?
					} else {
						dependency.clone()
					};
					if migrations.contains_key(&dependency) {
						Ok(dependency)
					} else if let Some(owner) = replacement_owners.get(&dependency) {
						Ok(owner.clone())
					} else {
						Err(MigrationError::DependencyError(format!(
							"Missing dependency {} required by {}",
							dependency, key
						)))
					}
				})
				.collect::<Result<Vec<_>>>()?;
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
			raw_graph.add_migration(key.clone(), raw_dependencies);
		}

		let mut cycle_nodes: Vec<String> = graph
			.detect_all_cycles()
			.into_iter()
			.chain(raw_graph.detect_all_cycles())
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
		raw_graph.topological_sort()?;

		Ok(Self {
			migrations,
			graph,
			raw_graph,
			replacement_owners,
		})
	}

	/// Return every loaded migration and its resolved raw dependencies in topological order.
	pub fn raw_ordered_migrations(&self) -> Result<Vec<(&Migration, &[MigrationKey])>> {
		self.raw_graph
			.topological_sort()?
			.into_iter()
			.map(|key| {
				let migration = self.migration(&key)?;
				let dependencies = self.raw_graph.get_dependencies(&key).unwrap_or_default();
				Ok((migration, dependencies))
			})
			.collect()
	}

	/// Build an immutable applied-state snapshot for selected applications.
	///
	/// Selecting applications includes all transitive dependencies, even when
	/// those dependencies belong to other applications. An empty selection
	/// includes the complete catalog.
	pub async fn snapshot(
		&self,
		recorder: &DatabaseMigrationRecorder,
		apps: &[String],
	) -> Result<MigrationSnapshot> {
		let known_apps: HashSet<&str> = self
			.migrations
			.keys()
			.map(|key| key.app_label.as_str())
			.collect();
		for app in apps {
			if !known_apps.contains(app.as_str()) {
				return Err(MigrationError::NotFound(format!("app {app}")));
			}
		}

		let recorded = recorder.get_applied_migrations_if_present().await?;
		let mut applied: HashMap<MigrationKey, _> = recorded
			.iter()
			.map(|record| {
				(
					MigrationKey::new(record.app.clone(), record.name.clone()),
					record.applied,
				)
			})
			.collect();
		let partial_replacements: HashSet<MigrationKey> = self
			.migrations
			.iter()
			.filter(|(key, migration)| {
				if migration.replaces.is_empty() || applied.contains_key(*key) {
					return false;
				}
				let applied_count = migration
					.replaces
					.iter()
					.filter(|(app, name)| applied.contains_key(&MigrationKey::new(app, name)))
					.count();
				applied_count > 0 && applied_count < migration.replaces.len()
			})
			.map(|(key, _)| key.clone())
			.collect();
		let raw_order = self.raw_graph.topological_sort()?;
		let ordered_keys: Vec<_> = self
			.graph
			.resolve_execution_order_with_replaces()?
			.into_iter()
			.flat_map(|key| {
				if !partial_replacements.contains(&key) {
					return vec![key];
				}
				let replaced: HashSet<_> = self.migrations[&key]
					.replaces
					.iter()
					.map(|(app, name)| MigrationKey::new(app, name))
					.collect();
				raw_order
					.iter()
					.filter(|candidate| replaced.contains(*candidate))
					.cloned()
					.collect()
			})
			.collect();
		let selected = if apps.is_empty() {
			ordered_keys.iter().cloned().collect()
		} else {
			let requested_apps: HashSet<&str> = apps.iter().map(String::as_str).collect();
			let mut selected = HashSet::new();
			let mut pending: Vec<MigrationKey> = ordered_keys
				.iter()
				.filter(|key| requested_apps.contains(key.app_label.as_str()))
				.cloned()
				.collect();

			while let Some(key) = pending.pop() {
				if !selected.insert(key.clone()) {
					continue;
				}
				pending.extend(
					self.graph
						.get_dependencies(&key)
						.unwrap_or_default()
						.iter()
						.map(|dependency| {
							self.graph
								.get_replacement(dependency)
								.cloned()
								.unwrap_or_else(|| dependency.clone())
						}),
				);
			}
			selected
		};

		let ordered = ordered_keys
			.into_iter()
			.filter(|key| selected.contains(key))
			.map(|key| {
				self.migrations
					.get(&key)
					.expect("catalog and graph keys must match")
					.clone()
			})
			.collect();
		for (key, migration) in &self.migrations {
			if !migration.replaces.is_empty()
				&& migration
					.replaces
					.iter()
					.all(|(app, name)| applied.contains_key(&MigrationKey::new(app, name)))
			{
				let applied_at = migration
					.replaces
					.iter()
					.filter_map(|(app, name)| applied.get(&MigrationKey::new(app, name)))
					.max()
					.copied()
					.expect("complete replacement history has applied records");
				applied.insert(key.clone(), applied_at);
			}
		}

		Ok(MigrationSnapshot { ordered, applied })
	}

	/// Reconstruct the project state immediately before a migration.
	pub fn state_before(&self, key: &MigrationKey) -> Result<ProjectState> {
		self.reconstruct_state(key, false, false)
	}

	/// Reconstruct the project state after replaying the resolved migration history.
	pub fn resolved_project_state(&self) -> Result<ProjectState> {
		self.replay_state(self.graph.resolve_execution_order_with_replaces()?, false)
	}

	/// Reconstruct the project state immediately after a migration.
	pub fn state_after(&self, key: &MigrationKey) -> Result<ProjectState> {
		self.reconstruct_state(key, true, false)
	}

	/// Reconstruct both states needed to inspect a rollback plan.
	///
	/// Lossy historical metadata is rejected only when the target contains a
	/// destructive operation whose reverse SQL must reconstruct that metadata.
	pub fn states_for_rollback(&self, key: &MigrationKey) -> Result<(ProjectState, ProjectState)> {
		let migration = self.migration(key)?;
		let validate_losslessness = migration.operations.iter().any(|operation| {
			matches!(
				operation,
				super::Operation::DropTable { .. }
					| super::Operation::DropColumn {
						old_definition: None,
						..
					} | super::Operation::AlterColumn {
					old_definition: None,
					..
				} | super::Operation::DropConstraint { .. }
			)
		});
		Ok((
			self.reconstruct_state(key, false, validate_losslessness)?,
			self.reconstruct_state(key, true, validate_losslessness)?,
		))
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
		fn collect_terminal_owners(
			current: &MigrationKey,
			owners: &HashMap<MigrationKey, Vec<MigrationKey>>,
			path: &mut HashSet<MigrationKey>,
			terminals: &mut HashSet<MigrationKey>,
		) -> Result<()> {
			if !path.insert(current.clone()) {
				return Err(MigrationError::CircularDependency {
					cycle: current.id(),
				});
			}
			if let Some(candidates) = owners.get(current) {
				for candidate in candidates {
					collect_terminal_owners(candidate, owners, path, terminals)?;
				}
			} else {
				terminals.insert(current.clone());
			}
			path.remove(current);
			Ok(())
		}

		let mut terminals = HashSet::new();
		for owner in owners {
			collect_terminal_owners(
				owner,
				replacement_owner_candidates,
				&mut HashSet::new(),
				&mut terminals,
			)?;
		}
		let mut terminals: Vec<_> = terminals.into_iter().collect();
		terminals.sort_by_key(MigrationKey::id);
		match terminals.as_slice() {
			[owner] => Ok(owner.clone()),
			_ => Err(MigrationError::InvalidMigration(format!(
				"Replacement {} has multiple terminal owners",
				replaced
			))),
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

	/// Return a migration from this validated catalog.
	pub fn migration(&self, key: &MigrationKey) -> Result<&Migration> {
		self.migrations
			.get(key)
			.ok_or_else(|| MigrationError::NotFound(key.id()))
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
				.raw_graph
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

		let ordered_keys = self.raw_graph.topological_sort()?;
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
			.flat_map(|key| self.raw_graph.get_dependencies(key).unwrap_or_default())
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
			for child in self.raw_graph.get_dependents(selected_key) {
				let superseded_by_selected_replacement = self
					.replacement_owners
					.get(child)
					.is_some_and(|owner| owner == selected_key);
				if !selected.contains(child)
					&& !superseded_by_selected_replacement
					&& !self.is_descendant_of(child, &end_key)
				{
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
			.raw_graph
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
					.raw_graph
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
				.raw_graph
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
				self.raw_graph
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
			stack.extend(self.raw_graph.get_dependents(&current).into_iter().cloned());
		}
		false
	}

	fn reconstruct_state(
		&self,
		target: &MigrationKey,
		include_target: bool,
		validate_losslessness: bool,
	) -> Result<ProjectState> {
		if !self.migrations.contains_key(target) {
			return Err(MigrationError::NotFound(target.id()));
		}

		let mut selected = HashSet::new();
		let mut pending = self
			.raw_graph
			.get_dependencies(target)
			.unwrap_or_default()
			.to_vec();
		if include_target {
			pending.push(target.clone());
		}

		while let Some(key) = pending.pop() {
			if !selected.insert(key.clone()) {
				continue;
			}
			pending.extend(
				self.raw_graph
					.get_dependencies(&key)
					.unwrap_or_default()
					.iter()
					.cloned(),
			);
		}

		let ordered = self
			.raw_graph
			.topological_sort()?
			.into_iter()
			.filter(|key| selected.contains(key));
		self.replay_state(ordered, validate_losslessness)
	}

	fn replay_state(
		&self,
		ordered: impl IntoIterator<Item = MigrationKey>,
		validate_losslessness: bool,
	) -> Result<ProjectState> {
		let mut state = ProjectState::default();
		for key in ordered {
			let migration = self
				.migrations
				.get(&key)
				.expect("catalog and graph keys must match");
			if migration.database_only {
				continue;
			}
			for operation in &migration.operations {
				if validate_losslessness {
					Self::validate_historical_replay_losslessness(operation)?;
				}
				operation.validate_for_partial_state(&state)?;
				operation.state_forwards(&migration.app_label, &mut state);
			}
		}
		Ok(state)
	}

	fn validate_historical_replay_losslessness(operation: &super::Operation) -> Result<()> {
		match operation {
			super::Operation::AlterTableComment { table, .. } => {
				return Err(MigrationError::InvalidMigration(format!(
					"historical state cannot preserve table comments for `{table}`"
				)));
			}
			super::Operation::CreateTable {
				name,
				columns,
				constraints,
				..
			} => {
				let declared: Vec<&str> =
					columns.iter().map(|column| column.name.as_str()).collect();
				let mut sorted = declared.clone();
				sorted.sort_unstable();
				if declared != sorted {
					return Err(MigrationError::InvalidMigration(format!(
						"historical state cannot preserve column order for table `{name}`"
					)));
				}
				if constraints.iter().any(|constraint| {
					matches!(
						constraint,
						super::Constraint::ForeignKey {
							deferrable: Some(_),
							..
						} | super::Constraint::OneToOne {
							deferrable: Some(_),
							..
						} | super::Constraint::Exclude { .. }
					)
				}) {
					return Err(MigrationError::InvalidMigration(format!(
						"historical state cannot preserve specialized constraints for table `{name}`"
					)));
				}
			}
			super::Operation::CreateInheritedTable { name, columns, .. } => {
				let declared: Vec<&str> =
					columns.iter().map(|column| column.name.as_str()).collect();
				let mut sorted = declared.clone();
				sorted.sort_unstable();
				if declared != sorted {
					return Err(MigrationError::InvalidMigration(format!(
						"historical state cannot preserve column order for inherited table `{name}`"
					)));
				}
			}
			_ => {}
		}
		Ok(())
	}

	fn same_app_ancestors_before_start(
		&self,
		start: &MigrationKey,
		app: &str,
	) -> HashSet<MigrationKey> {
		let select_ancestry = |dependencies: &[MigrationKey]| {
			let same_app = dependencies
				.iter()
				.filter(|dependency| dependency.app_label == app)
				.cloned()
				.collect::<Vec<_>>();
			if same_app.is_empty() {
				dependencies.to_vec()
			} else {
				same_app
			}
		};
		let mut stack = select_ancestry(self.raw_graph.get_dependencies(start).unwrap_or_default());
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
			let mut dependencies = select_ancestry(
				self.raw_graph
					.get_dependencies(&current)
					.unwrap_or_default(),
			);
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

	#[rstest::rstest]
	fn rejects_divergent_nested_replacement_owners() {
		// Arrange
		let original = Migration::new("0001_initial", "app");
		let mut first_replacement = Migration::new("0001_squashed_0002_a", "app");
		first_replacement.replaces = vec![("app".to_string(), "0001_initial".to_string())];
		let mut first_terminal = Migration::new("0001_squashed_0003_a", "app");
		first_terminal.replaces = vec![("app".to_string(), "0001_squashed_0002_a".to_string())];
		let mut second_replacement = Migration::new("0001_squashed_0002_b", "app");
		second_replacement.replaces = vec![("app".to_string(), "0001_initial".to_string())];
		let mut second_terminal = Migration::new("0001_squashed_0003_b", "app");
		second_terminal.replaces = vec![("app".to_string(), "0001_squashed_0002_b".to_string())];
		let context = DependencyResolutionContext::new().with_apps(["app".to_string()]);

		// Act
		let error = MigrationCatalog::from_loaded_with_context(
			vec![
				original,
				first_replacement,
				first_terminal,
				second_replacement,
				second_terminal,
			],
			&context,
		)
		.expect_err("divergent replacement branches must be rejected");

		// Assert
		assert!(matches!(
			error,
			MigrationError::InvalidMigration(message)
				if message == "Replacement app.0001_initial has multiple terminal owners"
		));
	}

	#[rstest::rstest]
	fn rejects_self_replacing_migrations() {
		let mut migration = Migration::new("0001_initial", "app");
		migration.replaces = vec![("app".to_string(), "0001_initial".to_string())];
		let context = DependencyResolutionContext::new().with_apps(["app".to_string()]);

		let error = MigrationCatalog::from_loaded_with_context(vec![migration], &context)
			.expect_err("self replacement must be rejected");

		assert!(matches!(
			error,
			MigrationError::InvalidMigration(message)
				if message == "Migration app.0001_initial cannot replace itself"
		));
	}

	#[rstest::rstest]
	fn historical_replay_rejects_lossy_table_metadata() {
		let comment = super::super::Operation::AlterTableComment {
			table: "posts".to_string(),
			comment: Some("Published posts".to_string()),
		};
		let unordered = super::super::Operation::CreateTable {
			name: "posts".to_string(),
			columns: vec![
				super::super::ColumnDefinition::new("title", super::super::FieldType::Text),
				super::super::ColumnDefinition::new("id", super::super::FieldType::Integer),
			],
			constraints: Vec::new(),
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		};
		let specialized = super::super::Operation::CreateTable {
			name: "reservations".to_string(),
			columns: vec![super::super::ColumnDefinition::new(
				"room",
				super::super::FieldType::Integer,
			)],
			constraints: vec![super::super::Constraint::Exclude {
				name: "exclude_room".to_string(),
				elements: vec![("room".to_string(), "=".to_string())],
				using: Some("gist".to_string()),
				where_clause: None,
			}],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		};

		let comment_error = MigrationCatalog::validate_historical_replay_losslessness(&comment)
			.expect_err("table comments are not represented in ProjectState");
		let order_error = MigrationCatalog::validate_historical_replay_losslessness(&unordered)
			.expect_err("column order is not represented in ProjectState");
		let constraint_error =
			MigrationCatalog::validate_historical_replay_losslessness(&specialized)
				.expect_err("specialized constraints are not represented losslessly");

		assert_eq!(
			comment_error.to_string(),
			"Invalid migration: historical state cannot preserve table comments for `posts`"
		);
		assert_eq!(
			order_error.to_string(),
			"Invalid migration: historical state cannot preserve column order for table `posts`"
		);
		assert_eq!(
			constraint_error.to_string(),
			"Invalid migration: historical state cannot preserve specialized constraints for table `reservations`"
		);
	}

	#[rstest::rstest]
	fn ordinary_column_order_is_allowed_when_no_destructive_rollback_needs_reconstruction() {
		let mut migration = Migration::new("0001_initial", "blog");
		migration.operations = vec![super::super::Operation::CreateTable {
			name: "posts".to_string(),
			columns: vec![
				super::super::ColumnDefinition::new("id", super::super::FieldType::Integer),
				super::super::ColumnDefinition::new("title", super::super::FieldType::Text),
				super::super::ColumnDefinition::new(
					"created_at",
					super::super::FieldType::DateTime,
				),
			],
			constraints: Vec::new(),
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}];
		let context = DependencyResolutionContext::new().with_apps(["blog".to_string()]);
		let catalog = MigrationCatalog::from_loaded_with_context(vec![migration], &context)
			.expect("ordinary migration must load");
		let key = MigrationKey::new("blog", "0001_initial");

		let state = catalog
			.state_after(&key)
			.expect("forward reconstruction must retain semantic column order");
		let rollback_states = catalog
			.states_for_rollback(&key)
			.expect("dropping a newly created table does not reconstruct its columns");

		assert!(state.find_model_by_table("posts").is_some());
		assert_eq!(rollback_states.1, state);
	}
}
