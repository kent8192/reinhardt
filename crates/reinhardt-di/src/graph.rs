//! Dependency graph analysis and visualization
//!
//! This module provides tools for analyzing dependency graphs in the DI system:
//! - Cycle detection using topological sort
//! - Dependency tree traversal
//! - Graphviz DOT format export for visualization

use crate::registry::DependencyRegistry;
use std::any::TypeId;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

/// Represents a dependency tree node
#[derive(Debug, Clone)]
pub struct DependencyNode {
	/// The `TypeId` of this dependency.
	pub type_id: TypeId,
	/// The human-readable type name.
	pub type_name: String,
	/// Direct dependencies of this node.
	pub dependencies: Vec<DependencyNode>,
}

/// Dependency graph analyzer
pub struct DependencyGraph {
	registry: Arc<DependencyRegistry>,
}

impl DependencyGraph {
	/// Create a new dependency graph analyzer
	pub fn new(registry: Arc<DependencyRegistry>) -> Self {
		Self { registry }
	}

	/// Detect circular dependencies using DFS-based cycle detection
	///
	/// Returns a vector of cycles, where each cycle is represented as a vector of TypeIds
	/// in the order they form the cycle.
	pub fn detect_cycles(&self) -> Vec<Vec<TypeId>> {
		let all_deps = self.registry.get_all_dependencies();
		let mut cycles = Vec::new();
		let mut visited = HashSet::new();
		let mut rec_stack = HashSet::new();
		let mut path = Vec::new();

		// Visit each node that hasn't been visited yet
		for &type_id in all_deps.keys() {
			if !visited.contains(&type_id) {
				Self::detect_cycles_dfs(
					type_id,
					&all_deps,
					&mut visited,
					&mut rec_stack,
					&mut path,
					&mut cycles,
				);
			}
		}

		cycles
	}

	/// DFS helper for cycle detection
	fn detect_cycles_dfs(
		current: TypeId,
		graph: &HashMap<TypeId, Vec<TypeId>>,
		visited: &mut HashSet<TypeId>,
		rec_stack: &mut HashSet<TypeId>,
		path: &mut Vec<TypeId>,
		cycles: &mut Vec<Vec<TypeId>>,
	) {
		visited.insert(current);
		rec_stack.insert(current);
		path.push(current);

		if let Some(deps) = graph.get(&current) {
			for &dep in deps {
				if !visited.contains(&dep) {
					Self::detect_cycles_dfs(dep, graph, visited, rec_stack, path, cycles);
				} else if rec_stack.contains(&dep) {
					// Found a cycle - extract the cycle from path
					if let Some(cycle_start) = path.iter().position(|&id| id == dep) {
						let cycle = path[cycle_start..].to_vec();
						cycles.push(cycle);
					}
				}
			}
		}

		path.pop();
		rec_stack.remove(&current);
	}

	/// Build a dependency tree starting from a root type
	///
	/// This performs a BFS traversal to build a tree of all dependencies.
	pub fn build_tree(&self, root: TypeId) -> Option<DependencyNode> {
		let all_deps = self.registry.get_all_dependencies();
		let type_names = self.registry.get_type_names();

		// Verify root exists in type_names
		let _type_name = type_names.get(&root)?.to_string();
		let mut visited = HashSet::new();

		Self::build_tree_recursive(root, &all_deps, &type_names, &mut visited)
	}

	/// Recursive helper for building dependency tree
	fn build_tree_recursive(
		current: TypeId,
		graph: &HashMap<TypeId, Vec<TypeId>>,
		type_names: &HashMap<TypeId, &'static str>,
		visited: &mut HashSet<TypeId>,
	) -> Option<DependencyNode> {
		// Prevent infinite recursion in case of cycles
		if visited.contains(&current) {
			return Some(DependencyNode {
				type_id: current,
				type_name: format!(
					"{} (circular)",
					type_names.get(&current).unwrap_or(&"Unknown")
				),
				dependencies: Vec::new(),
			});
		}

		visited.insert(current);

		let type_name = type_names
			.get(&current)
			.map(|s| s.to_string())
			.unwrap_or_else(|| format!("{:?}", current));

		let deps = graph.get(&current).cloned().unwrap_or_default();

		let child_nodes: Vec<DependencyNode> = deps
			.iter()
			.filter_map(|&dep_id| Self::build_tree_recursive(dep_id, graph, type_names, visited))
			.collect();

		visited.remove(&current);

		Some(DependencyNode {
			type_id: current,
			type_name,
			dependencies: child_nodes,
		})
	}

	/// Generate Graphviz DOT format for visualization
	///
	/// The output can be saved to a `.dot` file and rendered with Graphviz:
	/// ```bash
	/// dot -Tpng dependencies.dot -o dependencies.png
	/// ```
	pub fn to_dot(&self) -> String {
		let all_deps = self.registry.get_all_dependencies();
		let type_names = self.registry.get_type_names();

		let mut dot = String::from("digraph Dependencies {\n");
		dot.push_str("  node [shape=box, style=rounded];\n");
		dot.push_str("  rankdir=LR;\n\n");

		// Create nodes
		for (&type_id, &type_name) in &type_names {
			let node_id = format!("{:?}", type_id);
			let label = type_name.replace('"', "\\\"");
			dot.push_str(&format!("  \"{}\" [label=\"{}\"];\n", node_id, label));
		}

		dot.push('\n');

		// Create edges
		for (type_id, deps) in &all_deps {
			let from = format!("{:?}", type_id);
			for &dep_id in deps {
				let to = format!("{:?}", dep_id);
				dot.push_str(&format!("  \"{}\" -> \"{}\";\n", from, to));
			}
		}

		dot.push_str("}\n");
		dot
	}

