//! Model operations for migrations
//!
//! This module provides operations for creating, deleting, and renaming models,
//! inspired by Django's `django/db/migrations/operations/models.py`.
//!
//! # Example
//!
//! ```rust
//! use reinhardt_db::migrations::operations::models::{CreateModel, DeleteModel};
//! use reinhardt_db::migrations::operations::FieldDefinition;
//! use reinhardt_db::migrations::{ProjectState, FieldType};
//!
//! let mut state = ProjectState::new();
//!
//! // Create a model
//! let create = CreateModel::new(
//!     "User",
//!     vec![
//!         FieldDefinition::new("id", FieldType::Integer, true, false, Option::<&str>::None),
//!         FieldDefinition::new("email", FieldType::VarChar(255), false, false, Option::<&str>::None),
//!     ],
//! );
//! create.state_forwards("myapp", &mut state);
//! assert!(state.get_model("myapp", "User").is_some());
//!
//! // Delete a model
//! let delete = DeleteModel::new("User");
//! delete.state_forwards("myapp", &mut state);
//! assert!(state.get_model("myapp", "User").is_none());
//! ```

use super::{FieldState, ModelState, ProjectState};
use crate::backends::schema::BaseDatabaseSchemaEditor;
use crate::backends::types::DatabaseType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Validation errors that can occur during migration operations
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
	/// Composite primary key list is empty
	EmptyCompositePrimaryKey { table_name: String },
	/// Field specified in composite primary key does not exist in table
	NonExistentField {
		field_name: String,
		table_name: String,
		available_fields: Vec<String>,
	},
}

impl fmt::Display for ValidationError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			ValidationError::EmptyCompositePrimaryKey { table_name } => {
				write!(
					f,
					"Composite primary key for table '{}' cannot be empty",
					table_name
				)
			}
			ValidationError::NonExistentField {
				field_name,
				table_name,
				available_fields,
			} => {
				write!(
					f,
					"Field '{}' does not exist in table '{}'. Available fields: [{}]",
					field_name,
					table_name,
					available_fields.join(", ")
				)
			}
		}
	}
}

impl std::error::Error for ValidationError {}

/// Result type for migration operations
pub type ValidationResult<T> = Result<T, ValidationError>;

/// Quote an identifier for the given database type
///
/// # Arguments
///
/// * `identifier` - The identifier to quote (table name, column name, etc.)
/// * `database_type` - The database type to use for quoting
///
/// # Returns
///
/// Quoted identifier suitable for the database type:
/// - PostgreSQL/SQLite: `"identifier"`
/// - MySQL: `` `identifier` ``
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::operations::models::quote_identifier;
/// use reinhardt_db::backends::types::DatabaseType;
///
/// let postgres_quoted = quote_identifier("user", DatabaseType::Postgres);
/// assert_eq!(postgres_quoted, "\"user\"");
///
/// let mysql_quoted = quote_identifier("order", DatabaseType::Mysql);
/// assert_eq!(mysql_quoted, "`order`");
/// ```
pub fn quote_identifier(identifier: &str, database_type: DatabaseType) -> String {
	match database_type {
		DatabaseType::Postgres | DatabaseType::Sqlite => {
			// PostgreSQL and SQLite use double quotes
			// Escape existing double quotes by doubling them
			format!("\"{}\"", identifier.replace('"', "\"\""))
		}
		DatabaseType::Mysql => {
			// MySQL uses backticks
			// Escape existing backticks by doubling them
			format!("`{}`", identifier.replace('`', "``"))
		}
	}
}

/// Field definition for model operations
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::operations::FieldDefinition;
/// use reinhardt_db::migrations::FieldType;
///
/// let field = FieldDefinition::new("email", FieldType::VarChar(255), false, false, Some("''"));
/// assert_eq!(field.name, "email");
/// assert_eq!(field.field_type, FieldType::VarChar(255));
/// assert!(!field.primary_key);
/// assert!(!field.unique);
/// assert_eq!(field.default, Some("''".to_string()));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FieldDefinition {
	pub name: String,
	pub field_type: crate::migrations::FieldType,
	pub primary_key: bool,
	pub unique: bool,
	pub default: Option<String>,
	pub null: bool,

	// Generated Columns (all DBMS)
	pub generated: Option<String>,
	pub generated_stored: Option<bool>,
	#[cfg(any(feature = "db-mysql", feature = "db-sqlite"))]
	pub generated_virtual: Option<bool>,

	// Identity/Auto-increment
	#[cfg(feature = "db-postgres")]
	pub identity_always: Option<bool>,
	#[cfg(feature = "db-postgres")]
	pub identity_by_default: Option<bool>,
	#[cfg(feature = "db-mysql")]
	pub auto_increment: Option<bool>,
	#[cfg(feature = "db-sqlite")]
	pub autoincrement: Option<bool>,

	// Character Set & Collation
	pub collate: Option<String>,
	#[cfg(feature = "db-mysql")]
	pub character_set: Option<String>,

	// Comment
	#[cfg(any(feature = "db-postgres", feature = "db-mysql"))]
	pub comment: Option<String>,

	// Storage Optimization (PostgreSQL)
	#[cfg(feature = "db-postgres")]
	pub storage: Option<String>,
	#[cfg(feature = "db-postgres")]
	pub compression: Option<String>,

	// ON UPDATE Trigger (MySQL)
	#[cfg(feature = "db-mysql")]
	pub on_update_current_timestamp: Option<bool>,

	// Invisible Columns (MySQL)
	#[cfg(feature = "db-mysql")]
	pub invisible: Option<bool>,

	// Full-Text Index (PostgreSQL, MySQL)
	#[cfg(any(feature = "db-postgres", feature = "db-mysql"))]
	pub fulltext: Option<bool>,

	// Numeric Attributes (MySQL, deprecated)
	#[cfg(feature = "db-mysql")]
	pub unsigned: Option<bool>,
	#[cfg(feature = "db-mysql")]
	pub zerofill: Option<bool>,
}

