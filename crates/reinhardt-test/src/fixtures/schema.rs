//! Schema creation fixtures for tests
//!
//! This module provides low-level utilities to create database tables
//! directly from Model metadata, eliminating the need for duplicate
//! schema definitions in tests.
//!
//! ## Overview
//!
//! The main goal is to bridge the gap between:
//! - `#[model(...)]` macro definitions (which generate `Model` trait implementations)
//! - Database table creation (previously done manually with reinhardt-query)
//!
//! ## Usage Example
//!
//! ```rust,ignore
//! use reinhardt_test::fixtures::schema::{create_table_for_model, create_tables_for_models};
//! use reinhardt_db::orm::Model;
//!
//! #[model(app_label = "test", table_name = "articles")]
//! struct Article {
//!     #[field(primary_key = true)]
//!     id: Option<i64>,
//!     #[field(max_length = 200)]
//!     title: String,
//! }
//!
//! #[rstest]
//! #[tokio::test]
//! async fn test_article(
//!     #[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
//! ) {
//!     let (_, pool, _, url) = postgres_container.await;
//!     let connection = DatabaseConnection::connect_postgres(&url).await.unwrap();
//!
//!     // Create table directly from model metadata
//!     create_table_for_model::<Article>(&connection).await.unwrap();
//!
//!     // Test execution...
//! }
//! ```

use std::collections::HashMap;

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::{
	ColumnDefinition, Constraint, ForeignKeyAction, Migration, Operation,
	executor::DatabaseMigrationExecutor, field_type_string_to_field_type, to_snake_case,
};
use reinhardt_db::orm::Model;
use reinhardt_db::orm::fields::FieldKwarg;
use reinhardt_db::orm::inspection::{FieldInfo, RelationInfo};
use reinhardt_db::orm::relationship::RelationshipType;

/// Error type for schema operations
#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
	#[error("Field conversion error: {0}")]
	FieldConversion(String),

	#[error("Migration execution error: {0}")]
	MigrationExecution(String),

	#[error("Dependency resolution error: {0}")]
	DependencyResolution(String),

	#[error("Circular dependency detected: {0}")]
	CircularDependency(String),
}

/// Convert FieldKwarg HashMap to String HashMap for field_type_string_to_field_type
fn convert_attributes(attributes: &HashMap<String, FieldKwarg>) -> HashMap<String, String> {
	attributes
		.iter()
		.filter_map(|(k, v)| {
			let value_str = match v {
				FieldKwarg::String(s) => Some(s.clone()),
				FieldKwarg::Int(n) => Some(n.to_string()),
				FieldKwarg::Uint(n) => Some(n.to_string()),
				FieldKwarg::Bool(b) => Some(b.to_string()),
				FieldKwarg::Float(f) => Some(f.to_string()),
				FieldKwarg::Choices(_) => None,
				FieldKwarg::Callable(s) => Some(s.clone()),
			};
			value_str.map(|v| (k.clone(), v))
		})
		.collect()
}

/// Convert FieldInfo to ColumnDefinition
///
/// This function transforms the metadata extracted from a Model's field_metadata()
/// into a ColumnDefinition that can be used in migration operations.
///
/// # Arguments
///
/// * `field_info` - Field information from Model::field_metadata()
///
/// # Returns
///
/// * `Ok(ColumnDefinition)` - The column definition for migration
/// * `Err(SchemaError)` - Error if field type is unsupported
pub fn field_info_to_column_definition(
	field_info: &FieldInfo,
) -> Result<ColumnDefinition, SchemaError> {
	let attributes = convert_attributes(&field_info.attributes);
	let field_type = field_type_string_to_field_type(&field_info.field_type, &attributes)
		.map_err(SchemaError::FieldConversion)?;

	let name = field_info.name.clone();

	// Handle default value
	let default: Option<String> = field_info.default.as_ref().map(|d| match d {
		FieldKwarg::String(s) => format!("'{}'", s),
		FieldKwarg::Int(n) => n.to_string(),
		FieldKwarg::Uint(n) => n.to_string(),
		FieldKwarg::Bool(b) => b.to_string(),
		FieldKwarg::Float(f) => f.to_string(),
		FieldKwarg::Choices(_) => "NULL".to_string(),
		FieldKwarg::Callable(s) => s.clone(),
	});

	// Determine auto_increment from attributes
	let auto_increment = field_info
		.attributes
		.get("auto_increment")
		.map(|v| matches!(v, FieldKwarg::Bool(true)))
		.unwrap_or(false)
		|| field_info
			.attributes
			.get("identity_by_default")
			.map(|v| matches!(v, FieldKwarg::Bool(true)))
			.unwrap_or(false)
		|| field_info.primary_key; // Auto increment for primary keys by default

	Ok(ColumnDefinition {
		name,
		type_definition: field_type,
		not_null: !field_info.nullable,
		unique: field_info.unique,
		primary_key: field_info.primary_key,
		auto_increment,
		default,
	})
}

