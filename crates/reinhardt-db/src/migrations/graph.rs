//! Migration dependency graph
//!
//! This module provides the MigrationGraph structure for managing migration dependencies
//! and determining execution order through topological sorting.
//!
//! # Example
//!
//! ```rust
//! use reinhardt_db::migrations::graph::{MigrationGraph, MigrationKey};
//!
//! let mut graph = MigrationGraph::new();
//!
//! // Add migrations with dependencies
//! let key1 = MigrationKey::new("myapp", "0001_initial");
//! let key2 = MigrationKey::new("myapp", "0002_add_field");
//!
//! graph.add_migration(key1.clone(), vec![]);
//! graph.add_migration(key2.clone(), vec![key1.clone()]);
//!
//! // Get execution order
//! let order = graph.topological_sort().unwrap();
//! assert_eq!(order.len(), 2);
//! assert_eq!(order[0], key1);
//! assert_eq!(order[1], key2);
//! ```

use super::dependency::{DependencyResolutionContext, DependencyResolver, MigrationDependency};
use super::{Migration, MigrationError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

/// Key identifying a migration (app_label, migration_name)
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::graph::MigrationKey;
///
/// let key = MigrationKey::new("auth", "0001_initial");
/// assert_eq!(key.app_label, "auth");
/// assert_eq!(key.name, "0001_initial");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MigrationKey {
	/// The app label.
	pub app_label: String,
	/// The name.
	pub name: String,
}

impl MigrationKey {
	/// Create a new migration key
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::graph::MigrationKey;
	///
	/// let key = MigrationKey::new("users", "0001_initial");
	/// assert_eq!(key.app_label, "users");
	/// assert_eq!(key.name, "0001_initial");
	/// ```
	pub fn new(app_label: impl Into<String>, name: impl Into<String>) -> Self {
		Self {
			app_label: app_label.into(),
			name: name.into(),
		}
	}

	/// Get a string representation of this key
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::graph::MigrationKey;
	///
	/// let key = MigrationKey::new("auth", "0001_initial");
	/// assert_eq!(key.id(), "auth.0001_initial");
	/// ```
	pub fn id(&self) -> String {
		format!("{}.{}", self.app_label, self.name)
	}
}

impl std::fmt::Display for MigrationKey {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.id())
	}
}

/// Migration graph node
#[derive(Debug, Clone)]
pub struct MigrationNode {
	/// The key.
	pub key: MigrationKey,
	/// The dependencies.
	pub dependencies: Vec<MigrationKey>,
	/// The replaces.
	pub replaces: Vec<MigrationKey>,
}

impl MigrationNode {
	/// Create a new migration node
	pub fn new(key: MigrationKey, dependencies: Vec<MigrationKey>) -> Self {
		Self {
			key,
			dependencies,
			replaces: Vec::new(),
		}
	}

	/// Create a new migration node with replaces
	pub fn with_replaces(
		key: MigrationKey,
		dependencies: Vec<MigrationKey>,
		replaces: Vec<MigrationKey>,
	) -> Self {
		Self {
			key,
			dependencies,
			replaces,
		}
	}
}

/// Migration dependency graph
///
/// Manages migration dependencies and determines execution order.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::graph::{MigrationGraph, MigrationKey};
///
/// let mut graph = MigrationGraph::new();
///
/// let initial = MigrationKey::new("users", "0001_initial");
/// let add_field = MigrationKey::new("users", "0002_add_email");
///
/// graph.add_migration(initial.clone(), vec![]);
/// graph.add_migration(add_field.clone(), vec![initial.clone()]);
///
/// let order = graph.topological_sort().unwrap();
/// assert_eq!(order.len(), 2);
/// ```
pub struct MigrationGraph {
	nodes: HashMap<MigrationKey, MigrationNode>,
}

impl MigrationGraph {
	/// Create a new empty migration graph
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::graph::MigrationGraph;
	///
	/// let graph = MigrationGraph::new();
	/// assert!(graph.is_empty());
	/// ```
	pub fn new() -> Self {
		Self {
			nodes: HashMap::new(),
		}
	}

	/// Add a migration to the graph
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::graph::{MigrationGraph, MigrationKey};
	///
	/// let mut graph = MigrationGraph::new();
	/// let key = MigrationKey::new("auth", "0001_initial");
	///
	/// graph.add_migration(key.clone(), vec![]);
	/// assert!(graph.has_migration(&key));
	/// ```
	pub fn add_migration(&mut self, key: MigrationKey, dependencies: Vec<MigrationKey>) {
		let node = MigrationNode::new(key.clone(), dependencies);
		self.nodes.insert(key, node);
	}

	/// Check if a migration exists in the graph
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::graph::{MigrationGraph, MigrationKey};
	///
	/// let mut graph = MigrationGraph::new();
	/// let key = MigrationKey::new("auth", "0001_initial");
	///
	/// assert!(!graph.has_migration(&key));
	/// graph.add_migration(key.clone(), vec![]);
	/// assert!(graph.has_migration(&key));
	/// ```
	pub fn has_migration(&self, key: &MigrationKey) -> bool {
		self.nodes.contains_key(key)
	}

	/// Get a migration node
	pub fn get_node(&self, key: &MigrationKey) -> Option<&MigrationNode> {
		self.nodes.get(key)
	}

	/// Check if the graph is empty
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::graph::{MigrationGraph, MigrationKey};
	///
	/// let mut graph = MigrationGraph::new();
	/// assert!(graph.is_empty());
	///
	/// graph.add_migration(MigrationKey::new("auth", "0001_initial"), vec![]);
	/// assert!(!graph.is_empty());
	/// ```
	pub fn is_empty(&self) -> bool {
		self.nodes.is_empty()
	}

	/// Get the number of migrations in the graph
	pub fn len(&self) -> usize {
		self.nodes.len()
	}

	/// Get all migrations in the graph
	pub fn all_migrations(&self) -> Vec<&MigrationKey> {
		self.nodes.keys().collect()
	}

	/// Get direct dependencies of a migration
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::graph::{MigrationGraph, MigrationKey};
	///
	/// let mut graph = MigrationGraph::new();
	/// let key1 = MigrationKey::new("auth", "0001_initial");
	/// let key2 = MigrationKey::new("auth", "0002_add_field");
	///
	/// graph.add_migration(key1.clone(), vec![]);
	/// graph.add_migration(key2.clone(), vec![key1.clone()]);
	///
	/// let deps = graph.get_dependencies(&key2).unwrap();
	/// assert_eq!(deps.len(), 1);
	/// assert_eq!(deps[0], key1);
	/// ```
	pub fn get_dependencies(&self, key: &MigrationKey) -> Option<&[MigrationKey]> {
		self.nodes.get(key).map(|node| node.dependencies.as_slice())
	}

	/// Get all migrations that depend on this migration
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::graph::{MigrationGraph, MigrationKey};
	///
	/// let mut graph = MigrationGraph::new();
	/// let key1 = MigrationKey::new("auth", "0001_initial");
	/// let key2 = MigrationKey::new("auth", "0002_add_field");
	///
	/// graph.add_migration(key1.clone(), vec![]);
	/// graph.add_migration(key2.clone(), vec![key1.clone()]);
	///
	/// let dependents = graph.get_dependents(&key1);
	/// assert_eq!(dependents.len(), 1);
	/// assert_eq!(dependents[0], &key2);
	/// ```
	pub fn get_dependents(&self, key: &MigrationKey) -> Vec<&MigrationKey> {
		self.nodes
			.iter()
			.filter(|(_, node)| node.dependencies.contains(key))
			.map(|(k, _)| k)
			.collect()
	}