	/// Perform topological sort to detect cycles and get dependency order
	///
	/// Returns Ok(order) if no cycles exist, or Err(cycles) if cycles are found.
	pub fn topological_sort(&self) -> Result<Vec<TypeId>, Vec<Vec<TypeId>>> {
		let cycles = self.detect_cycles();
		if !cycles.is_empty() {
			return Err(cycles);
		}

		let all_deps = self.registry.get_all_dependencies();
		let mut in_degree: HashMap<TypeId, usize> = HashMap::new();
		let adj_list: HashMap<TypeId, Vec<TypeId>> = all_deps.clone();

		// Calculate in-degrees
		for deps in all_deps.values() {
			for &dep in deps {
				*in_degree.entry(dep).or_insert(0) += 1;
			}
		}

		// All nodes with in-degree 0
		let mut queue: VecDeque<TypeId> = all_deps
			.keys()
			.filter(|&&id| in_degree.get(&id).copied().unwrap_or(0) == 0)
			.copied()
			.collect();

		let mut result = Vec::new();

		while let Some(current) = queue.pop_front() {
			result.push(current);

			if let Some(deps) = adj_list.get(&current) {
				for &dep in deps {
					if let Some(degree) = in_degree.get_mut(&dep) {
						*degree -= 1;
						if *degree == 0 {
							queue.push_back(dep);
						}
					}
				}
			}
		}

		Ok(result)
	}

	/// Get all nodes with no dependencies (leaf nodes)
	pub fn get_leaf_nodes(&self) -> Vec<TypeId> {
		let all_deps = self.registry.get_all_dependencies();
		all_deps
			.iter()
			.filter(|(_, deps)| deps.is_empty())
			.map(|(&id, _)| id)
			.collect()
	}

	/// Get all nodes that no other nodes depend on (root nodes)
	pub fn get_root_nodes(&self) -> Vec<TypeId> {
		let all_deps = self.registry.get_all_dependencies();
		let mut depended_on: HashSet<TypeId> = HashSet::new();

		for deps in all_deps.values() {
			for &dep in deps {
				depended_on.insert(dep);
			}
		}

		all_deps
			.keys()
			.filter(|&id| !depended_on.contains(id))
			.copied()
			.collect()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::registry::DependencyRegistry;

	#[test]
	fn test_empty_graph() {
		let registry = Arc::new(DependencyRegistry::new());
		let graph = DependencyGraph::new(registry);

		let cycles = graph.detect_cycles();
		assert!(cycles.is_empty());

		let dot = graph.to_dot();
		assert!(dot.contains("digraph Dependencies"));
	}

	#[test]
	fn test_simple_dependency_chain() {
		let registry = Arc::new(DependencyRegistry::new());

		// Create a simple chain: A -> B -> C
		let type_id_a = TypeId::of::<i32>();
		let type_id_b = TypeId::of::<String>();
		let type_id_c = TypeId::of::<Vec<u8>>();

		registry.register_type_name(type_id_a, "ServiceA");
		registry.register_type_name(type_id_b, "ServiceB");
		registry.register_type_name(type_id_c, "ServiceC");

		registry.register_dependencies(type_id_a, vec![type_id_b]);
		registry.register_dependencies(type_id_b, vec![type_id_c]);
		registry.register_dependencies(type_id_c, vec![]);

		let graph = DependencyGraph::new(registry);

		// Should have no cycles
		let cycles = graph.detect_cycles();
		assert!(cycles.is_empty());

		// Topological sort should succeed
		let order = graph.topological_sort();
		assert!(order.is_ok());

		// Root should be A (no one depends on it)
		let roots = graph.get_root_nodes();
		assert_eq!(roots.len(), 1);
		assert_eq!(roots[0], type_id_a);

		// Leaf should be C (depends on nothing)
		let leaves = graph.get_leaf_nodes();
		assert_eq!(leaves.len(), 1);
		assert_eq!(leaves[0], type_id_c);
	}

	#[test]
	fn test_circular_dependency_detection() {
		let registry = Arc::new(DependencyRegistry::new());

		// Create a cycle: A -> B -> C -> A
		let type_id_a = TypeId::of::<i32>();
		let type_id_b = TypeId::of::<String>();
		let type_id_c = TypeId::of::<Vec<u8>>();

		registry.register_type_name(type_id_a, "ServiceA");
		registry.register_type_name(type_id_b, "ServiceB");
		registry.register_type_name(type_id_c, "ServiceC");

		registry.register_dependencies(type_id_a, vec![type_id_b]);
		registry.register_dependencies(type_id_b, vec![type_id_c]);
		registry.register_dependencies(type_id_c, vec![type_id_a]); // Creates cycle

		let graph = DependencyGraph::new(registry);

		// Should detect the cycle
		let cycles = graph.detect_cycles();
		assert!(!cycles.is_empty());
		assert!(cycles[0].len() >= 3); // Cycle should contain at least A, B, C

		// Topological sort should fail
		let order = graph.topological_sort();
		assert!(order.is_err());
	}

	#[test]
	fn test_dot_generation() {
		let registry = Arc::new(DependencyRegistry::new());

		let type_id_a = TypeId::of::<i32>();
		let type_id_b = TypeId::of::<String>();

		registry.register_type_name(type_id_a, "ServiceA");
		registry.register_type_name(type_id_b, "ServiceB");
		registry.register_dependencies(type_id_a, vec![type_id_b]);

		let graph = DependencyGraph::new(registry);
		let dot = graph.to_dot();

		assert!(dot.contains("digraph Dependencies"));
		assert!(dot.contains("ServiceA"));
		assert!(dot.contains("ServiceB"));
		assert!(dot.contains("->"));
	}
}