impl FieldDefinition {
	/// Create a new field definition
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::operations::FieldDefinition;
	/// use reinhardt_db::migrations::FieldType;
	///
	/// let field = FieldDefinition::new("id", FieldType::Integer, true, false, Option::<&str>::None);
	/// assert_eq!(field.name, "id");
	/// assert!(field.primary_key);
	/// ```
	pub fn new(
		name: impl Into<String>,
		field_type: crate::migrations::FieldType,
		primary_key: bool,
		unique: bool,
		default: Option<impl Into<String>>,
	) -> Self {
		Self {
			name: name.into(),
			field_type,
			primary_key,
			unique,
			default: default.map(|d| d.into()),
			null: false,
			// Generated Columns
			generated: None,
			generated_stored: None,
			#[cfg(any(feature = "db-mysql", feature = "db-sqlite"))]
			generated_virtual: None,
			// Identity/Auto-increment
			#[cfg(feature = "db-postgres")]
			identity_always: None,
			#[cfg(feature = "db-postgres")]
			identity_by_default: None,
			#[cfg(feature = "db-mysql")]
			auto_increment: None,
			#[cfg(feature = "db-sqlite")]
			autoincrement: None,
			// Character Set & Collation
			collate: None,
			#[cfg(feature = "db-mysql")]
			character_set: None,
			// Comment
			#[cfg(any(feature = "db-postgres", feature = "db-mysql"))]
			comment: None,
			// Storage Optimization
			#[cfg(feature = "db-postgres")]
			storage: None,
			#[cfg(feature = "db-postgres")]
			compression: None,
			// ON UPDATE Trigger
			#[cfg(feature = "db-mysql")]
			on_update_current_timestamp: None,
			// Invisible Columns
			#[cfg(feature = "db-mysql")]
			invisible: None,
			// Full-Text Index
			#[cfg(any(feature = "db-postgres", feature = "db-mysql"))]
			fulltext: None,
			// Numeric Attributes (MySQL, deprecated)
			#[cfg(feature = "db-mysql")]
			unsigned: None,
			#[cfg(feature = "db-mysql")]
			zerofill: None,
		}
	}

	/// Set nullable
	pub fn nullable(mut self, null: bool) -> Self {
		self.null = null;
		self
	}

	/// Generate SQL column definition
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::operations::FieldDefinition;
	/// use reinhardt_db::migrations::FieldType;
	///
	/// let field = FieldDefinition::new("email", FieldType::VarChar(255), false, true, Some("''"));
	/// let sql = field.to_sql_definition();
	/// assert!(sql.contains("VARCHAR(255)"));
	/// assert!(sql.contains("UNIQUE"));
	/// assert!(sql.contains("DEFAULT ''"));
	/// ```
	pub fn to_sql_definition(&self) -> String {
		let mut parts = vec![self.field_type.to_sql_string()];

		// Generated Columns (GENERATED ALWAYS AS ... STORED/VIRTUAL)
		if let Some(ref generated_expr) = self.generated {
			parts.push(format!("GENERATED ALWAYS AS ({})", generated_expr));

			// Determine STORED or VIRTUAL
			let is_stored = self.generated_stored.unwrap_or(false);

			#[cfg(any(feature = "db-mysql", feature = "db-sqlite"))]
			let is_virtual = self.generated_virtual.unwrap_or(false);
			#[cfg(not(any(feature = "db-mysql", feature = "db-sqlite")))]
			let is_virtual = false;

			if is_stored {
				parts.push("STORED".to_string());
			} else if is_virtual {
				parts.push("VIRTUAL".to_string());
			}
		}

		// Identity/Auto-increment
		#[cfg(feature = "db-postgres")]
		if self.identity_always.unwrap_or(false) {
			parts.push("GENERATED ALWAYS AS IDENTITY".to_string());
		}
		#[cfg(feature = "db-postgres")]
		if self.identity_by_default.unwrap_or(false) {
			parts.push("GENERATED BY DEFAULT AS IDENTITY".to_string());
		}
		#[cfg(feature = "db-mysql")]
		if self.auto_increment.unwrap_or(false) {
			parts.push("AUTO_INCREMENT".to_string());
		}
		#[cfg(feature = "db-sqlite")]
		if self.autoincrement.unwrap_or(false) {
			parts.push("AUTOINCREMENT".to_string());
		}

		if self.primary_key {
			parts.push("PRIMARY KEY".to_string());
		}

		if self.unique && !self.primary_key {
			parts.push("UNIQUE".to_string());
		}

		if !self.null && !self.primary_key {
			parts.push("NOT NULL".to_string());
		}

		if let Some(ref default) = self.default {
			parts.push(format!("DEFAULT {}", default));
		}

		// Character Set & Collation
		#[cfg(feature = "db-mysql")]
		if let Some(ref character_set) = self.character_set {
			parts.push(format!("CHARACTER SET {}", character_set));
		}

		if let Some(ref collate) = self.collate {
			parts.push(format!("COLLATE {}", collate));
		}

		// Comment (MySQL only in column definition)
		#[cfg(feature = "db-mysql")]
		if let Some(ref comment) = self.comment {
			parts.push(format!("COMMENT '{}'", comment.replace('\'', "''")));
		}

		// Storage Optimization (PostgreSQL)
		#[cfg(feature = "db-postgres")]
		if let Some(ref storage) = self.storage {
			parts.push(format!("STORAGE {}", storage.to_uppercase()));
		}
		#[cfg(feature = "db-postgres")]
		if let Some(ref compression) = self.compression {
			parts.push(format!("COMPRESSION {}", compression));
		}

		// ON UPDATE Trigger (MySQL)
		#[cfg(feature = "db-mysql")]
		if self.on_update_current_timestamp.unwrap_or(false) {
			parts.push("ON UPDATE CURRENT_TIMESTAMP".to_string());
		}

		// Invisible Columns (MySQL)
		#[cfg(feature = "db-mysql")]
		if self.invisible.unwrap_or(false) {
			parts.push("INVISIBLE".to_string());
		}

		// Full-Text Index
		// Note: Full-text index is typically created separately as an index,
		// not as part of the column definition. This field is used to mark
		// columns that should have full-text indexes created for them.
		// The actual index creation will be handled by the migration system.

		// Numeric Attributes (MySQL, deprecated)
		#[cfg(feature = "db-mysql")]
		if self.unsigned.unwrap_or(false) {
			parts.push("UNSIGNED".to_string());
		}
		#[cfg(feature = "db-mysql")]
		if self.zerofill.unwrap_or(false) {
			parts.push("ZEROFILL".to_string());
		}

		parts.join(" ")
	}
}