	/// Perform topological sort to determine migration execution order
	///
	/// Returns migrations in the order they should be executed.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::graph::{MigrationGraph, MigrationKey};
	///
	/// let mut graph = MigrationGraph::new();
	///
	/// let key1 = MigrationKey::new("auth", "0001_initial");
	/// let key2 = MigrationKey::new("auth", "0002_add_field");
	/// let key3 = MigrationKey::new("auth", "0003_alter_field");
	///
	/// graph.add_migration(key1.clone(), vec![]);
	/// graph.add_migration(key2.clone(), vec![key1.clone()]);
	/// graph.add_migration(key3.clone(), vec![key2.clone()]);
	///
	/// let order = graph.topological_sort().unwrap();
	/// assert_eq!(order.len(), 3);
	/// assert_eq!(order[0], key1);
	/// assert_eq!(order[1], key2);
	/// assert_eq!(order[2], key3);
	/// ```
	pub fn topological_sort(&self) -> Result<Vec<MigrationKey>> {
		// Calculate in-degree for each node
		let mut in_degree: HashMap<MigrationKey, usize> = HashMap::new();

		for key in self.nodes.keys() {
			in_degree.entry(key.clone()).or_insert(0);
		}

		for node in self.nodes.values() {
			for dep in &node.dependencies {
				// Only count dependencies that are within the graph.
				// Dependencies outside the graph are assumed to be already applied.
				if self.nodes.contains_key(dep) {
					*in_degree.entry(node.key.clone()).or_insert(0) += 1;
				}
			}
		}

		// Find all nodes with in-degree 0 (no dependencies within the graph)
		// Sort deterministically by (app_label, name) to ensure consistent ordering
		let mut zero_degree: Vec<MigrationKey> = in_degree
			.iter()
			.filter(|&(_, &degree)| degree == 0)
			.map(|(key, _)| key.clone())
			.collect();
		zero_degree.sort_by(|a, b| {
			a.app_label
				.cmp(&b.app_label)
				.then_with(|| a.name.cmp(&b.name))
		});
		let mut queue: VecDeque<MigrationKey> = zero_degree.into_iter().collect();

		let mut result = Vec::new();

		while let Some(key) = queue.pop_front() {
			result.push(key.clone());

			// Reduce in-degree for all dependents and collect newly freed nodes
			let mut newly_freed = Vec::new();
			for (other_key, node) in &self.nodes {
				if node.dependencies.contains(&key)
					&& let Some(degree) = in_degree.get_mut(other_key)
				{
					*degree -= 1;
					if *degree == 0 {
						newly_freed.push(other_key.clone());
					}
				}
			}
			// Sort newly freed nodes for deterministic BFS order
			newly_freed.sort_by(|a, b| {
				a.app_label
					.cmp(&b.app_label)
					.then_with(|| a.name.cmp(&b.name))
			});
			queue.extend(newly_freed);
		}

		// Check for circular dependencies
		if result.len() != self.nodes.len() {
			let remaining: Vec<String> = self
				.nodes
				.keys()
				.filter(|k| !result.contains(k))
				.map(|k| k.id())
				.collect();

			return Err(MigrationError::CircularDependency {
				cycle: format!("Circular dependency detected: {}", remaining.join(", ")),
			});
		}

		Ok(result)
	}

	/// Get leaf nodes (migrations with no dependents)
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::graph::{MigrationGraph, MigrationKey};
	///
	/// let mut graph = MigrationGraph::new();
	///
	/// let key1 = MigrationKey::new("auth", "0001_initial");
	/// let key2 = MigrationKey::new("auth", "0002_add_field");
	///
	/// graph.add_migration(key1.clone(), vec![]);
	/// graph.add_migration(key2.clone(), vec![key1.clone()]);
	///
	/// let leaves = graph.get_leaf_nodes();
	/// assert_eq!(leaves.len(), 1);
	/// assert_eq!(leaves[0], &key2);
	/// ```
	pub fn get_leaf_nodes(&self) -> Vec<&MigrationKey> {
		self.nodes
			.keys()
			.filter(|key| self.get_dependents(key).is_empty())
			.collect()
	}

	/// Get leaf nodes for a specific app
	///
	/// Returns all leaf nodes (migrations with no dependents within the graph)
	/// that belong to the specified app label.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::graph::{MigrationGraph, MigrationKey};
	///
	/// let mut graph = MigrationGraph::new();
	///
	/// let key1 = MigrationKey::new("auth", "0001_initial");
	/// let key2 = MigrationKey::new("auth", "0002_add_field");
	/// let key3 = MigrationKey::new("users", "0001_initial");
	///
	/// graph.add_migration(key1.clone(), vec![]);
	/// graph.add_migration(key2.clone(), vec![key1.clone()]);
	/// graph.add_migration(key3.clone(), vec![]);
	///
	/// let auth_leaves = graph.get_leaf_nodes_for_app("auth");
	/// assert_eq!(auth_leaves.len(), 1);
	/// assert_eq!(auth_leaves[0], &key2);
	/// ```
	pub fn get_leaf_nodes_for_app(&self, app_label: &str) -> Vec<&MigrationKey> {
		let mut leaves: Vec<&MigrationKey> = self
			.get_leaf_nodes()
			.into_iter()
			.filter(|key| key.app_label == app_label)
			.collect();
		leaves.sort_by(|a, b| a.name.cmp(&b.name));
		leaves
	}

	/// Detect migration conflicts (apps with multiple leaf nodes)
	///
	/// Returns a map of app labels to their conflicting leaf nodes.
	/// Only apps with 2 or more leaf nodes are included.
	/// Results are sorted by leaf name for deterministic output.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::graph::{MigrationGraph, MigrationKey};
	///
	/// let mut graph = MigrationGraph::new();
	///
	/// let key1 = MigrationKey::new("auth", "0001_initial");
	/// let key2a = MigrationKey::new("auth", "0002_add_field");
	/// let key2b = MigrationKey::new("auth", "0002_add_index");
	///
	/// graph.add_migration(key1.clone(), vec![]);
	/// graph.add_migration(key2a.clone(), vec![key1.clone()]);
	/// graph.add_migration(key2b.clone(), vec![key1.clone()]);
	///
	/// let conflicts = graph.detect_conflicts();
	/// assert_eq!(conflicts.len(), 1);
	/// assert!(conflicts.contains_key("auth"));
	/// assert_eq!(conflicts["auth"].len(), 2);
	/// ```
	pub fn detect_conflicts(&self) -> HashMap<String, Vec<&MigrationKey>> {
		let leaves = self.get_leaf_nodes();
		let mut app_leaves: HashMap<String, Vec<&MigrationKey>> = HashMap::new();

		for leaf in leaves {
			app_leaves
				.entry(leaf.app_label.clone())
				.or_default()
				.push(leaf);
		}

		// Sort leaves by name for deterministic output and filter to conflicts only
		let mut conflicts: HashMap<String, Vec<&MigrationKey>> = HashMap::new();
		for (app, mut leaves) in app_leaves {
			if leaves.len() >= 2 {
				leaves.sort_by(|a, b| a.name.cmp(&b.name));
				conflicts.insert(app, leaves);
			}
		}

		conflicts
	}

	/// Get root nodes (migrations with no dependencies)
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::graph::{MigrationGraph, MigrationKey};
	///
	/// let mut graph = MigrationGraph::new();
	///
	/// let key1 = MigrationKey::new("auth", "0001_initial");
	/// let key2 = MigrationKey::new("auth", "0002_add_field");
	///
	/// graph.add_migration(key1.clone(), vec![]);
	/// graph.add_migration(key2.clone(), vec![key1.clone()]);
	///
	/// let roots = graph.get_root_nodes();
	/// assert_eq!(roots.len(), 1);
	/// assert_eq!(roots[0], &key1);
	/// ```
	pub fn get_root_nodes(&self) -> Vec<&MigrationKey> {
		self.nodes
			.values()
			.filter(|node| node.dependencies.is_empty())
			.map(|node| &node.key)
			.collect()
	}