/// Extract model dependencies from relationship metadata
///
/// This function analyzes the relationship metadata to determine which other
/// models this model depends on (via ForeignKey or OneToOne relationships).
///
/// # Arguments
///
/// * `relationship_metadata` - Relationship information from Model::relationship_metadata()
///
/// # Returns
///
/// A list of model names that this model depends on
pub fn extract_model_dependencies(relationship_metadata: &[RelationInfo]) -> Vec<String> {
	relationship_metadata
		.iter()
		.filter_map(|rel| match rel.relationship_type {
			RelationshipType::ManyToOne | RelationshipType::OneToOne => {
				Some(rel.related_model.clone())
			}
			_ => None,
		})
		.collect()
}

/// Resolve table name for a model name
///
/// This function resolves a model name to its corresponding table name by:
/// 1. First looking up in the provided model_infos (if available)
/// 2. Falling back to snake_case conversion if not found
///
/// # Arguments
///
/// * `model_name` - The model name to resolve (e.g., "User", "BlogPost")
/// * `model_infos` - Optional slice of ModelSchemaInfo for lookup
///
/// # Returns
///
/// The resolved table name
pub fn resolve_table_name_for_model(
	model_name: &str,
	model_infos: Option<&[ModelSchemaInfo]>,
) -> String {
	if let Some(infos) = model_infos {
		for info in infos {
			if info.name == model_name {
				return info.table_name.clone();
			}
		}
	}
	// Fall back to snake_case conversion
	to_snake_case(model_name)
}

/// Parse a string to ForeignKeyAction
///
/// Converts common FK action strings (case-insensitive) to the corresponding
/// ForeignKeyAction enum variant.
///
/// # Arguments
///
/// * `s` - The string to parse (e.g., "CASCADE", "SET NULL", "RESTRICT")
///
/// # Returns
///
/// * `ForeignKeyAction` - The parsed action, defaults to `Cascade` if unrecognized
pub fn parse_fk_action(s: &str) -> ForeignKeyAction {
	match s.to_uppercase().as_str() {
		"CASCADE" => ForeignKeyAction::Cascade,
		"SET NULL" | "SETNULL" | "SET_NULL" => ForeignKeyAction::SetNull,
		"SET DEFAULT" | "SETDEFAULT" | "SET_DEFAULT" => ForeignKeyAction::SetDefault,
		"RESTRICT" => ForeignKeyAction::Restrict,
		"NO ACTION" | "NOACTION" | "NO_ACTION" => ForeignKeyAction::NoAction,
		_ => ForeignKeyAction::Cascade,
	}
}

/// Extract FK actions (on_delete, on_update) from field attributes
///
/// Looks for "on_delete" and "on_update" keys in the field attributes HashMap
/// and converts them to ForeignKeyAction values.
///
/// # Arguments
///
/// * `field_attrs` - The field attributes containing potential FK action values
///
/// # Returns
///
/// * `(ForeignKeyAction, ForeignKeyAction)` - Tuple of (on_delete, on_update) actions.
///   Defaults to (Cascade, Cascade) if not specified.
pub fn extract_fk_actions(
	field_attrs: &HashMap<String, FieldKwarg>,
) -> (ForeignKeyAction, ForeignKeyAction) {
	let on_delete = field_attrs
		.get("on_delete")
		.and_then(|v| match v {
			FieldKwarg::String(s) => Some(parse_fk_action(s)),
			_ => None,
		})
		.unwrap_or(ForeignKeyAction::Cascade);

	let on_update = field_attrs
		.get("on_update")
		.and_then(|v| match v {
			FieldKwarg::String(s) => Some(parse_fk_action(s)),
			_ => None,
		})
		.unwrap_or(ForeignKeyAction::Cascade);

	(on_delete, on_update)
}