/// Create a new model (table)
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::operations::models::CreateModel;
/// use reinhardt_db::migrations::operations::FieldDefinition;
/// use reinhardt_db::migrations::{ProjectState, FieldType};
///
/// let mut state = ProjectState::new();
/// let create = CreateModel::new(
///     "User",
///     vec![
///         FieldDefinition::new("id", FieldType::Integer, true, false, Option::<&str>::None),
///         FieldDefinition::new("name", FieldType::VarChar(100), false, false, Option::<&str>::None),
///     ],
/// );
///
/// create.state_forwards("myapp", &mut state);
/// let model = state.get_model("myapp", "User").unwrap();
/// assert_eq!(model.fields.len(), 2);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateModel {
	pub name: String,
	pub fields: Vec<FieldDefinition>,
	pub options: HashMap<String, String>,
	pub bases: Vec<String>,
	/// Composite primary key fields (field names)
	pub composite_primary_key: Option<Vec<String>>,
}

impl CreateModel {
	/// Create a new CreateModel operation
	pub fn new(name: impl Into<String>, fields: Vec<FieldDefinition>) -> Self {
		Self {
			name: name.into(),
			fields,
			options: HashMap::new(),
			bases: vec![],
			composite_primary_key: None,
		}
	}

	/// Add model options
	pub fn with_options(mut self, options: HashMap<String, String>) -> Self {
		self.options = options;
		self
	}

	/// Add base classes for inheritance
	pub fn with_bases(mut self, bases: Vec<String>) -> Self {
		self.bases = bases;
		self
	}

	/// Set composite primary key with validation
	///
	/// # Errors
	///
	/// Returns `ValidationError::EmptyCompositePrimaryKey` if the fields list is empty.
	/// Returns `ValidationError::NonExistentField` if any field name doesn't exist in the table.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::operations::models::CreateModel;
	/// use reinhardt_db::migrations::operations::FieldDefinition;
	/// use reinhardt_db::migrations::FieldType;
	///
	/// let create = CreateModel::new(
	///     "post_tags",
	///     vec![
	///         FieldDefinition::new("post_id", FieldType::Integer, false, false, Option::<&str>::None),
	///         FieldDefinition::new("tag_id", FieldType::Integer, false, false, Option::<&str>::None),
	///     ],
	/// )
	/// .with_composite_primary_key(vec!["post_id".to_string(), "tag_id".to_string()])
	/// .expect("Valid composite primary key");
	///
	/// assert!(create.composite_primary_key.is_some());
	/// ```
	pub fn with_composite_primary_key(mut self, fields: Vec<String>) -> ValidationResult<Self> {
		// Validation: Check for empty list
		if fields.is_empty() {
			return Err(ValidationError::EmptyCompositePrimaryKey {
				table_name: self.name.clone(),
			});
		}

		// Validation: Verify all fields exist in table schema
		let available_field_names: Vec<String> =
			self.fields.iter().map(|f| f.name.clone()).collect();

		for field_name in &fields {
			if !available_field_names.contains(field_name) {
				return Err(ValidationError::NonExistentField {
					field_name: field_name.clone(),
					table_name: self.name.clone(),
					available_fields: available_field_names.clone(),
				});
			}
		}

		self.composite_primary_key = Some(fields);
		Ok(self)
	}

	/// Apply to project state (forward)
	pub fn state_forwards(&self, app_label: &str, state: &mut ProjectState) {
		let mut model = ModelState::new(app_label, &self.name);

		for field_def in &self.fields {
			let field = FieldState::new(
				field_def.name.clone(),
				field_def.field_type.clone(),
				field_def.primary_key,
			);
			model.add_field(field);
		}

		state.add_model(model);
	}

