//! Field operations for migrations
//!
//! This module provides operations for adding, removing, altering, and renaming fields,
//! inspired by Django's `django/db/migrations/operations/fields.py`.
//!
//! # Example
//!
//! ```rust
//! use reinhardt_db::migrations::operations::fields::{AddField, RemoveField};
//! use reinhardt_db::migrations::operations::FieldDefinition;
//! use reinhardt_db::migrations::operations::models::CreateModel;
//! use reinhardt_db::migrations::{ProjectState, FieldType};
//!
//! let mut state = ProjectState::new();
//!
//! // Create a model first
//! let create = CreateModel::new(
//!     "User",
//!     vec![FieldDefinition::new("id", FieldType::Integer, true, false, Option::<&str>::None)],
//! );
//! create.state_forwards("myapp", &mut state);
//!
//! // Add a field
//! let add = AddField::new("User", FieldDefinition::new("email", FieldType::VarChar(255), false, false, Option::<&str>::None));
//! add.state_forwards("myapp", &mut state);
//! assert_eq!(state.get_model("myapp", "User").unwrap().fields.len(), 2);
//!
//! // Remove a field
//! let remove = RemoveField::new("User", "email");
//! remove.state_forwards("myapp", &mut state);
//! assert_eq!(state.get_model("myapp", "User").unwrap().fields.len(), 1);
//! ```

use super::{FieldState, ProjectState};
use crate::backends::schema::BaseDatabaseSchemaEditor;
use serde::{Deserialize, Serialize};

pub use super::models::FieldDefinition;

/// Add a field to an existing model
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::operations::fields::AddField;
/// use reinhardt_db::migrations::operations::FieldDefinition;
/// use reinhardt_db::migrations::operations::models::CreateModel;
/// use reinhardt_db::migrations::{ProjectState, FieldType};
///
/// let mut state = ProjectState::new();
///
/// // Create a model first
/// let create = CreateModel::new(
///     "User",
///     vec![FieldDefinition::new("id", FieldType::Integer, true, false, Option::<&str>::None)],
/// );
/// create.state_forwards("myapp", &mut state);
///
/// // Add a field
/// let add = AddField::new("User", FieldDefinition::new("email", FieldType::VarChar(255), false, false, Option::<&str>::None));
/// add.state_forwards("myapp", &mut state);
///
/// let model = state.get_model("myapp", "User").unwrap();
/// assert_eq!(model.fields.len(), 2);
/// assert!(model.fields.contains_key("email"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddField {
	pub model_name: String,
	pub field: FieldDefinition,
	pub preserve_default: bool,
}

impl AddField {
	/// Create a new AddField operation
	pub fn new(model_name: impl Into<String>, field: FieldDefinition) -> Self {
		Self {
			model_name: model_name.into(),
			field,
			preserve_default: true,
		}
	}

	/// Set whether to preserve the default value after adding
	pub fn with_preserve_default(mut self, preserve: bool) -> Self {
		self.preserve_default = preserve;
		self
	}

	/// Apply to project state (forward)
	pub fn state_forwards(&self, app_label: &str, state: &mut ProjectState) {
		if let Some(model) = state.get_model_mut(app_label, &self.model_name) {
			let field = FieldState::new(
				self.field.name.clone(),
				self.field.field_type.clone(),
				self.field.primary_key,
			);
			model.add_field(field);
		}
	}

	/// Generate SQL using schema editor
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::migrations::operations::fields::AddField;
	/// use reinhardt_db::migrations::operations::FieldDefinition;
	/// use reinhardt_db::migrations::FieldType;
	/// use reinhardt_db::backends::schema::factory::{SchemaEditorFactory, DatabaseType};
	///
	/// let add = AddField::new("users", FieldDefinition::new("email", FieldType::VarChar(255), false, false, Option::<&str>::None));
	/// let factory = SchemaEditorFactory::new();
	/// let editor = factory.create_for_database(DatabaseType::PostgreSQL);
	///
	/// let sql = add.database_forwards(editor.as_ref());
	/// assert_eq!(sql.len(), 1);
	/// assert!(sql[0].contains("ALTER TABLE"));
	/// assert!(sql[0].contains("ADD COLUMN"));
	/// assert!(sql[0].contains("\"email\""));
	/// ```
	pub fn database_forwards(&self, schema_editor: &dyn BaseDatabaseSchemaEditor) -> Vec<String> {
		let definition = self.field.to_sql_definition();
		let stmt =
			schema_editor.add_column_statement(&self.model_name, &self.field.name, &definition);
		vec![schema_editor.build_alter_table_sql(&stmt)]
	}
}