	/// Remove a migration from the graph
	pub fn remove_migration(&mut self, key: &MigrationKey) {
		self.nodes.remove(key);
	}

	/// Clear all migrations from the graph
	pub fn clear(&mut self) {
		self.nodes.clear();
	}

	/// Add a migration with replaces information
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::graph::{MigrationGraph, MigrationKey};
	///
	/// let mut graph = MigrationGraph::new();
	/// let squashed = MigrationKey::new("auth", "0001_squashed_0003");
	/// let old1 = MigrationKey::new("auth", "0001_initial");
	/// let old2 = MigrationKey::new("auth", "0002_add_field");
	///
	/// graph.add_migration_with_replaces(
	///     squashed.clone(),
	///     vec![],
	///     vec![old1.clone(), old2.clone()],
	/// );
	/// ```
	pub fn add_migration_with_replaces(
		&mut self,
		key: MigrationKey,
		dependencies: Vec<MigrationKey>,
		replaces: Vec<MigrationKey>,
	) {
		let node = MigrationNode::with_replaces(key.clone(), dependencies, replaces);
		self.nodes.insert(key, node);
	}

	/// Find a path between two migration states
	///
	/// Uses breadth-first search to find the shortest path from `from` to `to`.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::graph::{MigrationGraph, MigrationKey};
	///
	/// let mut graph = MigrationGraph::new();
	/// let key1 = MigrationKey::new("auth", "0001_initial");
	/// let key2 = MigrationKey::new("auth", "0002_add_field");
	/// let key3 = MigrationKey::new("auth", "0003_alter_field");
	///
	/// graph.add_migration(key1.clone(), vec![]);
	/// graph.add_migration(key2.clone(), vec![key1.clone()]);
	/// graph.add_migration(key3.clone(), vec![key2.clone()]);
	///
	/// let path = graph.find_migration_path(&key1, &key3).unwrap();
	/// assert_eq!(path.len(), 3);
	/// assert_eq!(path[0], key1);
	/// assert_eq!(path[1], key2);
	/// assert_eq!(path[2], key3);
	/// ```
	pub fn find_migration_path(
		&self,
		from: &MigrationKey,
		to: &MigrationKey,
	) -> Result<Vec<MigrationKey>> {
		use std::collections::VecDeque;

		if from == to {
			return Ok(vec![from.clone()]);
		}

		if !self.has_migration(from) {
			return Err(MigrationError::NodeNotFound {
				message: "Source migration not found".to_string(),
				node: from.id(),
			});
		}

		if !self.has_migration(to) {
			return Err(MigrationError::NodeNotFound {
				message: "Target migration not found".to_string(),
				node: to.id(),
			});
		}

		// BFS to find shortest path
		let mut queue = VecDeque::new();
		let mut visited = HashMap::new();
		let mut parent: HashMap<MigrationKey, MigrationKey> = HashMap::new();

		queue.push_back(from.clone());
		visited.insert(from.clone(), true);

		while let Some(current) = queue.pop_front() {
			if &current == to {
				// Reconstruct path
				let mut path = vec![to.clone()];
				let mut node = to.clone();

				while let Some(p) = parent.get(&node) {
					path.push(p.clone());
					node = p.clone();
				}

				path.reverse();
				return Ok(path);
			}

			// Visit all dependents (forward direction)
			for dependent in self.get_dependents(&current) {
				if !visited.contains_key(dependent) {
					visited.insert(dependent.clone(), true);
					parent.insert(dependent.clone(), current.clone());
					queue.push_back(dependent.clone());
				}
			}
		}

		Err(MigrationError::DependencyError(format!(
			"No path found from {} to {}",
			from.id(),
			to.id()
		)))
	}

	/// Find a backward path (for rollback) between two migration states
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::graph::{MigrationGraph, MigrationKey};
	///
	/// let mut graph = MigrationGraph::new();
	/// let key1 = MigrationKey::new("auth", "0001_initial");
	/// let key2 = MigrationKey::new("auth", "0002_add_field");
	/// let key3 = MigrationKey::new("auth", "0003_alter_field");
	///
	/// graph.add_migration(key1.clone(), vec![]);
	/// graph.add_migration(key2.clone(), vec![key1.clone()]);
	/// graph.add_migration(key3.clone(), vec![key2.clone()]);
	///
	/// let path = graph.find_backward_path(&key3, &key1).unwrap();
	/// assert_eq!(path.len(), 3);
	/// assert_eq!(path[0], key3);
	/// assert_eq!(path[1], key2);
	/// assert_eq!(path[2], key1);
	/// ```
	pub fn find_backward_path(
		&self,
		from: &MigrationKey,
		to: &MigrationKey,
	) -> Result<Vec<MigrationKey>> {
		use std::collections::VecDeque;

		if from == to {
			return Ok(vec![from.clone()]);
		}

		if !self.has_migration(from) {
			return Err(MigrationError::NodeNotFound {
				message: "Source migration not found".to_string(),
				node: from.id(),
			});
		}

		if !self.has_migration(to) {
			return Err(MigrationError::NodeNotFound {
				message: "Target migration not found".to_string(),
				node: to.id(),
			});
		}

		// BFS to find path following dependencies backward
		let mut queue = VecDeque::new();
		let mut visited = HashMap::new();
		let mut parent: HashMap<MigrationKey, MigrationKey> = HashMap::new();

		queue.push_back(from.clone());
		visited.insert(from.clone(), true);

		while let Some(current) = queue.pop_front() {
			if &current == to {
				// Reconstruct path
				let mut path = vec![to.clone()];
				let mut node = to.clone();

				while let Some(p) = parent.get(&node) {
					path.push(p.clone());
					node = p.clone();
				}

				path.reverse();
				return Ok(path);
			}

			// Visit all dependencies (backward direction)
			if let Some(deps) = self.get_dependencies(&current) {
				for dep in deps {
					if !visited.contains_key(dep) {
						visited.insert(dep.clone(), true);
						parent.insert(dep.clone(), current.clone());
						queue.push_back(dep.clone());
					}
				}
			}
		}

		Err(MigrationError::DependencyError(format!(
			"No backward path found from {} to {}",
			from.id(),
			to.id()
		)))
	}