/// Infer table name from model name using snake_case conversion
///
/// This function converts a PascalCase model name to a snake_case table name.
/// For example: "BlogPost" -> "blog_post", "UserProfile" -> "user_profile"
///
/// # Arguments
///
/// * `model_name` - The model name to convert
///
/// # Returns
///
/// * `String` - The inferred table name in snake_case
pub fn infer_table_name(model_name: &str) -> String {
	to_snake_case(model_name)
}

/// Find field info for a relationship by matching FK column name
///
/// This helper finds the FieldInfo that corresponds to a relationship's
/// foreign key column, allowing us to extract FK actions from field attributes.
fn find_field_info_for_relation<'a>(
	relation_info: &RelationInfo,
	fields: &'a [FieldInfo],
) -> Option<&'a FieldInfo> {
	let fk_column = relation_info.foreign_key.as_deref().unwrap_or("");

	// Try to find by explicit foreign_key name
	if !fk_column.is_empty()
		&& let Some(field) = fields.iter().find(|f| f.name == fk_column)
	{
		return Some(field);
	}

	// Try to find by derived name (relation_name + "_id")
	let derived_fk = format!("{}_id", relation_info.name);
	fields.iter().find(|f| f.name == derived_fk)
}

/// Convert RelationInfo to Constraint with FK action extraction from field attributes
///
/// This function converts relationship metadata to a constraint definition.
/// Only ManyToOne and OneToOne relationships generate constraints on the source table.
/// FK actions (on_delete, on_update) are extracted from the corresponding field's attributes.
///
/// # Arguments
///
/// * `relation_info` - The relationship information to convert
/// * `source_table_name` - The name of the source table
/// * `model_infos` - Optional slice of ModelSchemaInfo for resolving related table names
/// * `fields` - Optional slice of FieldInfo for extracting FK actions from field attributes
///
/// # Returns
///
/// * `Some(Constraint)` - For ManyToOne and OneToOne relationships
/// * `None` - For OneToMany and ManyToMany relationships (FK is on the related table)
pub fn relation_info_to_constraint(
	relation_info: &RelationInfo,
	source_table_name: &str,
	model_infos: Option<&[ModelSchemaInfo]>,
	fields: Option<&[FieldInfo]>,
) -> Option<Constraint> {
	// Extract FK actions from field attributes if available
	let (on_delete, on_update) = fields
		.and_then(|f| find_field_info_for_relation(relation_info, f))
		.map(|field_info| extract_fk_actions(&field_info.attributes))
		.unwrap_or((ForeignKeyAction::Cascade, ForeignKeyAction::Cascade));

	match relation_info.relationship_type {
		RelationshipType::ManyToOne => {
			let referenced_table =
				resolve_table_name_for_model(&relation_info.related_model, model_infos);

			// Use the explicit foreign_key if provided, otherwise derive from relationship name
			let fk_column = relation_info
				.foreign_key
				.clone()
				.unwrap_or_else(|| format!("{}_id", relation_info.name));

			let constraint_name = format!(
				"fk_{}_{}_{}_id",
				source_table_name, fk_column, referenced_table
			);

			Some(Constraint::ForeignKey {
				name: constraint_name,
				columns: vec![fk_column],
				referenced_table,
				referenced_columns: vec!["id".to_string()],
				on_delete,
				on_update,
				deferrable: None,
			})
		}
		RelationshipType::OneToOne => {
			let referenced_table =
				resolve_table_name_for_model(&relation_info.related_model, model_infos);

			// Use the explicit foreign_key if provided, otherwise derive from relationship name
			let fk_column = relation_info
				.foreign_key
				.clone()
				.unwrap_or_else(|| format!("{}_id", relation_info.name));

			let constraint_name = format!(
				"oo_{}_{}_{}_id",
				source_table_name, fk_column, referenced_table
			);

			Some(Constraint::OneToOne {
				name: constraint_name,
				column: fk_column,
				referenced_table,
				referenced_column: "id".to_string(),
				on_delete,
				on_update,
				deferrable: None,
			})
		}
		// OneToMany and ManyToMany don't create FK constraints on the source table
		RelationshipType::OneToMany | RelationshipType::ManyToMany => None,
	}
}