	/// Generate SQL using schema editor
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::operations::models::CreateModel;
	/// use reinhardt_db::migrations::operations::FieldDefinition;
	/// use reinhardt_db::migrations::FieldType;
	///
	/// let create = CreateModel::new(
	///     "users",
	///     vec![
	///         FieldDefinition::new("id", FieldType::Integer, true, false, Option::<&str>::None),
	///         FieldDefinition::new("email", FieldType::VarChar(255), false, false, Option::<&str>::None),
	///     ],
	/// );
	///
	/// // Convert to SQL - actual DB operations would use schema editor
	/// let columns: Vec<(&str, String)> = create.fields
	///     .iter()
	///     .map(|f| (f.name.as_str(), f.to_sql_definition()))
	///     .collect();
	///
	/// assert_eq!(columns.len(), 2);
	/// assert_eq!(columns[0].0, "id");
	/// assert!(columns[0].1.contains("PRIMARY KEY"));
	/// ```
	pub fn database_forwards(&self, schema_editor: &dyn BaseDatabaseSchemaEditor) -> Vec<String> {
		let mut sql_statements = Vec::new();

		// If composite primary key is defined, don't mark individual fields as primary keys
		let has_composite_pk = self.composite_primary_key.is_some();

		// Convert field definitions to column specifications for schema editor
		let column_defs: Vec<String> = self
			.fields
			.iter()
			.map(|f| {
				// For composite PKs, don't add PRIMARY KEY to individual field definitions
				if has_composite_pk && f.primary_key {
					// Build field definition without PRIMARY KEY keyword
					let mut parts = vec![f.field_type.to_sql_string()];

					// Primary key fields are always NOT NULL
					parts.push("NOT NULL".to_string());

					if f.unique {
						parts.push("UNIQUE".to_string());
					}
					if let Some(ref default) = f.default {
						parts.push(format!("DEFAULT {}", default));
					}
					parts.join(" ")
				} else {
					f.to_sql_definition()
				}
			})
			.collect();

		// Build column pairs: (name, type_definition)
		let columns: Vec<(&str, &str)> = self
			.fields
			.iter()
			.zip(column_defs.iter())
			.map(|(f, def)| (f.name.as_str(), def.as_str()))
			.collect();

		// Generate CREATE TABLE SQL using database-specific query builder
		let stmt = schema_editor.create_table_statement(&self.name, &columns);
		let mut create_sql = schema_editor.build_create_table_sql(&stmt);

		// Add composite primary key constraint if defined
		if let Some(ref pk_fields) = self.composite_primary_key {
			let db_type = schema_editor.database_type();
			let pk_name = format!("{}_pkey", self.name);
			let quoted_pk_name = quote_identifier(&pk_name, db_type);
			let quoted_pk_fields = pk_fields
				.iter()
				.map(|f| quote_identifier(f, db_type))
				.collect::<Vec<_>>()
				.join(", ");
			let constraint_sql = format!(
				"CONSTRAINT {} PRIMARY KEY ({})",
				quoted_pk_name, quoted_pk_fields
			);

			// Insert constraint before closing parenthesis
			// CREATE TABLE foo (col1 INT, col2 INT); becomes
			// CREATE TABLE foo (col1 INT, col2 INT, CONSTRAINT foo_pkey PRIMARY KEY (col1, col2));
			if create_sql.ends_with(");") {
				let insert_pos = create_sql.len() - 2; // Before ");"
				create_sql.insert_str(insert_pos, &format!(", {}", constraint_sql));
			} else if create_sql.ends_with(")") {
				let insert_pos = create_sql.len() - 1; // Before ")"
				create_sql.insert_str(insert_pos, &format!(", {}", constraint_sql));
			}
		}

		// Table-level attributes (SQLite)
		// Add STRICT and/or WITHOUT ROWID if specified
		#[cfg(feature = "db-sqlite")]
		{
			let mut table_options = Vec::new();

			if let Some(strict_val) = self.options.get("strict")
				&& strict_val == "true"
			{
				table_options.push("STRICT");
			}

			if let Some(without_rowid_val) = self.options.get("without_rowid")
				&& without_rowid_val == "true"
			{
				table_options.push("WITHOUT ROWID");
			}

			if !table_options.is_empty() {
				// Remove trailing semicolon if present
				if create_sql.ends_with(';') {
					create_sql.pop();
				}
				// Add table options
				create_sql.push(' ');
				create_sql.push_str(&table_options.join(" "));
				create_sql.push(';');
			}
		}

		sql_statements.push(create_sql);
		sql_statements
	}
}

/// Delete a model (drop table)
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::operations::models::{CreateModel, DeleteModel};
/// use reinhardt_db::migrations::operations::FieldDefinition;
/// use reinhardt_db::migrations::{ProjectState, FieldType};
///
/// let mut state = ProjectState::new();
///
/// // First create a model
/// let create = CreateModel::new(
///     "User",
///     vec![FieldDefinition::new("id", FieldType::Integer, true, false, Option::<&str>::None)],
/// );
/// create.state_forwards("myapp", &mut state);
/// assert!(state.get_model("myapp", "User").is_some());
///
/// // Then delete it
/// let delete = DeleteModel::new("User");
/// delete.state_forwards("myapp", &mut state);
/// assert!(state.get_model("myapp", "User").is_none());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteModel {
	pub name: String,
}

impl DeleteModel {
	/// Create a new DeleteModel operation
	pub fn new(name: impl Into<String>) -> Self {
		Self { name: name.into() }
	}

	/// Apply to project state (forward)
	pub fn state_forwards(&self, app_label: &str, state: &mut ProjectState) {
		state.remove_model(app_label, &self.name);
	}

	/// Generate SQL using schema editor
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::migrations::operations::models::DeleteModel;
	/// use reinhardt_db::backends::schema::factory::{SchemaEditorFactory, DatabaseType};
	///
	/// let delete = DeleteModel::new("users");
	/// let factory = SchemaEditorFactory::new();
	/// let editor = factory.create_for_database(DatabaseType::PostgreSQL);
	///
	/// let sql = delete.database_forwards(editor.as_ref());
	/// assert_eq!(sql.len(), 1);
	/// assert!(sql[0].contains("DROP TABLE"));
	/// assert!(sql[0].contains("\"users\""));
	/// ```
	pub fn database_forwards(&self, schema_editor: &dyn BaseDatabaseSchemaEditor) -> Vec<String> {
		let stmt = schema_editor.drop_table_statement(&self.name, false);
		vec![schema_editor.build_drop_table_sql(&stmt)]
	}
}

