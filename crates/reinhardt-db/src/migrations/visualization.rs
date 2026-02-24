//! Schema history visualization tools
//!
//! This module provides tools for visualizing migration history and schema evolution,
//! inspired by tools like Rails schema visualizers and Flyway's schema history reporting.
//!
//! # Example
//!
//! ```rust
//! use reinhardt_db::migrations::visualization::{MigrationVisualizer, OutputFormat};
//! use reinhardt_db::migrations::Migration;
//!
//! let migration1 = Migration::new("0001_initial", "myapp");
//! let migration2 = Migration::new("0002_add_field", "myapp")
//!     .add_dependency("myapp", "0001_initial");
//!
//! let migrations = vec![migration1, migration2];
//! let visualizer = MigrationVisualizer::new();
//! let graph = visualizer.generate_dependency_graph(&migrations, OutputFormat::Text);
//! ```

use super::Migration;
use super::recorder::MigrationRecord;
use std::collections::{HashMap, HashSet};

/// Output format for visualization
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::visualization::OutputFormat;
///
/// let format = OutputFormat::Text;
/// assert_eq!(format.extension(), "txt");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
	/// Plain text output
	Text,
	/// Markdown format
	Markdown,
	/// DOT graph format (for Graphviz)
	Dot,
	/// JSON format
	Json,
}

impl OutputFormat {
	/// Get file extension for this format
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::visualization::OutputFormat;
	///
	/// assert_eq!(OutputFormat::Text.extension(), "txt");
	/// assert_eq!(OutputFormat::Markdown.extension(), "md");
	/// assert_eq!(OutputFormat::Dot.extension(), "dot");
	/// assert_eq!(OutputFormat::Json.extension(), "json");
	/// ```
	pub fn extension(&self) -> &str {
		match self {
			OutputFormat::Text => "txt",
			OutputFormat::Markdown => "md",
			OutputFormat::Dot => "dot",
			OutputFormat::Json => "json",
		}
	}
}

/// Migration history entry
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::visualization::HistoryEntry;
///
/// let entry = HistoryEntry {
///     app_label: "myapp".to_string(),
///     migration_name: "0001_initial".to_string(),
///     applied_at: "2025-01-01 00:00:00".to_string(),
///     operations_count: 5,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct HistoryEntry {
	pub app_label: String,
	pub migration_name: String,
	pub applied_at: String,
	pub operations_count: usize,
}

impl HistoryEntry {
	/// Create from MigrationRecord
	pub fn from_record(record: &MigrationRecord, operations_count: usize) -> Self {
		Self {
			app_label: record.app.clone(),
			migration_name: record.name.clone(),
			applied_at: record.applied.to_rfc3339(),
			operations_count,
		}
	}
}

/// Migration visualizer
///
/// Generates visual representations of migration history and dependencies.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::visualization::MigrationVisualizer;
///
/// let visualizer = MigrationVisualizer::new();
/// ```
pub struct MigrationVisualizer {
	_private: (),
}

impl MigrationVisualizer {
	/// Create a new migration visualizer
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::visualization::MigrationVisualizer;
	///
	/// let visualizer = MigrationVisualizer::new();
	/// ```
	pub fn new() -> Self {
		Self { _private: () }
	}

	/// Generate dependency graph
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::visualization::{MigrationVisualizer, OutputFormat};
	/// use reinhardt_db::migrations::Migration;
	///
	/// let migration1 = Migration::new("0001_initial", "myapp");
	/// let migration2 = Migration::new("0002_add_field", "myapp")
	///     .add_dependency("myapp", "0001_initial");
	///
	/// let migrations = vec![migration1, migration2];
	/// let visualizer = MigrationVisualizer::new();
	/// let graph = visualizer.generate_dependency_graph(&migrations, OutputFormat::Text);
	/// assert!(graph.contains("0001_initial"));
	/// assert!(graph.contains("0002_add_field"));
	/// ```
	pub fn generate_dependency_graph(
		&self,
		migrations: &[Migration],
		format: OutputFormat,
	) -> String {
		match format {
			OutputFormat::Text => self.generate_text_graph(migrations),
			OutputFormat::Markdown => self.generate_markdown_graph(migrations),
			OutputFormat::Dot => self.generate_dot_graph(migrations),
			OutputFormat::Json => self.generate_json_graph(migrations),
		}
	}

