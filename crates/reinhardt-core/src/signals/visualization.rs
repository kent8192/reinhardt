//! Signal visualization for graphical representation of signal connections
//!
//! Generates visual representations of signal flows and connections using
//! various formats like DOT (Graphviz), Mermaid, and ASCII diagrams.
//!
//! # Examples
//!
//! ```
//! use reinhardt_core::signals::visualization::{SignalGraph, SignalNode, SignalEdge};
//!
//! let mut graph = SignalGraph::new();
//!
//! // Add signal node
//! graph.add_signal_node("user_created", "Sent when user is created");
//!
//! // Add receiver node
//! graph.add_receiver_node("send_email", "Sends welcome email", 10);
//!
//! // Connect signal to receiver
//! graph.add_edge("user_created", "send_email", None);
//!
//! // Generate DOT format
//! let dot = graph.to_dot();
//! assert!(dot.contains("user_created"));
//! ```

use std::collections::{HashMap, HashSet};

/// Node type in the signal graph
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeType {
	/// A signal that can be sent
	Signal,
	/// A receiver that handles signals
	Receiver,
	/// A middleware that intercepts signals
	Middleware,
}

/// A node in the signal graph
#[derive(Debug, Clone)]
pub struct SignalNode {
	/// Node identifier
	pub id: String,
	/// Node type
	pub node_type: NodeType,
	/// Description of the node
	pub description: String,
	/// Priority (for receivers)
	pub priority: Option<i32>,
	/// Whether this node is critical
	pub is_critical: bool,
}

/// An edge connecting two nodes in the signal graph
#[derive(Debug, Clone)]
pub struct SignalEdge {
	/// Source node ID
	pub from: String,
	/// Target node ID
	pub to: String,
	/// Optional label for the edge
	pub label: Option<String>,
	/// Whether this connection is conditional
	pub is_conditional: bool,
}

/// Graph representation of signal connections
pub struct SignalGraph {
	nodes: HashMap<String, SignalNode>,
	edges: Vec<SignalEdge>,
}

impl SignalGraph {
	/// Create a new empty signal graph
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::visualization::SignalGraph;
	///
	/// let graph = SignalGraph::new();
	/// ```
	pub fn new() -> Self {
		Self {
			nodes: HashMap::new(),
			edges: Vec::new(),
		}
	}

	/// Add a signal node to the graph
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::visualization::SignalGraph;
	///
	/// let mut graph = SignalGraph::new();
	/// graph.add_signal_node("user_created", "Sent when user is created");
	/// ```
	pub fn add_signal_node(&mut self, id: &str, description: &str) {
		self.nodes.insert(
			id.to_string(),
			SignalNode {
				id: id.to_string(),
				node_type: NodeType::Signal,
				description: description.to_string(),
				priority: None,
				is_critical: false,
			},
		);
	}

	/// Add a receiver node to the graph
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::visualization::SignalGraph;
	///
	/// let mut graph = SignalGraph::new();
	/// graph.add_receiver_node("send_email", "Sends welcome email", 10);
	/// ```
	pub fn add_receiver_node(&mut self, id: &str, description: &str, priority: i32) {
		self.nodes.insert(
			id.to_string(),
			SignalNode {
				id: id.to_string(),
				node_type: NodeType::Receiver,
				description: description.to_string(),
				priority: Some(priority),
				is_critical: false,
			},
		);
	}

	/// Add a middleware node to the graph
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::visualization::SignalGraph;
	///
	/// let mut graph = SignalGraph::new();
	/// graph.add_middleware_node("logger", "Logs all signals");
	/// ```
	pub fn add_middleware_node(&mut self, id: &str, description: &str) {
		self.nodes.insert(
			id.to_string(),
			SignalNode {
				id: id.to_string(),
				node_type: NodeType::Middleware,
				description: description.to_string(),
				priority: None,
				is_critical: false,
			},
		);
	}

	/// Mark a node as critical
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::visualization::SignalGraph;
	///
	/// let mut graph = SignalGraph::new();
	/// graph.add_receiver_node("payment_processor", "Process payment", 10);
	/// graph.mark_as_critical("payment_processor");
	/// ```
	pub fn mark_as_critical(&mut self, node_id: &str) {
		if let Some(node) = self.nodes.get_mut(node_id) {
			node.is_critical = true;
		}
	}

