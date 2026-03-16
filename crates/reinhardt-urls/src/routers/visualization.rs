//! Route visualization for documentation and debugging
//!
//! This module provides utilities for visualizing route structures in various formats,
//! including ASCII art trees and DOT (Graphviz) format.
//!
//! # Examples
//!
//! ```
//! use reinhardt_urls::routers::visualization::{RouteVisualizer, VisualizationFormat};
//! use reinhardt_urls::routers::introspection::RouteInspector;
//! use hyper::Method;
//!
//! let mut inspector = RouteInspector::new();
//! inspector.add_route("/api/v1/users/", vec![Method::GET], Some("api:v1:users:list"), None);
//! inspector.add_route("/api/v1/users/{id}/", vec![Method::GET], Some("api:v1:users:detail"), None);
//!
//! let visualizer = RouteVisualizer::from_inspector(&inspector);
//! let tree = visualizer.render(VisualizationFormat::Tree);
//! println!("{}", tree);
//! ```

use super::introspection::{RouteInfo, RouteInspector};
use super::namespace::Namespace;
use std::collections::{HashMap, HashSet};

/// Visualization output format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualizationFormat {
	/// ASCII tree structure
	Tree,

	/// DOT format (Graphviz)
	Dot,

	/// Markdown table
	Markdown,

	/// Plain text list
	List,
}

/// Route visualizer
///
/// # Examples
///
/// ```
/// use reinhardt_urls::routers::visualization::{RouteVisualizer, VisualizationFormat};
/// use reinhardt_urls::routers::introspection::RouteInspector;
/// use hyper::Method;
///
/// let mut inspector = RouteInspector::new();
/// inspector.add_route("/users/", vec![Method::GET], Some("users:list"), None);
///
/// let visualizer = RouteVisualizer::from_inspector(&inspector);
/// let output = visualizer.render(VisualizationFormat::Tree);
/// println!("{}", output);
/// ```
pub struct RouteVisualizer {
	routes: Vec<RouteInfo>,
	// Allow dead_code: constructed from routes for future namespace-based visualization formats
	#[allow(dead_code)]
	namespace_tree: NamespaceTree,
}

impl RouteVisualizer {
	/// Create a visualizer from a RouteInspector
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::visualization::RouteVisualizer;
	/// use reinhardt_urls::routers::introspection::RouteInspector;
	/// use hyper::Method;
	///
	/// let mut inspector = RouteInspector::new();
	/// inspector.add_route("/users/", vec![Method::GET], None::<String>, None);
	///
	/// let visualizer = RouteVisualizer::from_inspector(&inspector);
	/// ```
	pub fn from_inspector(inspector: &RouteInspector) -> Self {
		let routes = inspector.all_routes().to_vec();
		let namespace_tree = NamespaceTree::from_routes(&routes);

		Self {
			routes,
			namespace_tree,
		}
	}

	/// Render the route structure in the specified format
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::visualization::{RouteVisualizer, VisualizationFormat};
	/// use reinhardt_urls::routers::introspection::RouteInspector;
	/// use hyper::Method;
	///
	/// let mut inspector = RouteInspector::new();
	/// inspector.add_route("/users/", vec![Method::GET], Some("users:list"), None);
	///
	/// let visualizer = RouteVisualizer::from_inspector(&inspector);
	/// let tree = visualizer.render(VisualizationFormat::Tree);
	/// assert!(tree.contains("users"));
	/// ```
	pub fn render(&self, format: VisualizationFormat) -> String {
		match format {
			VisualizationFormat::Tree => self.render_tree(),
			VisualizationFormat::Dot => self.render_dot(),
			VisualizationFormat::Markdown => self.render_markdown(),
			VisualizationFormat::List => self.render_list(),
		}
	}