	/// Generate timeline view of migrations
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::visualization::{MigrationVisualizer, HistoryEntry};
	///
	/// let entries = vec![
	///     HistoryEntry {
	///         app_label: "myapp".to_string(),
	///         migration_name: "0001_initial".to_string(),
	///         applied_at: "2025-01-01 00:00:00".to_string(),
	///         operations_count: 3,
	///     },
	/// ];
	///
	/// let visualizer = MigrationVisualizer::new();
	/// let timeline = visualizer.generate_timeline(&entries);
	/// assert!(timeline.contains("0001_initial"));
	/// ```
	pub fn generate_timeline(&self, history: &[HistoryEntry]) -> String {
		let mut output = String::new();
		output.push_str("Migration Timeline\n");
		output.push_str("==================\n\n");

		for entry in history {
			output.push_str(&format!(
				"[{}] {}.{} ({} operations)\n",
				entry.applied_at, entry.app_label, entry.migration_name, entry.operations_count
			));
		}

		output
	}

	/// Generate schema evolution report
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::visualization::MigrationVisualizer;
	/// use reinhardt_db::migrations::Migration;
	///
	/// let migrations = vec![Migration::new("0001_initial", "myapp")];
	/// let visualizer = MigrationVisualizer::new();
	/// let report = visualizer.generate_evolution_report(&migrations);
	/// assert!(report.contains("Schema Evolution"));
	/// ```
	pub fn generate_evolution_report(&self, migrations: &[Migration]) -> String {
		let mut output = String::new();
		output.push_str("Schema Evolution Report\n");
		output.push_str("=======================\n\n");

		// Group by app
		let mut by_app: HashMap<String, Vec<&Migration>> = HashMap::new();
		for migration in migrations {
			by_app
				.entry(migration.app_label.to_string())
				.or_default()
				.push(migration);
		}

		for (app, app_migrations) in by_app {
			output.push_str(&format!("\nApp: {}\n", app));
			output.push_str(&format!("{}\n", "-".repeat(app.len() + 5)));

			for migration in app_migrations {
				output.push_str(&format!(
					"  - {}: {} operations\n",
					migration.name,
					migration.operations.len()
				));
			}
		}

		output
	}

	fn generate_text_graph(&self, migrations: &[Migration]) -> String {
		let mut output = String::new();
		output.push_str("Migration Dependency Graph\n");
		output.push_str("==========================\n\n");

		for migration in migrations {
			output.push_str(&format!("{}.{}\n", migration.app_label, migration.name));

			if !migration.dependencies.is_empty() {
				output.push_str("  Dependencies:\n");
				for (app, name) in &migration.dependencies {
					output.push_str(&format!("    - {}.{}\n", app, name));
				}
			}

			output.push('\n');
		}

		output
	}

	fn generate_markdown_graph(&self, migrations: &[Migration]) -> String {
		let mut output = String::new();
		output.push_str("# Migration Dependency Graph\n\n");

		for migration in migrations {
			output.push_str(&format!(
				"## {}.{}\n\n",
				migration.app_label, migration.name
			));

			if !migration.dependencies.is_empty() {
				output.push_str("**Dependencies:**\n\n");
				for (app, name) in &migration.dependencies {
					output.push_str(&format!("- {}.{}\n", app, name));
				}
			}

			output.push_str(&format!(
				"\n**Operations:** {}\n\n",
				migration.operations.len()
			));
		}

		output
	}

	fn generate_dot_graph(&self, migrations: &[Migration]) -> String {
		let mut output = String::new();
		output.push_str("digraph migrations {\n");
		output.push_str("  rankdir=LR;\n");
		output.push_str("  node [shape=box];\n\n");

		// Generate nodes
		for migration in migrations {
			let node_id = format!("{}_{}", migration.app_label, migration.name);
			output.push_str(&format!(
				"  {} [label=\"{}.{}\"];\n",
				node_id.replace('-', "_"),
				migration.app_label,
				migration.name
			));
		}

		output.push('\n');

		// Generate edges
		for migration in migrations {
			let to_id = format!("{}_{}", migration.app_label, migration.name);
			for (dep_app, dep_name) in &migration.dependencies {
				let from_id = format!("{}_{}", dep_app, dep_name);
				output.push_str(&format!(
					"  {} -> {};\n",
					from_id.replace('-', "_"),
					to_id.replace('-', "_")
				));
			}
		}

		output.push_str("}\n");
		output
	}