/// Rename a model (rename table)
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::operations::models::{CreateModel, RenameModel};
/// use reinhardt_db::migrations::operations::FieldDefinition;
/// use reinhardt_db::migrations::{ProjectState, FieldType};
///
/// let mut state = ProjectState::new();
///
/// // Create a model
/// let create = CreateModel::new(
///     "User",
///     vec![FieldDefinition::new("id", FieldType::Integer, true, false, Option::<&str>::None)],
/// );
/// create.state_forwards("myapp", &mut state);
///
/// // Rename it
/// let rename = RenameModel::new("User", "Customer");
/// rename.state_forwards("myapp", &mut state);
///
/// assert!(state.get_model("myapp", "User").is_none());
/// assert!(state.get_model("myapp", "Customer").is_some());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameModel {
	pub old_name: String,
	pub new_name: String,
}

impl RenameModel {
	/// Create a new RenameModel operation
	pub fn new(old_name: impl Into<String>, new_name: impl Into<String>) -> Self {
		Self {
			old_name: old_name.into(),
			new_name: new_name.into(),
		}
	}

	/// Apply to project state (forward)
	pub fn state_forwards(&self, app_label: &str, state: &mut ProjectState) {
		state.rename_model(app_label, &self.old_name, self.new_name.clone());
	}

	/// Generate SQL using schema editor
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::migrations::operations::models::RenameModel;
	/// use reinhardt_db::backends::schema::factory::{SchemaEditorFactory, DatabaseType};
	///
	/// let rename = RenameModel::new("users", "customers");
	/// let factory = SchemaEditorFactory::new();
	/// let editor = factory.create_for_database(DatabaseType::PostgreSQL);
	///
	/// let sql = rename.database_forwards(editor.as_ref());
	/// assert_eq!(sql.len(), 1);
	/// assert!(sql[0].contains("ALTER TABLE"));
	/// assert!(sql[0].contains("\"users\""));
	/// assert!(sql[0].contains("\"customers\""));
	/// ```
	pub fn database_forwards(&self, schema_editor: &dyn BaseDatabaseSchemaEditor) -> Vec<String> {
		// Quote identifiers based on database type
		let db_type = schema_editor.database_type();
		let old_name = quote_identifier(&self.old_name, db_type);
		let new_name = quote_identifier(&self.new_name, db_type);

		vec![format!("ALTER TABLE {} RENAME TO {}", old_name, new_name)]
	}
}

/// Move a model from one app to another
///
/// This operation moves a model from one application to another while preserving
/// its data and structure. Unlike Django which requires manual migration steps,
/// Reinhardt provides an explicit MoveModel operation.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::operations::models::{CreateModel, MoveModel};
/// use reinhardt_db::migrations::operations::FieldDefinition;
/// use reinhardt_db::migrations::{ProjectState, FieldType};
///
/// let mut state = ProjectState::new();
///
/// // Create a model in myapp
/// let create = CreateModel::new(
///     "User",
///     vec![FieldDefinition::new("id", FieldType::Integer, true, false, Option::<&str>::None)],
/// );
/// create.state_forwards("myapp", &mut state);
///
/// // Move it to auth app
/// let move_op = MoveModel::new("User", "myapp", "auth");
/// move_op.state_forwards("auth", &mut state);
///
/// assert!(state.get_model("myapp", "User").is_none());
/// assert!(state.get_model("auth", "User").is_some());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveModel {
	pub model_name: String,
	pub from_app: String,
	pub to_app: String,
	pub rename_table: bool,
	pub old_table_name: Option<String>,
	pub new_table_name: Option<String>,
}

impl MoveModel {
	/// Create a new MoveModel operation
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::operations::models::MoveModel;
	///
	/// let move_op = MoveModel::new("User", "myapp", "auth");
	/// assert_eq!(move_op.model_name, "User");
	/// assert_eq!(move_op.from_app, "myapp");
	/// assert_eq!(move_op.to_app, "auth");
	/// assert!(!move_op.rename_table);
	/// ```
	pub fn new(
		model_name: impl Into<String>,
		from_app: impl Into<String>,
		to_app: impl Into<String>,
	) -> Self {
		Self {
			model_name: model_name.into(),
			from_app: from_app.into(),
			to_app: to_app.into(),
			rename_table: false,
			old_table_name: None,
			new_table_name: None,
		}
	}

	/// Enable table renaming during the move
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::operations::models::MoveModel;
	///
	/// let move_op = MoveModel::new("User", "myapp", "auth")
	///     .with_table_rename("myapp_user", "auth_user");
	///
	/// assert!(move_op.rename_table);
	/// assert_eq!(move_op.old_table_name, Some("myapp_user".to_string()));
	/// assert_eq!(move_op.new_table_name, Some("auth_user".to_string()));
	/// ```
	pub fn with_table_rename(
		mut self,
		old_table: impl Into<String>,
		new_table: impl Into<String>,
	) -> Self {
		self.rename_table = true;
		self.old_table_name = Some(old_table.into());
		self.new_table_name = Some(new_table.into());
		self
	}

	/// Apply to project state (forward)
	///
	/// This removes the model from the source app and adds it to the target app.
	pub fn state_forwards(&self, _app_label: &str, state: &mut ProjectState) {
		// Remove from source app
		if let Some(model) = state
			.models
			.remove(&(self.from_app.clone(), self.model_name.clone()))
		{
			// Update app_label
			let mut new_model = model;
			new_model.app_label = self.to_app.clone();

			// Add to target app
			state
				.models
				.insert((self.to_app.clone(), self.model_name.clone()), new_model);
		}
	}

