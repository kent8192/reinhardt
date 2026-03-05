//! PostgreSQL-specific migration operations
//!
//! This module provides PostgreSQL-specific operations inspired by Django's
//! `django/contrib/postgres/operations.py`. These operations allow you to:
//!
//! - Create and manage PostgreSQL extensions
//! - Use PostgreSQL-specific index types (GIN, GiST, BRIN, etc.)
//! - Work with PostgreSQL functions and triggers
//!
//! # Example
//!
//! ```rust
//! use reinhardt_db::migrations::operations::postgres::{CreateExtension, CreateCollation};
//!
//! // Create the hstore extension
//! let ext = CreateExtension::new("hstore");
//!
//! // Create a custom collation
//! let collation = CreateCollation::new("german", "de_DE");
//! ```

use crate::backends::schema::BaseDatabaseSchemaEditor;
use crate::migrations::ProjectState;
use pg_escape::quote_literal;
use serde::{Deserialize, Serialize};

/// Create a PostgreSQL extension
///
/// PostgreSQL extensions add additional functionality to the database.
/// Common extensions include hstore, postgis, pg_trgm, and many others.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::operations::postgres::CreateExtension;
/// use reinhardt_db::migrations::ProjectState;
///
/// let mut state = ProjectState::new();
/// let ext = CreateExtension::new("hstore");
///
/// // Extensions don't modify project state
/// ext.state_forwards("myapp", &mut state);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateExtension {
	pub name: String,
	pub schema: Option<String>,
	pub version: Option<String>,
}

impl CreateExtension {
	/// Create a new CreateExtension operation
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::operations::postgres::CreateExtension;
	///
	/// let ext = CreateExtension::new("hstore");
	/// assert_eq!(ext.name, "hstore");
	/// assert!(ext.schema.is_none());
	/// ```
	pub fn new(name: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			schema: None,
			version: None,
		}
	}

	/// Set the schema where the extension should be created
	pub fn with_schema(mut self, schema: impl Into<String>) -> Self {
		self.schema = Some(schema.into());
		self
	}

	/// Set a specific version of the extension
	pub fn with_version(mut self, version: impl Into<String>) -> Self {
		self.version = Some(version.into());
		self
	}

	/// Apply to project state (extensions don't modify state)
	pub fn state_forwards(&self, _app_label: &str, _state: &mut ProjectState) {
		// Extensions are database-level and don't affect the application schema
	}

	/// Generate SQL using schema editor
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::migrations::operations::postgres::CreateExtension;
	/// use reinhardt_db::backends::schema::factory::{SchemaEditorFactory, DatabaseType};
	///
	/// let ext = CreateExtension::new("hstore").with_schema("public");
	/// let factory = SchemaEditorFactory::new();
	/// let editor = factory.create_for_database(DatabaseType::PostgreSQL);
	///
	/// let sql = ext.database_forwards(editor.as_ref());
	/// assert_eq!(sql.len(), 1);
	/// assert!(sql[0].contains("CREATE EXTENSION"));
	/// assert!(sql[0].contains("hstore"));
	/// ```
	pub fn database_forwards(&self, _schema_editor: &dyn BaseDatabaseSchemaEditor) -> Vec<String> {
		let mut parts = vec!["CREATE EXTENSION IF NOT EXISTS"];
		// Always use double quotes for PostgreSQL identifier safety
		parts.push(Box::leak(format!("\"{}\"", self.name).into_boxed_str()));

		if let Some(ref schema) = self.schema {
			parts.push("SCHEMA");
			parts.push(Box::leak(format!("\"{}\"", schema).into_boxed_str()));
		}

		if let Some(ref version) = self.version {
			parts.push("VERSION");
			parts.push(Box::leak(quote_literal(version).into()));
		}

		vec![format!("{};", parts.join(" "))]
	}

	/// Generate reverse SQL
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::migrations::operations::postgres::CreateExtension;
	/// use reinhardt_db::backends::schema::factory::{SchemaEditorFactory, DatabaseType};
	///
	/// let ext = CreateExtension::new("hstore");
	/// let factory = SchemaEditorFactory::new();
	/// let editor = factory.create_for_database(DatabaseType::PostgreSQL);
	///
	/// let sql = ext.database_backwards(editor.as_ref());
	/// assert_eq!(sql.len(), 1);
	/// assert!(sql[0].contains("DROP EXTENSION"));
	/// ```
	pub fn database_backwards(&self, _schema_editor: &dyn BaseDatabaseSchemaEditor) -> Vec<String> {
		vec![format!("DROP EXTENSION IF EXISTS \"{}\";", self.name)]
	}
}