	fn generate_json_graph(&self, migrations: &[Migration]) -> String {
		use serde_json::json;

		let nodes: Vec<_> = migrations
			.iter()
			.map(|m| {
				json!({
					"id": format!("{}.{}", m.app_label, m.name),
					"app": m.app_label,
					"name": m.name,
					"operations": m.operations.len(),
				})
			})
			.collect();

		let edges: Vec<_> = migrations
			.iter()
			.flat_map(|m| {
				m.dependencies.iter().map(move |(dep_app, dep_name)| {
					json!({
						"from": format!("{}.{}", dep_app, dep_name),
						"to": format!("{}.{}", m.app_label, m.name),
					})
				})
			})
			.collect();

		let graph = json!({
			"nodes": nodes,
			"edges": edges,
		});

		serde_json::to_string_pretty(&graph).unwrap_or_default()
	}
}

impl Default for MigrationVisualizer {
	fn default() -> Self {
		Self::new()
	}
}

/// Migration statistics
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::visualization::MigrationStats;
/// use reinhardt_db::migrations::Migration;
///
/// let migrations = vec![
///     Migration::new("0001_initial", "app1"),
///     Migration::new("0002_add_field", "app1"),
///     Migration::new("0001_initial", "app2"),
/// ];
///
/// let stats = MigrationStats::from_migrations(&migrations);
/// assert_eq!(stats.total_migrations, 3);
/// assert_eq!(stats.apps_count, 2);
/// ```
#[derive(Debug, Clone)]
pub struct MigrationStats {
	pub total_migrations: usize,
	pub apps_count: usize,
	pub total_operations: usize,
	pub by_app: HashMap<String, usize>,
}

impl MigrationStats {
	/// Calculate statistics from migrations
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::visualization::MigrationStats;
	/// use reinhardt_db::migrations::Migration;
	///
	/// let migrations = vec![Migration::new("0001_initial", "myapp")];
	/// let stats = MigrationStats::from_migrations(&migrations);
	/// assert_eq!(stats.total_migrations, 1);
	/// ```
	pub fn from_migrations(migrations: &[Migration]) -> Self {
		let mut by_app = HashMap::new();
		let mut total_operations = 0;

		for migration in migrations {
			*by_app.entry(migration.app_label.to_string()).or_insert(0) += 1;
			total_operations += migration.operations.len();
		}

		let apps: HashSet<_> = migrations.iter().map(|m| m.app_label.clone()).collect();

		Self {
			total_migrations: migrations.len(),
			apps_count: apps.len(),
			total_operations,
			by_app,
		}
	}