	/// Resolve execution order with replaces support
	///
	/// This method handles migrations that replace others (squashed migrations).
	/// When a migration replaces others, the replaced migrations are excluded
	/// from the execution order.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::graph::{MigrationGraph, MigrationKey};
	///
	/// let mut graph = MigrationGraph::new();
	/// let old1 = MigrationKey::new("auth", "0001_initial");
	/// let old2 = MigrationKey::new("auth", "0002_add_field");
	/// let squashed = MigrationKey::new("auth", "0001_squashed_0002");
	///
	/// graph.add_migration(old1.clone(), vec![]);
	/// graph.add_migration(old2.clone(), vec![old1.clone()]);
	/// graph.add_migration_with_replaces(
	///     squashed.clone(),
	///     vec![],
	///     vec![old1.clone(), old2.clone()],
	/// );
	///
	/// let order = graph.resolve_execution_order_with_replaces().unwrap();
	/// // Should contain only the squashed migration, not the old ones
	/// assert_eq!(order.len(), 1);
	/// assert_eq!(order[0], squashed);
	/// ```
	pub fn resolve_execution_order_with_replaces(&self) -> Result<Vec<MigrationKey>> {
		// Find all migrations that are replaced by others
		let mut replaced: HashSet<MigrationKey> = HashSet::new();
		let mut replacement_map: HashMap<MigrationKey, MigrationKey> = HashMap::new();

		for (key, node) in &self.nodes {
			for replaced_key in &node.replaces {
				replaced.insert(replaced_key.clone());
				replacement_map.insert(replaced_key.clone(), key.clone());
			}
		}

		// Create a filtered graph without replaced migrations
		let mut filtered_graph = MigrationGraph::new();
		for (key, node) in &self.nodes {
			if !replaced.contains(key) {
				// Transform dependencies: replace old migrations with their replacements
				let mut filtered_deps: HashSet<MigrationKey> = HashSet::new();

				for dep in &node.dependencies {
					if let Some(replacement) = replacement_map.get(dep) {
						// This dependency is replaced, use the replacement instead
						filtered_deps.insert(replacement.clone());
					} else if !replaced.contains(dep) {
						// This dependency is not replaced, keep it
						filtered_deps.insert(dep.clone());
					}
					// If dependency is replaced but not in map, skip it
				}

				filtered_graph.add_migration(key.clone(), filtered_deps.into_iter().collect());
			}
		}

		// Perform topological sort on filtered graph
		filtered_graph.topological_sort()
	}

	/// Check if a migration is replaced by another migration
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::graph::{MigrationGraph, MigrationKey};
	///
	/// let mut graph = MigrationGraph::new();
	/// let old = MigrationKey::new("auth", "0001_initial");
	/// let squashed = MigrationKey::new("auth", "0001_squashed_0002");
	///
	/// graph.add_migration_with_replaces(
	///     squashed.clone(),
	///     vec![],
	///     vec![old.clone()],
	/// );
	///
	/// assert!(graph.is_replaced(&old));
	/// assert!(!graph.is_replaced(&squashed));
	/// ```
	pub fn is_replaced(&self, key: &MigrationKey) -> bool {
		self.nodes.values().any(|node| node.replaces.contains(key))
	}

	/// Get the migration that replaces a given migration
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::graph::{MigrationGraph, MigrationKey};
	///
	/// let mut graph = MigrationGraph::new();
	/// let old = MigrationKey::new("auth", "0001_initial");
	/// let squashed = MigrationKey::new("auth", "0001_squashed_0002");
	///
	/// graph.add_migration_with_replaces(
	///     squashed.clone(),
	///     vec![],
	///     vec![old.clone()],
	/// );
	///
	/// let replacement = graph.get_replacement(&old);
	/// assert_eq!(replacement, Some(&squashed));
	/// ```
	pub fn get_replacement(&self, key: &MigrationKey) -> Option<&MigrationKey> {
		self.nodes
			.iter()
			.find(|(_, node)| node.replaces.contains(key))
			.map(|(k, _)| k)
	}

	/// Detect cycles considering replaces relationships
	///
	/// This is more comprehensive than the basic topological sort cycle detection,
	/// as it also checks for cycles that might be introduced through replaces.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::graph::{MigrationGraph, MigrationKey};
	///
	/// let mut graph = MigrationGraph::new();
	/// let key1 = MigrationKey::new("auth", "0001_initial");
	/// let key2 = MigrationKey::new("auth", "0002_add_field");
	///
	/// graph.add_migration(key1.clone(), vec![]);
	/// graph.add_migration(key2.clone(), vec![key1.clone()]);
	///
	/// let cycles = graph.detect_all_cycles();
	/// assert!(cycles.is_empty());
	/// ```
	pub fn detect_all_cycles(&self) -> Vec<Vec<MigrationKey>> {
		let mut cycles = Vec::new();
		let mut visited = HashSet::new();
		let mut rec_stack = HashSet::new();
		let mut path = Vec::new();

		for key in self.nodes.keys() {
			if !visited.contains(key) {
				self.detect_cycle_dfs(key, &mut visited, &mut rec_stack, &mut path, &mut cycles);
			}
		}

		cycles
	}

	fn detect_cycle_dfs(
		&self,
		key: &MigrationKey,
		visited: &mut HashSet<MigrationKey>,
		rec_stack: &mut HashSet<MigrationKey>,
		path: &mut Vec<MigrationKey>,
		cycles: &mut Vec<Vec<MigrationKey>>,
	) {
		visited.insert(key.clone());
		rec_stack.insert(key.clone());
		path.push(key.clone());

		if let Some(node) = self.nodes.get(key) {
			for dep in &node.dependencies {
				if !visited.contains(dep) {
					self.detect_cycle_dfs(dep, visited, rec_stack, path, cycles);
				} else if rec_stack.contains(dep) {
					// Found a cycle
					let cycle_start = path.iter().position(|k| k == dep).unwrap();
					let cycle: Vec<MigrationKey> = path[cycle_start..].to_vec();
					cycles.push(cycle);
				}
			}
		}

		path.pop();
		rec_stack.remove(key);
	}

	/// Add a migration to the graph, resolving all dependency types.
	///
	/// This method handles:
	/// - Required dependencies (standard)
	/// - Swappable dependencies (resolved via settings)
	/// - Optional dependencies (only if condition is met)
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::graph::{MigrationGraph, MigrationKey};
	/// use reinhardt_db::migrations::Migration;
	/// use reinhardt_db::migrations::dependency::{
	///     DependencyResolutionContext, SwappableDependency, OptionalDependency, DependencyCondition
	/// };
	///
	/// let mut graph = MigrationGraph::new();
	///
	/// // Set up context with custom user model
	/// let context = DependencyResolutionContext::new()
	///     .with_app("auth")
	///     .with_app("custom_auth")
	///     .with_setting("AUTH_USER_MODEL", "custom_auth.CustomUser");
	///
	/// // Create a migration with swappable dependency
	/// let migration = Migration::new("0001_create_profile", "profiles")
	///     .add_swappable_dependency(SwappableDependency::new(
	///         "AUTH_USER_MODEL",
	///         "auth",
	///         "User",
	///         "0001_initial",
	///     ));
	///
	/// graph.add_migration_with_context(&migration, &context);
	///
	/// // The dependency should be resolved to custom_auth
	/// let key = MigrationKey::new("profiles", "0001_create_profile");
	/// let deps = graph.get_dependencies(&key).unwrap();
	/// assert_eq!(deps.len(), 1);
	/// assert_eq!(deps[0].app_label, "custom_auth");
	/// ```
	pub fn add_migration_with_context(
		&mut self,
		migration: &Migration,
		context: &DependencyResolutionContext,
	) {
		let key = MigrationKey::new(migration.app_label.clone(), migration.name.clone());
		let resolver = DependencyResolver::new(context);

		// Collect all dependencies
		let mut all_dependencies: Vec<MigrationKey> = Vec::new();

		// Add required dependencies
		for (app_label, migration_name) in &migration.dependencies {
			all_dependencies.push(MigrationKey::new(app_label.clone(), migration_name.clone()));
		}

		// Resolve swappable dependencies
		for swappable in &migration.swappable_dependencies {
			let dep = MigrationDependency::Swappable(swappable.clone());
			if let Some((app_label, migration_name)) = resolver.resolve(&dep) {
				all_dependencies.push(MigrationKey::new(app_label, migration_name));
			}
		}

		// Resolve optional dependencies (only if condition is met)
		for optional in &migration.optional_dependencies {
			let dep = MigrationDependency::Optional(optional.clone());
			if let Some((app_label, migration_name)) = resolver.resolve(&dep) {
				all_dependencies.push(MigrationKey::new(app_label, migration_name));
			}
		}

		// Handle replaces
		let replaces: Vec<MigrationKey> = migration
			.replaces
			.iter()
			.map(|(app, name)| MigrationKey::new(app.clone(), name.clone()))
			.collect();

		if replaces.is_empty() {
			self.add_migration(key, all_dependencies);
		} else {
			self.add_migration_with_replaces(key, all_dependencies, replaces);
		}
	}