/// Drop a PostgreSQL extension
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::operations::postgres::DropExtension;
///
/// let drop = DropExtension::new("hstore");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropExtension {
	pub name: String,
}

impl DropExtension {
	/// Create a new DropExtension operation
	pub fn new(name: impl Into<String>) -> Self {
		Self { name: name.into() }
	}

	/// Apply to project state (extensions don't modify state)
	pub fn state_forwards(&self, _app_label: &str, _state: &mut ProjectState) {}

	/// Generate SQL
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::migrations::operations::postgres::DropExtension;
	/// use reinhardt_db::backends::schema::factory::{SchemaEditorFactory, DatabaseType};
	///
	/// let drop = DropExtension::new("hstore");
	/// let factory = SchemaEditorFactory::new();
	/// let editor = factory.create_for_database(DatabaseType::PostgreSQL);
	///
	/// let sql = drop.database_forwards(editor.as_ref());
	/// assert_eq!(sql.len(), 1);
	/// assert!(sql[0].contains("DROP EXTENSION"));
	/// ```
	pub fn database_forwards(&self, _schema_editor: &dyn BaseDatabaseSchemaEditor) -> Vec<String> {
		vec![format!("DROP EXTENSION IF EXISTS \"{}\";", self.name)]
	}

	/// Generate reverse SQL (recreate extension)
	pub fn database_backwards(&self, _schema_editor: &dyn BaseDatabaseSchemaEditor) -> Vec<String> {
		vec![format!("CREATE EXTENSION IF NOT EXISTS \"{}\";", self.name)]
	}
}

/// Create a PostgreSQL collation
///
/// Collations define how text is sorted and compared in the database.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::operations::postgres::CreateCollation;
///
/// let collation = CreateCollation::new("german", "de_DE");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCollation {
	pub name: String,
	pub locale: String,
	pub provider: Option<String>,
}

impl CreateCollation {
	/// Create a new collation
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::operations::postgres::CreateCollation;
	///
	/// let collation = CreateCollation::new("german", "de_DE");
	/// assert_eq!(collation.name, "german");
	/// assert_eq!(collation.locale, "de_DE");
	/// ```
	pub fn new(name: impl Into<String>, locale: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			locale: locale.into(),
			provider: None,
		}
	}

	/// Set the collation provider (icu or libc)
	pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
		self.provider = Some(provider.into());
		self
	}

	/// Apply to project state
	pub fn state_forwards(&self, _app_label: &str, _state: &mut ProjectState) {}

	/// Generate SQL
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::migrations::operations::postgres::CreateCollation;
	/// use reinhardt_db::backends::schema::factory::{SchemaEditorFactory, DatabaseType};
	///
	/// let collation = CreateCollation::new("german", "de_DE");
	/// let factory = SchemaEditorFactory::new();
	/// let editor = factory.create_for_database(DatabaseType::PostgreSQL);
	///
	/// let sql = collation.database_forwards(editor.as_ref());
	/// assert_eq!(sql.len(), 1);
	/// assert!(sql[0].contains("CREATE COLLATION"));
	/// assert!(sql[0].contains("german"));
	/// ```
	pub fn database_forwards(&self, _schema_editor: &dyn BaseDatabaseSchemaEditor) -> Vec<String> {
		// Always use double quotes for PostgreSQL identifier safety
		let mut sql = format!(
			"CREATE COLLATION IF NOT EXISTS \"{}\" (LOCALE = {}",
			self.name,
			quote_literal(&self.locale)
		);

		if let Some(ref provider) = self.provider {
			sql.push_str(&format!(", PROVIDER = {}", quote_literal(provider)));
		}

		sql.push_str(");");
		vec![sql]
	}

	/// Generate reverse SQL
	pub fn database_backwards(&self, _schema_editor: &dyn BaseDatabaseSchemaEditor) -> Vec<String> {
		vec![format!("DROP COLLATION IF EXISTS \"{}\";", self.name)]
	}
}

/// Commonly used PostgreSQL extensions
pub mod extensions {
	use super::CreateExtension;

	/// Create the hstore extension for key-value storage
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::operations::postgres::extensions::hstore;
	///
	/// let ext = hstore();
	/// assert_eq!(ext.name, "hstore");
	/// ```
	pub fn hstore() -> CreateExtension {
		CreateExtension::new("hstore")
	}

	/// Create the pg_trgm extension for trigram matching
	pub fn pg_trgm() -> CreateExtension {
		CreateExtension::new("pg_trgm")
	}

	/// Create the uuid-ossp extension for UUID generation
	pub fn uuid_ossp() -> CreateExtension {
		CreateExtension::new("uuid-ossp")
	}