	/// Add an edge between two nodes
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::visualization::SignalGraph;
	///
	/// let mut graph = SignalGraph::new();
	/// graph.add_signal_node("user_created", "User created signal");
	/// graph.add_receiver_node("send_email", "Send email", 0);
	/// graph.add_edge("user_created", "send_email", Some("on_create".to_string()));
	/// ```
	pub fn add_edge(&mut self, from: &str, to: &str, label: Option<String>) {
		self.edges.push(SignalEdge {
			from: from.to_string(),
			to: to.to_string(),
			label,
			is_conditional: false,
		});
	}

	/// Add a conditional edge (e.g., filtered signal)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::visualization::SignalGraph;
	///
	/// let mut graph = SignalGraph::new();
	/// graph.add_signal_node("user_action", "User action signal");
	/// graph.add_receiver_node("admin_handler", "Admin handler", 0);
	/// graph.add_conditional_edge("user_action", "admin_handler", "if admin");
	/// ```
	pub fn add_conditional_edge(&mut self, from: &str, to: &str, condition: &str) {
		self.edges.push(SignalEdge {
			from: from.to_string(),
			to: to.to_string(),
			label: Some(condition.to_string()),
			is_conditional: true,
		});
	}

	/// Generate DOT (Graphviz) format representation
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::visualization::SignalGraph;
	///
	/// let mut graph = SignalGraph::new();
	/// graph.add_signal_node("signal1", "First signal");
	/// graph.add_receiver_node("receiver1", "First receiver", 0);
	/// graph.add_edge("signal1", "receiver1", None);
	///
	/// let dot = graph.to_dot();
	/// assert!(dot.contains("digraph SignalGraph"));
	/// ```
	pub fn to_dot(&self) -> String {
		let mut output = String::from("digraph SignalGraph {\n");
		output.push_str("  rankdir=LR;\n");
		output.push_str("  node [shape=box, style=rounded];\n\n");

		// Define nodes (escape labels to prevent DOT injection)
		for (id, node) in &self.nodes {
			let (shape, color) = match node.node_type {
				NodeType::Signal => ("ellipse", "lightblue"),
				NodeType::Receiver => ("box", "lightgreen"),
				NodeType::Middleware => ("diamond", "lightyellow"),
			};

			let border_color = if node.is_critical { "red" } else { "black" };
			let priority_label = node
				.priority
				.map(|p| format!("\\nPriority: {}", p))
				.unwrap_or_default();

			let escaped_id = escape_dot_label(id);
			let escaped_desc = escape_dot_label(&node.description);

			output.push_str(&format!(
				"  \"{}\" [shape={}, fillcolor={}, style=\"filled,rounded\", color={}, label=\"{}{}\\n{}\"];\n",
				escaped_id, shape, color, border_color, escaped_id, priority_label, escaped_desc
			));
		}

		output.push('\n');

		// Define edges (escape labels to prevent DOT injection)
		for edge in &self.edges {
			let style = if edge.is_conditional {
				"style=dashed"
			} else {
				"style=solid"
			};

			let label = edge
				.label
				.as_ref()
				.map(|l| format!("label=\"{}\"", escape_dot_label(l)))
				.unwrap_or_default();

			output.push_str(&format!(
				"  \"{}\" -> \"{}\" [{}{}{}];\n",
				escape_dot_label(&edge.from),
				escape_dot_label(&edge.to),
				style,
				if label.is_empty() { "" } else { ", " },
				label
			));
		}

		output.push_str("}\n");
		output
	}

	/// Generate Mermaid diagram format
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::visualization::SignalGraph;
	///
	/// let mut graph = SignalGraph::new();
	/// graph.add_signal_node("signal1", "First signal");
	/// graph.add_receiver_node("receiver1", "First receiver", 0);
	/// graph.add_edge("signal1", "receiver1", None);
	///
	/// let mermaid = graph.to_mermaid();
	/// assert!(mermaid.contains("graph LR"));
	/// ```
	pub fn to_mermaid(&self) -> String {
		let mut output = String::from("graph LR\n");

		// Define nodes
		for (id, node) in &self.nodes {
			let (shape_start, shape_end) = match node.node_type {
				NodeType::Signal => ("([", "])"),
				NodeType::Receiver => ("[", "]"),
				NodeType::Middleware => ("{", "}"),
			};

			let critical_marker = if node.is_critical { "⚠ " } else { "" };
			let priority_label = node
				.priority
				.map(|p| format!(" P{}", p))
				.unwrap_or_default();

			output.push_str(&format!(
				"  {}{}{}{}{}{}\n",
				id, shape_start, critical_marker, node.description, priority_label, shape_end
			));
		}

		output.push('\n');

		// Define edges
		for edge in &self.edges {
			let arrow = if edge.is_conditional { "-.->" } else { "-->" };
			let label = edge
				.label
				.as_ref()
				.map(|l| format!("|{}|", l))
				.unwrap_or_default();

			output.push_str(&format!("  {} {}{} {}\n", edge.from, arrow, label, edge.to));
		}

		output
	}