/// Remove a field from a model
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::operations::fields::RemoveField;
/// use reinhardt_db::migrations::operations::FieldDefinition;
/// use reinhardt_db::migrations::operations::models::CreateModel;
/// use reinhardt_db::migrations::{ProjectState, FieldType};
///
/// let mut state = ProjectState::new();
///
/// // Create a model with fields
/// let create = CreateModel::new(
///     "User",
///     vec![
///         FieldDefinition::new("id", FieldType::Integer, true, false, Option::<&str>::None),
///         FieldDefinition::new("email", FieldType::VarChar(255), false, false, Option::<&str>::None),
///     ],
/// );
/// create.state_forwards("myapp", &mut state);
///
/// // Remove a field
/// let remove = RemoveField::new("User", "email");
/// remove.state_forwards("myapp", &mut state);
///
/// let model = state.get_model("myapp", "User").unwrap();
/// assert_eq!(model.fields.len(), 1);
/// assert!(!model.fields.contains_key("email"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveField {
	pub model_name: String,
	pub field_name: String,
}

impl RemoveField {
	/// Create a new RemoveField operation
	pub fn new(model_name: impl Into<String>, field_name: impl Into<String>) -> Self {
		Self {
			model_name: model_name.into(),
			field_name: field_name.into(),
		}
	}

	/// Apply to project state (forward)
	pub fn state_forwards(&self, app_label: &str, state: &mut ProjectState) {
		if let Some(model) = state.get_model_mut(app_label, &self.model_name) {
			model.remove_field(&self.field_name);
		}
	}

	/// Generate SQL using schema editor
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::migrations::operations::fields::RemoveField;
	/// use reinhardt_db::backends::schema::factory::{SchemaEditorFactory, DatabaseType};
	///
	/// let remove = RemoveField::new("users", "email");
	/// let factory = SchemaEditorFactory::new();
	/// let editor = factory.create_for_database(DatabaseType::PostgreSQL);
	///
	/// let sql = remove.database_forwards(editor.as_ref());
	/// assert_eq!(sql.len(), 1);
	/// assert!(sql[0].contains("ALTER TABLE"));
	/// assert!(sql[0].contains("DROP COLUMN"));
	/// assert!(sql[0].contains("\"email\""));
	/// ```
	pub fn database_forwards(&self, schema_editor: &dyn BaseDatabaseSchemaEditor) -> Vec<String> {
		let stmt = schema_editor.drop_column_statement(&self.model_name, &self.field_name);
		vec![schema_editor.build_alter_table_sql(&stmt)]
	}
}

/// Alter a field's definition
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::operations::fields::AlterField;
/// use reinhardt_db::migrations::operations::FieldDefinition;
/// use reinhardt_db::migrations::operations::models::CreateModel;
/// use reinhardt_db::migrations::{ProjectState, FieldType};
///
/// let mut state = ProjectState::new();
///
/// // Create a model with a field
/// let create = CreateModel::new(
///     "User",
///     vec![
///         FieldDefinition::new("id", FieldType::Integer, true, false, Option::<&str>::None),
///         FieldDefinition::new("email", FieldType::VarChar(100), false, false, Option::<&str>::None),
///     ],
/// );
/// create.state_forwards("myapp", &mut state);
///
/// // Alter the field to make it longer
/// let alter = AlterField::new("User", FieldDefinition::new("email", FieldType::VarChar(255), false, false, Option::<&str>::None));
/// alter.state_forwards("myapp", &mut state);
///
/// let model = state.get_model("myapp", "User").unwrap();
/// let field = model.fields.get("email").unwrap();
/// assert_eq!(field.field_type, FieldType::VarChar(255));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlterField {
	pub model_name: String,
	pub field: FieldDefinition,
}

impl AlterField {
	/// Create a new AlterField operation
	pub fn new(model_name: impl Into<String>, field: FieldDefinition) -> Self {
		Self {
			model_name: model_name.into(),
			field,
		}
	}

	/// Apply to project state (forward)
	pub fn state_forwards(&self, app_label: &str, state: &mut ProjectState) {
		if let Some(model) = state.get_model_mut(app_label, &self.model_name) {
			let field = FieldState::new(
				self.field.name.clone(),
				self.field.field_type.clone(),
				self.field.primary_key,
			);
			model.alter_field(&self.field.name, field);
		}
	}