/// Resolve model creation order using topological sort
///
/// This function takes a list of (model_name, dependencies) pairs and returns
/// the models sorted in an order where dependencies are created before dependents.
///
/// # Arguments
///
/// * `models` - List of (model_name, `Vec<dependency_names>`)
///
/// # Returns
///
/// * `Ok(Vec<String>)` - Model names in creation order
/// * `Err(SchemaError)` - If circular dependency is detected
pub fn resolve_model_order(models: &[(String, Vec<String>)]) -> Result<Vec<String>, SchemaError> {
	use std::collections::{HashSet, VecDeque};

	let model_names: HashSet<String> = models.iter().map(|(name, _)| name.clone()).collect();
	let mut in_degree: HashMap<String, usize> = HashMap::new();
	let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();

	// Initialize in-degree and adjacency list
	for (name, _) in models {
		in_degree.insert(name.clone(), 0);
		adjacency.insert(name.clone(), Vec::new());
	}

	// Build the graph
	for (name, deps) in models {
		for dep in deps {
			// Only count dependencies that are in our model set
			if model_names.contains(dep) {
				*in_degree.get_mut(name).unwrap() += 1;
				adjacency.get_mut(dep).unwrap().push(name.clone());
			}
		}
	}

	// Kahn's algorithm for topological sort
	let mut queue: VecDeque<String> = in_degree
		.iter()
		.filter(|&(_, &degree)| degree == 0)
		.map(|(name, _)| name.clone())
		.collect();

	let mut sorted = Vec::new();

	while let Some(node) = queue.pop_front() {
		sorted.push(node.clone());

		if let Some(neighbors) = adjacency.get(&node) {
			for neighbor in neighbors {
				if let Some(degree) = in_degree.get_mut(neighbor) {
					*degree -= 1;
					if *degree == 0 {
						queue.push_back(neighbor.clone());
					}
				}
			}
		}
	}

	// Check for circular dependency
	if sorted.len() != models.len() {
		let sorted_set: std::collections::HashSet<_> = sorted.iter().cloned().collect();
		let remaining: Vec<_> = model_names.difference(&sorted_set).collect();
		return Err(SchemaError::CircularDependency(format!(
			"Circular dependency detected involving: {:?}",
			remaining
		)));
	}

	Ok(sorted)
}

/// Model schema information for batch table creation
// Fixes #871
pub struct ModelSchemaInfo {
	/// Model name (used for dependency resolution)
	pub name: String,
	/// Table name in the database
	pub table_name: String,
	/// App label for the model
	pub app_label: String,
	/// Field metadata from the model
	pub fields: Vec<FieldInfo>,
	/// Relationship metadata from the model
	pub relationships: Vec<RelationInfo>,
}

impl ModelSchemaInfo {
	/// Create ModelSchemaInfo from a Model type
	pub fn from_model<M: Model>() -> Self {
		Self {
			name: std::any::type_name::<M>()
				.split("::")
				.last()
				.unwrap_or("Unknown")
				.to_string(),
			table_name: M::table_name().to_string(),
			app_label: M::app_label().to_string(),
			fields: M::field_metadata(),
			relationships: M::relationship_metadata(),
		}
	}

	/// Get dependencies for this model
	pub fn dependencies(&self) -> Vec<String> {
		extract_model_dependencies(&self.relationships)
	}
}

/// Create a CreateTable Operation from a Model type
///
/// This function extracts metadata from the Model trait implementation
/// and generates a CreateTable operation that can be executed by MigrationExecutor.
/// FK constraints are automatically generated from relationship metadata.
///
/// # Type Parameters
///
/// * `M` - A type implementing the Model trait
///
/// # Returns
///
/// * `Ok(Operation)` - The CreateTable operation with auto-generated FK constraints
/// * `Err(SchemaError)` - Error if field conversion fails
pub fn create_table_operation_from_model<M: Model>() -> Result<Operation, SchemaError> {
	create_table_operation_from_model_with_context::<M>(None)
}

/// Create a CreateTable Operation from a Model type with model context
///
/// This variant accepts optional model_infos for accurate FK constraint generation
/// when creating multiple related tables. FK actions (on_delete, on_update) are
/// automatically extracted from field attributes.
///
/// # Type Parameters
///
/// * `M` - A type implementing the Model trait
///
/// # Arguments
///
/// * `model_infos` - Optional slice of ModelSchemaInfo for resolving related table names
///
/// # Returns
///
/// * `Ok(Operation)` - The CreateTable operation with auto-generated FK constraints
/// * `Err(SchemaError)` - Error if field conversion fails
pub fn create_table_operation_from_model_with_context<M: Model>(
	model_infos: Option<&[ModelSchemaInfo]>,
) -> Result<Operation, SchemaError> {
	let table_name = M::table_name().to_string();

	// Get field metadata for FK action extraction
	let field_metadata = M::field_metadata();

	// Convert field metadata to column definitions
	let columns: Vec<ColumnDefinition> = field_metadata
		.iter()
		.map(field_info_to_column_definition)
		.collect::<Result<Vec<_>, _>>()?;

	// Generate FK constraints from relationship metadata with FK actions from field attributes
	let constraints: Vec<Constraint> = M::relationship_metadata()
		.iter()
		.filter_map(|rel| {
			relation_info_to_constraint(rel, &table_name, model_infos, Some(&field_metadata))
		})
		.collect();

	Ok(Operation::CreateTable {
		name: table_name,
		columns,
		constraints,
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	})
}