	/// Build a migration graph from a list of migrations.
	///
	/// This method creates a complete graph with all dependencies resolved
	/// using the provided context.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::graph::MigrationGraph;
	/// use reinhardt_db::migrations::Migration;
	/// use reinhardt_db::migrations::dependency::DependencyResolutionContext;
	///
	/// let migrations = vec![
	///     Migration::new("0001_initial", "auth"),
	///     Migration::new("0002_add_field", "auth")
	///         .add_dependency("auth", "0001_initial"),
	/// ];
	///
	/// let context = DependencyResolutionContext::new()
	///     .with_app("auth");
	///
	/// let graph = MigrationGraph::from_migrations(&migrations, &context);
	/// assert_eq!(graph.len(), 2);
	/// ```
	pub fn from_migrations(
		migrations: &[Migration],
		context: &DependencyResolutionContext,
	) -> Self {
		let mut graph = Self::new();
		for migration in migrations {
			graph.add_migration_with_context(migration, context);
		}
		graph
	}

	/// Resolve execution order considering swappable and optional dependencies.
	///
	/// This is a convenience method that combines `from_migrations` with
	/// `topological_sort` or `resolve_execution_order_with_replaces`.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::graph::MigrationGraph;
	/// use reinhardt_db::migrations::Migration;
	/// use reinhardt_db::migrations::dependency::DependencyResolutionContext;
	///
	/// let migrations = vec![
	///     Migration::new("0001_initial", "auth"),
	///     Migration::new("0001_initial", "users")
	///         .add_dependency("auth", "0001_initial"),
	/// ];
	///
	/// let context = DependencyResolutionContext::new()
	///     .with_apps(["auth", "users"]);
	///
	/// let order = MigrationGraph::resolve_execution_order(&migrations, &context).unwrap();
	/// assert_eq!(order.len(), 2);
	/// assert_eq!(order[0].app_label, "auth");
	/// ```
	pub fn resolve_execution_order(
		migrations: &[Migration],
		context: &DependencyResolutionContext,
	) -> Result<Vec<MigrationKey>> {
		let graph = Self::from_migrations(migrations, context);

		// Check if any migration has replaces
		let has_replaces = migrations.iter().any(|m| !m.replaces.is_empty());

		if has_replaces {
			graph.resolve_execution_order_with_replaces()
		} else {
			graph.topological_sort()
		}
	}
}

impl Default for MigrationGraph {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[test]
	fn test_migration_key_creation() {
		let key = MigrationKey::new("auth", "0001_initial");
		assert_eq!(key.app_label, "auth");
		assert_eq!(key.name, "0001_initial");
		assert_eq!(key.id(), "auth.0001_initial");
	}

	#[test]
	fn test_migration_key_display() {
		let key = MigrationKey::new("users", "0002_add_email");
		assert_eq!(format!("{}", key), "users.0002_add_email");
	}

	#[test]
	fn test_graph_creation() {
		let graph = MigrationGraph::new();
		assert!(graph.is_empty());
		assert_eq!(graph.len(), 0);
	}

	#[test]
	fn test_add_migration() {
		let mut graph = MigrationGraph::new();
		let key = MigrationKey::new("auth", "0001_initial");

		graph.add_migration(key.clone(), vec![]);
		assert!(!graph.is_empty());
		assert_eq!(graph.len(), 1);
		assert!(graph.has_migration(&key));
	}

	#[test]
	fn test_get_dependencies() {
		let mut graph = MigrationGraph::new();
		let key1 = MigrationKey::new("auth", "0001_initial");
		let key2 = MigrationKey::new("auth", "0002_add_field");

		graph.add_migration(key1.clone(), vec![]);
		graph.add_migration(key2.clone(), vec![key1.clone()]);

		let deps = graph.get_dependencies(&key2).unwrap();
		assert_eq!(deps.len(), 1);
		assert_eq!(deps[0], key1);
	}

	#[test]
	fn test_get_dependents() {
		let mut graph = MigrationGraph::new();
		let key1 = MigrationKey::new("auth", "0001_initial");
		let key2 = MigrationKey::new("auth", "0002_add_field");
		let key3 = MigrationKey::new("auth", "0003_alter_field");

		graph.add_migration(key1.clone(), vec![]);
		graph.add_migration(key2.clone(), vec![key1.clone()]);
		graph.add_migration(key3.clone(), vec![key2.clone()]);

		let dependents = graph.get_dependents(&key1);
		assert_eq!(dependents.len(), 1);
		assert_eq!(dependents[0], &key2);

		let dependents2 = graph.get_dependents(&key2);
		assert_eq!(dependents2.len(), 1);
		assert_eq!(dependents2[0], &key3);
	}

	#[test]
	fn test_topological_sort_simple() {
		let mut graph = MigrationGraph::new();
		let key1 = MigrationKey::new("auth", "0001_initial");
		let key2 = MigrationKey::new("auth", "0002_add_field");

		graph.add_migration(key1.clone(), vec![]);
		graph.add_migration(key2.clone(), vec![key1.clone()]);

		let order = graph.topological_sort().unwrap();
		assert_eq!(order.len(), 2);
		assert_eq!(order[0], key1);
		assert_eq!(order[1], key2);
	}

	#[test]
	fn test_topological_sort_complex() {
		let mut graph = MigrationGraph::new();

		let key1 = MigrationKey::new("auth", "0001_initial");
		let key2 = MigrationKey::new("auth", "0002_add_field");
		let key3 = MigrationKey::new("users", "0001_initial");
		let key4 = MigrationKey::new("users", "0002_add_auth");

		graph.add_migration(key1.clone(), vec![]);
		graph.add_migration(key2.clone(), vec![key1.clone()]);
		graph.add_migration(key3.clone(), vec![]);
		graph.add_migration(key4.clone(), vec![key2.clone(), key3.clone()]);

		let order = graph.topological_sort().unwrap();
		assert_eq!(order.len(), 4);

		// key1 must come before key2
		let pos1 = order.iter().position(|k| k == &key1).unwrap();
		let pos2 = order.iter().position(|k| k == &key2).unwrap();
		assert!(pos1 < pos2);

		// key2 and key3 must come before key4
		let pos4 = order.iter().position(|k| k == &key4).unwrap();
		assert!(pos2 < pos4);
		let pos3 = order.iter().position(|k| k == &key3).unwrap();
		assert!(pos3 < pos4);
	}

	#[test]
	fn test_circular_dependency_detection() {
		let mut graph = MigrationGraph::new();

		let key1 = MigrationKey::new("auth", "0001_initial");
		let key2 = MigrationKey::new("auth", "0002_add_field");

		// Create circular dependency
		graph.add_migration(key1.clone(), vec![key2.clone()]);
		graph.add_migration(key2.clone(), vec![key1.clone()]);

		let result = graph.topological_sort();
		assert!(result.is_err());

		if let Err(MigrationError::CircularDependency { cycle }) = result {
			assert!(cycle.contains("Circular dependency"));
		} else {
			panic!("Expected CircularDependency error");
		}
	}