	/// Generate SQL using schema editor
	///
	/// Note: Altering columns is database-specific and complex.
	/// This is a simplified version that may need enhancement.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::migrations::operations::fields::AlterField;
	/// use reinhardt_db::migrations::operations::FieldDefinition;
	/// use reinhardt_db::migrations::FieldType;
	/// use reinhardt_db::backends::schema::factory::{SchemaEditorFactory, DatabaseType};
	///
	/// let alter = AlterField::new("users", FieldDefinition::new("email", FieldType::VarChar(500), false, false, Option::<&str>::None));
	/// let factory = SchemaEditorFactory::new();
	/// let editor = factory.create_for_database(DatabaseType::PostgreSQL);
	///
	/// let sql = alter.database_forwards(editor.as_ref());
	/// assert!(!sql.is_empty());
	/// ```
	pub fn database_forwards(&self, schema_editor: &dyn BaseDatabaseSchemaEditor) -> Vec<String> {
		// Use database-specific ALTER COLUMN statement from schema editor
		// Each database backend (PostgreSQL, MySQL, SQLite, CockroachDB) provides
		// its own implementation via the alter_column_statement() method
		vec![schema_editor.alter_column_statement(
			&self.model_name,
			&self.field.name,
			&self.field.field_type.to_sql_string(),
		)]
	}
}

/// Rename a field
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::operations::fields::RenameField;
/// use reinhardt_db::migrations::operations::FieldDefinition;
/// use reinhardt_db::migrations::operations::models::CreateModel;
/// use reinhardt_db::migrations::{ProjectState, FieldType};
///
/// let mut state = ProjectState::new();
///
/// // Create a model with a field
/// let create = CreateModel::new(
///     "User",
///     vec![
///         FieldDefinition::new("id", FieldType::Integer, true, false, Option::<&str>::None),
///         FieldDefinition::new("email", FieldType::VarChar(255), false, false, Option::<&str>::None),
///     ],
/// );
/// create.state_forwards("myapp", &mut state);
///
/// // Rename the field
/// let rename = RenameField::new("User", "email", "email_address");
/// rename.state_forwards("myapp", &mut state);
///
/// let model = state.get_model("myapp", "User").unwrap();
/// assert!(!model.fields.contains_key("email"));
/// assert!(model.fields.contains_key("email_address"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameField {
	pub model_name: String,
	pub old_name: String,
	pub new_name: String,
}

impl RenameField {
	/// Create a new RenameField operation
	pub fn new(
		model_name: impl Into<String>,
		old_name: impl Into<String>,
		new_name: impl Into<String>,
	) -> Self {
		Self {
			model_name: model_name.into(),
			old_name: old_name.into(),
			new_name: new_name.into(),
		}
	}

	/// Apply to project state (forward)
	pub fn state_forwards(&self, app_label: &str, state: &mut ProjectState) {
		if let Some(model) = state.get_model_mut(app_label, &self.model_name) {
			model.rename_field(&self.old_name, self.new_name.clone());
		}
	}

	/// Generate SQL using schema editor
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_db::migrations::operations::fields::RenameField;
	/// use reinhardt_db::backends::schema::factory::{SchemaEditorFactory, DatabaseType};
	///
	/// let rename = RenameField::new("users", "email", "email_address");
	/// let factory = SchemaEditorFactory::new();
	/// let editor = factory.create_for_database(DatabaseType::PostgreSQL);
	///
	/// let sql = rename.database_forwards(editor.as_ref());
	/// assert_eq!(sql.len(), 1);
	/// assert!(sql[0].contains("ALTER TABLE"));
	/// assert!(sql[0].contains("RENAME COLUMN"));
	/// assert!(sql[0].contains("\"email\""));
	/// assert!(sql[0].contains("\"email_address\""));
	/// ```
	pub fn database_forwards(&self, schema_editor: &dyn BaseDatabaseSchemaEditor) -> Vec<String> {
		vec![schema_editor.rename_column_statement(
			&self.model_name,
			&self.old_name,
			&self.new_name,
		)]
	}
}

// MigrationOperation trait implementation for Django-style naming
use crate::migrations::operation_trait::MigrationOperation;

impl MigrationOperation for AddField {
	fn migration_name_fragment(&self) -> Option<String> {
		Some(format!(
			"{}_{}",
			self.model_name.to_lowercase(),
			self.field.name.to_lowercase()
		))
	}

	fn describe(&self) -> String {
		format!("Add field {} to {}", self.field.name, self.model_name)
	}
}

impl MigrationOperation for RemoveField {
	fn migration_name_fragment(&self) -> Option<String> {
		Some(format!(
			"remove_{}_{}",
			self.model_name.to_lowercase(),
			self.field_name.to_lowercase()
		))
	}

	fn describe(&self) -> String {
		format!("Remove field {} from {}", self.field_name, self.model_name)
	}
}

impl MigrationOperation for AlterField {
	fn migration_name_fragment(&self) -> Option<String> {
		Some(format!(
			"alter_{}_{}",
			self.model_name.to_lowercase(),
			self.field.name.to_lowercase()
		))
	}

