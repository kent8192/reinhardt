//! Model registry validation
//!
//! This module provides validation functions to detect conflicts and issues
//! in the model registry, such as duplicate model names, table name collisions,
//! and circular relationships.

use crate::registry::{
	ModelMetadata, RelationshipType, get_registered_models, get_registered_relationships,
};
use std::collections::{HashMap, HashSet};

/// Validation error type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
	/// Duplicate model name within the same app.
	DuplicateModelName {
		/// The application label where the duplicate was found.
		app_label: String,
		/// The duplicated model name.
		model_name: String,
		/// Number of occurrences of this model name.
		count: usize,
	},
	/// Duplicate table name across different apps.
	DuplicateTableName {
		/// The duplicated table name.
		table_name: String,
		/// List of models that share this table name.
		models: Vec<String>,
	},
	/// Circular relationship detected.
	CircularRelationship {
		/// The cycle path as a sequence of model names.
		path: Vec<String>,
	},
}

impl std::fmt::Display for ValidationError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ValidationError::DuplicateModelName {
				app_label,
				model_name,
				count,
			} => {
				write!(
					f,
					"Duplicate model name '{model_name}' in app '{app_label}' ({count} occurrences)"
				)
			}
			ValidationError::DuplicateTableName { table_name, models } => {
				write!(
					f,
					"Duplicate table name '{table_name}' used by models: {}",
					models.join(", ")
				)
			}
			ValidationError::CircularRelationship { path } => {
				write!(f, "Circular relationship detected: {}", path.join(" → "))
			}
		}
	}
}

impl std::error::Error for ValidationError {}

/// Validation result type
pub type ValidationResult<T> = Result<T, Vec<ValidationError>>;

/// Check for duplicate model names within the same app
///
/// # Examples
///
/// ```rust
/// use reinhardt_apps::validation::check_duplicate_model_names;
///
/// // Check for duplicate model names in the global registry
/// let errors = check_duplicate_model_names();
/// // Returns errors if any models with the same app_label and model_name exist
/// ```
pub fn check_duplicate_model_names() -> Vec<ValidationError> {
	let models = get_registered_models();
	let mut errors = Vec::new();

	// Group models by app_label and model_name
	let mut app_models: HashMap<(&str, &str), Vec<&ModelMetadata>> = HashMap::new();
	for model in models {
		app_models
			.entry((model.app_label, model.model_name))
			.or_default()
			.push(model);
	}

	// Find duplicates
	for ((app_label, model_name), model_list) in app_models {
		if model_list.len() > 1 {
			errors.push(ValidationError::DuplicateModelName {
				app_label: app_label.to_string(),
				model_name: model_name.to_string(),
				count: model_list.len(),
			});
		}
	}

	errors
}

/// Check for duplicate table names across apps
///
/// # Examples
///
/// ```rust
/// use reinhardt_apps::validation::check_duplicate_table_names;
///
/// // Check for duplicate table names in the global registry
/// let errors = check_duplicate_table_names();
/// // Returns errors if any models use the same table_name
/// ```
pub fn check_duplicate_table_names() -> Vec<ValidationError> {
	let models = get_registered_models();
	let mut errors = Vec::new();

	// Group models by table_name
	let mut table_models: HashMap<&str, Vec<&ModelMetadata>> = HashMap::new();
	for model in models {
		table_models
			.entry(model.table_name)
			.or_default()
			.push(model);
	}

	// Find duplicates
	for (table_name, model_list) in table_models {
		if model_list.len() > 1 {
			let model_names: Vec<String> = model_list
				.iter()
				.map(|m| format!("{}.{}", m.app_label, m.model_name))
				.collect();
			errors.push(ValidationError::DuplicateTableName {
				table_name: table_name.to_string(),
				models: model_names,
			});
		}
	}

	errors
}