	/// Apply to project state (backward/reverse)
	///
	/// This moves the model back to its original app.
	pub fn state_backwards(&self, _app_label: &str, state: &mut ProjectState) {
		// Remove from target app
		if let Some(model) = state
			.models
			.remove(&(self.to_app.clone(), self.model_name.clone()))
		{
			// Restore original app_label
			let mut original_model = model;
			original_model.app_label = self.from_app.clone();

			// Add back to source app
			state.models.insert(
				(self.from_app.clone(), self.model_name.clone()),
				original_model,
			);
		}
	}

	/// Generate SQL using schema editor
	///
	/// If rename_table is true, generates ALTER TABLE RENAME statement.
	/// Otherwise, no SQL is needed since app_label is a Python/Rust concept
	/// that doesn't affect the database schema.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::migrations::operations::models::MoveModel;
	/// use reinhardt_db::backends::schema::factory::{SchemaEditorFactory, DatabaseType};
	///
	/// // Without table rename
	/// let move_op1 = MoveModel::new("User", "myapp", "auth");
	/// let factory = SchemaEditorFactory::new();
	/// let editor = factory.create_for_database(DatabaseType::PostgreSQL);
	/// let sql1 = move_op1.database_forwards(editor.as_ref());
	/// assert!(sql1.is_empty()); // No SQL needed
	///
	/// // With table rename
	/// let move_op2 = MoveModel::new("User", "myapp", "auth")
	///     .with_table_rename("myapp_user", "auth_user");
	/// let sql2 = move_op2.database_forwards(editor.as_ref());
	/// assert_eq!(sql2.len(), 1);
	/// assert!(sql2[0].contains("ALTER TABLE"));
	/// ```
	pub fn database_forwards(&self, schema_editor: &dyn BaseDatabaseSchemaEditor) -> Vec<String> {
		if self.rename_table {
			if let (Some(old_table), Some(new_table)) = (&self.old_table_name, &self.new_table_name)
			{
				// Quote identifiers based on database type
				let db_type = schema_editor.database_type();
				let old_name = quote_identifier(old_table, db_type);
				let new_name = quote_identifier(new_table, db_type);

				vec![format!("ALTER TABLE {} RENAME TO {}", old_name, new_name)]
			} else {
				vec![]
			}
		} else {
			// App label is a framework concept, no database changes needed
			vec![]
		}
	}

	/// Generate reverse SQL
	pub fn database_backwards(&self, schema_editor: &dyn BaseDatabaseSchemaEditor) -> Vec<String> {
		if self.rename_table {
			if let (Some(old_table), Some(new_table)) = (&self.old_table_name, &self.new_table_name)
			{
				// Reverse: rename back to original
				// Quote identifiers based on database type
				let db_type = schema_editor.database_type();
				let old_name = quote_identifier(old_table, db_type);
				let new_name = quote_identifier(new_table, db_type);

				vec![format!("ALTER TABLE {} RENAME TO {}", new_name, old_name)]
			} else {
				vec![]
			}
		} else {
			vec![]
		}
	}
}

// MigrationOperation trait implementation for Django-style naming
use crate::migrations::operation_trait::MigrationOperation;

impl MigrationOperation for CreateModel {
	fn migration_name_fragment(&self) -> Option<String> {
		Some(self.name.to_lowercase())
	}

	fn describe(&self) -> String {
		format!("Create model {}", self.name)
	}
}

impl MigrationOperation for DeleteModel {
	fn migration_name_fragment(&self) -> Option<String> {
		Some(format!("delete_{}", self.name.to_lowercase()))
	}

	fn describe(&self) -> String {
		format!("Delete model {}", self.name)
	}
}

impl MigrationOperation for RenameModel {
	fn migration_name_fragment(&self) -> Option<String> {
		Some(format!(
			"rename_{}_to_{}",
			self.old_name.to_lowercase(),
			self.new_name.to_lowercase()
		))
	}

	fn describe(&self) -> String {
		format!("Rename model {} to {}", self.old_name, self.new_name)
	}
}

impl MigrationOperation for MoveModel {
	fn migration_name_fragment(&self) -> Option<String> {
		Some(format!(
			"move_{}_to_{}",
			self.model_name.to_lowercase(),
			self.to_app.to_lowercase()
		))
	}