	/// Generate statistics report
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::visualization::MigrationStats;
	/// use reinhardt_db::migrations::Migration;
	///
	/// let migrations = vec![Migration::new("0001_initial", "myapp")];
	/// let stats = MigrationStats::from_migrations(&migrations);
	/// let report = stats.generate_report();
	/// assert!(report.contains("Total Migrations"));
	/// ```
	pub fn generate_report(&self) -> String {
		let mut output = String::new();
		output.push_str("Migration Statistics\n");
		output.push_str("===================\n\n");

		output.push_str(&format!("Total Migrations: {}\n", self.total_migrations));
		output.push_str(&format!("Total Apps: {}\n", self.apps_count));
		output.push_str(&format!("Total Operations: {}\n\n", self.total_operations));

		output.push_str("By App:\n");
		for (app, count) in &self.by_app {
			output.push_str(&format!("  {}: {} migrations\n", app, count));
		}

		output
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_output_format_extension() {
		assert_eq!(OutputFormat::Text.extension(), "txt");
		assert_eq!(OutputFormat::Markdown.extension(), "md");
		assert_eq!(OutputFormat::Dot.extension(), "dot");
		assert_eq!(OutputFormat::Json.extension(), "json");
	}

	#[test]
	fn test_visualizer_text_graph() {
		let migration1 = Migration::new("0001_initial", "myapp");
		let migration2 =
			Migration::new("0002_add_field", "myapp").add_dependency("myapp", "0001_initial");

		let migrations = vec![migration1, migration2];
		let visualizer = MigrationVisualizer::new();
		let graph = visualizer.generate_dependency_graph(&migrations, OutputFormat::Text);

		assert!(graph.contains("Migration Dependency Graph"));
		assert!(graph.contains("myapp.0001_initial"));
		assert!(graph.contains("myapp.0002_add_field"));
	}

	#[test]
	fn test_visualizer_markdown_graph() {
		let migration = Migration::new("0001_initial", "myapp");
		let migrations = vec![migration];

		let visualizer = MigrationVisualizer::new();
		let graph = visualizer.generate_dependency_graph(&migrations, OutputFormat::Markdown);

		assert!(graph.contains("# Migration Dependency Graph"));
		assert!(graph.contains("## myapp.0001_initial"));
	}

	#[test]
	fn test_visualizer_dot_graph() {
		let migration1 = Migration::new("0001_initial", "myapp");
		let migration2 =
			Migration::new("0002_add_field", "myapp").add_dependency("myapp", "0001_initial");

		let migrations = vec![migration1, migration2];
		let visualizer = MigrationVisualizer::new();
		let graph = visualizer.generate_dependency_graph(&migrations, OutputFormat::Dot);

		assert!(graph.contains("digraph migrations"));
		assert!(graph.contains("myapp_0001_initial"));
		assert!(graph.contains("myapp_0002_add_field"));
		assert!(graph.contains("->"));
	}

	#[test]
	fn test_visualizer_json_graph() {
		let migration = Migration::new("0001_initial", "myapp");
		let migrations = vec![migration];

		let visualizer = MigrationVisualizer::new();
		let graph = visualizer.generate_dependency_graph(&migrations, OutputFormat::Json);

		assert!(graph.contains("nodes"));
		assert!(graph.contains("edges"));
		assert!(graph.contains("myapp.0001_initial"));
	}

	#[test]
	fn test_generate_timeline() {
		let entries = vec![
			HistoryEntry {
				app_label: "myapp".to_string(),
				migration_name: "0001_initial".to_string(),
				applied_at: "2025-01-01 00:00:00".to_string(),
				operations_count: 3,
			},
			HistoryEntry {
				app_label: "myapp".to_string(),
				migration_name: "0002_add_field".to_string(),
				applied_at: "2025-01-02 00:00:00".to_string(),
				operations_count: 1,
			},
		];

		let visualizer = MigrationVisualizer::new();
		let timeline = visualizer.generate_timeline(&entries);

		assert!(timeline.contains("Migration Timeline"));
		assert!(timeline.contains("0001_initial"));
		assert!(timeline.contains("0002_add_field"));
		assert!(timeline.contains("3 operations"));
	}

	#[test]
	fn test_generate_evolution_report() {
		let migrations = vec![
			Migration::new("0001_initial", "app1"),
			Migration::new("0002_add_field", "app1"),
			Migration::new("0001_initial", "app2"),
		];

		let visualizer = MigrationVisualizer::new();
		let report = visualizer.generate_evolution_report(&migrations);

		assert!(report.contains("Schema Evolution Report"));
		assert!(report.contains("App: app1"));
		assert!(report.contains("App: app2"));
	}

	#[test]
	fn test_migration_stats() {
		let migrations = vec![
			Migration::new("0001_initial", "app1"),
			Migration::new("0002_add_field", "app1"),
			Migration::new("0001_initial", "app2"),
		];

		let stats = MigrationStats::from_migrations(&migrations);

		assert_eq!(stats.total_migrations, 3);
		assert_eq!(stats.apps_count, 2);
		assert_eq!(stats.by_app.get("app1"), Some(&2));
		assert_eq!(stats.by_app.get("app2"), Some(&1));
	}

	#[test]
	fn test_stats_report() {
		let migrations = vec![
			Migration::new("0001_initial", "myapp"),
			Migration::new("0002_add_field", "myapp"),
		];

		let stats = MigrationStats::from_migrations(&migrations);
		let report = stats.generate_report();

		assert!(report.contains("Migration Statistics"));
		assert!(report.contains("Total Migrations: 2"));
		assert!(report.contains("myapp: 2 migrations"));
	}
}