	#[test]
	fn test_get_leaf_nodes() {
		let mut graph = MigrationGraph::new();

		let key1 = MigrationKey::new("auth", "0001_initial");
		let key2 = MigrationKey::new("auth", "0002_add_field");
		let key3 = MigrationKey::new("users", "0001_initial");

		graph.add_migration(key1.clone(), vec![]);
		graph.add_migration(key2.clone(), vec![key1.clone()]);
		graph.add_migration(key3.clone(), vec![]);

		let leaves = graph.get_leaf_nodes();
		assert_eq!(leaves.len(), 2);
		assert!(leaves.contains(&&key2));
		assert!(leaves.contains(&&key3));
	}

	#[test]
	fn test_get_root_nodes() {
		let mut graph = MigrationGraph::new();

		let key1 = MigrationKey::new("auth", "0001_initial");
		let key2 = MigrationKey::new("auth", "0002_add_field");
		let key3 = MigrationKey::new("users", "0001_initial");

		graph.add_migration(key1.clone(), vec![]);
		graph.add_migration(key2.clone(), vec![key1.clone()]);
		graph.add_migration(key3.clone(), vec![]);

		let roots = graph.get_root_nodes();
		assert_eq!(roots.len(), 2);
		assert!(roots.contains(&&key1));
		assert!(roots.contains(&&key3));
	}

	#[test]
	fn test_remove_migration() {
		let mut graph = MigrationGraph::new();
		let key = MigrationKey::new("auth", "0001_initial");

		graph.add_migration(key.clone(), vec![]);
		assert!(graph.has_migration(&key));

		graph.remove_migration(&key);
		assert!(!graph.has_migration(&key));
		assert!(graph.is_empty());
	}

	#[test]
	fn test_clear() {
		let mut graph = MigrationGraph::new();

		graph.add_migration(MigrationKey::new("auth", "0001_initial"), vec![]);
		graph.add_migration(MigrationKey::new("users", "0001_initial"), vec![]);

		assert_eq!(graph.len(), 2);

		graph.clear();
		assert!(graph.is_empty());
		assert_eq!(graph.len(), 0);
	}

	#[test]
	fn test_multiple_root_nodes_sort() {
		let mut graph = MigrationGraph::new();

		let auth_0001 = MigrationKey::new("auth", "0001_initial");
		let users_0001 = MigrationKey::new("users", "0001_initial");
		let posts_0001 = MigrationKey::new("posts", "0001_initial");

		graph.add_migration(auth_0001.clone(), vec![]);
		graph.add_migration(users_0001.clone(), vec![]);
		graph.add_migration(posts_0001.clone(), vec![]);

		let order = graph.topological_sort().unwrap();
		assert_eq!(order.len(), 3);
		// All three should be in the result
		assert!(order.contains(&auth_0001));
		assert!(order.contains(&users_0001));
		assert!(order.contains(&posts_0001));
	}

	#[test]
	fn test_complex_dependency_chain() {
		let mut graph = MigrationGraph::new();

		// Create a diamond-shaped dependency graph
		//     A
		//    / \
		//   B   C
		//    \ /
		//     D
		let a = MigrationKey::new("app", "0001_a");
		let b = MigrationKey::new("app", "0002_b");
		let c = MigrationKey::new("app", "0003_c");
		let d = MigrationKey::new("app", "0004_d");

		graph.add_migration(a.clone(), vec![]);
		graph.add_migration(b.clone(), vec![a.clone()]);
		graph.add_migration(c.clone(), vec![a.clone()]);
		graph.add_migration(d.clone(), vec![b.clone(), c.clone()]);

		let order = graph.topological_sort().unwrap();
		assert_eq!(order.len(), 4);

		let pos_a = order.iter().position(|k| k == &a).unwrap();
		let pos_b = order.iter().position(|k| k == &b).unwrap();
		let pos_c = order.iter().position(|k| k == &c).unwrap();
		let pos_d = order.iter().position(|k| k == &d).unwrap();

		// A must come before B and C
		assert!(pos_a < pos_b);
		assert!(pos_a < pos_c);

		// B and C must come before D
		assert!(pos_b < pos_d);
		assert!(pos_c < pos_d);
	}

	#[test]
	fn test_migration_replaces() {
		let mut graph = MigrationGraph::new();

		let old1 = MigrationKey::new("auth", "0001_initial");
		let old2 = MigrationKey::new("auth", "0002_add_field");
		let squashed = MigrationKey::new("auth", "0001_squashed_0002");

		graph.add_migration(old1.clone(), vec![]);
		graph.add_migration(old2.clone(), vec![old1.clone()]);
		graph.add_migration_with_replaces(
			squashed.clone(),
			vec![],
			vec![old1.clone(), old2.clone()],
		);

		// Check replaces detection
		assert!(graph.is_replaced(&old1));
		assert!(graph.is_replaced(&old2));
		assert!(!graph.is_replaced(&squashed));

		// Check replacement retrieval
		assert_eq!(graph.get_replacement(&old1), Some(&squashed));
		assert_eq!(graph.get_replacement(&old2), Some(&squashed));
		assert_eq!(graph.get_replacement(&squashed), None);
	}

	#[test]
	fn test_resolve_execution_order_with_replaces() {
		let mut graph = MigrationGraph::new();

		let old1 = MigrationKey::new("auth", "0001_initial");
		let old2 = MigrationKey::new("auth", "0002_add_field");
		let old3 = MigrationKey::new("auth", "0003_alter_field");
		let squashed = MigrationKey::new("auth", "0001_squashed_0002");

		graph.add_migration(old1.clone(), vec![]);
		graph.add_migration(old2.clone(), vec![old1.clone()]);
		graph.add_migration(old3.clone(), vec![old2.clone()]);
		graph.add_migration_with_replaces(
			squashed.clone(),
			vec![],
			vec![old1.clone(), old2.clone()],
		);

		let order = graph.resolve_execution_order_with_replaces().unwrap();

		// Should contain squashed and old3, but not old1 or old2
		assert_eq!(order.len(), 2);
		assert!(order.contains(&squashed));
		assert!(order.contains(&old3));
		assert!(!order.contains(&old1));
		assert!(!order.contains(&old2));

		// Squashed should come before old3
		let pos_squashed = order.iter().position(|k| k == &squashed).unwrap();
		let pos_old3 = order.iter().position(|k| k == &old3).unwrap();
		assert!(pos_squashed < pos_old3);
	}

	#[test]
	fn test_find_migration_path_simple() {
		let mut graph = MigrationGraph::new();

		let key1 = MigrationKey::new("auth", "0001_initial");
		let key2 = MigrationKey::new("auth", "0002_add_field");
		let key3 = MigrationKey::new("auth", "0003_alter_field");

		graph.add_migration(key1.clone(), vec![]);
		graph.add_migration(key2.clone(), vec![key1.clone()]);
		graph.add_migration(key3.clone(), vec![key2.clone()]);

		let path = graph.find_migration_path(&key1, &key3).unwrap();
		assert_eq!(path.len(), 3);
		assert_eq!(path[0], key1);
		assert_eq!(path[1], key2);
		assert_eq!(path[2], key3);
	}

	#[test]
	fn test_find_migration_path_same_node() {
		let mut graph = MigrationGraph::new();

		let key = MigrationKey::new("auth", "0001_initial");
		graph.add_migration(key.clone(), vec![]);

		let path = graph.find_migration_path(&key, &key).unwrap();
		assert_eq!(path.len(), 1);
		assert_eq!(path[0], key);
	}

	#[test]
	fn test_find_migration_path_no_path() {
		let mut graph = MigrationGraph::new();

		let key1 = MigrationKey::new("auth", "0001_initial");
		let key2 = MigrationKey::new("users", "0001_initial");

		graph.add_migration(key1.clone(), vec![]);
		graph.add_migration(key2.clone(), vec![]);

		let result = graph.find_migration_path(&key1, &key2);
		assert!(result.is_err());
	}