	fn describe(&self) -> String {
		format!("Alter field {} on {}", self.field.name, self.model_name)
	}
}

impl MigrationOperation for RenameField {
	fn migration_name_fragment(&self) -> Option<String> {
		Some(format!(
			"rename_{}_{}",
			self.model_name.to_lowercase(),
			self.new_name.to_lowercase()
		))
	}

	fn describe(&self) -> String {
		format!(
			"Rename field {} to {} on {}",
			self.old_name, self.new_name, self.model_name
		)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::migrations::FieldType;
	use crate::migrations::operations::models::CreateModel;

	#[test]
	fn test_add_field_state_forwards() {
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

		// Add a field
		let add = AddField::new(
			"User",
			FieldDefinition::new(
				"email",
				FieldType::VarChar(255),
				false,
				false,
				None::<String>,
			),
		);
		add.state_forwards("myapp", &mut state);

		let model = state.get_model("myapp", "User").unwrap();
		assert_eq!(model.fields.len(), 2);
		assert!(model.fields.contains_key("email"));
	}

	#[test]
	fn test_remove_field_state_forwards() {
		let mut state = ProjectState::new();

		// Create a model with fields
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
			],
		);
		create.state_forwards("myapp", &mut state);

		// Remove a field
		let remove = RemoveField::new("User", "email");
		remove.state_forwards("myapp", &mut state);

		let model = state.get_model("myapp", "User").unwrap();
		assert_eq!(model.fields.len(), 1);
		assert!(!model.fields.contains_key("email"));
	}

	#[test]
	fn test_alter_field_state_forwards() {
		let mut state = ProjectState::new();

		// Create a model with a field
		let create = CreateModel::new(
			"User",
			vec![
				FieldDefinition::new("id", FieldType::Integer, true, false, None::<String>),
				FieldDefinition::new(
					"email",
					FieldType::VarChar(100),
					false,
					false,
					None::<String>,
				),
			],
		);
		create.state_forwards("myapp", &mut state);

		// Alter the field
		let alter = AlterField::new(
			"User",
			FieldDefinition::new(
				"email",
				FieldType::VarChar(255),
				false,
				false,
				None::<String>,
			),
		);
		alter.state_forwards("myapp", &mut state);

		let model = state.get_model("myapp", "User").unwrap();
		let field = model.fields.get("email").unwrap();
		assert_eq!(field.field_type, FieldType::VarChar(255));
	}

	#[test]
	fn test_rename_field_state_forwards() {
		let mut state = ProjectState::new();

		// Create a model with a field
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
			],
		);
		create.state_forwards("myapp", &mut state);

		// Rename the field
		let rename = RenameField::new("User", "email", "email_address");
		rename.state_forwards("myapp", &mut state);

		let model = state.get_model("myapp", "User").unwrap();
		assert!(!model.fields.contains_key("email"));
		assert!(model.fields.contains_key("email_address"));
	}

	#[cfg(feature = "postgres")]
	#[test]
	fn test_add_field_database_forwards() {
		use crate::backends::schema::test_utils::MockSchemaEditor;

		let add = AddField::new(
			"users",
			FieldDefinition::new(
				"email",
				FieldType::VarChar(255),
				false,
				false,
				None::<String>,
			),
		);
		let editor = MockSchemaEditor::new();

		let sql = add.database_forwards(&editor);
		assert_eq!(sql.len(), 1);
		assert!(sql[0].contains("ALTER TABLE"));
		assert!(sql[0].contains("ADD COLUMN"));
		assert!(sql[0].contains("\"email\""));
	}

	#[cfg(feature = "postgres")]
	#[test]
	fn test_remove_field_database_forwards() {
		use crate::backends::schema::test_utils::MockSchemaEditor;

		let remove = RemoveField::new("users", "email");
		let editor = MockSchemaEditor::new();

		let sql = remove.database_forwards(&editor);
		assert_eq!(sql.len(), 1);
		assert!(sql[0].contains("ALTER TABLE"));
		assert!(sql[0].contains("DROP COLUMN"));
		assert!(sql[0].contains("\"email\""));
	}

	#[cfg(feature = "postgres")]
	#[test]
	fn test_rename_field_database_forwards() {
		use crate::backends::schema::test_utils::MockSchemaEditor;

		let rename = RenameField::new("users", "email", "email_address");
		let editor = MockSchemaEditor::new();

		let sql = rename.database_forwards(&editor);
		assert_eq!(sql.len(), 1);
		assert!(sql[0].contains("ALTER TABLE"));
		assert!(sql[0].contains("RENAME COLUMN"));
		assert!(sql[0].contains("\"email\""));
		assert!(sql[0].contains("\"email_address\""));
	}
}