	/// Render as ASCII tree
	fn render_tree(&self) -> String {
		let mut output = String::new();
		output.push_str("Route Tree\n");
		output.push_str("==========\n\n");

		if self.routes.is_empty() {
			output.push_str("(no routes)\n");
			return output;
		}

		// Group by namespace
		let mut by_namespace: HashMap<String, Vec<&RouteInfo>> = HashMap::new();
		let mut no_namespace: Vec<&RouteInfo> = Vec::new();

		for route in &self.routes {
			if let Some(ref ns) = route.namespace {
				by_namespace.entry(ns.clone()).or_default().push(route);
			} else {
				no_namespace.push(route);
			}
		}

		// Render namespaced routes
		let mut namespaces: Vec<_> = by_namespace.keys().collect();
		namespaces.sort();

		for (i, ns) in namespaces.iter().enumerate() {
			let is_last_namespace = i == namespaces.len() - 1 && no_namespace.is_empty();
			let prefix = if is_last_namespace {
				"└── "
			} else {
				"├── "
			};
			let child_prefix = if is_last_namespace { "    " } else { "│   " };

			output.push_str(&format!("{}{}\n", prefix, ns));

			if let Some(routes) = by_namespace.get(*ns) {
				for (j, route) in routes.iter().enumerate() {
					let is_last = j == routes.len() - 1;
					let route_prefix = if is_last { "└── " } else { "├── " };

					let methods = route
						.methods
						.iter()
						.map(|m| m.as_str())
						.collect::<Vec<_>>()
						.join(", ");

					output.push_str(&format!(
						"{}{}{} [{}]\n",
						child_prefix, route_prefix, route.path, methods
					));
				}
			}
		}

		// Render non-namespaced routes
		for (i, route) in no_namespace.iter().enumerate() {
			let is_last = i == no_namespace.len() - 1;
			let prefix = if is_last { "└── " } else { "├── " };

			let methods = route
				.methods
				.iter()
				.map(|m| m.as_str())
				.collect::<Vec<_>>()
				.join(", ");

			output.push_str(&format!("{}{} [{}]\n", prefix, route.path, methods));
		}

		output
	}

	/// Render as DOT (Graphviz) format
	fn render_dot(&self) -> String {
		let mut output = String::new();
		output.push_str("digraph Routes {\n");
		output.push_str("  rankdir=LR;\n");
		output.push_str("  node [shape=box];\n\n");

		// Add root node
		output.push_str("  root [label=\"Routes\"];\n\n");

		// Group by namespace
		let mut by_namespace: HashMap<String, Vec<&RouteInfo>> = HashMap::new();
		for route in &self.routes {
			if let Some(ref ns) = route.namespace {
				by_namespace.entry(ns.clone()).or_default().push(route);
			}
		}

		// Add namespace nodes
		let mut node_id = 0;
		let mut namespace_nodes: HashMap<String, String> = HashMap::new();

		for (ns, routes) in &by_namespace {
			let ns_node = format!("ns{}", node_id);
			node_id += 1;

			output.push_str(&format!("  {} [label=\"{}\"];\n", ns_node, ns));
			output.push_str(&format!("  root -> {};\n", ns_node));
			namespace_nodes.insert(ns.clone(), ns_node.clone());

			// Add route nodes
			for route in routes {
				let route_node = format!("route{}", node_id);
				node_id += 1;

				let methods = route
					.methods
					.iter()
					.map(|m| m.as_str())
					.collect::<Vec<_>>()
					.join(", ");

				let label = format!("{}\\n[{}]", route.path, methods);
				output.push_str(&format!("  {} [label=\"{}\"];\n", route_node, label));
				output.push_str(&format!("  {} -> {};\n", ns_node, route_node));
			}
		}

		output.push_str("}\n");
		output
	}

	/// Render as Markdown table
	fn render_markdown(&self) -> String {
		let mut output = String::new();
		output.push_str("# Route Table\n\n");
		output.push_str("| Path | Methods | Name | Namespace |\n");
		output.push_str("|------|---------|------|----------|\n");

		for route in &self.routes {
			let methods = route
				.methods
				.iter()
				.map(|m| m.as_str())
				.collect::<Vec<_>>()
				.join(", ");

			let name = route.name.as_deref().unwrap_or("-");
			let namespace = route.namespace.as_deref().unwrap_or("-");

			output.push_str(&format!(
				"| {} | {} | {} | {} |\n",
				route.path, methods, name, namespace
			));
		}

		output
	}

	/// Render as plain text list
	fn render_list(&self) -> String {
		let mut output = String::new();
		output.push_str("Route List\n");
		output.push_str("==========\n\n");

		for route in &self.routes {
			let methods = route
				.methods
				.iter()
				.map(|m| m.as_str())
				.collect::<Vec<_>>()
				.join(", ");

			output.push_str(&format!("{} [{}]", route.path, methods));

			if let Some(ref name) = route.name {
				output.push_str(&format!(" ({})", name));
			}

			output.push('\n');
		}

		output
	}
}

/// Namespace tree structure
struct NamespaceTree {
	// Allow dead_code: populated during construction for future hierarchical namespace rendering
	#[allow(dead_code)]
	nodes: HashMap<String, NamespaceNode>,
}