	/// Generate ASCII art diagram
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::visualization::SignalGraph;
	///
	/// let mut graph = SignalGraph::new();
	/// graph.add_signal_node("signal1", "Signal");
	/// graph.add_receiver_node("receiver1", "Receiver", 0);
	/// graph.add_edge("signal1", "receiver1", None);
	///
	/// let ascii = graph.to_ascii();
	/// assert!(ascii.contains("signal1"));
	/// ```
	pub fn to_ascii(&self) -> String {
		let mut output = String::from("Signal Flow Diagram\n");
		output.push_str("===================\n\n");

		// Group receivers by signal
		let mut signal_to_receivers: HashMap<String, Vec<String>> = HashMap::new();

		for edge in &self.edges {
			if let Some(from_node) = self.nodes.get(&edge.from)
				&& from_node.node_type == NodeType::Signal
			{
				signal_to_receivers
					.entry(edge.from.clone())
					.or_default()
					.push(edge.to.clone());
			}
		}

		// Generate ASCII representation
		for (signal_id, receivers) in signal_to_receivers {
			if let Some(signal_node) = self.nodes.get(&signal_id) {
				output.push_str(&format!("({}) {}\n", signal_id, signal_node.description));
				output.push_str("  |\n");

				for (i, receiver_id) in receivers.iter().enumerate() {
					if let Some(receiver_node) = self.nodes.get(receiver_id) {
						let is_last = i == receivers.len() - 1;
						let connector = if is_last {
							"  └──>"
						} else {
							"  ├──>"
						};
						let critical = if receiver_node.is_critical {
							" ⚠"
						} else {
							""
						};
						let priority = receiver_node
							.priority
							.map(|p| format!(" [P{}]", p))
							.unwrap_or_default();

						output.push_str(&format!(
							"{} [{}] {}{}{}\n",
							connector, receiver_id, receiver_node.description, priority, critical
						));
					}
				}

				output.push('\n');
			}
		}

		output
	}

	/// Get list of all nodes
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::visualization::SignalGraph;
	///
	/// let mut graph = SignalGraph::new();
	/// graph.add_signal_node("signal1", "Signal 1");
	/// graph.add_receiver_node("receiver1", "Receiver 1", 0);
	///
	/// let nodes = graph.nodes();
	/// assert_eq!(nodes.len(), 2);
	/// ```
	pub fn nodes(&self) -> Vec<&SignalNode> {
		self.nodes.values().collect()
	}

	/// Get list of all edges
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::visualization::SignalGraph;
	///
	/// let mut graph = SignalGraph::new();
	/// graph.add_signal_node("signal1", "Signal 1");
	/// graph.add_receiver_node("receiver1", "Receiver 1", 0);
	/// graph.add_edge("signal1", "receiver1", None);
	///
	/// let edges = graph.edges();
	/// assert_eq!(edges.len(), 1);
	/// ```
	pub fn edges(&self) -> &[SignalEdge] {
		&self.edges
	}

	/// Find all receivers connected to a signal
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::visualization::SignalGraph;
	///
	/// let mut graph = SignalGraph::new();
	/// graph.add_signal_node("signal1", "Signal 1");
	/// graph.add_receiver_node("receiver1", "Receiver 1", 0);
	/// graph.add_edge("signal1", "receiver1", None);
	///
	/// let receivers = graph.find_receivers("signal1");
	/// assert_eq!(receivers.len(), 1);
	/// ```
	pub fn find_receivers(&self, signal_id: &str) -> Vec<&SignalNode> {
		let receiver_ids: HashSet<_> = self
			.edges
			.iter()
			.filter(|e| e.from == signal_id)
			.map(|e| e.to.as_str())
			.collect();

		receiver_ids
			.into_iter()
			.filter_map(|id| self.nodes.get(id))
			.collect()
	}