	/// Create the postgis extension for geographic data
	pub fn postgis() -> CreateExtension {
		CreateExtension::new("postgis")
	}

	/// Create the btree_gin extension for B-tree GIN indexes
	pub fn btree_gin() -> CreateExtension {
		CreateExtension::new("btree_gin")
	}

	/// Create the btree_gist extension for B-tree GiST indexes
	pub fn btree_gist() -> CreateExtension {
		CreateExtension::new("btree_gist")
	}

	/// Create the citext extension for case-insensitive text
	pub fn citext() -> CreateExtension {
		CreateExtension::new("citext")
	}

	/// Create the unaccent extension for removing accents
	pub fn unaccent() -> CreateExtension {
		CreateExtension::new("unaccent")
	}
}

// MigrationOperation trait implementation for Django-style naming
use crate::migrations::operation_trait::MigrationOperation;

impl MigrationOperation for CreateExtension {
	fn migration_name_fragment(&self) -> Option<String> {
		Some(format!("create_extension_{}", self.name.to_lowercase()))
	}

	fn describe(&self) -> String {
		format!("Create PostgreSQL extension {}", self.name)
	}
}

impl MigrationOperation for DropExtension {
	fn migration_name_fragment(&self) -> Option<String> {
		Some(format!("drop_extension_{}", self.name.to_lowercase()))
	}

	fn describe(&self) -> String {
		format!("Drop PostgreSQL extension {}", self.name)
	}
}

impl MigrationOperation for CreateCollation {
	fn migration_name_fragment(&self) -> Option<String> {
		Some(format!("create_collation_{}", self.name.to_lowercase()))
	}

	fn describe(&self) -> String {
		format!("Create collation {}", self.name)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_create_extension_basic() {
		let ext = CreateExtension::new("hstore");
		assert_eq!(ext.name, "hstore");
		assert!(ext.schema.is_none());
		assert!(ext.version.is_none());
	}

	#[test]
	fn test_create_extension_with_schema() {
		let ext = CreateExtension::new("hstore").with_schema("public");
		assert_eq!(ext.name, "hstore");
		assert_eq!(ext.schema, Some("public".to_string()));
	}

	#[test]
	fn test_create_extension_with_version() {
		let ext = CreateExtension::new("postgis").with_version("3.0.0");
		assert_eq!(ext.version, Some("3.0.0".to_string()));
	}

	#[cfg(feature = "postgres")]
	#[test]
	fn test_create_extension_database_forwards() {
		use crate::backends::schema::test_utils::MockSchemaEditor;

		let ext = CreateExtension::new("hstore");
		let editor = MockSchemaEditor::new();

		let sql = ext.database_forwards(&editor);
		assert_eq!(sql.len(), 1);
		assert!(sql[0].contains("CREATE EXTENSION IF NOT EXISTS"));
		assert!(sql[0].contains("\"hstore\""));
	}

	#[cfg(feature = "postgres")]
	#[test]
	fn test_create_extension_with_schema_sql() {
		use crate::backends::schema::test_utils::MockSchemaEditor;

		let ext = CreateExtension::new("hstore").with_schema("public");
		let editor = MockSchemaEditor::new();

		let sql = ext.database_forwards(&editor);
		assert!(sql[0].contains("SCHEMA"));
		assert!(sql[0].contains("\"public\""));
	}

	#[cfg(feature = "postgres")]
	#[test]
	fn test_drop_extension() {
		use crate::backends::schema::test_utils::MockSchemaEditor;

		let drop = DropExtension::new("hstore");
		let editor = MockSchemaEditor::new();

		let sql = drop.database_forwards(&editor);
		assert_eq!(sql.len(), 1);
		assert!(sql[0].contains("DROP EXTENSION IF EXISTS"));
		assert!(sql[0].contains("\"hstore\""));
	}

	#[cfg(feature = "postgres")]
	#[test]
	fn test_create_collation() {
		use crate::backends::schema::test_utils::MockSchemaEditor;

		let collation = CreateCollation::new("german", "de_DE");
		let editor = MockSchemaEditor::new();

		let sql = collation.database_forwards(&editor);
		assert_eq!(sql.len(), 1);
		assert!(sql[0].contains("CREATE COLLATION IF NOT EXISTS"));
		assert!(sql[0].contains("\"german\""));
		assert!(sql[0].contains("de_DE"));
	}

	#[test]
	fn test_extension_helpers() {
		let hstore = extensions::hstore();
		assert_eq!(hstore.name, "hstore");

		let pg_trgm = extensions::pg_trgm();
		assert_eq!(pg_trgm.name, "pg_trgm");

		let postgis = extensions::postgis();
		assert_eq!(postgis.name, "postgis");
	}
}
