//! Model Schema Registration for WASM Plugins
//!
//! This module provides model schema registration functionality that allows
//! WASM plugins to declare their database models. The registered schemas
//! can be used for:
//!
//! - Migration generation (via raw SQL)
//! - Documentation and introspection
//! - Runtime validation
//!
//! # Limitations
//!
//! Full ORM model integration with compile-time registration (using `#[model(...)]`)
//! is only available for Static (Rust) plugins. WASM plugins receive a simplified
//! interface focused on schema declaration and raw SQL migrations.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Database column type.
///
/// This enum is marked `#[non_exhaustive]` to allow adding new column types
/// in future minor versions without breaking downstream code. Match arms
/// should include a wildcard pattern.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ColumnType {
	/// Integer (4 bytes)
	Integer,
	/// Big integer (8 bytes)
	BigInteger,
	/// Variable-length text
	Text,
	/// Fixed-length string with max length
	Varchar(u32),
	/// Boolean
	Boolean,
	/// Timestamp with timezone
	Timestamp,
	/// UUID
	Uuid,
	/// JSON/JSONB
	Json,
	/// Decimal with precision (total digits) and scale (digits after decimal point).
	///
	/// - `precision`: Total number of digits (1-38 typically)
	/// - `scale`: Number of digits after the decimal point
	// Inline struct field documentation is not supported in Rust
	#[allow(missing_docs)]
	Decimal { precision: u8, scale: u8 },
	/// Foreign key reference to another table
	ForeignKey(String),
}

/// Column definition for a model field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnDef {
	/// Column name
	pub name: String,
	/// Column type
	pub column_type: ColumnType,
	/// Whether the column allows NULL values
	pub nullable: bool,
	/// Whether this is a primary key
	pub primary_key: bool,
	/// Whether this column has a unique constraint
	pub unique_value: bool,
	/// Default value expression (SQL literal or function call)
	pub default_value: Option<String>,
}

/// Index definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexDef {
	/// Index name
	pub name: String,
	/// Column names included in the index
	pub columns: Vec<String>,
	/// Whether this is a unique index
	pub unique_value: bool,
}

/// Model schema definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelSchema {
	/// Database table name
	pub table_name: String,
	/// Column definitions
	pub columns: Vec<ColumnDef>,
	/// Index definitions
	pub indexes: Vec<IndexDef>,
}

impl ModelSchema {
	/// Create a new model schema.
	pub fn new(table_name: impl Into<String>) -> Self {
		Self {
			table_name: table_name.into(),
			columns: Vec::new(),
			indexes: Vec::new(),
		}
	}

	/// Add a column to the schema.
	pub fn column(mut self, column: ColumnDef) -> Self {
		self.columns.push(column);
		self
	}

	/// Add an index to the schema.
	pub fn index(mut self, index: IndexDef) -> Self {
		self.indexes.push(index);
		self
	}
}

/// Raw SQL migration for plugins that need full control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqlMigration {
	/// Migration version/name (e.g., "0001_create_plugin_tables")
	pub version: String,
	/// Description of what this migration does
	pub description: String,
	/// SQL to apply when migrating up
	pub up_sql: String,
	/// SQL to apply when rolling back
	pub down_sql: String,
}

impl SqlMigration {
	/// Create a new SQL migration.
	pub fn new(
		version: impl Into<String>,
		description: impl Into<String>,
		up_sql: impl Into<String>,
		down_sql: impl Into<String>,
	) -> Self {
		Self {
			version: version.into(),
			description: description.into(),
			up_sql: up_sql.into(),
			down_sql: down_sql.into(),
		}
	}
}

/// Model registry for tracking plugin-registered schemas and migrations.
///
/// This registry stores model schemas and migrations registered by WASM plugins.
/// It can be shared across multiple plugin instances to provide a unified view
/// of all plugin-defined models.
#[derive(Debug, Default)]
pub struct ModelRegistry {
	/// Registered model schemas, keyed by (plugin_name, table_name)
	schemas: RwLock<HashMap<(String, String), ModelSchema>>,
	/// Registered SQL migrations, keyed by (plugin_name, version)
	migrations: RwLock<HashMap<(String, String), SqlMigration>>,
}

impl ModelRegistry {
	/// Create a new empty model registry.
	pub fn new() -> Self {
		Self::default()
	}