/// Check for circular relationships
///
/// Detects circular foreign key relationships like A → B → C → A
///
/// # Examples
///
/// ```rust
/// use reinhardt_apps::validation::check_circular_relationships;
///
/// let errors = check_circular_relationships();
/// assert_eq!(errors.len(), 0);
/// ```
pub fn check_circular_relationships() -> Vec<ValidationError> {
	let relationships = get_registered_relationships();
	let mut errors = Vec::new();

	// Build adjacency list for ForeignKey relationships only
	let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();
	for rel in relationships {
		if matches!(rel.relationship_type, RelationshipType::ForeignKey) {
			graph.entry(rel.from_model).or_default().push(rel.to_model);
		}
	}

	// Detect cycles using DFS
	let mut visited = HashSet::new();
	let mut rec_stack = HashSet::new();

	for &start in graph.keys() {
		if let Some(cycle) =
			detect_cycle(&graph, start, &mut visited, &mut rec_stack, &mut Vec::new())
		{
			errors.push(ValidationError::CircularRelationship { path: cycle });
		}
	}

	errors
}

/// Helper function to detect cycles in a directed graph using DFS
fn detect_cycle<'a>(
	graph: &HashMap<&'a str, Vec<&'a str>>,
	node: &'a str,
	visited: &mut HashSet<&'a str>,
	rec_stack: &mut HashSet<&'a str>,
	path: &mut Vec<String>,
) -> Option<Vec<String>> {
	if rec_stack.contains(node) {
		// Found a cycle
		path.push(node.to_string());
		// Find where the cycle starts
		let cycle_start = path.iter().position(|n| n == node).unwrap_or(0);
		return Some(path[cycle_start..].to_vec());
	}

	if visited.contains(node) {
		return None;
	}

	visited.insert(node);
	rec_stack.insert(node);
	path.push(node.to_string());

	if let Some(neighbors) = graph.get(node) {
		for &neighbor in neighbors {
			if let Some(cycle) = detect_cycle(graph, neighbor, visited, rec_stack, path) {
				return Some(cycle);
			}
		}
	}

	path.pop();
	rec_stack.remove(node);
	None
}

/// Run all validation checks and return errors
///
/// # Examples
///
/// ```rust
/// use reinhardt_apps::validation::validate_registry;
///
/// let errors = validate_registry();
/// if !errors.is_empty() {
///     eprintln!("Registry validation errors:");
///     for error in errors {
///         eprintln!("  - {}", error);
///     }
/// }
/// ```
pub fn validate_registry() -> Vec<ValidationError> {
	let mut errors = Vec::new();

	errors.extend(check_duplicate_model_names());
	errors.extend(check_duplicate_table_names());
	errors.extend(check_circular_relationships());

	errors
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_validation_error_display() {
		let error = ValidationError::DuplicateModelName {
			app_label: "myapp".to_string(),
			model_name: "User".to_string(),
			count: 2,
		};
		assert_eq!(
			error.to_string(),
			"Duplicate model name 'User' in app 'myapp' (2 occurrences)"
		);

		let error = ValidationError::DuplicateTableName {
			table_name: "users".to_string(),
			models: vec!["auth.User".to_string(), "accounts.User".to_string()],
		};
		assert_eq!(
			error.to_string(),
			"Duplicate table name 'users' used by models: auth.User, accounts.User"
		);

		let error = ValidationError::CircularRelationship {
			path: vec!["A".to_string(), "B".to_string(), "A".to_string()],
		};
		assert_eq!(
			error.to_string(),
			"Circular relationship detected: A → B → A"
		);
	}

	#[test]
	fn test_validation_error_equality() {
		let error1 = ValidationError::DuplicateModelName {
			app_label: "app".to_string(),
			model_name: "Model".to_string(),
			count: 2,
		};
		let error2 = ValidationError::DuplicateModelName {
			app_label: "app".to_string(),
			model_name: "Model".to_string(),
			count: 2,
		};
		let error3 = ValidationError::DuplicateModelName {
			app_label: "app".to_string(),
			model_name: "Other".to_string(),
			count: 2,
		};

		assert_eq!(error1, error2);
		assert_ne!(error1, error3);
	}
}