	fn describe(&self) -> String {
		format!(
			"Move model {} from {} to {}",
			self.model_name, self.from_app, self.to_app
		)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::migrations::FieldType;

	#[test]
	fn test_field_definition_to_sql() {
		let field = FieldDefinition::new("id", FieldType::Integer, true, false, None::<String>);
		let sql = field.to_sql_definition();
		assert_eq!(sql, "INTEGER PRIMARY KEY");

		let field2 =
			FieldDefinition::new("email", FieldType::VarChar(255), false, true, Some("''"));
		let sql2 = field2.to_sql_definition();
		assert_eq!(sql2, "VARCHAR(255) UNIQUE NOT NULL DEFAULT ''");
	}

	#[test]
	fn test_create_model_state_forwards() {
		let mut state = ProjectState::new();
		let create = CreateModel::new(
			"User",
			vec![
				FieldDefinition::new("id", FieldType::Integer, true, false, None::<String>),
				FieldDefinition::new(
					"name",
					FieldType::VarChar(100),
					false,
					false,
					None::<String>,
				),
			],
		);

		create.state_forwards("myapp", &mut state);

		let model = state.get_model("myapp", "User").unwrap();
		assert_eq!(model.name, "User");
		assert_eq!(model.fields.len(), 2);
		assert_eq!(model.fields.get("id").unwrap().name, "id");
		assert_eq!(model.fields.get("name").unwrap().name, "name");
	}

	#[test]
	fn test_delete_model_state_forwards() {
		let mut state = ProjectState::new();

		// Create a model first
		let create = CreateModel::new(
			"User",
			vec![FieldDefinition::new(
				"id",
				FieldType::Integer,
				true,
				false,
				None::<String>,
			)],
		);
		create.state_forwards("myapp", &mut state);
		assert!(state.get_model("myapp", "User").is_some());

		// Delete it
		let delete = DeleteModel::new("User");
		delete.state_forwards("myapp", &mut state);
		assert!(state.get_model("myapp", "User").is_none());
	}

	#[test]
	fn test_rename_model_state_forwards() {
		let mut state = ProjectState::new();

		// Create a model first
		let create = CreateModel::new(
			"User",
			vec![FieldDefinition::new(
				"id",
				FieldType::Integer,
				true,
				false,
				None::<String>,
			)],
		);
		create.state_forwards("myapp", &mut state);

		// Rename it
		let rename = RenameModel::new("User", "Customer");
		rename.state_forwards("myapp", &mut state);

		assert!(state.get_model("myapp", "User").is_none());
		let model = state.get_model("myapp", "Customer").unwrap();
		assert_eq!(model.name, "Customer");
	}

	#[cfg(feature = "db-postgres")]
	#[test]
	fn test_delete_model_database_forwards() {
		use crate::backends::schema::test_utils::MockSchemaEditor;

		let delete = DeleteModel::new("users");
		let editor = MockSchemaEditor::new();

		let sql = delete.database_forwards(&editor);
		assert_eq!(sql.len(), 1);
		assert_eq!(sql[0], "DROP TABLE IF EXISTS \"users\"");
	}

	#[cfg(feature = "db-postgres")]
	#[test]
	fn test_rename_model_database_forwards() {
		use crate::backends::schema::test_utils::MockSchemaEditor;

		let rename = RenameModel::new("users", "customers");
		let editor = MockSchemaEditor::new();

		let sql = rename.database_forwards(&editor);
		assert_eq!(sql.len(), 1);
		assert_eq!(sql[0], "ALTER TABLE \"users\" RENAME TO \"customers\"");
	}

	#[test]
	fn test_field_definition_nullable() {
		let field = FieldDefinition::new(
			"email",
			FieldType::VarChar(255),
			false,
			false,
			None::<String>,
		)
		.nullable(true);

		assert!(field.null);
		let sql = field.to_sql_definition();
		assert_eq!(sql, "VARCHAR(255)");
	}

	#[test]
	fn test_create_model_with_options() {
		let mut options = HashMap::new();
		options.insert("db_table".to_string(), "custom_users".to_string());

		let create = CreateModel::new(
			"User",
			vec![FieldDefinition::new(
				"id",
				FieldType::Integer,
				true,
				false,
				None::<String>,
			)],
		)
		.with_options(options.clone());

		assert_eq!(create.options, options);
		assert_eq!(
			create.options.get("db_table"),
			Some(&"custom_users".to_string())
		);
	}

	#[test]
	fn test_create_model_with_bases() {
		let bases = vec!["BaseModel".to_string(), "Timestamped".to_string()];

		let create = CreateModel::new(
			"User",
			vec![FieldDefinition::new(
				"id",
				FieldType::Integer,
				true,
				false,
				None::<String>,
			)],
		)
		.with_bases(bases.clone());

		assert_eq!(create.bases, bases);
		assert_eq!(create.bases.len(), 2);
	}

	#[test]
	fn test_create_model_multiple_fields() {
		let mut state = ProjectState::new();

		let create = CreateModel::new(
			"User",
			vec![
				FieldDefinition::new("id", FieldType::Integer, true, false, None::<String>),
				FieldDefinition::new(
					"username",
					FieldType::VarChar(50),
					false,
					true,
					None::<String>,
				),
				FieldDefinition::new(
					"email",
					FieldType::VarChar(255),
					false,
					true,
					None::<String>,
				),
				FieldDefinition::new("is_active", FieldType::Boolean, false, false, Some("true")),
			],
		);

		create.state_forwards("myapp", &mut state);

		let model = state.get_model("myapp", "User").unwrap();
		assert_eq!(model.fields.len(), 4);
		assert_eq!(model.fields.get("id").unwrap().name, "id");
		assert_eq!(model.fields.get("username").unwrap().name, "username");
		assert_eq!(model.fields.get("email").unwrap().name, "email");
		assert_eq!(model.fields.get("is_active").unwrap().name, "is_active");
	}

	#[test]
	fn test_field_definition_with_default() {
		let field = FieldDefinition::new(
			"status",
			FieldType::VarChar(20),
			false,
			false,
			Some("'pending'"),
		);

		assert_eq!(field.default, Some("'pending'".to_string()));

		let sql = field.to_sql_definition();
		assert_eq!(sql, "VARCHAR(20) NOT NULL DEFAULT 'pending'");
	}

	#[test]
	fn test_delete_model_removes_from_state() {
		let mut state = ProjectState::new();

		// Create multiple models
		let create1 = CreateModel::new(
			"User",
			vec![FieldDefinition::new(
				"id",
				FieldType::Integer,
				true,
				false,
				None::<String>,
			)],
		);
		let create2 = CreateModel::new(
			"Post",
			vec![FieldDefinition::new(
				"id",
				FieldType::Integer,
				true,
				false,
				None::<String>,
			)],
		);

		create1.state_forwards("myapp", &mut state);
		create2.state_forwards("myapp", &mut state);

		assert!(state.get_model("myapp", "User").is_some());
		assert!(state.get_model("myapp", "Post").is_some());

		// Delete only User
		let delete = DeleteModel::new("User");
		delete.state_forwards("myapp", &mut state);

		assert!(state.get_model("myapp", "User").is_none());
		assert!(state.get_model("myapp", "Post").is_some());
	}

	#[test]
	fn test_rename_model_preserves_fields() {
		let mut state = ProjectState::new();

		// Create a model with multiple fields
		let create = CreateModel::new(
			"User",
			vec![
				FieldDefinition::new("id", FieldType::Integer, true, false, None::<String>),
				FieldDefinition::new(
					"name",
					FieldType::VarChar(100),
					false,
					false,
					None::<String>,
				),
			],
		);
		create.state_forwards("myapp", &mut state);

		// Rename it
		let rename = RenameModel::new("User", "Account");
		rename.state_forwards("myapp", &mut state);

		// Check that fields are preserved
		let model = state.get_model("myapp", "Account").unwrap();
		assert_eq!(model.fields.len(), 2);
		assert_eq!(model.fields.get("id").unwrap().name, "id");
		assert_eq!(model.fields.get("name").unwrap().name, "name");
	}

	#[test]
	fn test_move_model_basic() {
		let mut state = ProjectState::new();

		// Create a model in myapp
		let create = CreateModel::new(
			"User",
			vec![FieldDefinition::new(
				"id",
				FieldType::Integer,
				true,
				false,
				None::<String>,
			)],
		);
		create.state_forwards("myapp", &mut state);
		assert!(state.get_model("myapp", "User").is_some());

		// Move to auth app
		let move_op = MoveModel::new("User", "myapp", "auth");
		move_op.state_forwards("auth", &mut state);

		// Check model is moved
		assert!(state.get_model("myapp", "User").is_none());
		assert!(state.get_model("auth", "User").is_some());

		// Check app_label is updated
		let model = state.get_model("auth", "User").unwrap();
		assert_eq!(model.app_label, "auth");
	}

	#[test]
	fn test_move_model_preserves_fields() {
		let mut state = ProjectState::new();

		// Create a model with multiple fields
		let create = CreateModel::new(
			"User",
			vec![
				FieldDefinition::new("id", FieldType::Integer, true, false, None::<String>),
				FieldDefinition::new(
					"email",
					FieldType::VarChar(255),
					false,
					false,
					None::<String>,
				),
				FieldDefinition::new(
					"name",
					FieldType::VarChar(100),
					false,
					false,
					None::<String>,
				),
			],
		);
		create.state_forwards("myapp", &mut state);

		// Move it
		let move_op = MoveModel::new("User", "myapp", "auth");
		move_op.state_forwards("auth", &mut state);

		// Check fields are preserved
		let model = state.get_model("auth", "User").unwrap();
		assert_eq!(model.fields.len(), 3);
		assert_eq!(model.fields.get("id").unwrap().name, "id");
		assert_eq!(model.fields.get("email").unwrap().name, "email");
		assert_eq!(model.fields.get("name").unwrap().name, "name");
	}

	#[test]
	fn test_move_model_backwards() {
		let mut state = ProjectState::new();

		// Create and move model
		let create = CreateModel::new(
			"User",
			vec![FieldDefinition::new(
				"id",
				FieldType::Integer,
				true,
				false,
				None::<String>,
			)],
		);
		create.state_forwards("myapp", &mut state);

		let move_op = MoveModel::new("User", "myapp", "auth");
		move_op.state_forwards("auth", &mut state);
		assert!(state.get_model("auth", "User").is_some());

		// Reverse the move
		move_op.state_backwards("myapp", &mut state);

		// Check model is back in original app
		assert!(state.get_model("auth", "User").is_none());
		assert!(state.get_model("myapp", "User").is_some());

		let model = state.get_model("myapp", "User").unwrap();
		assert_eq!(model.app_label, "myapp");
	}

	#[cfg(feature = "db-postgres")]
	#[test]
	fn test_move_model_without_table_rename() {
		use crate::backends::schema::test_utils::MockSchemaEditor;

		let move_op = MoveModel::new("User", "myapp", "auth");
		let editor = MockSchemaEditor::new();

		let sql = move_op.database_forwards(&editor);
		// No SQL needed when not renaming table
		assert_eq!(sql.len(), 0);
	}

	#[cfg(feature = "db-postgres")]
	#[test]
	fn test_move_model_with_table_rename() {
		use crate::backends::schema::test_utils::MockSchemaEditor;

		let move_op =
			MoveModel::new("User", "myapp", "auth").with_table_rename("myapp_user", "auth_user");

		let editor = MockSchemaEditor::new();

		let sql = move_op.database_forwards(&editor);
		assert_eq!(sql.len(), 1);
		assert_eq!(sql[0], "ALTER TABLE \"myapp_user\" RENAME TO \"auth_user\"");
	}

	#[cfg(feature = "db-postgres")]
	#[test]
	fn test_move_model_backward_sql() {
		use crate::backends::schema::test_utils::MockSchemaEditor;

		let move_op =
			MoveModel::new("User", "myapp", "auth").with_table_rename("myapp_user", "auth_user");

		let editor = MockSchemaEditor::new();

		let sql = move_op.database_backwards(&editor);
		assert_eq!(sql.len(), 1);
		// Reverse: auth_user back to myapp_user
		assert_eq!(sql[0], "ALTER TABLE \"auth_user\" RENAME TO \"myapp_user\"");
	}
}