	#[test]
	fn test_find_backward_path() {
		let mut graph = MigrationGraph::new();

		let key1 = MigrationKey::new("auth", "0001_initial");
		let key2 = MigrationKey::new("auth", "0002_add_field");
		let key3 = MigrationKey::new("auth", "0003_alter_field");

		graph.add_migration(key1.clone(), vec![]);
		graph.add_migration(key2.clone(), vec![key1.clone()]);
		graph.add_migration(key3.clone(), vec![key2.clone()]);

		let path = graph.find_backward_path(&key3, &key1).unwrap();
		assert_eq!(path.len(), 3);
		assert_eq!(path[0], key3);
		assert_eq!(path[1], key2);
		assert_eq!(path[2], key1);
	}

	#[test]
	fn test_detect_all_cycles_no_cycle() {
		let mut graph = MigrationGraph::new();

		let key1 = MigrationKey::new("auth", "0001_initial");
		let key2 = MigrationKey::new("auth", "0002_add_field");
		let key3 = MigrationKey::new("auth", "0003_alter_field");

		graph.add_migration(key1.clone(), vec![]);
		graph.add_migration(key2.clone(), vec![key1.clone()]);
		graph.add_migration(key3.clone(), vec![key2.clone()]);

		let cycles = graph.detect_all_cycles();
		assert!(cycles.is_empty());
	}

	#[test]
	fn test_detect_all_cycles_with_cycle() {
		let mut graph = MigrationGraph::new();

		let key1 = MigrationKey::new("auth", "0001_initial");
		let key2 = MigrationKey::new("auth", "0002_add_field");

		// Create a cycle
		graph.add_migration(key1.clone(), vec![key2.clone()]);
		graph.add_migration(key2.clone(), vec![key1.clone()]);

		let cycles = graph.detect_all_cycles();
		assert!(!cycles.is_empty());
		assert_eq!(cycles.len(), 1);
	}

	#[test]
	fn test_complex_replaces_scenario() {
		let mut graph = MigrationGraph::new();

		// App1 migrations
		let app1_0001 = MigrationKey::new("app1", "0001_initial");
		let app1_0002 = MigrationKey::new("app1", "0002_add_field");
		let app1_squashed = MigrationKey::new("app1", "0001_squashed_0002");

		// App2 migrations depending on app1
		let app2_0001 = MigrationKey::new("app2", "0001_initial");
		let app2_0002 = MigrationKey::new("app2", "0002_add_relation");

		graph.add_migration(app1_0001.clone(), vec![]);
		graph.add_migration(app1_0002.clone(), vec![app1_0001.clone()]);
		graph.add_migration_with_replaces(
			app1_squashed.clone(),
			vec![],
			vec![app1_0001.clone(), app1_0002.clone()],
		);
		graph.add_migration(app2_0001.clone(), vec![app1_0002.clone()]);
		graph.add_migration(app2_0002.clone(), vec![app2_0001.clone()]);

		let order = graph.resolve_execution_order_with_replaces().unwrap();

		// Should not contain replaced migrations
		assert!(!order.contains(&app1_0001));
		assert!(!order.contains(&app1_0002));

		// Should contain squashed and app2 migrations
		assert!(order.contains(&app1_squashed));
		assert!(order.contains(&app2_0001));
		assert!(order.contains(&app2_0002));

		// Squashed should come before app2_0001
		let pos_squashed = order.iter().position(|k| k == &app1_squashed).unwrap();
		let pos_app2_0001 = order.iter().position(|k| k == &app2_0001).unwrap();
		let pos_app2_0002 = order.iter().position(|k| k == &app2_0002).unwrap();

		assert!(pos_squashed < pos_app2_0001);
		assert!(pos_app2_0001 < pos_app2_0002);
	}

	#[test]
	fn test_multi_app_diamond_dependency() {
		let mut graph = MigrationGraph::new();

		// Create a diamond dependency across multiple apps
		//      auth.0001
		//      /        \
		// users.0001   posts.0001
		//      \        /
		//    comments.0001

		let auth_0001 = MigrationKey::new("auth", "0001_initial");
		let users_0001 = MigrationKey::new("users", "0001_initial");
		let posts_0001 = MigrationKey::new("posts", "0001_initial");
		let comments_0001 = MigrationKey::new("comments", "0001_initial");

		graph.add_migration(auth_0001.clone(), vec![]);
		graph.add_migration(users_0001.clone(), vec![auth_0001.clone()]);
		graph.add_migration(posts_0001.clone(), vec![auth_0001.clone()]);
		graph.add_migration(
			comments_0001.clone(),
			vec![users_0001.clone(), posts_0001.clone()],
		);

		let order = graph.topological_sort().unwrap();
		assert_eq!(order.len(), 4);

		let pos_auth = order.iter().position(|k| k == &auth_0001).unwrap();
		let pos_users = order.iter().position(|k| k == &users_0001).unwrap();
		let pos_posts = order.iter().position(|k| k == &posts_0001).unwrap();
		let pos_comments = order.iter().position(|k| k == &comments_0001).unwrap();

		// auth must come before users and posts
		assert!(pos_auth < pos_users);
		assert!(pos_auth < pos_posts);

		// users and posts must come before comments
		assert!(pos_users < pos_comments);
		assert!(pos_posts < pos_comments);
	}

	#[test]
	fn test_path_with_multiple_routes() {
		let mut graph = MigrationGraph::new();

		// Create a graph with multiple possible routes
		//     A
		//    / \
		//   B   C
		//   |   |
		//   D   E
		//    \ /
		//     F

		let a = MigrationKey::new("app", "0001_a");
		let b = MigrationKey::new("app", "0002_b");
		let c = MigrationKey::new("app", "0003_c");
		let d = MigrationKey::new("app", "0004_d");
		let e = MigrationKey::new("app", "0005_e");
		let f = MigrationKey::new("app", "0006_f");

		graph.add_migration(a.clone(), vec![]);
		graph.add_migration(b.clone(), vec![a.clone()]);
		graph.add_migration(c.clone(), vec![a.clone()]);
		graph.add_migration(d.clone(), vec![b.clone()]);
		graph.add_migration(e.clone(), vec![c.clone()]);
		graph.add_migration(f.clone(), vec![d.clone(), e.clone()]);

		// Find path from A to F (should find one of the valid paths)
		let path = graph.find_migration_path(&a, &f).unwrap();
		assert!(path.len() >= 3);
		assert_eq!(path[0], a);
		assert_eq!(path[path.len() - 1], f);
	}

	#[rstest]
	fn test_get_leaf_nodes_for_app_single_leaf() {
		// Arrange
		let mut graph = MigrationGraph::new();
		let key1 = MigrationKey::new("auth", "0001_initial");
		let key2 = MigrationKey::new("auth", "0002_add_field");
		graph.add_migration(key1.clone(), vec![]);
		graph.add_migration(key2.clone(), vec![key1]);

		// Act
		let leaves = graph.get_leaf_nodes_for_app("auth");

		// Assert
		assert_eq!(leaves.len(), 1);
		assert_eq!(leaves[0], &key2);
	}