impl NamespaceTree {
	fn from_routes(routes: &[RouteInfo]) -> Self {
		let mut nodes = HashMap::new();

		for route in routes {
			if let Some(ref ns_str) = route.namespace {
				let ns = Namespace::new(ns_str);

				// Create nodes for all levels of the namespace
				let mut current_path = String::new();
				for component in ns.components() {
					if !current_path.is_empty() {
						current_path.push(':');
					}
					current_path.push_str(component);

					nodes
						.entry(current_path.clone())
						.or_insert_with(|| NamespaceNode {
							name: component.clone(),
							full_path: current_path.clone(),
							children: HashSet::new(),
							routes: Vec::new(),
						});
				}

				// Add route to leaf node (the namespace itself, not the full route name)
				if let Some(node) = nodes.get_mut(ns_str) {
					node.routes.push(route.clone());
				}
			}
		}

		// Build parent-child relationships
		let all_paths: Vec<String> = nodes.keys().cloned().collect();
		for path in all_paths {
			if let Some(ns) = nodes.get(&path).map(|n| Namespace::new(&n.full_path))
				&& let Some(parent) = ns.parent()
			{
				let parent_path = parent.full_path().to_string();
				if let Some(parent_node) = nodes.get_mut(&parent_path) {
					parent_node.children.insert(path.clone());
				}
			}
		}

		Self { nodes }
	}
}

/// Namespace tree node
struct NamespaceNode {
	// Allow dead_code: stored for display purposes in future namespace tree visualization
	#[allow(dead_code)]
	name: String,
	full_path: String,
	children: HashSet<String>,
	routes: Vec<RouteInfo>,
}

#[cfg(test)]
mod tests {
	use super::*;
	use hyper::Method;

	#[test]
	fn test_visualizer_tree_format() {
		let mut inspector = RouteInspector::new();
		inspector.add_route("/users/", vec![Method::GET], Some("api:users:list"), None);
		inspector.add_route(
			"/users/{id}/",
			vec![Method::GET],
			Some("api:users:detail"),
			None,
		);

		let visualizer = RouteVisualizer::from_inspector(&inspector);
		let tree = visualizer.render(VisualizationFormat::Tree);

		assert!(tree.contains("api"));
		assert!(tree.contains("/users/"));
		assert!(tree.contains("GET"));
	}

	#[test]
	fn test_visualizer_markdown_format() {
		let mut inspector = RouteInspector::new();
		inspector.add_route("/users/", vec![Method::GET], Some("users:list"), None);

		let visualizer = RouteVisualizer::from_inspector(&inspector);
		let markdown = visualizer.render(VisualizationFormat::Markdown);

		assert!(markdown.contains("| Path |"));
		assert!(markdown.contains("/users/"));
		assert!(markdown.contains("GET"));
	}

	#[test]
	fn test_visualizer_list_format() {
		let mut inspector = RouteInspector::new();
		inspector.add_route("/users/", vec![Method::GET], Some("users:list"), None);

		let visualizer = RouteVisualizer::from_inspector(&inspector);
		let list = visualizer.render(VisualizationFormat::List);

		assert!(list.contains("/users/"));
		assert!(list.contains("[GET]"));
	}

	#[test]
	fn test_visualizer_dot_format() {
		let mut inspector = RouteInspector::new();
		inspector.add_route("/users/", vec![Method::GET], Some("api:users:list"), None);

		let visualizer = RouteVisualizer::from_inspector(&inspector);
		let dot = visualizer.render(VisualizationFormat::Dot);

		assert!(dot.contains("digraph Routes"));
		assert!(dot.contains("api"));
		assert!(dot.contains("/users/"));
	}

	#[test]
	fn test_visualizer_empty() {
		let inspector = RouteInspector::new();
		let visualizer = RouteVisualizer::from_inspector(&inspector);
		let tree = visualizer.render(VisualizationFormat::Tree);

		assert!(tree.contains("(no routes)"));
	}

	#[test]
	fn test_namespace_tree_building() {
		let mut inspector = RouteInspector::new();
		inspector.add_route(
			"/users/",
			vec![Method::GET],
			Some("api:v1:users:list"),
			None,
		);
		inspector.add_route(
			"/posts/",
			vec![Method::GET],
			Some("api:v1:posts:list"),
			None,
		);

		let routes = inspector.all_routes().to_vec();
		let tree = NamespaceTree::from_routes(&routes);

		assert!(tree.nodes.contains_key("api"));
		assert!(tree.nodes.contains_key("api:v1"));
		assert!(tree.nodes.contains_key("api:v1:users"));
	}
}