	/// Register a model schema for a plugin.
	///
	/// Returns an error if a schema with the same table name is already registered
	/// by the same plugin.
	pub fn register_model(&self, plugin_name: &str, schema: ModelSchema) -> Result<(), String> {
		let key = (plugin_name.to_string(), schema.table_name.clone());
		let mut schemas = self.schemas.write();

		if schemas.contains_key(&key) {
			return Err(format!(
				"Model '{}' already registered by plugin '{}'",
				schema.table_name, plugin_name
			));
		}

		schemas.insert(key, schema);
		Ok(())
	}

	/// Register a SQL migration for a plugin.
	///
	/// Returns an error if a migration with the same version is already registered
	/// by the same plugin.
	pub fn register_migration(
		&self,
		plugin_name: &str,
		migration: SqlMigration,
	) -> Result<(), String> {
		let key = (plugin_name.to_string(), migration.version.clone());
		let mut migrations = self.migrations.write();

		if migrations.contains_key(&key) {
			return Err(format!(
				"Migration '{}' already registered by plugin '{}'",
				migration.version, plugin_name
			));
		}

		migrations.insert(key, migration);
		Ok(())
	}

	/// List all model table names registered by a specific plugin.
	pub fn list_models(&self, plugin_name: &str) -> Vec<String> {
		self.schemas
			.read()
			.iter()
			.filter(|((p, _), _)| p == plugin_name)
			.map(|((_, table_name), _)| table_name.clone())
			.collect()
	}

	/// Get a model schema by plugin name and table name.
	pub fn get_model(&self, plugin_name: &str, table_name: &str) -> Option<ModelSchema> {
		let key = (plugin_name.to_string(), table_name.to_string());
		self.schemas.read().get(&key).cloned()
	}

	/// List all migrations registered by a specific plugin.
	pub fn list_migrations(&self, plugin_name: &str) -> Vec<SqlMigration> {
		self.migrations
			.read()
			.iter()
			.filter(|((p, _), _)| p == plugin_name)
			.map(|(_, m)| m.clone())
			.collect()
	}

	/// Get all registered schemas.
	pub fn all_schemas(&self) -> Vec<(String, ModelSchema)> {
		self.schemas
			.read()
			.iter()
			.map(|((plugin, _), schema)| (plugin.clone(), schema.clone()))
			.collect()
	}

	/// Get all registered migrations.
	pub fn all_migrations(&self) -> Vec<(String, SqlMigration)> {
		self.migrations
			.read()
			.iter()
			.map(|((plugin, _), migration)| (plugin.clone(), migration.clone()))
			.collect()
	}

	/// Remove all schemas and migrations registered by a specific plugin.
	///
	/// Returns the number of items removed.
	pub fn remove_plugin_entries(&self, plugin_name: &str) -> usize {
		let mut schemas = self.schemas.write();
		let mut migrations = self.migrations.write();

		let schema_count = schemas.len();
		let migration_count = migrations.len();

		schemas.retain(|(p, _), _| p != plugin_name);
		migrations.retain(|(p, _), _| p != plugin_name);

		(schema_count - schemas.len()) + (migration_count - migrations.len())
	}

	/// Get the total number of registered schemas.
	pub fn schema_count(&self) -> usize {
		self.schemas.read().len()
	}

	/// Get the total number of registered migrations.
	pub fn migration_count(&self) -> usize {
		self.migrations.read().len()
	}
}