/// Create a Migration containing CreateTable for a single model
///
/// # Type Parameters
///
/// * `M` - A type implementing the Model trait
///
/// # Arguments
///
/// * `migration_name` - Name for the migration (e.g., "0001_create_users")
///
/// # Returns
///
/// * `Ok(Migration)` - The migration containing the CreateTable operation
/// * `Err(SchemaError)` - Error if operation creation fails
pub fn create_migration_from_model<M: Model>(
	migration_name: &str,
) -> Result<Migration, SchemaError> {
	let operation = create_table_operation_from_model::<M>()?;

	Ok(Migration {
		name: migration_name.to_string(),
		app_label: M::app_label().to_string(),
		operations: vec![operation],
		dependencies: vec![],
		replaces: vec![],
		atomic: true,
		initial: Some(true),
		state_only: false,
		database_only: false,
		optional_dependencies: vec![],
		swappable_dependencies: vec![],
	})
}

/// Create table operations from multiple models with dependency resolution
///
/// This function creates CreateTable operations for multiple models,
/// ordering them based on foreign key dependencies. FK constraints are
/// automatically generated from relationship metadata.
///
/// # Arguments
///
/// * `model_infos` - List of ModelSchemaInfo for each model
///
/// # Returns
///
/// * `Ok(Vec<Operation>)` - Operations in dependency-resolved order with FK constraints
/// * `Err(SchemaError)` - Error if resolution or conversion fails
pub fn create_table_operations_from_models(
	model_infos: Vec<ModelSchemaInfo>,
) -> Result<Vec<Operation>, SchemaError> {
	// Build dependency graph
	let models_with_deps: Vec<(String, Vec<String>)> = model_infos
		.iter()
		.map(|info| (info.name.clone(), info.dependencies()))
		.collect();

	// Resolve order
	let ordered_names = resolve_model_order(&models_with_deps)?;

	// Create operations in resolved order
	let name_to_info: HashMap<String, &ModelSchemaInfo> = model_infos
		.iter()
		.map(|info| (info.name.clone(), info))
		.collect();

	let mut operations = Vec::new();
	for name in ordered_names {
		if let Some(info) = name_to_info.get(&name) {
			let columns: Vec<ColumnDefinition> = info
				.fields
				.iter()
				.map(field_info_to_column_definition)
				.collect::<Result<Vec<_>, _>>()?;

			// Generate FK constraints from relationship metadata with FK actions from field attributes
			let constraints: Vec<Constraint> = info
				.relationships
				.iter()
				.filter_map(|rel| {
					relation_info_to_constraint(
						rel,
						&info.table_name,
						Some(&model_infos),
						Some(&info.fields),
					)
				})
				.collect();

			operations.push(Operation::CreateTable {
				name: info.table_name.clone(),
				columns,
				constraints,
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			});
		}
	}

	Ok(operations)
}

/// Create a database table for a single model
///
/// This is the main entry point for creating a table from a model definition.
/// It creates the migration and executes it against the provided database connection.
///
/// # Type Parameters
///
/// * `M` - A type implementing the Model trait
///
/// # Arguments
///
/// * `connection` - Database connection to execute the migration on
///
/// # Returns
///
/// * `Ok(())` - Table was created successfully
/// * `Err(SchemaError)` - Error during migration creation or execution
pub async fn create_table_for_model<M: Model>(
	connection: &DatabaseConnection,
) -> Result<(), SchemaError> {
	let migration = create_migration_from_model::<M>("0001_auto_create")?;

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());
	executor
		.apply_migrations(&[migration])
		.await
		.map_err(|e| SchemaError::MigrationExecution(e.to_string()))?;

	Ok(())
}