	/// Find all signals that trigger a receiver
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::visualization::SignalGraph;
	///
	/// let mut graph = SignalGraph::new();
	/// graph.add_signal_node("signal1", "Signal 1");
	/// graph.add_receiver_node("receiver1", "Receiver 1", 0);
	/// graph.add_edge("signal1", "receiver1", None);
	///
	/// let signals = graph.find_signals_for_receiver("receiver1");
	/// assert_eq!(signals.len(), 1);
	/// ```
	pub fn find_signals_for_receiver(&self, receiver_id: &str) -> Vec<&SignalNode> {
		let signal_ids: HashSet<_> = self
			.edges
			.iter()
			.filter(|e| e.to == receiver_id)
			.map(|e| e.from.as_str())
			.collect();

		signal_ids
			.into_iter()
			.filter_map(|id| self.nodes.get(id))
			.collect()
	}
}

impl Default for SignalGraph {
	fn default() -> Self {
		Self::new()
	}
}

/// Escape special characters for DOT format labels.
///
/// Prevents content injection by escaping backslash, double-quote,
/// and newline characters that have special meaning in DOT syntax.
fn escape_dot_label(s: &str) -> String {
	s.replace('\\', "\\\\")
		.replace('"', "\\\"")
		.replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_add_nodes() {
		let mut graph = SignalGraph::new();

		graph.add_signal_node("user_created", "User created signal");
		graph.add_receiver_node("send_email", "Send welcome email", 10);

		assert_eq!(graph.nodes.len(), 2);
	}

	#[test]
	fn test_add_edges() {
		let mut graph = SignalGraph::new();

		graph.add_signal_node("signal1", "Signal 1");
		graph.add_receiver_node("receiver1", "Receiver 1", 0);
		graph.add_edge("signal1", "receiver1", None);

		assert_eq!(graph.edges.len(), 1);
	}

	#[test]
	fn test_to_dot() {
		let mut graph = SignalGraph::new();

		graph.add_signal_node("user_created", "User created");
		graph.add_receiver_node("send_email", "Send email", 10);
		graph.add_edge("user_created", "send_email", Some("notify".to_string()));

		let dot = graph.to_dot();
		assert!(dot.contains("digraph SignalGraph"));
		assert!(dot.contains("user_created"));
		assert!(dot.contains("send_email"));
		assert!(dot.contains("notify"));
	}

	#[test]
	fn test_to_mermaid() {
		let mut graph = SignalGraph::new();

		graph.add_signal_node("signal1", "Signal");
		graph.add_receiver_node("receiver1", "Receiver", 0);
		graph.add_edge("signal1", "receiver1", None);

		let mermaid = graph.to_mermaid();
		assert!(mermaid.contains("graph LR"));
		assert!(mermaid.contains("signal1"));
	}

	#[test]
	fn test_to_ascii() {
		let mut graph = SignalGraph::new();

		graph.add_signal_node("signal1", "Test signal");
		graph.add_receiver_node("receiver1", "Test receiver", 0);
		graph.add_edge("signal1", "receiver1", None);

		let ascii = graph.to_ascii();
		assert!(ascii.contains("signal1"));
		assert!(ascii.contains("receiver1"));
	}

	#[test]
	fn test_mark_as_critical() {
		let mut graph = SignalGraph::new();

		graph.add_receiver_node("payment", "Payment processor", 10);
		graph.mark_as_critical("payment");

		let node = graph.nodes.get("payment").unwrap();
		assert!(node.is_critical);
	}

	#[test]
	fn test_find_receivers() {
		let mut graph = SignalGraph::new();

		graph.add_signal_node("signal1", "Signal");
		graph.add_receiver_node("receiver1", "Receiver 1", 0);
		graph.add_receiver_node("receiver2", "Receiver 2", 0);
		graph.add_edge("signal1", "receiver1", None);
		graph.add_edge("signal1", "receiver2", None);

		let receivers = graph.find_receivers("signal1");
		assert_eq!(receivers.len(), 2);
	}

	#[test]
	fn test_conditional_edge() {
		let mut graph = SignalGraph::new();

		graph.add_signal_node("user_action", "User action");
		graph.add_receiver_node("admin_handler", "Admin handler", 0);
		graph.add_conditional_edge("user_action", "admin_handler", "if admin");

		let edge = &graph.edges[0];
		assert!(edge.is_conditional);
	}
}