	#[rstest]
	fn test_get_leaf_nodes_for_app_two_branches() {
		// Arrange
		let mut graph = MigrationGraph::new();
		let key1 = MigrationKey::new("auth", "0001_initial");
		let key2a = MigrationKey::new("auth", "0002_add_field");
		let key2b = MigrationKey::new("auth", "0002_add_index");
		graph.add_migration(key1.clone(), vec![]);
		graph.add_migration(key2a.clone(), vec![key1.clone()]);
		graph.add_migration(key2b.clone(), vec![key1]);

		// Act
		let leaves = graph.get_leaf_nodes_for_app("auth");

		// Assert
		assert_eq!(leaves.len(), 2);
		assert_eq!(leaves[0].name, "0002_add_field");
		assert_eq!(leaves[1].name, "0002_add_index");
	}

	#[rstest]
	fn test_get_leaf_nodes_for_app_three_branches() {
		// Arrange
		let mut graph = MigrationGraph::new();
		let key1 = MigrationKey::new("auth", "0001_initial");
		let key2a = MigrationKey::new("auth", "0002_a");
		let key2b = MigrationKey::new("auth", "0002_b");
		let key2c = MigrationKey::new("auth", "0002_c");
		graph.add_migration(key1.clone(), vec![]);
		graph.add_migration(key2a.clone(), vec![key1.clone()]);
		graph.add_migration(key2b.clone(), vec![key1.clone()]);
		graph.add_migration(key2c.clone(), vec![key1]);

		// Act
		let leaves = graph.get_leaf_nodes_for_app("auth");

		// Assert
		assert_eq!(leaves.len(), 3);
	}

	#[rstest]
	fn test_get_leaf_nodes_for_app_nonexistent() {
		// Arrange
		let mut graph = MigrationGraph::new();
		graph.add_migration(MigrationKey::new("auth", "0001_initial"), vec![]);

		// Act
		let leaves = graph.get_leaf_nodes_for_app("nonexistent");

		// Assert
		assert!(leaves.is_empty());
	}

	#[rstest]
	fn test_get_leaf_nodes_for_app_empty_graph() {
		// Arrange
		let graph = MigrationGraph::new();

		// Act
		let leaves = graph.get_leaf_nodes_for_app("auth");

		// Assert
		assert!(leaves.is_empty());
	}

	#[rstest]
	fn test_detect_conflicts_no_conflicts() {
		// Arrange
		let mut graph = MigrationGraph::new();
		let key1 = MigrationKey::new("auth", "0001_initial");
		let key2 = MigrationKey::new("auth", "0002_add_field");
		graph.add_migration(key1.clone(), vec![]);
		graph.add_migration(key2.clone(), vec![key1]);

		// Act
		let conflicts = graph.detect_conflicts();

		// Assert
		assert!(conflicts.is_empty());
	}

	#[rstest]
	fn test_detect_conflicts_single_app() {
		// Arrange
		let mut graph = MigrationGraph::new();
		let key1 = MigrationKey::new("auth", "0001_initial");
		let key2a = MigrationKey::new("auth", "0002_add_field");
		let key2b = MigrationKey::new("auth", "0002_add_index");
		graph.add_migration(key1.clone(), vec![]);
		graph.add_migration(key2a.clone(), vec![key1.clone()]);
		graph.add_migration(key2b.clone(), vec![key1]);

		// Act
		let conflicts = graph.detect_conflicts();

		// Assert
		assert_eq!(conflicts.len(), 1);
		assert!(conflicts.contains_key("auth"));
		assert_eq!(conflicts["auth"].len(), 2);
	}

	#[rstest]
	fn test_detect_conflicts_multiple_apps() {
		// Arrange
		let mut graph = MigrationGraph::new();
		let auth1 = MigrationKey::new("auth", "0001_initial");
		let auth2a = MigrationKey::new("auth", "0002_a");
		let auth2b = MigrationKey::new("auth", "0002_b");
		let users1 = MigrationKey::new("users", "0001_initial");
		let users2a = MigrationKey::new("users", "0002_a");
		let users2b = MigrationKey::new("users", "0002_b");
		graph.add_migration(auth1.clone(), vec![]);
		graph.add_migration(auth2a.clone(), vec![auth1.clone()]);
		graph.add_migration(auth2b.clone(), vec![auth1]);
		graph.add_migration(users1.clone(), vec![]);
		graph.add_migration(users2a.clone(), vec![users1.clone()]);
		graph.add_migration(users2b.clone(), vec![users1]);

		// Act
		let conflicts = graph.detect_conflicts();

		// Assert
		assert_eq!(conflicts.len(), 2);
		assert!(conflicts.contains_key("auth"));
		assert!(conflicts.contains_key("users"));
	}

	#[rstest]
	fn test_detect_conflicts_mixed() {
		// Arrange: auth has conflict, users does not
		let mut graph = MigrationGraph::new();
		let auth1 = MigrationKey::new("auth", "0001_initial");
		let auth2a = MigrationKey::new("auth", "0002_a");
		let auth2b = MigrationKey::new("auth", "0002_b");
		let users1 = MigrationKey::new("users", "0001_initial");
		let users2 = MigrationKey::new("users", "0002_add_field");
		graph.add_migration(auth1.clone(), vec![]);
		graph.add_migration(auth2a.clone(), vec![auth1.clone()]);
		graph.add_migration(auth2b.clone(), vec![auth1]);
		graph.add_migration(users1.clone(), vec![]);
		graph.add_migration(users2.clone(), vec![users1]);

		// Act
		let conflicts = graph.detect_conflicts();

		// Assert
		assert_eq!(conflicts.len(), 1);
		assert!(conflicts.contains_key("auth"));
		assert!(!conflicts.contains_key("users"));
	}

	#[rstest]
	fn test_detect_conflicts_empty_graph() {
		// Arrange
		let graph = MigrationGraph::new();

		// Act
		let conflicts = graph.detect_conflicts();

		// Assert
		assert!(conflicts.is_empty());
	}

	#[rstest]
	fn test_detect_conflicts_cross_app_deps() {
		// Arrange: auth branches with cross-app dependency on users
		let mut graph = MigrationGraph::new();
		let users1 = MigrationKey::new("users", "0001_initial");
		let auth1 = MigrationKey::new("auth", "0001_initial");
		let auth2a = MigrationKey::new("auth", "0002_add_field");
		let auth2b = MigrationKey::new("auth", "0002_add_fk");
		graph.add_migration(users1.clone(), vec![]);
		graph.add_migration(auth1.clone(), vec![]);
		graph.add_migration(auth2a.clone(), vec![auth1.clone()]);
		graph.add_migration(auth2b.clone(), vec![auth1, users1]);

		// Act
		let conflicts = graph.detect_conflicts();

		// Assert
		assert_eq!(conflicts.len(), 1);
		assert!(conflicts.contains_key("auth"));
		assert_eq!(conflicts["auth"].len(), 2);
	}

	#[rstest]
	fn test_detect_conflicts_sorted_output() {
		// Arrange
		let mut graph = MigrationGraph::new();
		let key1 = MigrationKey::new("auth", "0001_initial");
		let key2c = MigrationKey::new("auth", "0002_c_zebra");
		let key2a = MigrationKey::new("auth", "0002_a_alpha");
		let key2b = MigrationKey::new("auth", "0002_b_beta");
		graph.add_migration(key1.clone(), vec![]);
		graph.add_migration(key2c.clone(), vec![key1.clone()]);
		graph.add_migration(key2a.clone(), vec![key1.clone()]);
		graph.add_migration(key2b.clone(), vec![key1]);

		// Act
		let conflicts = graph.detect_conflicts();

		// Assert
		let auth_conflicts = &conflicts["auth"];
		assert_eq!(auth_conflicts[0].name, "0002_a_alpha");
		assert_eq!(auth_conflicts[1].name, "0002_b_beta");
		assert_eq!(auth_conflicts[2].name, "0002_c_zebra");
	}
}