/// Shared model registry instance type.
pub type SharedModelRegistry = std::sync::Arc<ModelRegistry>;

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_column_type_serde() {
		let types = vec![
			ColumnType::Integer,
			ColumnType::BigInteger,
			ColumnType::Text,
			ColumnType::Varchar(255),
			ColumnType::Boolean,
			ColumnType::Timestamp,
			ColumnType::Uuid,
			ColumnType::Json,
			ColumnType::Decimal {
				precision: 10,
				scale: 2,
			},
			ColumnType::ForeignKey("users".to_string()),
		];

		for col_type in types {
			let json = serde_json::to_string(&col_type).unwrap();
			let parsed: ColumnType = serde_json::from_str(&json).unwrap();
			assert_eq!(col_type, parsed);
		}
	}

	#[test]
	fn test_model_schema_builder() {
		let schema = ModelSchema::new("users")
			.column(ColumnDef {
				name: "id".to_string(),
				column_type: ColumnType::Integer,
				nullable: false,
				primary_key: true,
				unique_value: true,
				default_value: None,
			})
			.column(ColumnDef {
				name: "email".to_string(),
				column_type: ColumnType::Varchar(255),
				nullable: false,
				primary_key: false,
				unique_value: true,
				default_value: None,
			})
			.index(IndexDef {
				name: "idx_users_email".to_string(),
				columns: vec!["email".to_string()],
				unique_value: true,
			});

		assert_eq!(schema.table_name, "users");
		assert_eq!(schema.columns.len(), 2);
		assert_eq!(schema.indexes.len(), 1);
	}

	#[test]
	fn test_model_registry_register_model() {
		let registry = ModelRegistry::new();

		let schema = ModelSchema::new("products").column(ColumnDef {
			name: "id".to_string(),
			column_type: ColumnType::Integer,
			nullable: false,
			primary_key: true,
			unique_value: true,
			default_value: None,
		});

		// First registration succeeds
		assert!(
			registry
				.register_model("test-plugin", schema.clone())
				.is_ok()
		);

		// Duplicate registration fails
		assert!(
			registry
				.register_model("test-plugin", schema.clone())
				.is_err()
		);

		// Same table name from different plugin succeeds
		assert!(registry.register_model("other-plugin", schema).is_ok());
	}

	#[test]
	fn test_model_registry_register_migration() {
		let registry = ModelRegistry::new();

		let migration = SqlMigration::new(
			"0001_initial",
			"Create initial tables",
			"CREATE TABLE products (id INT PRIMARY KEY);",
			"DROP TABLE products;",
		);

		// First registration succeeds
		assert!(
			registry
				.register_migration("test-plugin", migration.clone())
				.is_ok()
		);

		// Duplicate registration fails
		assert!(
			registry
				.register_migration("test-plugin", migration.clone())
				.is_err()
		);

		// Same version from different plugin succeeds
		assert!(
			registry
				.register_migration("other-plugin", migration)
				.is_ok()
		);
	}

	#[test]
	fn test_model_registry_list_models() {
		let registry = ModelRegistry::new();

		registry
			.register_model("plugin-a", ModelSchema::new("users"))
			.unwrap();
		registry
			.register_model("plugin-a", ModelSchema::new("orders"))
			.unwrap();
		registry
			.register_model("plugin-b", ModelSchema::new("products"))
			.unwrap();

		let plugin_a_models = registry.list_models("plugin-a");
		assert_eq!(plugin_a_models.len(), 2);
		assert!(plugin_a_models.contains(&"users".to_string()));
		assert!(plugin_a_models.contains(&"orders".to_string()));

		let plugin_b_models = registry.list_models("plugin-b");
		assert_eq!(plugin_b_models.len(), 1);
		assert!(plugin_b_models.contains(&"products".to_string()));
	}

	#[test]
	fn test_model_registry_remove_plugin_entries() {
		let registry = ModelRegistry::new();

		registry
			.register_model("plugin-a", ModelSchema::new("users"))
			.unwrap();
		registry
			.register_model("plugin-a", ModelSchema::new("orders"))
			.unwrap();
		registry
			.register_migration("plugin-a", SqlMigration::new("0001", "desc", "up", "down"))
			.unwrap();
		registry
			.register_model("plugin-b", ModelSchema::new("products"))
			.unwrap();

		assert_eq!(registry.schema_count(), 3);
		assert_eq!(registry.migration_count(), 1);

		let removed = registry.remove_plugin_entries("plugin-a");
		assert_eq!(removed, 3); // 2 schemas + 1 migration

		assert_eq!(registry.schema_count(), 1);
		assert_eq!(registry.migration_count(), 0);
	}

	#[test]
	fn test_model_registry_get_model() {
		let registry = ModelRegistry::new();

		let schema = ModelSchema::new("users").column(ColumnDef {
			name: "id".to_string(),
			column_type: ColumnType::Integer,
			nullable: false,
			primary_key: true,
			unique_value: true,
			default_value: None,
		});

		registry.register_model("test-plugin", schema).unwrap();

		let retrieved = registry.get_model("test-plugin", "users");
		assert!(retrieved.is_some());
		assert_eq!(retrieved.unwrap().table_name, "users");

		let not_found = registry.get_model("test-plugin", "nonexistent");
		assert!(not_found.is_none());
	}
}