/// Create database tables for multiple models with dependency resolution
///
/// This function creates tables for all provided models, automatically
/// resolving foreign key dependencies to ensure tables are created in the correct order.
///
/// # Arguments
///
/// * `connection` - Database connection to execute migrations on
/// * `model_infos` - List of ModelSchemaInfo for each model to create
///
/// # Returns
///
/// * `Ok(())` - All tables were created successfully
/// * `Err(SchemaError)` - Error during resolution, creation, or execution
pub async fn create_tables_for_models(
	connection: &DatabaseConnection,
	model_infos: Vec<ModelSchemaInfo>,
) -> Result<(), SchemaError> {
	let operations = create_table_operations_from_models(model_infos)?;

	if operations.is_empty() {
		return Ok(());
	}

	let migration = Migration {
		name: "0001_auto_batch_create".to_string(),
		app_label: "test".to_string(),
		operations,
		dependencies: vec![],
		replaces: vec![],
		atomic: true,
		initial: Some(true),
		state_only: false,
		database_only: false,
		optional_dependencies: vec![],
		swappable_dependencies: vec![],
	};

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());
	executor
		.apply_migrations(&[migration])
		.await
		.map_err(|e| SchemaError::MigrationExecution(e.to_string()))?;

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_resolve_model_order_simple() {
		let models = vec![
			("User".to_string(), vec![]),
			("Post".to_string(), vec!["User".to_string()]),
			(
				"Comment".to_string(),
				vec!["Post".to_string(), "User".to_string()],
			),
		];

		let order = resolve_model_order(&models).unwrap();

		// User must come before Post and Comment
		let user_idx = order.iter().position(|n| n == "User").unwrap();
		let post_idx = order.iter().position(|n| n == "Post").unwrap();
		let comment_idx = order.iter().position(|n| n == "Comment").unwrap();

		assert!(user_idx < post_idx);
		assert!(user_idx < comment_idx);
		assert!(post_idx < comment_idx);
	}

	#[rstest]
	fn test_resolve_model_order_circular() {
		let models = vec![
			("A".to_string(), vec!["B".to_string()]),
			("B".to_string(), vec!["A".to_string()]),
		];

		let result = resolve_model_order(&models);
		assert!(result.is_err());
	}

	#[rstest]
	fn test_resolve_model_order_external_deps() {
		// Dependencies on models not in the set should be ignored
		let models = vec![
			("User".to_string(), vec!["ExternalModel".to_string()]),
			("Post".to_string(), vec!["User".to_string()]),
		];

		let order = resolve_model_order(&models).unwrap();
		assert_eq!(order.len(), 2);
	}

	#[rstest]
	fn test_extract_model_dependencies() {
		use reinhardt_db::orm::inspection::RelationInfo;

		let relations = vec![
			RelationInfo::new(
				"author".to_string(),
				RelationshipType::ManyToOne,
				"User".to_string(),
			),
			RelationInfo::new(
				"tags".to_string(),
				RelationshipType::ManyToMany,
				"Tag".to_string(),
			),
		];

		let deps = extract_model_dependencies(&relations);

		// Only ManyToOne creates a dependency
		assert_eq!(deps.len(), 1);
		assert!(deps.contains(&"User".to_string()));
	}

	#[rstest]
	fn test_resolve_table_name_for_model_with_model_infos() {
		let model_infos = vec![ModelSchemaInfo {
			name: "User".to_string(),
			table_name: "custom_users".to_string(),
			app_label: "test".to_string(),
			fields: vec![],
			relationships: vec![],
		}];

		let table_name = resolve_table_name_for_model("User", Some(&model_infos));
		assert_eq!(table_name, "custom_users");
	}

	#[rstest]
	fn test_resolve_table_name_for_model_fallback_to_snake_case() {
		let table_name = resolve_table_name_for_model("BlogPost", None);
		assert_eq!(table_name, "blog_post");
	}

	#[rstest]
	fn test_resolve_table_name_for_model_not_found_in_infos() {
		let model_infos = vec![ModelSchemaInfo {
			name: "User".to_string(),
			table_name: "users".to_string(),
			app_label: "test".to_string(),
			fields: vec![],
			relationships: vec![],
		}];

		// Model not in infos falls back to snake_case
		let table_name = resolve_table_name_for_model("BlogPost", Some(&model_infos));
		assert_eq!(table_name, "blog_post");
	}

	#[rstest]
	fn test_relation_info_to_constraint_many_to_one() {
		let relation = RelationInfo::new("author", RelationshipType::ManyToOne, "User")
			.with_foreign_key("author_id");

		let constraint = relation_info_to_constraint(&relation, "posts", None, None);

		assert!(constraint.is_some());
		match constraint.unwrap() {
			Constraint::ForeignKey {
				name,
				columns,
				referenced_table,
				referenced_columns,
				on_delete,
				on_update,
				..
			} => {
				assert_eq!(name, "fk_posts_author_id_user_id");
				assert_eq!(columns, vec!["author_id".to_string()]);
				assert_eq!(referenced_table, "user");
				assert_eq!(referenced_columns, vec!["id".to_string()]);
				assert!(matches!(on_delete, ForeignKeyAction::Cascade));
				assert!(matches!(on_update, ForeignKeyAction::Cascade));
			}
			_ => panic!("Expected ForeignKey constraint"),
		}
	}

	#[rstest]
	fn test_relation_info_to_constraint_many_to_one_without_explicit_fk() {
		let relation = RelationInfo::new("author", RelationshipType::ManyToOne, "User");

		let constraint = relation_info_to_constraint(&relation, "posts", None, None);

		assert!(constraint.is_some());
		match constraint.unwrap() {
			Constraint::ForeignKey { columns, .. } => {
				// Should derive FK column from relationship name
				assert_eq!(columns, vec!["author_id".to_string()]);
			}
			_ => panic!("Expected ForeignKey constraint"),
		}
	}

	#[rstest]
	fn test_relation_info_to_constraint_one_to_one() {
		let relation = RelationInfo::new("profile", RelationshipType::OneToOne, "UserProfile")
			.with_foreign_key("profile_id");

		let constraint = relation_info_to_constraint(&relation, "users", None, None);

		assert!(constraint.is_some());
		match constraint.unwrap() {
			Constraint::OneToOne {
				name,
				column,
				referenced_table,
				referenced_column,
				on_delete,
				on_update,
				..
			} => {
				assert_eq!(name, "oo_users_profile_id_user_profile_id");
				assert_eq!(column, "profile_id");
				assert_eq!(referenced_table, "user_profile");
				assert_eq!(referenced_column, "id");
				assert!(matches!(on_delete, ForeignKeyAction::Cascade));
				assert!(matches!(on_update, ForeignKeyAction::Cascade));
			}
			_ => panic!("Expected OneToOne constraint"),
		}
	}

	#[rstest]
	fn test_relation_info_to_constraint_one_to_many_returns_none() {
		let relation = RelationInfo::new("posts", RelationshipType::OneToMany, "Post");

		let constraint = relation_info_to_constraint(&relation, "users", None, None);

		// OneToMany should not create a constraint on the source table
		assert!(constraint.is_none());
	}

	#[rstest]
	fn test_relation_info_to_constraint_many_to_many_returns_none() {
		let relation = RelationInfo::new("tags", RelationshipType::ManyToMany, "Tag");

		let constraint = relation_info_to_constraint(&relation, "posts", None, None);

		// ManyToMany should not create a constraint on the source table
		assert!(constraint.is_none());
	}

	#[rstest]
	fn test_relation_info_to_constraint_with_model_infos() {
		let model_infos = vec![ModelSchemaInfo {
			name: "User".to_string(),
			table_name: "app_users".to_string(),
			app_label: "test".to_string(),
			fields: vec![],
			relationships: vec![],
		}];

		let relation = RelationInfo::new("author", RelationshipType::ManyToOne, "User")
			.with_foreign_key("author_id");

		let constraint = relation_info_to_constraint(&relation, "posts", Some(&model_infos), None);

		assert!(constraint.is_some());
		match constraint.unwrap() {
			Constraint::ForeignKey {
				referenced_table, ..
			} => {
				// Should use table name from model_infos
				assert_eq!(referenced_table, "app_users");
			}
			_ => panic!("Expected ForeignKey constraint"),
		}
	}

	#[rstest]
	fn test_parse_fk_action_cascade() {
		assert!(matches!(
			parse_fk_action("CASCADE"),
			ForeignKeyAction::Cascade
		));
		assert!(matches!(
			parse_fk_action("cascade"),
			ForeignKeyAction::Cascade
		));
		assert!(matches!(
			parse_fk_action("Cascade"),
			ForeignKeyAction::Cascade
		));
	}

	#[rstest]
	fn test_parse_fk_action_set_null() {
		assert!(matches!(
			parse_fk_action("SET NULL"),
			ForeignKeyAction::SetNull
		));
		assert!(matches!(
			parse_fk_action("SETNULL"),
			ForeignKeyAction::SetNull
		));
		assert!(matches!(
			parse_fk_action("SET_NULL"),
			ForeignKeyAction::SetNull
		));
	}

	#[rstest]
	fn test_parse_fk_action_restrict() {
		assert!(matches!(
			parse_fk_action("RESTRICT"),
			ForeignKeyAction::Restrict
		));
	}

	#[rstest]
	fn test_parse_fk_action_no_action() {
		assert!(matches!(
			parse_fk_action("NO ACTION"),
			ForeignKeyAction::NoAction
		));
		assert!(matches!(
			parse_fk_action("NOACTION"),
			ForeignKeyAction::NoAction
		));
		assert!(matches!(
			parse_fk_action("NO_ACTION"),
			ForeignKeyAction::NoAction
		));
	}

	#[rstest]
	fn test_parse_fk_action_set_default() {
		assert!(matches!(
			parse_fk_action("SET DEFAULT"),
			ForeignKeyAction::SetDefault
		));
		assert!(matches!(
			parse_fk_action("SETDEFAULT"),
			ForeignKeyAction::SetDefault
		));
		assert!(matches!(
			parse_fk_action("SET_DEFAULT"),
			ForeignKeyAction::SetDefault
		));
	}

	#[rstest]
	fn test_parse_fk_action_unknown_defaults_to_cascade() {
		assert!(matches!(
			parse_fk_action("UNKNOWN"),
			ForeignKeyAction::Cascade
		));
		assert!(matches!(parse_fk_action(""), ForeignKeyAction::Cascade));
	}

	#[rstest]
	fn test_extract_fk_actions_with_both_actions() {
		let mut attrs = HashMap::new();
		attrs.insert(
			"on_delete".to_string(),
			FieldKwarg::String("SET NULL".to_string()),
		);
		attrs.insert(
			"on_update".to_string(),
			FieldKwarg::String("RESTRICT".to_string()),
		);

		let (on_delete, on_update) = extract_fk_actions(&attrs);
		assert!(matches!(on_delete, ForeignKeyAction::SetNull));
		assert!(matches!(on_update, ForeignKeyAction::Restrict));
	}

	#[rstest]
	fn test_extract_fk_actions_with_only_on_delete() {
		let mut attrs = HashMap::new();
		attrs.insert(
			"on_delete".to_string(),
			FieldKwarg::String("RESTRICT".to_string()),
		);

		let (on_delete, on_update) = extract_fk_actions(&attrs);
		assert!(matches!(on_delete, ForeignKeyAction::Restrict));
		// on_update should default to Cascade
		assert!(matches!(on_update, ForeignKeyAction::Cascade));
	}

	#[rstest]
	fn test_extract_fk_actions_empty_attrs_defaults_to_cascade() {
		let attrs = HashMap::new();

		let (on_delete, on_update) = extract_fk_actions(&attrs);
		assert!(matches!(on_delete, ForeignKeyAction::Cascade));
		assert!(matches!(on_update, ForeignKeyAction::Cascade));
	}

	#[rstest]
	fn test_infer_table_name() {
		assert_eq!(infer_table_name("BlogPost"), "blog_post");
		assert_eq!(infer_table_name("UserProfile"), "user_profile");
		assert_eq!(infer_table_name("User"), "user");
	}

	#[rstest]
	fn test_relation_info_to_constraint_with_field_attrs() {
		// Create field info with on_delete attribute
		let mut attrs = HashMap::new();
		attrs.insert(
			"on_delete".to_string(),
			FieldKwarg::String("SET NULL".to_string()),
		);
		attrs.insert(
			"on_update".to_string(),
			FieldKwarg::String("NO ACTION".to_string()),
		);

		let field_info = FieldInfo {
			name: "author_id".to_string(),
			field_type: "BigInteger".to_string(),
			nullable: true,
			primary_key: false,
			unique: false,
			blank: false,
			editable: true,
			default: None,
			db_default: None,
			db_column: None,
			choices: None,
			attributes: attrs,
		};

		let relation = RelationInfo::new("author", RelationshipType::ManyToOne, "User")
			.with_foreign_key("author_id");

		let constraint = relation_info_to_constraint(&relation, "posts", None, Some(&[field_info]));

		assert!(constraint.is_some());
		match constraint.unwrap() {
			Constraint::ForeignKey {
				on_delete,
				on_update,
				..
			} => {
				assert!(matches!(on_delete, ForeignKeyAction::SetNull));
				assert!(matches!(on_update, ForeignKeyAction::NoAction));
			}
			_ => panic!("Expected ForeignKey constraint"),
		}
	}
}
