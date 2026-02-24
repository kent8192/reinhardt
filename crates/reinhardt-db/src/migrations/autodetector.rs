//! Migration autodetector

use petgraph::Undirected;
use petgraph::graph::Graph;
use petgraph::visit::EdgeRef;
use regex::Regex;
use std::collections::{BTreeMap, HashMap};
use strsim::{jaro_winkler, levenshtein};

use super::model_registry::ManyToManyMetadata;

/// ForeignKey action for ON DELETE and ON UPDATE clauses
#[derive(
	Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum ForeignKeyAction {
	/// Restricts deletion/update (default)
	Restrict,
	/// Cascades deletion/update to dependent rows
	Cascade,
	/// Sets foreign key to NULL
	SetNull,
	/// No action (similar to Restrict but deferred)
	NoAction,
	/// Sets foreign key to default value
	SetDefault,
}

impl ForeignKeyAction {
	/// Convert to SQL keyword for use in constraint definitions
	pub fn to_sql_keyword(&self) -> &'static str {
		match self {
			ForeignKeyAction::Restrict => "RESTRICT",
			ForeignKeyAction::Cascade => "CASCADE",
			ForeignKeyAction::SetNull => "SET NULL",
			ForeignKeyAction::NoAction => "NO ACTION",
			ForeignKeyAction::SetDefault => "SET DEFAULT",
		}
	}
}

impl From<ForeignKeyAction> for reinhardt_query::prelude::ForeignKeyAction {
	fn from(action: ForeignKeyAction) -> Self {
		match action {
			ForeignKeyAction::Restrict => reinhardt_query::prelude::ForeignKeyAction::Restrict,
			ForeignKeyAction::Cascade => reinhardt_query::prelude::ForeignKeyAction::Cascade,
			ForeignKeyAction::SetNull => reinhardt_query::prelude::ForeignKeyAction::SetNull,
			ForeignKeyAction::NoAction => reinhardt_query::prelude::ForeignKeyAction::NoAction,
			ForeignKeyAction::SetDefault => reinhardt_query::prelude::ForeignKeyAction::SetDefault,
		}
	}
}

impl From<reinhardt_query::prelude::ForeignKeyAction> for ForeignKeyAction {
	fn from(action: reinhardt_query::prelude::ForeignKeyAction) -> Self {
		match action {
			reinhardt_query::prelude::ForeignKeyAction::Restrict => ForeignKeyAction::Restrict,
			reinhardt_query::prelude::ForeignKeyAction::Cascade => ForeignKeyAction::Cascade,
			reinhardt_query::prelude::ForeignKeyAction::SetNull => ForeignKeyAction::SetNull,
			reinhardt_query::prelude::ForeignKeyAction::NoAction => ForeignKeyAction::NoAction,
			reinhardt_query::prelude::ForeignKeyAction::SetDefault => ForeignKeyAction::SetDefault,
			// reinhardt-query's ForeignKeyAction is non-exhaustive, so we need a catch-all
			_ => ForeignKeyAction::NoAction,
		}
	}
}

/// Convert a name to snake_case
///
/// Handles:
/// - Acronyms: inserts underscores at acronym-word boundaries
/// - Multiple separators: collapses consecutive `_`, `-`, ` `, `.` to single `_`
/// - Mixed case: properly handles camelCase and PascalCase
///
/// # Examples
///
/// ```rust,ignore
/// # use reinhardt_db::migrations::to_snake_case;
/// assert_eq!(to_snake_case("User"), "user");
/// assert_eq!(to_snake_case("BlogPost"), "blog_post");
/// assert_eq!(to_snake_case("HTTPResponse"), "http_response");
/// assert_eq!(to_snake_case("APIKey"), "api_key");
/// assert_eq!(to_snake_case("XMLParser"), "xml_parser");
/// assert_eq!(to_snake_case("User__Profile"), "user_profile");
/// assert_eq!(to_snake_case("public.users"), "public_users");
/// ```
pub fn to_snake_case(name: &str) -> String {
	if name.is_empty() {
		return String::new();
	}

	let mut result = String::with_capacity(name.len() + 4);
	let chars: Vec<char> = name.chars().collect();
	let mut prev_was_separator = true; // Treat start as separator to avoid leading underscore

	for i in 0..chars.len() {
		let ch = chars[i];

		// Handle separators: _, -, space, .
		if ch == '_' || ch == '-' || ch == ' ' || ch == '.' {
			// Only add underscore if previous char was not a separator
			if !prev_was_separator && !result.is_empty() {
				result.push('_');
			}
			prev_was_separator = true;
		} else if ch.is_ascii_uppercase() {
			if !prev_was_separator && i > 0 {
				let prev = chars[i - 1];
				let next = chars.get(i + 1);

				// Add underscore if:
				// 1. Previous char is lowercase (normal camelCase boundary)
				// OR
				// 2. Previous char is uppercase AND next char exists AND is lowercase
				//    (this handles acronyms like HTTPRequest → http_request)
				if prev.is_ascii_lowercase()
					|| (prev.is_ascii_uppercase() && next.is_some_and(|&n| n.is_ascii_lowercase()))
				{
					result.push('_');
				}
			}
			result.push(ch.to_ascii_lowercase());
			prev_was_separator = false;
		} else {
			result.push(ch.to_ascii_lowercase());
			prev_was_separator = false;
		}
	}

	result
}

/// Convert a snake_case name to PascalCase
///
/// Handles multiple separators: `_`, `.`, `-`, space
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_db::migrations::autodetector::to_pascal_case;
///
/// assert_eq!(to_pascal_case("user"), "User");
/// assert_eq!(to_pascal_case("blog_post"), "BlogPost");
/// assert_eq!(to_pascal_case("http_response"), "HttpResponse");
/// assert_eq!(to_pascal_case("following"), "Following");
/// assert_eq!(to_pascal_case("blocked_users"), "BlockedUsers");
/// assert_eq!(to_pascal_case("public.users"), "PublicUsers");
/// ```
pub fn to_pascal_case(name: &str) -> String {
	name.split(['_', '.', '-', ' '])
		.filter(|word| !word.is_empty())
		.map(|word| {
			let mut chars = word.chars();
			match chars.next() {
				Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
				None => String::new(),
			}
		})
		.collect()
}

/// ForeignKey reference information
#[derive(Debug, Clone, PartialEq)]
pub struct ForeignKeyInfo {
	/// Referenced table name
	pub referenced_table: String,
	/// Referenced column name (usually "id")
	pub referenced_column: String,
	/// ON DELETE action (Cascade, SetNull, Restrict, NoAction, SetDefault)
	pub on_delete: ForeignKeyAction,
	/// ON UPDATE action (Cascade, SetNull, Restrict, NoAction, SetDefault)
	pub on_update: ForeignKeyAction,
}

/// Field state for migration detection
#[derive(Debug, Clone)]
pub struct FieldState {
	pub name: String,
	pub field_type: super::FieldType,
	pub nullable: bool,
	pub params: std::collections::HashMap<String, String>,
	/// ForeignKey information if this field is a foreign key
	pub foreign_key: Option<ForeignKeyInfo>,
}

impl FieldState {
	pub fn new(name: impl Into<String>, field_type: super::FieldType, nullable: bool) -> Self {
		Self {
			name: name.into(),
			field_type,
			nullable,
			params: std::collections::HashMap::new(),
			foreign_key: None,
		}
	}

	/// Create a new FieldState with ForeignKey information
	pub fn with_foreign_key(
		name: impl Into<String>,
		field_type: super::FieldType,
		nullable: bool,
		foreign_key: ForeignKeyInfo,
	) -> Self {
		Self {
			name: name.into(),
			field_type,
			nullable,
			params: std::collections::HashMap::new(),
			foreign_key: Some(foreign_key),
		}
	}
}

/// Model state for migration detection
///
/// Django equivalent: `ModelState` in django/db/migrations/state.py
#[derive(Debug, Clone)]
pub struct ModelState {
	/// Application label (e.g., "auth", "blog")
	pub app_label: String,
	/// Model name (e.g., "User", "Post")
	pub name: String,
	/// Database table name (e.g., "users", "blog_posts")
	pub table_name: String,
	/// Fields: field_name -> FieldState
	pub fields: std::collections::BTreeMap<String, FieldState>,
	/// Model options (db_table, ordering, etc.)
	pub options: std::collections::HashMap<String, String>,
	/// Base model for inheritance
	pub base_model: Option<String>,
	/// Inheritance type: "single_table" or "joined_table"
	pub inheritance_type: Option<String>,
	/// Discriminator column for single table inheritance
	pub discriminator_column: Option<String>,
	/// Indexes: index_name -> IndexDefinition
	pub indexes: Vec<IndexDefinition>,
	/// Constraints: constraint_name -> ConstraintDefinition
	pub constraints: Vec<ConstraintDefinition>,
	/// ManyToMany relationships
	pub many_to_many_fields: Vec<ManyToManyMetadata>,
}

/// Index definition for a model
#[derive(Debug, Clone, PartialEq)]
pub struct IndexDefinition {
	/// Index name
	pub name: String,
	/// Fields to index (in order)
	pub fields: Vec<String>,
	/// Whether this is a unique index
	pub unique: bool,
}

/// Constraint definition for a model
#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintDefinition {
	/// Constraint name
	pub name: String,
	/// Constraint type (e.g., "check", "unique", "foreign_key")
	pub constraint_type: String,
	/// Fields involved in the constraint
	pub fields: Vec<String>,
	/// Additional constraint expression (e.g., CHECK condition)
	pub expression: Option<String>,
	/// ForeignKey-specific information (only for foreign_key type)
	pub foreign_key_info: Option<ForeignKeyConstraintInfo>,
}

/// ForeignKey constraint information
#[derive(Debug, Clone, PartialEq)]
pub struct ForeignKeyConstraintInfo {
	/// Referenced table name
	pub referenced_table: String,
	/// Referenced columns (usually ["id"])
	pub referenced_columns: Vec<String>,
	/// ON DELETE action
	pub on_delete: ForeignKeyAction,
	/// ON UPDATE action
	pub on_update: ForeignKeyAction,
}

impl ConstraintDefinition {
	/// Convert ConstraintDefinition to operations::Constraint
	pub fn to_constraint(&self) -> super::operations::Constraint {
		match self.constraint_type.as_str() {
			"unique" => super::operations::Constraint::Unique {
				name: self.name.clone(),
				columns: self.fields.clone(),
			},
			"check" => super::operations::Constraint::Check {
				name: self.name.clone(),
				expression: self.expression.clone().unwrap_or_default(),
			},
			"foreign_key" => {
				if let Some(fk_info) = &self.foreign_key_info {
					super::operations::Constraint::ForeignKey {
						name: self.name.clone(),
						columns: self.fields.clone(),
						referenced_table: fk_info.referenced_table.clone(),
						referenced_columns: fk_info.referenced_columns.clone(),
						on_delete: fk_info.on_delete,
						on_update: fk_info.on_update,
						deferrable: None,
					}
				} else {
					// Fallback if foreign_key_info is missing
					super::operations::Constraint::ForeignKey {
						name: self.name.clone(),
						columns: self.fields.clone(),
						referenced_table: String::new(),
						referenced_columns: vec!["id".to_string()],
						on_delete: ForeignKeyAction::Cascade,
						on_update: ForeignKeyAction::Cascade,
						deferrable: None,
					}
				}
			}
			"one_to_one" => {
				if let Some(fk_info) = &self.foreign_key_info {
					super::operations::Constraint::OneToOne {
						name: self.name.clone(),
						column: self.fields.first().cloned().unwrap_or_default(),
						referenced_table: fk_info.referenced_table.clone(),
						referenced_column: fk_info
							.referenced_columns
							.first()
							.cloned()
							.unwrap_or_else(|| "id".to_string()),
						on_delete: fk_info.on_delete,
						on_update: fk_info.on_update,
						deferrable: None,
					}
				} else {
					// Fallback
					super::operations::Constraint::OneToOne {
						name: self.name.clone(),
						column: self.fields.first().cloned().unwrap_or_default(),
						referenced_table: String::new(),
						referenced_column: "id".to_string(),
						on_delete: ForeignKeyAction::Cascade,
						on_update: ForeignKeyAction::Cascade,
						deferrable: None,
					}
				}
			}
			_ => {
				// Default to Check constraint with empty expression
				super::operations::Constraint::Check {
					name: self.name.clone(),
					expression: self.expression.clone().unwrap_or_default(),
				}
			}
		}
	}
}

impl ModelState {
	/// Create a new ModelState with app_label and name
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::ModelState;
	///
	/// let model = ModelState::new("myapp", "User");
	/// assert_eq!(model.app_label, "myapp");
	/// assert_eq!(model.name, "User");
	/// assert_eq!(model.table_name, "user");
	/// assert_eq!(model.fields.len(), 0);
	/// ```
	pub fn new(app_label: impl Into<String>, name: impl Into<String>) -> Self {
		let name_str = name.into();
		// Convert model name to table name (e.g., "User" -> "user", "BlogPost" -> "blog_post")
		let table_name = to_snake_case(&name_str);

		Self {
			app_label: app_label.into(),
			name: name_str,
			table_name,
			fields: std::collections::BTreeMap::new(),
			options: std::collections::HashMap::new(),
			base_model: None,
			inheritance_type: None,
			discriminator_column: None,
			indexes: Vec::new(),
			constraints: Vec::new(),
			many_to_many_fields: Vec::new(),
		}
	}

	/// Add a field to this model
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{ModelState, FieldState, FieldType};
	///
	/// let mut model = ModelState::new("myapp", "User");
	/// let field = FieldState::new("email", FieldType::VarChar(255), false);
	/// model.add_field(field);
	/// assert_eq!(model.fields.len(), 1);
	/// assert!(model.has_field("email"));
	/// ```
	pub fn add_field(&mut self, field: FieldState) {
		self.fields.insert(field.name.clone(), field);
	}

	/// Get a field by name
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{ModelState, FieldState, FieldType};
	///
	/// let mut model = ModelState::new("myapp", "User");
	/// let field = FieldState::new("email", FieldType::VarChar(255), false);
	/// model.add_field(field);
	///
	/// let retrieved = model.get_field("email");
	/// assert!(retrieved.is_some());
	/// assert_eq!(retrieved.unwrap().field_type, FieldType::VarChar(255));
	/// ```
	pub fn get_field(&self, name: &str) -> Option<&FieldState> {
		self.fields.get(name)
	}

	/// Check if a field exists
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{ModelState, FieldState, FieldType};
	///
	/// let mut model = ModelState::new("myapp", "User");
	/// let field = FieldState::new("email", FieldType::VarChar(255), false);
	/// model.add_field(field);
	///
	/// assert!(model.has_field("email"));
	/// assert!(!model.has_field("username"));
	/// ```
	pub fn has_field(&self, name: &str) -> bool {
		self.fields.contains_key(name)
	}

	/// Rename a field
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{ModelState, FieldState, FieldType};
	///
	/// let mut model = ModelState::new("myapp", "User");
	/// let field = FieldState::new("email", FieldType::VarChar(255), false);
	/// model.add_field(field);
	///
	/// model.rename_field("email", "email_address".to_string());
	/// assert!(!model.has_field("email"));
	/// assert!(model.has_field("email_address"));
	/// ```
	pub fn rename_field(&mut self, old_name: &str, new_name: String) {
		if let Some(mut field) = self.fields.remove(old_name) {
			field.name = new_name.clone();
			self.fields.insert(new_name, field);
		}
	}

	/// Add a constraint to this model
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{ModelState, ConstraintDefinition};
	///
	/// let mut model = ModelState::new("myapp", "User");
	/// let constraint = ConstraintDefinition {
	///     name: "unique_email".to_string(),
	///     constraint_type: "unique".to_string(),
	///     fields: vec!["email".to_string()],
	///     expression: None,
	///     foreign_key_info: None,
	/// };
	/// model.add_constraint(constraint);
	/// assert_eq!(model.constraints.len(), 1);
	/// ```
	pub fn add_constraint(&mut self, constraint: ConstraintDefinition) {
		self.constraints.push(constraint);
	}

	/// Add a ForeignKey constraint from field information
	pub fn add_foreign_key_constraint_from_field(&mut self, field_name: &str) {
		if let Some(field) = self.fields.get(field_name)
			&& let Some(ref fk_info) = field.foreign_key
		{
			let constraint = ConstraintDefinition {
				name: format!("fk_{}_{}", self.table_name, field_name),
				constraint_type: "foreign_key".to_string(),
				fields: vec![field_name.to_string()],
				expression: None,
				foreign_key_info: Some(ForeignKeyConstraintInfo {
					referenced_table: fk_info.referenced_table.clone(),
					referenced_columns: vec![fk_info.referenced_column.clone()],
					on_delete: fk_info.on_delete,
					on_update: fk_info.on_update,
				}),
			};
			self.add_constraint(constraint);
		}
	}
}

/// Project state for migration detection
///
/// Django equivalent: `ProjectState` in django/db/migrations/state.py
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_db::migrations::{ProjectState, ModelState, FieldState, FieldType};
///
/// let mut state = ProjectState::new();
/// let mut model = ModelState::new("myapp", "User");
/// model.add_field(FieldState::new("id", FieldType::Integer, false));
/// state.add_model(model);
///
/// assert!(state.get_model("myapp", "User").is_some());
/// ```
#[derive(Debug, Clone)]
pub struct ProjectState {
	/// Models: (app_label, model_name) -> ModelState
	pub models: std::collections::BTreeMap<(String, String), ModelState>,
}

impl Default for ProjectState {
	fn default() -> Self {
		Self::new()
	}
}

impl ProjectState {
	pub fn to_database_schema(&self) -> super::schema_diff::DatabaseSchema {
		let mut tables = BTreeMap::new();

		for ((_app_label, _model_name), model_state) in &self.models {
			let mut columns = BTreeMap::new();
			for (field_name, field_state) in &model_state.fields {
				// FieldType enum already contains all type information including length
				// (e.g., VarChar(255), Decimal { precision, scale }). Direct mapping is correct.
				// Database-specific SQL generation is handled by ColumnTypeDefinition::to_sql_for_dialect.
				let data_type = field_state.field_type.clone();
				let nullable = field_state.nullable;
				let primary_key = field_state
					.params
					.get("primary_key")
					.is_some_and(|s| s == "true");
				let auto_increment = field_state
					.params
					.get("auto_increment")
					.is_some_and(|s| s == "true");
				let default = field_state.params.get("default").cloned();

				columns.insert(
					field_name.clone(),
					super::schema_diff::ColumnSchema {
						name: field_name.clone(),
						data_type,
						nullable,
						default,
						primary_key,
						auto_increment,
					},
				);
			}
			// Convert constraints from ModelState to ConstraintSchema
			let constraints: Vec<super::schema_diff::ConstraintSchema> = model_state
				.constraints
				.iter()
				.map(|c| super::schema_diff::ConstraintSchema {
					name: c.name.clone(),
					constraint_type: c.constraint_type.clone(),
					definition: c.fields.join(", "),
					foreign_key_info: None,
				})
				.collect();

			tables.insert(
				model_state.table_name.clone(),
				super::schema_diff::TableSchema {
					name: model_state.table_name.clone(),
					columns,
					indexes: Vec::new(),
					constraints,
				},
			);
		}

		super::schema_diff::DatabaseSchema { tables }
	}

	/// Convert ProjectState to DatabaseSchema for a specific app
	///
	/// This method filters models by app_label before converting to DatabaseSchema,
	/// allowing per-app migration generation.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::ProjectState;
	///
	/// let state = ProjectState::from_global_registry();
	/// let schema = state.to_database_schema_for_app("users");
	/// // schema contains only tables for the "users" app
	/// ```
	pub fn to_database_schema_for_app(
		&self,
		app_label: &str,
	) -> super::schema_diff::DatabaseSchema {
		let mut tables = BTreeMap::new();

		for ((this_app_label, _model_name), model_state) in &self.models {
			// Filter by app_label
			if this_app_label == app_label {
				let mut columns = BTreeMap::new();
				for (field_name, field_state) in &model_state.fields {
					let data_type = field_state.field_type.clone();
					let nullable = field_state.nullable;
					let primary_key = field_state
						.params
						.get("primary_key")
						.is_some_and(|s| s == "true");
					let auto_increment = field_state
						.params
						.get("auto_increment")
						.is_some_and(|s| s == "true");
					let default = field_state.params.get("default").cloned();

					columns.insert(
						field_name.clone(),
						super::schema_diff::ColumnSchema {
							name: field_name.clone(),
							data_type,
							nullable,
							default,
							primary_key,
							auto_increment,
						},
					);
				}

				// Convert constraints from ModelState to ConstraintSchema
				let constraints: Vec<super::schema_diff::ConstraintSchema> = model_state
					.constraints
					.iter()
					.map(|c| super::schema_diff::ConstraintSchema {
						name: c.name.clone(),
						constraint_type: c.constraint_type.clone(),
						definition: c.fields.join(", "),
						foreign_key_info: None,
					})
					.collect();

				tables.insert(
					model_state.table_name.clone(),
					super::schema_diff::TableSchema {
						name: model_state.table_name.clone(),
						columns,
						indexes: Vec::new(),
						constraints,
					},
				);
			}
		}

		super::schema_diff::DatabaseSchema { tables }
	}

	/// Create a new empty ProjectState
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::ProjectState;
	///
	/// let state = ProjectState::new();
	/// assert_eq!(state.models.len(), 0);
	/// ```
	pub fn new() -> Self {
		Self {
			models: std::collections::BTreeMap::new(),
		}
	}

	/// Add a model to this project state
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{ProjectState, ModelState};
	///
	/// let mut state = ProjectState::new();
	/// let model = ModelState::new("myapp", "User");
	/// state.add_model(model);
	///
	/// assert_eq!(state.models.len(), 1);
	/// assert!(state.get_model("myapp", "User").is_some());
	/// ```
	pub fn add_model(&mut self, model: ModelState) {
		let key = (model.app_label.clone(), model.name.clone());
		self.models.insert(key, model);
	}

	/// Get a model by app_label and model_name
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{ProjectState, ModelState};
	///
	/// let mut state = ProjectState::new();
	/// let model = ModelState::new("myapp", "User");
	/// state.add_model(model);
	///
	/// let retrieved = state.get_model("myapp", "User");
	/// assert!(retrieved.is_some());
	/// assert_eq!(retrieved.unwrap().name, "User");
	/// ```
	pub fn get_model(&self, app_label: &str, model_name: &str) -> Option<&ModelState> {
		self.models
			.get(&(app_label.to_string(), model_name.to_string()))
	}

	/// Get a mutable reference to a model
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{ProjectState, ModelState, FieldState, FieldType};
	///
	/// let mut state = ProjectState::new();
	/// let model = ModelState::new("myapp", "User");
	/// state.add_model(model);
	///
	/// if let Some(model) = state.get_model_mut("myapp", "User") {
	///     let field = FieldState::new("email", FieldType::VarChar(255), false);
	///     model.add_field(field);
	/// }
	///
	/// assert!(state.get_model("myapp", "User").unwrap().has_field("email"));
	/// ```
	pub fn get_model_mut(&mut self, app_label: &str, model_name: &str) -> Option<&mut ModelState> {
		self.models
			.get_mut(&(app_label.to_string(), model_name.to_string()))
	}

	/// Get primary key field type for a model
	///
	/// Returns the field type of the primary key, defaulting to Uuid if not found
	/// or if the model is not in the state.
	///
	/// # Examples
	///
	/// ```ignore
	/// # // This method is private and cannot be called from external code
	/// use reinhardt_db::migrations::{ProjectState, ModelState, FieldState, FieldType};
	///
	/// let mut state = ProjectState::new();
	/// let mut model = ModelState::new("myapp", "User");
	/// model.add_field(FieldState::new("id", FieldType::Integer, false));
	/// state.add_model(model);
	///
	/// let pk_type = state.get_primary_key_type("myapp", "User");
	/// assert_eq!(pk_type, FieldType::Integer);
	/// ```
	fn get_primary_key_type(&self, app_label: &str, model_name: &str) -> super::FieldType {
		// JSON update
		if let Some(model_state) = self.get_model(app_label, model_name) {
			// Search the “id” field (by default primary key name)
			if let Some((_, id_field)) = model_state
				.fields
				.iter()
				.find(|(name, _)| name.as_str() == "id")
			{
				return id_field.field_type.clone();
			}

			// Search fields with the primary_key parameter
			if let Some((_, pk_field)) = model_state
				.fields
				.iter()
				.find(|(_, f)| f.params.get("primary_key").map(String::as_str) == Some("true"))
			{
				return pk_field.field_type.clone();
			}
		}

		// If not found in to_state, search the global registry
		if let Some(model_meta) =
			super::model_registry::global_registry().get_model(app_label, model_name)
		{
			// Search the “id” fields
			if let Some(id_field) = model_meta.fields.get("id") {
				return id_field.field_type.clone();
			}

			// Search fields with the primary_key parameter
			for field_meta in model_meta.fields.values() {
				if field_meta.params.get("primary_key").map(String::as_str) == Some("true") {
					return field_meta.field_type.clone();
				}
			}
		}

		// The default is UUID (current hardcoded value)
		super::FieldType::Uuid
	}

	/// Get a model by table name
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{ProjectState, ModelState};
	///
	/// let mut state = ProjectState::new();
	/// let mut model = ModelState::new("myapp", "User");
	/// model.table_name = "myapp_user".to_string();
	/// state.add_model(model);
	///
	/// let retrieved = state.get_model_by_table_name("myapp", "myapp_user");
	/// assert!(retrieved.is_some());
	/// assert_eq!(retrieved.unwrap().name, "User");
	/// ```
	pub fn get_model_by_table_name(
		&self,
		app_label: &str,
		table_name: &str,
	) -> Option<&ModelState> {
		self.models
			.values()
			.find(|model| model.app_label == app_label && model.table_name == table_name)
	}

	/// Filter models by app_label and return a new ProjectState containing only those models
	///
	/// This method is used to create app-specific ProjectState for per-app migration generation.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{ProjectState, ModelState};
	///
	/// let mut state = ProjectState::new();
	/// state.add_model(ModelState::new("users", "User"));
	/// state.add_model(ModelState::new("users", "Profile"));
	/// state.add_model(ModelState::new("posts", "Post"));
	///
	/// let users_state = state.filter_by_app("users");
	/// assert_eq!(users_state.models.len(), 2);
	/// assert!(users_state.get_model("users", "User").is_some());
	/// assert!(users_state.get_model("users", "Profile").is_some());
	/// assert!(users_state.get_model("posts", "Post").is_none());
	/// ```
	pub fn filter_by_app(&self, app_label: &str) -> Self {
		let mut filtered = Self::new();
		for ((app, _model_name), model_state) in &self.models {
			if app == app_label {
				filtered.add_model(model_state.clone());
			}
		}
		filtered
	}

	/// Remove a model from this project state
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{ProjectState, ModelState};
	///
	/// let mut state = ProjectState::new();
	/// let model = ModelState::new("myapp", "User");
	/// state.add_model(model);
	///
	/// state.remove_model("myapp", "User");
	/// assert!(state.get_model("myapp", "User").is_none());
	/// ```
	pub fn remove_model(&mut self, app_label: &str, model_name: &str) -> Option<ModelState> {
		self.models
			.remove(&(app_label.to_string(), model_name.to_string()))
	}

	/// Rename a model
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{ProjectState, ModelState};
	///
	/// let mut state = ProjectState::new();
	/// let model = ModelState::new("myapp", "User");
	/// state.add_model(model);
	///
	/// state.rename_model("myapp", "User", "Account".to_string());
	/// assert!(state.get_model("myapp", "User").is_none());
	/// assert!(state.get_model("myapp", "Account").is_some());
	/// ```
	pub fn rename_model(&mut self, app_label: &str, old_name: &str, new_name: String) {
		if let Some(mut model) = self
			.models
			.remove(&(app_label.to_string(), old_name.to_string()))
		{
			model.name = new_name.clone();
			self.models.insert((app_label.to_string(), new_name), model);
		}
	}

	/// Load ProjectState from the global model registry
	///
	/// Django equivalent: `ProjectState.from_apps()` in django/db/migrations/state.py:594-600
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::ProjectState;
	///
	/// let state = ProjectState::from_global_registry();
	// state will contain all models registered in the global registry
	/// ```
	pub fn from_global_registry() -> Self {
		use super::model_registry::global_registry;

		let registry = global_registry();
		let models_metadata = registry.get_models();

		let mut state = ProjectState::new();
		let mut intermediate_tables = Vec::new();

		// First, add all regular models
		for metadata in &models_metadata {
			let model_state = metadata.to_model_state();
			state.add_model(model_state);
		}

		// Then, generate intermediate tables for ManyToMany relationships
		for metadata in &models_metadata {
			for m2m in &metadata.many_to_many_fields {
				// Generate intermediate table for this ManyToMany relationship
				let intermediate_table = state.create_intermediate_table_for_m2m(
					&metadata.app_label,
					&metadata.model_name,
					&metadata.table_name,
					m2m,
				);
				intermediate_tables.push(intermediate_table);
			}
		}

		// Add all intermediate tables to state
		for table in intermediate_tables {
			state.add_model(table);
		}

		state
	}

	/// Create an intermediate table ModelState for a ManyToMany relationship
	///
	/// This generates a ModelState representing the intermediate/junction table
	/// for a ManyToMany relationship.
	///
	/// # Arguments
	///
	/// * `source_app_label` - The app label of the source model (e.g., "auth")
	/// * `source_model_name` - The name of the source model (e.g., "User")
	/// * `source_table_name` - The table name of the source model (e.g., "auth_user")
	/// * `m2m` - ManyToMany relationship metadata
	///
	/// # Returns
	///
	/// A `ModelState` representing the intermediate table with:
	/// - Auto-increment primary key `id`
	/// - Foreign key to source model: `from_{source_model}_id`
	/// - Foreign key to target model: `to_{target_model}_id`
	/// - Foreign key constraints with CASCADE
	/// - Unique constraint on (from_id, to_id)
	fn create_intermediate_table_for_m2m(
		&self,
		source_app_label: &str,
		source_model_name: &str,
		source_table_name: &str,
		m2m: &super::model_registry::ManyToManyMetadata,
	) -> ModelState {
		// Generate table name: {source_table_name}_{field_name}
		// Example: "auth_user" + "_" + "following" = "auth_user_following"
		let table_name = m2m
			.through
			.clone()
			.unwrap_or_else(|| format!("{}_{}", source_table_name, m2m.field_name));

		// Generate model name: PascalCase version of field_name
		// Example: "following" -> "UserFollowing"
		let model_name = format!("{}{}", source_model_name, to_pascal_case(&m2m.field_name));

		let mut model_state = ModelState::new(source_app_label, &model_name);
		model_state.table_name = table_name.clone();

		// Add primary key field: id
		let mut id_field = FieldState::new("id".to_string(), super::FieldType::Integer, false);
		id_field
			.params
			.insert("primary_key".to_string(), "true".to_string());
		id_field
			.params
			.insert("auto_increment".to_string(), "true".to_string());
		model_state.add_field(id_field);

		// Determine source and target field names
		let source_field_name = m2m
			.source_field
			.clone()
			.unwrap_or_else(|| format!("from_{}_id", to_snake_case(source_model_name)));
		let target_field_name = m2m
			.target_field
			.clone()
			.unwrap_or_else(|| format!("to_{}_id", to_snake_case(&m2m.to_model)));

		// Determine the primary key type for the source and target from the registry
		let source_pk_type = self.get_primary_key_type(source_app_label, source_model_name);
		// Extract target app_label from to_model (may be in "app.Model" format)
		let (target_app, target_model) = if m2m.to_model.contains('.') {
			let parts: Vec<&str> = m2m.to_model.split('.').collect();
			(parts[0], parts[1])
		} else {
			(source_app_label, m2m.to_model.as_str())
		};

		let target_pk_type = self.get_primary_key_type(target_app, target_model);

		// Add foreign key to source model: from_{source_model}_id
		let mut from_field =
			FieldState::new(source_field_name.clone(), source_pk_type.clone(), false);
		from_field
			.params
			.insert("not_null".to_string(), "true".to_string());
		from_field.foreign_key = Some(ForeignKeyInfo {
			referenced_table: source_table_name.to_string(),
			referenced_column: "id".to_string(),
			on_delete: ForeignKeyAction::Cascade,
			on_update: ForeignKeyAction::Cascade,
		});
		model_state.add_field(from_field);

		// Add foreign key to target model: to_{target_model}_id
		// Get target table name from ProjectState, fallback to naming convention if model not found
		let target_table_name = self
			.get_model(target_app, target_model)
			.map(|m| m.table_name.clone())
			.unwrap_or_else(|| format!("{}_{}", target_app, to_snake_case(target_model)));
		let mut to_field = FieldState::new(target_field_name.clone(), target_pk_type, false);
		to_field
			.params
			.insert("not_null".to_string(), "true".to_string());
		to_field.foreign_key = Some(ForeignKeyInfo {
			referenced_table: target_table_name,
			referenced_column: "id".to_string(),
			on_delete: ForeignKeyAction::Cascade,
			on_update: ForeignKeyAction::Cascade,
		});
		model_state.add_field(to_field);

		// Add foreign key constraints
		model_state.add_foreign_key_constraint_from_field(&source_field_name);
		model_state.add_foreign_key_constraint_from_field(&target_field_name);

		// Add unique constraint on (from_id, to_id)
		let unique_constraint = ConstraintDefinition {
			name: format!("{}_unique", table_name),
			constraint_type: "unique".to_string(),
			fields: vec![source_field_name, target_field_name],
			expression: None,
			foreign_key_info: None,
		};
		model_state.constraints.push(unique_constraint);

		model_state
	}

	/// Load ProjectState from a list of migrations
	///
	/// This method constructs a ProjectState by applying all operations
	/// from the provided migrations in order. This is useful for determining
	/// what the database schema should look like after applying all migrations.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{ProjectState, Migration};
	///
	/// let migrations = vec![/* ... */];
	/// let state = ProjectState::from_migrations(&migrations);
	/// // state will contain all models as they would exist after applying all migrations
	/// ```
	pub fn from_migrations(migrations: &[super::migration::Migration]) -> Self {
		let mut state = Self::new();
		for migration in migrations {
			state.apply_migration_operations(&migration.operations, &migration.app_label);
		}
		state
	}

	/// Apply migration operations to this project state
	///
	/// This method processes each operation and updates the ProjectState accordingly.
	/// It handles:
	/// - CreateTable: Creates a new model
	/// - DropTable: Removes a model
	/// - AddColumn: Adds a field to a model
	/// - DropColumn: Removes a field from a model
	/// - AlterColumn: Modifies a field
	/// - RenameTable: Renames a model's table
	/// - RenameColumn: Renames a field
	/// - Other operations are logged but not applied to state
	pub fn apply_migration_operations(
		&mut self,
		operations: &[super::operations::Operation],
		app_label: &str,
	) {
		use super::operations::Operation;

		for op in operations {
			match op {
				Operation::CreateTable { name, columns, .. } => {
					// Create a new model from the table definition
					// Use the provided app_label instead of hardcoding "auto"
					// Convert table name to model name (PascalCase)
					let model_name = Self::table_name_to_model_name(name, app_label);
					let mut model = ModelState::new(app_label, model_name);
					model.table_name = name.to_string();

					// Convert columns to fields
					for col in columns {
						let field = self.column_def_to_field_state(col);
						model.add_field(field);
					}

					self.add_model(model);
				}
				Operation::DropTable { name } => {
					// Find and remove the model with this table name
					let keys_to_remove: Vec<_> = self
						.models
						.iter()
						.filter(|(_, model)| model.table_name == *name)
						.map(|(key, _)| key.clone())
						.collect();

					for key in keys_to_remove {
						self.models.remove(&key);
					}
				}
				Operation::AddColumn { table, column, .. } => {
					// Find the model with this table name and add the field
					let field = self.column_def_to_field_state(column);
					if let Some(model) = self.find_model_by_table_mut(table) {
						model.add_field(field);
					}
				}
				Operation::DropColumn { table, column } => {
					// Find the model and remove the field
					if let Some(model) = self.find_model_by_table_mut(table) {
						model.fields.remove(column);
					}
				}
				Operation::AlterColumn {
					table,
					column,
					new_definition,
					..
				} => {
					// Find the model and update the field
					let new_field = self.column_def_to_field_state(new_definition);
					// Keep the old field name but update everything else
					let mut updated_field = new_field;
					updated_field.name = column.to_string();

					// If model exists, update the field
					if let Some(model) = self.find_model_by_table_mut(table) {
						model.fields.insert(column.to_string(), updated_field);
					} else {
						// If model doesn't exist, create it and add the field
						// This handles the case where AlterColumn is used in initial migrations
						// before CreateTable (which shouldn't happen, but does in some legacy migrations)
						let model_name = Self::table_name_to_model_name(table, app_label);
						let mut model = ModelState::new(app_label, model_name);
						model.table_name = table.to_string();
						model.add_field(updated_field);
						self.add_model(model);
					}
				}
				Operation::RenameTable { old_name, new_name } => {
					// Find the model with old table name and update it
					if let Some(model) = self.find_model_by_table_mut(old_name) {
						model.table_name = new_name.to_string();
					}
				}
				Operation::RenameColumn {
					table,
					old_name,
					new_name,
				} => {
					// Find the model and rename the field
					if let Some(model) = self.find_model_by_table_mut(table) {
						model.rename_field(old_name, new_name.to_string());
					}
				}
				// Other operations don't affect the schema state in ways we track
				_ => {
					// Operations like CreateIndex, DropIndex, RunSQL, etc.
					// are not currently tracked in ProjectState
				}
			}
		}
	}

	/// Helper: Find a model by table name (immutable)
	pub fn find_model_by_table(&self, table_name: &str) -> Option<&ModelState> {
		self.models
			.values()
			.find(|model| model.table_name == table_name)
	}

	/// Helper: Find a model by table name (mutable)
	pub fn find_model_by_table_mut(&mut self, table_name: &str) -> Option<&mut ModelState> {
		self.models
			.values_mut()
			.find(|model| model.table_name == table_name)
	}

	/// Helper: Convert table name to model name (PascalCase)
	///
	/// Examples:
	/// - `auth_user` → `User` (with app_label="auth")
	/// - `auth_password_reset_token` → `PasswordResetToken`
	/// - `dm_message` → `DMMessage`
	/// - `dm_room` → `DMRoom`
	/// - `profile_profile` → `Profile`
	fn table_name_to_model_name(table_name: &str, app_label: &str) -> String {
		// Remove app_label prefix if present (e.g., "auth_user" → "user")
		let prefix = format!("{}_", app_label);
		let name_without_prefix = if table_name.starts_with(&prefix) {
			&table_name[prefix.len()..]
		} else {
			table_name
		};

		// Convert snake_case to PascalCase
		name_without_prefix
			.split('_')
			.map(|word| {
				let mut chars = word.chars();
				match chars.next() {
					Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
					None => String::new(),
				}
			})
			.collect()
	}

	/// Helper: Convert ColumnDefinition to FieldState
	fn column_def_to_field_state(&self, col: &super::operations::ColumnDefinition) -> FieldState {
		let mut params = std::collections::HashMap::new();

		if col.primary_key {
			params.insert("primary_key".to_string(), "true".to_string());
		}
		if col.auto_increment {
			params.insert("auto_increment".to_string(), "true".to_string());
		}
		if col.unique {
			params.insert("unique".to_string(), "true".to_string());
		}
		if let Some(default) = &col.default {
			params.insert("default".to_string(), default.to_string());
		}

		FieldState {
			name: col.name.to_string(),
			field_type: col.type_definition.clone(),
			nullable: !col.not_null,
			params,
			foreign_key: None,
		}
	}
}

/// Configuration for similarity threshold calculation
///
/// This struct controls how aggressive the autodetector is when matching
/// models and fields across apps for rename/move detection.
///
/// Uses a hybrid similarity metric combining:
/// - Jaro-Winkler distance: Best for detecting prefix similarities (e.g., "UserModel" vs "UserProfile")
/// - Levenshtein distance: Best for detecting edit operations (e.g., "User" vs "Users")
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_db::migrations::SimilarityConfig;
///
/// // Default configuration (70% threshold for models, 80% for fields)
/// let config = SimilarityConfig::default();
/// assert_eq!(config.model_threshold(), 0.7);
///
/// // Custom conservative configuration (higher threshold = fewer matches)
/// let config = SimilarityConfig::new(0.85, 0.90).unwrap();
///
/// // Liberal configuration (lower threshold = more matches, but more false positives)
/// let config = SimilarityConfig::new(0.60, 0.70).unwrap();
///
/// // Custom with specific algorithm weights
/// let config = SimilarityConfig::with_weights(0.75, 0.85, 0.6, 0.4).unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct SimilarityConfig {
	/// Threshold for model similarity (0.45 - 0.95)
	/// Higher values mean stricter matching (fewer false positives)
	model_threshold: f64,
	/// Threshold for field similarity (0.45 - 0.95)
	/// Higher values mean stricter matching
	field_threshold: f64,
	/// Weight for Jaro-Winkler component (0.0 - 1.0, default 0.7)
	/// Higher values prioritize prefix matching
	jaro_winkler_weight: f64,
	/// Weight for Levenshtein component (0.0 - 1.0, default 0.3)
	/// Higher values prioritize edit distance
	/// Note: jaro_winkler_weight + levenshtein_weight should equal 1.0
	levenshtein_weight: f64,
}

impl SimilarityConfig {
	/// Create a new SimilarityConfig with custom thresholds
	///
	/// # Arguments
	///
	/// * `model_threshold` - Similarity threshold for model matching (0.45 - 0.95)
	/// * `field_threshold` - Similarity threshold for field matching (0.45 - 0.95)
	///
	/// # Errors
	///
	/// Returns an error if thresholds are outside the valid range (0.45 - 0.95).
	/// Values below 0.45 would produce too many false positives.
	/// Values above 0.95 would make matching nearly impossible.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::SimilarityConfig;
	///
	/// let config = SimilarityConfig::new(0.75, 0.85).unwrap();
	/// assert_eq!(config.model_threshold(), 0.75);
	/// assert_eq!(config.field_threshold(), 0.85);
	///
	/// // Invalid threshold (too low)
	/// assert!(SimilarityConfig::new(0.4, 0.8).is_err());
	///
	/// // Invalid threshold (too high)
	/// assert!(SimilarityConfig::new(0.96, 0.8).is_err());
	/// ```
	pub fn new(model_threshold: f64, field_threshold: f64) -> Result<Self, String> {
		Self::with_weights(model_threshold, field_threshold, 0.7, 0.3)
	}

	/// Create a new SimilarityConfig with custom thresholds and algorithm weights
	///
	/// # Arguments
	///
	/// * `model_threshold` - Similarity threshold for model matching (0.45 - 0.95)
	/// * `field_threshold` - Similarity threshold for field matching (0.45 - 0.95)
	/// * `jaro_winkler_weight` - Weight for Jaro-Winkler component (0.0 - 1.0)
	/// * `levenshtein_weight` - Weight for Levenshtein component (0.0 - 1.0)
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - Thresholds are outside the valid range (0.45 - 0.95)
	/// - Weights are outside the valid range (0.0 - 1.0)
	/// - Weights don't sum to approximately 1.0 (within 0.01 tolerance)
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::SimilarityConfig;
	///
	/// // Prefer Jaro-Winkler for prefix matching
	/// let config = SimilarityConfig::with_weights(0.75, 0.85, 0.8, 0.2).unwrap();
	///
	/// // Prefer Levenshtein for edit distance
	/// let config = SimilarityConfig::with_weights(0.75, 0.85, 0.3, 0.7).unwrap();
	///
	/// // Invalid: weights don't sum to 1.0
	/// assert!(SimilarityConfig::with_weights(0.75, 0.85, 0.5, 0.3).is_err());
	/// ```
	pub fn with_weights(
		model_threshold: f64,
		field_threshold: f64,
		jaro_winkler_weight: f64,
		levenshtein_weight: f64,
	) -> Result<Self, String> {
		// Validate thresholds are in reasonable range
		// Minimum 0.45: below this produces too many false positives
		// Maximum 0.95: above this makes matching nearly impossible
		if !(0.45..=0.95).contains(&model_threshold) {
			return Err(format!(
				"model_threshold must be between 0.45 and 0.95, got {}",
				model_threshold
			));
		}
		if !(0.45..=0.95).contains(&field_threshold) {
			return Err(format!(
				"field_threshold must be between 0.45 and 0.95, got {}",
				field_threshold
			));
		}

		// Validate weights are in valid range
		if !(0.0..=1.0).contains(&jaro_winkler_weight) {
			return Err(format!(
				"jaro_winkler_weight must be between 0.0 and 1.0, got {}",
				jaro_winkler_weight
			));
		}
		if !(0.0..=1.0).contains(&levenshtein_weight) {
			return Err(format!(
				"levenshtein_weight must be between 0.0 and 1.0, got {}",
				levenshtein_weight
			));
		}

		// Validate weights sum to approximately 1.0 (allow small floating point errors)
		let weight_sum = jaro_winkler_weight + levenshtein_weight;
		if (weight_sum - 1.0).abs() > 0.01 {
			return Err(format!(
				"jaro_winkler_weight + levenshtein_weight must sum to 1.0, got {} + {} = {}",
				jaro_winkler_weight, levenshtein_weight, weight_sum
			));
		}

		Ok(Self {
			model_threshold,
			field_threshold,
			jaro_winkler_weight,
			levenshtein_weight,
		})
	}

	/// Get the model similarity threshold
	pub fn model_threshold(&self) -> f64 {
		self.model_threshold
	}

	/// Get the field similarity threshold
	pub fn field_threshold(&self) -> f64 {
		self.field_threshold
	}
}

impl Default for SimilarityConfig {
	/// Default configuration with balanced thresholds and weights
	///
	/// - Model threshold: 0.7 (70% similarity required)
	/// - Field threshold: 0.8 (80% similarity required)
	/// - Jaro-Winkler weight: 0.7 (70% weight for prefix matching)
	/// - Levenshtein weight: 0.3 (30% weight for edit distance)
	fn default() -> Self {
		Self {
			model_threshold: 0.7,
			field_threshold: 0.8,
			jaro_winkler_weight: 0.7,
			levenshtein_weight: 0.3,
		}
	}
}

/// Migration autodetector
///
/// Django equivalent: `MigrationAutodetector` in django/db/migrations/autodetector.py
///
/// Detects schema changes between two ProjectStates and generates migrations.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_db::migrations::{MigrationAutodetector, ProjectState, ModelState, FieldState, FieldType};
///
/// let mut from_state = ProjectState::new();
/// let mut to_state = ProjectState::new();
///
// Add a new model to to_state
/// let mut model = ModelState::new("myapp", "User");
/// model.add_field(FieldState::new("id", FieldType::Integer, false));
/// to_state.add_model(model);
///
/// let detector = MigrationAutodetector::new(from_state, to_state);
/// let changes = detector.detect_changes();
///
// Should detect the new model creation
/// assert_eq!(changes.created_models.len(), 1);
/// ```
pub struct MigrationAutodetector {
	from_state: ProjectState,
	to_state: ProjectState,
	similarity_config: SimilarityConfig,
}

/// Type alias for moved model information: (from_app, to_app, model_name, rename_table, old_table, new_table)
type MovedModelInfo = (String, String, String, bool, Option<String>, Option<String>);

/// Type alias for model match result: ((deleted_app, deleted_model), (created_app, created_model), similarity_score)
type ModelMatchResult = ((String, String), (String, String), f64);

/// Detected changes between two project states
#[derive(Debug, Clone, Default)]
pub struct DetectedChanges {
	/// Models that were created: (app_label, model_name)
	pub created_models: Vec<(String, String)>,
	/// Models that were deleted: (app_label, model_name)
	pub deleted_models: Vec<(String, String)>,
	/// Fields that were added: (app_label, model_name, field_name)
	pub added_fields: Vec<(String, String, String)>,
	/// Fields that were removed: (app_label, model_name, field_name)
	pub removed_fields: Vec<(String, String, String)>,
	/// Fields that were altered: (app_label, model_name, field_name)
	pub altered_fields: Vec<(String, String, String)>,
	/// Models that were renamed: (app_label, old_name, new_name)
	pub renamed_models: Vec<(String, String, String)>,
	/// Models that were moved between apps: (from_app, to_app, model_name, rename_table, old_table, new_table)
	pub moved_models: Vec<MovedModelInfo>,
	/// Fields that were renamed: (app_label, model_name, old_name, new_name)
	pub renamed_fields: Vec<(String, String, String, String)>,
	/// Indexes that were added: (app_label, model_name, IndexDefinition)
	pub added_indexes: Vec<(String, String, IndexDefinition)>,
	/// Indexes that were removed: (app_label, model_name, index_name)
	pub removed_indexes: Vec<(String, String, String)>,
	/// Constraints that were added: (app_label, model_name, ConstraintDefinition)
	pub added_constraints: Vec<(String, String, ConstraintDefinition)>,
	/// Constraints that were removed: (app_label, model_name, constraint_name)
	pub removed_constraints: Vec<(String, String, String)>,
	/// Model dependencies for ordering operations
	/// Maps (app_label, model_name) -> `Vec<(dependent_app, dependent_model)>`
	/// A model depends on another if it has ForeignKey or ManyToMany fields pointing to it
	pub model_dependencies: std::collections::BTreeMap<(String, String), Vec<(String, String)>>,
	/// ManyToMany intermediate tables that were created
	/// Contains (app_label, source_model, through_table, ManyToManyMetadata)
	pub created_many_to_many: Vec<(String, String, String, ManyToManyMetadata)>,
}

impl DetectedChanges {
	/// Order models for migration operations based on dependencies
	///
	/// Uses topological sort (Kahn's algorithm) to determine the correct order
	/// for creating or moving models. This ensures that referenced models are
	/// processed before models that reference them.
	///
	/// # Algorithm: Kahn's Algorithm (Topological Sort)
	/// - Time Complexity: O(V + E) where V is models, E is dependencies
	/// - Detects circular dependencies and handles them gracefully
	/// - Returns models in dependency order (bottom-up)
	///
	/// # Returns
	/// A vector of (app_label, model_name) tuples in dependency order.
	/// Models with no dependencies come first, models depending on others come last.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{DetectedChanges};
	/// use std::collections::BTreeMap;
	///
	/// let mut changes = DetectedChanges::default();
	/// changes.created_models.push(("accounts".to_string(), "User".to_string()));
	/// changes.created_models.push(("blog".to_string(), "Post".to_string()));
	///
	/// // Post depends on User
	/// let mut deps = BTreeMap::new();
	/// deps.insert(
	///     ("blog".to_string(), "Post".to_string()),
	///     vec![("accounts".to_string(), "User".to_string())],
	/// );
	/// changes.model_dependencies = deps;
	///
	/// let ordered = changes.order_models_by_dependency();
	/// // User comes before Post
	/// assert_eq!(ordered[0], ("accounts".to_string(), "User".to_string()));
	/// assert_eq!(ordered[1], ("blog".to_string(), "Post".to_string()));
	/// ```
	pub fn order_models_by_dependency(&self) -> Vec<(String, String)> {
		use std::collections::{HashMap, HashSet, VecDeque};

		// Build in-degree map (count of incoming edges)
		let mut in_degree: HashMap<(String, String), usize> = HashMap::new();
		let mut all_models: HashSet<(String, String)> = HashSet::new();

		// Collect all models (both created and dependencies)
		for model in &self.created_models {
			all_models.insert(model.clone());
			in_degree.entry(model.clone()).or_insert(0);
		}

		for model in &self.moved_models {
			let model_key = (model.1.clone(), model.2.clone()); // (to_app, model_name)
			all_models.insert(model_key.clone());
			in_degree.entry(model_key).or_insert(0);
		}

		// Build in-degree counts from dependencies
		for (dependent, dependencies) in &self.model_dependencies {
			for dependency in dependencies {
				all_models.insert(dependency.clone());
				in_degree.entry(dependency.clone()).or_insert(0);
				*in_degree.entry(dependent.clone()).or_insert(0) += 1;
			}
		}

		// Kahn's algorithm: Start with models that have no dependencies
		let mut queue: VecDeque<(String, String)> = VecDeque::new();
		for model in &all_models {
			if in_degree.get(model).copied().unwrap_or(0) == 0 {
				queue.push_back(model.clone());
			}
		}

		let mut ordered = Vec::new();

		while let Some(model) = queue.pop_front() {
			ordered.push(model.clone());

			// Reduce in-degree for models that depend on this model
			// model_dependencies maps dependent -> dependencies
			// So we need to find all models that have `model` in their dependencies
			for (dependent, dependencies) in &self.model_dependencies {
				if dependencies.contains(&model)
					&& let Some(degree) = in_degree.get_mut(dependent)
				{
					*degree -= 1;
					if *degree == 0 {
						queue.push_back(dependent.clone());
					}
				}
			}
		}

		// If not all models are ordered, there's a circular dependency
		if ordered.len() < all_models.len() {
			// Fall back to original order with a warning
			let unordered_models: Vec<_> = all_models
				.iter()
				.filter(|model| !ordered.contains(model))
				.map(|(app, name)| format!("{}.{}", app, name))
				.collect();

			eprintln!(
				"⚠️  Warning: Circular dependency detected in models: [{}]",
				unordered_models.join(", ")
			);
			eprintln!(
				"    Falling back to original order. Migration operations may need manual reordering."
			);

			all_models.into_iter().collect()
		} else {
			ordered
		}
	}

	/// Check for circular dependencies in model relationships
	///
	/// Detects cycles in the dependency graph using depth-first search.
	///
	/// # Returns
	/// - `Ok(())` if no circular dependencies exist
	/// - `Err(Vec<(String, String)>)` with the cycle path if found
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{DetectedChanges};
	/// use std::collections::BTreeMap;
	///
	/// let mut changes = DetectedChanges::default();
	///
	/// // Create circular dependency: A -> B -> C -> A
	/// let mut deps = BTreeMap::new();
	/// deps.insert(
	///     ("app".to_string(), "A".to_string()),
	///     vec![("app".to_string(), "B".to_string())],
	/// );
	/// deps.insert(
	///     ("app".to_string(), "B".to_string()),
	///     vec![("app".to_string(), "C".to_string())],
	/// );
	/// deps.insert(
	///     ("app".to_string(), "C".to_string()),
	///     vec![("app".to_string(), "A".to_string())],
	/// );
	/// changes.model_dependencies = deps;
	///
	/// assert!(changes.check_circular_dependencies().is_err());
	/// ```
	pub fn check_circular_dependencies(&self) -> Result<(), Vec<(String, String)>> {
		use std::collections::HashSet;

		let mut visited: HashSet<(String, String)> = HashSet::new();
		let mut rec_stack: HashSet<(String, String)> = HashSet::new();
		let mut path: Vec<(String, String)> = Vec::new();

		fn dfs(
			model: &(String, String),
			deps: &BTreeMap<(String, String), Vec<(String, String)>>,
			visited: &mut HashSet<(String, String)>,
			rec_stack: &mut HashSet<(String, String)>,
			path: &mut Vec<(String, String)>,
		) -> Option<Vec<(String, String)>> {
			visited.insert(model.clone());
			rec_stack.insert(model.clone());
			path.push(model.clone());

			if let Some(dependencies) = deps.get(model) {
				for dep in dependencies {
					if !visited.contains(dep) {
						if let Some(cycle) = dfs(dep, deps, visited, rec_stack, path) {
							return Some(cycle);
						}
					} else if rec_stack.contains(dep) {
						// Found cycle
						let cycle_start = path.iter().position(|m| m == dep).unwrap();
						return Some(path[cycle_start..].to_vec());
					}
				}
			}

			path.pop();
			rec_stack.remove(model);
			None
		}

		for model in self.model_dependencies.keys() {
			if !visited.contains(model)
				&& let Some(cycle) = dfs(
					model,
					&self.model_dependencies,
					&mut visited,
					&mut rec_stack,
					&mut path,
				) {
				return Err(cycle);
			}
		}

		Ok(())
	}

	/// Remove operations from DetectedChanges based on OperationRef list
	///
	/// This method is called when a user rejects an inferred intent during
	/// interactive migration detection. It removes the specific operations
	/// that the rejected intent was tracking, preventing them from being
	/// included in the generated migration.
	///
	/// # Arguments
	///
	/// * `refs` - Slice of OperationRef indicating which operations to remove
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{DetectedChanges, OperationRef};
	///
	/// let mut changes = DetectedChanges::default();
	/// changes.renamed_models.push((
	///     "blog".to_string(),
	///     "Post".to_string(),
	///     "BlogPost".to_string(),
	/// ));
	/// changes.added_fields.push((
	///     "blog".to_string(),
	///     "BlogPost".to_string(),
	///     "slug".to_string(),
	/// ));
	///
	/// // Remove the renamed model operation
	/// changes.remove_operations(&[OperationRef::RenamedModel {
	///     app_label: "blog".to_string(),
	///     old_name: "Post".to_string(),
	///     new_name: "BlogPost".to_string(),
	/// }]);
	///
	/// assert!(changes.renamed_models.is_empty());
	/// // added_fields is not affected
	/// assert_eq!(changes.added_fields.len(), 1);
	/// ```
	pub fn remove_operations(&mut self, refs: &[OperationRef]) {
		for op_ref in refs {
			match op_ref {
				OperationRef::RenamedModel {
					app_label,
					old_name,
					new_name,
				} => {
					self.renamed_models.retain(|(app, old, new)| {
						!(app == app_label && old == old_name && new == new_name)
					});
				}
				OperationRef::MovedModel {
					from_app,
					to_app,
					model_name,
				} => {
					// MovedModelInfo is (from_app, to_app, model_name, rename_table, old_table, new_table)
					self.moved_models.retain(|info| {
						!(&info.0 == from_app && &info.1 == to_app && &info.2 == model_name)
					});
				}
				OperationRef::AddedField {
					app_label,
					model_name,
					field_name,
				} => {
					self.added_fields.retain(|(app, model, field)| {
						!(app == app_label && model == model_name && field == field_name)
					});
				}
				OperationRef::RenamedField {
					app_label,
					model_name,
					old_name,
					new_name,
				} => {
					self.renamed_fields.retain(|(app, model, old, new)| {
						!(app == app_label
							&& model == model_name
							&& old == old_name && new == new_name)
					});
				}
				OperationRef::RemovedField {
					app_label,
					model_name,
					field_name,
				} => {
					self.removed_fields.retain(|(app, model, field)| {
						!(app == app_label && model == model_name && field == field_name)
					});
				}
				OperationRef::AlteredField {
					app_label,
					model_name,
					field_name,
				} => {
					self.altered_fields.retain(|(app, model, field)| {
						!(app == app_label && model == model_name && field == field_name)
					});
				}
				OperationRef::CreatedModel {
					app_label,
					model_name,
				} => {
					self.created_models
						.retain(|(app, model)| !(app == app_label && model == model_name));
				}
				OperationRef::DeletedModel {
					app_label,
					model_name,
				} => {
					self.deleted_models
						.retain(|(app, model)| !(app == app_label && model == model_name));
				}
			}
		}
	}
}

// ============================================================================
// Advanced Change Inference System
// ============================================================================

/// Change history entry for temporal pattern analysis
///
/// Tracks individual changes with timestamps to identify patterns over time.
/// This enables the autodetector to learn from past migrations and make
/// better predictions about future changes.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_db::migrations::autodetector::ChangeHistoryEntry;
/// use std::time::SystemTime;
///
/// let entry = ChangeHistoryEntry {
///     timestamp: SystemTime::now(),
///     change_type: "RenameModel".to_string(),
///     app_label: "blog".to_string(),
///     model_name: "Post".to_string(),
///     field_name: None,
///     old_value: Some("BlogPost".to_string()),
///     new_value: Some("Post".to_string()),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct ChangeHistoryEntry {
	/// When this change occurred
	pub timestamp: std::time::SystemTime,
	/// Type of change (e.g., "RenameModel", "AddField", "MoveModel")
	pub change_type: String,
	/// App label of the affected model
	pub app_label: String,
	/// Model name
	pub model_name: String,
	/// Field name (if field-level change)
	pub field_name: Option<String>,
	/// Old value (for renames/alterations)
	pub old_value: Option<String>,
	/// New value (for renames/alterations)
	pub new_value: Option<String>,
}

/// Pattern frequency for learning from historical changes
///
/// Tracks how often certain patterns appear to predict future changes.
/// For example, if "User -> Account" rename happened 5 times in history,
/// similar patterns will get higher confidence scores.
#[derive(Debug, Clone)]
pub struct PatternFrequency {
	/// The pattern being tracked (e.g., "RenameModel:User->Account")
	pub pattern: String,
	/// Number of times this pattern occurred
	pub frequency: usize,
	/// Last time this pattern was seen
	pub last_seen: std::time::SystemTime,
	/// Contexts where this pattern appeared
	pub contexts: Vec<String>,
}

/// Change tracker for temporal pattern analysis
///
/// Maintains a history of schema changes and analyzes patterns over time
/// to improve autodetection accuracy. This implements Django's concept of
/// "migration squashing" intelligence - learning which changes commonly
/// occur together.
///
/// # Algorithm: Temporal Pattern Mining
/// - Time Complexity: O(n) for insertion, O(n log n) for pattern analysis
/// - Space Complexity: O(h) where h is history size
/// - Uses sliding window for recent changes (last 100 by default)
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_db::migrations::ChangeTracker;
///
/// let mut tracker = ChangeTracker::new();
///
/// // Track a model rename
/// tracker.record_model_rename("blog", "BlogPost", "Post");
///
/// // Track a field addition
/// tracker.record_field_addition("blog", "Post", "slug");
///
/// // Get pattern frequency
/// let patterns = tracker.get_frequent_patterns(2); // Min frequency: 2
/// ```
#[derive(Debug, Clone)]
pub struct ChangeTracker {
	/// Complete history of changes
	history: Vec<ChangeHistoryEntry>,
	/// Pattern frequency map
	patterns: HashMap<String, PatternFrequency>,
	/// Maximum history size (for memory efficiency)
	max_history_size: usize,
}

impl ChangeTracker {
	/// Create a new change tracker with default settings
	///
	/// Default max history size: 1000 entries
	pub fn new() -> Self {
		Self {
			history: Vec::new(),
			patterns: HashMap::new(),
			max_history_size: 1000,
		}
	}

	/// Create a change tracker with custom history size
	pub fn with_capacity(max_size: usize) -> Self {
		Self {
			history: Vec::with_capacity(max_size),
			patterns: HashMap::new(),
			max_history_size: max_size,
		}
	}

	/// Record a model rename in the history
	///
	/// # Arguments
	/// * `app_label` - App containing the model
	/// * `old_name` - Original model name
	/// * `new_name` - New model name
	pub fn record_model_rename(&mut self, app_label: &str, old_name: &str, new_name: &str) {
		let entry = ChangeHistoryEntry {
			timestamp: std::time::SystemTime::now(),
			change_type: "RenameModel".to_string(),
			app_label: app_label.to_string(),
			model_name: new_name.to_string(),
			field_name: None,
			old_value: Some(old_name.to_string()),
			new_value: Some(new_name.to_string()),
		};

		self.add_entry(entry);
		self.update_pattern(
			&format!("RenameModel:{}->{}", old_name, new_name),
			app_label,
		);
	}

	/// Record a model move between apps
	pub fn record_model_move(&mut self, from_app: &str, to_app: &str, model_name: &str) {
		let entry = ChangeHistoryEntry {
			timestamp: std::time::SystemTime::now(),
			change_type: "MoveModel".to_string(),
			app_label: to_app.to_string(),
			model_name: model_name.to_string(),
			field_name: None,
			old_value: Some(from_app.to_string()),
			new_value: Some(to_app.to_string()),
		};

		self.add_entry(entry);
		self.update_pattern(
			&format!("MoveModel:{}->{}:{}", from_app, to_app, model_name),
			to_app,
		);
	}

	/// Record a field addition
	pub fn record_field_addition(&mut self, app_label: &str, model_name: &str, field_name: &str) {
		let entry = ChangeHistoryEntry {
			timestamp: std::time::SystemTime::now(),
			change_type: "AddField".to_string(),
			app_label: app_label.to_string(),
			model_name: model_name.to_string(),
			field_name: Some(field_name.to_string()),
			old_value: None,
			new_value: Some(field_name.to_string()),
		};

		self.add_entry(entry);
		self.update_pattern(
			&format!("AddField:{}:{}", model_name, field_name),
			app_label,
		);
	}

	/// Record a field rename
	pub fn record_field_rename(
		&mut self,
		app_label: &str,
		model_name: &str,
		old_name: &str,
		new_name: &str,
	) {
		let entry = ChangeHistoryEntry {
			timestamp: std::time::SystemTime::now(),
			change_type: "RenameField".to_string(),
			app_label: app_label.to_string(),
			model_name: model_name.to_string(),
			field_name: Some(new_name.to_string()),
			old_value: Some(old_name.to_string()),
			new_value: Some(new_name.to_string()),
		};

		self.add_entry(entry);
		self.update_pattern(
			&format!("RenameField:{}:{}->{}", model_name, old_name, new_name),
			app_label,
		);
	}

	/// Add an entry to history with size management
	fn add_entry(&mut self, entry: ChangeHistoryEntry) {
		self.history.push(entry);

		// Maintain max history size
		if self.history.len() > self.max_history_size {
			self.history.remove(0);
		}
	}

	/// Update pattern frequency
	fn update_pattern(&mut self, pattern: &str, context: &str) {
		self.patterns
			.entry(pattern.to_string())
			.and_modify(|pf| {
				pf.frequency += 1;
				pf.last_seen = std::time::SystemTime::now();
				if !pf.contexts.contains(&context.to_string()) {
					pf.contexts.push(context.to_string());
				}
			})
			.or_insert(PatternFrequency {
				pattern: pattern.to_string(),
				frequency: 1,
				last_seen: std::time::SystemTime::now(),
				contexts: vec![context.to_string()],
			});
	}

	/// Get patterns that occur at least `min_frequency` times
	///
	/// Returns patterns sorted by frequency (descending)
	pub fn get_frequent_patterns(&self, min_frequency: usize) -> Vec<PatternFrequency> {
		let mut patterns: Vec<_> = self
			.patterns
			.values()
			.filter(|p| p.frequency >= min_frequency)
			.cloned()
			.collect();

		patterns.sort_by(|a, b| b.frequency.cmp(&a.frequency));
		patterns
	}

	/// Get recent changes within the specified duration
	///
	/// # Arguments
	/// * `duration` - Time window (e.g., Duration::from_secs(3600) for last hour)
	pub fn get_recent_changes(&self, duration: std::time::Duration) -> Vec<&ChangeHistoryEntry> {
		let now = std::time::SystemTime::now();
		self.history
			.iter()
			.filter(|entry| {
				now.duration_since(entry.timestamp)
					.map(|d| d < duration)
					.unwrap_or(false)
			})
			.collect()
	}

	/// Analyze co-occurring patterns
	///
	/// Returns pairs of patterns that frequently appear together
	/// within a time window (default: 1 hour)
	pub fn analyze_cooccurrence(
		&self,
		window: std::time::Duration,
	) -> HashMap<(String, String), usize> {
		let mut cooccurrences = HashMap::new();

		for i in 0..self.history.len() {
			for j in (i + 1)..self.history.len() {
				if let Ok(diff) = self.history[j]
					.timestamp
					.duration_since(self.history[i].timestamp)
					&& diff <= window
				{
					let pattern1 = format!(
						"{}:{}",
						self.history[i].change_type, self.history[i].model_name
					);
					let pattern2 = format!(
						"{}:{}",
						self.history[j].change_type, self.history[j].model_name
					);
					let key = if pattern1 < pattern2 {
						(pattern1, pattern2)
					} else {
						(pattern2, pattern1)
					};
					*cooccurrences.entry(key).or_insert(0) += 1;
				}
			}
		}

		cooccurrences
	}

	/// Clear all history (useful for testing)
	pub fn clear(&mut self) {
		self.history.clear();
		self.patterns.clear();
	}

	/// Get total number of changes tracked
	pub fn len(&self) -> usize {
		self.history.len()
	}

	/// Check if history is empty
	pub fn is_empty(&self) -> bool {
		self.history.is_empty()
	}
}

impl Default for ChangeTracker {
	fn default() -> Self {
		Self::new()
	}
}

/// Pattern match result
///
/// Represents a single match found by the PatternMatcher.
#[derive(Debug, Clone)]
pub struct PatternMatch {
	/// The pattern that matched
	pub pattern: String,
	/// Starting position in the text
	pub start: usize,
	/// Ending position in the text
	pub end: usize,
	/// The matched text
	pub matched_text: String,
}

/// Pattern matcher using Aho-Corasick algorithm
///
/// Efficiently searches for multiple patterns simultaneously in model/field names.
/// This is useful for detecting common naming patterns like:
/// - "User" -> "Account" conversions
/// - "created_at" -> "timestamp" renames
/// - Common prefix/suffix patterns
///
/// # Algorithm: Aho-Corasick
/// - Time Complexity: O(n + m + z) where n=text length, m=total pattern length, z=matches
/// - Space Complexity: O(m) for the automaton
/// - Advantage: Simultaneous multi-pattern matching in linear time
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_db::migrations::PatternMatcher;
///
/// let mut matcher = PatternMatcher::new();
/// matcher.add_pattern("User");
/// matcher.add_pattern("Post");
/// matcher.build();
///
/// let matches = matcher.find_all("User has many Posts");
/// assert_eq!(matches.len(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct PatternMatcher {
	/// Patterns to search for
	patterns: Vec<String>,
	/// Aho-Corasick automaton (built lazily)
	automaton: Option<aho_corasick::AhoCorasick>,
}

impl PatternMatcher {
	/// Create a new empty pattern matcher
	pub fn new() -> Self {
		Self {
			patterns: Vec::new(),
			automaton: None,
		}
	}

	/// Add a pattern to search for
	///
	/// Patterns are case-sensitive by default.
	/// Call `build()` after adding all patterns.
	pub fn add_pattern(&mut self, pattern: &str) {
		self.patterns.push(pattern.to_string());
		// Invalidate automaton - needs rebuild
		self.automaton = None;
	}

	/// Add multiple patterns at once
	pub fn add_patterns<I, S>(&mut self, patterns: I)
	where
		I: IntoIterator<Item = S>,
		S: AsRef<str>,
	{
		for pattern in patterns {
			self.patterns.push(pattern.as_ref().to_string());
		}
		self.automaton = None;
	}

	/// Build the Aho-Corasick automaton
	///
	/// Must be called after adding patterns and before searching.
	/// Returns Err if patterns is empty or build fails.
	pub fn build(&mut self) -> Result<(), String> {
		if self.patterns.is_empty() {
			return Err("No patterns to build automaton".to_string());
		}

		self.automaton = Some(
			aho_corasick::AhoCorasick::new(&self.patterns)
				.map_err(|e| format!("Failed to build Aho-Corasick automaton: {}", e))?,
		);

		Ok(())
	}

	/// Find all pattern matches in the given text
	///
	/// Returns empty vector if no matches found or automaton not built.
	pub fn find_all(&self, text: &str) -> Vec<PatternMatch> {
		let Some(ref automaton) = self.automaton else {
			return Vec::new();
		};

		automaton
			.find_iter(text)
			.map(|mat| PatternMatch {
				pattern: self.patterns[mat.pattern().as_usize()].clone(),
				start: mat.start(),
				end: mat.end(),
				matched_text: text[mat.start()..mat.end()].to_string(),
			})
			.collect()
	}

	/// Check if any pattern matches the text
	pub fn contains_any(&self, text: &str) -> bool {
		self.automaton
			.as_ref()
			.map(|ac| ac.is_match(text))
			.unwrap_or(false)
	}

	/// Find the first match in the text
	pub fn find_first(&self, text: &str) -> Option<PatternMatch> {
		let automaton = self.automaton.as_ref()?;
		let mat = automaton.find(text)?;

		Some(PatternMatch {
			pattern: self.patterns[mat.pattern().as_usize()].clone(),
			start: mat.start(),
			end: mat.end(),
			matched_text: text[mat.start()..mat.end()].to_string(),
		})
	}

	/// Replace all pattern matches with replacements
	///
	/// # Arguments
	/// * `text` - The text to search in
	/// * `replacements` - Map from pattern to replacement string
	///
	/// # Returns
	/// Modified text with all patterns replaced
	pub fn replace_all(&self, text: &str, replacements: &HashMap<String, String>) -> String {
		let Some(ref automaton) = self.automaton else {
			return text.to_string();
		};

		let mut result = String::new();
		let mut last_end = 0;

		for mat in automaton.find_iter(text) {
			// Add text before match
			result.push_str(&text[last_end..mat.start()]);

			// Add replacement or original if no replacement found
			let pattern = &self.patterns[mat.pattern().as_usize()];
			if let Some(replacement) = replacements.get(pattern) {
				result.push_str(replacement);
			} else {
				result.push_str(&text[mat.start()..mat.end()]);
			}

			last_end = mat.end();
		}

		// Add remaining text
		result.push_str(&text[last_end..]);
		result
	}

	/// Get all patterns currently registered
	pub fn patterns(&self) -> &[String] {
		&self.patterns
	}

	/// Clear all patterns
	pub fn clear(&mut self) {
		self.patterns.clear();
		self.automaton = None;
	}

	/// Check if automaton is built and ready
	pub fn is_built(&self) -> bool {
		self.automaton.is_some()
	}
}

impl Default for PatternMatcher {
	fn default() -> Self {
		Self::new()
	}
}

// ============================================================================
// Inference Types
// ============================================================================

/// Condition for an inference rule
#[derive(Debug, Clone, PartialEq)]
pub enum RuleCondition {
	/// Model rename pattern
	ModelRename {
		from_pattern: String,
		to_pattern: String,
	},
	/// Model move pattern
	ModelMove { app_pattern: String },
	/// Field addition pattern
	FieldAddition { field_name_pattern: String },
	/// Field rename pattern
	FieldRename {
		from_pattern: String,
		to_pattern: String,
	},
	/// Multiple model renames
	MultipleModelRenames { min_count: usize },
	/// Multiple field additions
	MultipleFieldAdditions {
		model_pattern: String,
		min_count: usize,
	},
}

/// Reference to a specific operation in DetectedChanges
///
/// Used to track which operations an inferred intent relates to,
/// enabling removal of operations when the user rejects an intent.
#[derive(Debug, Clone, PartialEq)]
pub enum OperationRef {
	/// Reference to a renamed model: (app_label, old_name, new_name)
	RenamedModel {
		app_label: String,
		old_name: String,
		new_name: String,
	},
	/// Reference to a moved model: (from_app, to_app, model_name)
	MovedModel {
		from_app: String,
		to_app: String,
		model_name: String,
	},
	/// Reference to an added field: (app_label, model_name, field_name)
	AddedField {
		app_label: String,
		model_name: String,
		field_name: String,
	},
	/// Reference to a renamed field: (app_label, model_name, old_name, new_name)
	RenamedField {
		app_label: String,
		model_name: String,
		old_name: String,
		new_name: String,
	},
	/// Reference to a removed field: (app_label, model_name, field_name)
	RemovedField {
		app_label: String,
		model_name: String,
		field_name: String,
	},
	/// Reference to an altered field: (app_label, model_name, field_name)
	AlteredField {
		app_label: String,
		model_name: String,
		field_name: String,
	},
	/// Reference to a created model: (app_label, model_name)
	CreatedModel {
		app_label: String,
		model_name: String,
	},
	/// Reference to a deleted model: (app_label, model_name)
	DeletedModel {
		app_label: String,
		model_name: String,
	},
}

/// Inferred intent from detected changes
#[derive(Debug, Clone, PartialEq)]
pub struct InferredIntent {
	/// Type of intent (e.g., "Refactoring", "Add timestamp tracking")
	pub intent_type: String,
	/// Confidence score (0.0 - 1.0)
	pub confidence: f64,
	/// Human-readable description
	pub description: String,
	/// Evidence supporting this intent
	pub evidence: Vec<String>,
	/// References to operations in DetectedChanges that this intent relates to
	///
	/// When the user rejects this intent, these operations will be removed
	/// from DetectedChanges to prevent migration generation.
	pub related_operations: Vec<OperationRef>,
}

/// Rule for inferring intent from change patterns
#[derive(Debug, Clone)]
pub struct InferenceRule {
	/// Rule name
	pub name: String,
	/// Required conditions (all must match)
	pub conditions: Vec<RuleCondition>,
	/// Optional conditions (boost confidence if matched)
	pub optional_conditions: Vec<RuleCondition>,
	/// Intent type to infer
	pub intent_type: String,
	/// Base confidence (0.0 - 1.0)
	pub base_confidence: f64,
	/// Confidence boost per matched optional condition
	pub confidence_boost_per_optional: f64,
}

/// Inference engine for detecting composite change intents
///
/// Analyzes multiple detected changes to infer high-level intentions.
/// For example:
/// - AddIndex + AlterField(to larger type) → Performance optimization
/// - RenameModel + AddForeignKey → Relationship refactoring
/// - AddField + RemoveField → Data migration
///
/// # Algorithm: Rule-Based Inference
/// - Matches detected changes against predefined rules
/// - Calculates confidence scores based on pattern matching
/// - Returns ranked list of possible intents
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_db::migrations::InferenceEngine;
///
/// let mut engine = InferenceEngine::new();
/// engine.add_default_rules();
///
/// // Analyze changes with proper arguments
/// let model_renames = vec![];
/// let model_moves = vec![];
/// let field_additions = vec![
///     ("users".to_string(), "User".to_string(), "email".to_string())
/// ];
/// let field_renames = vec![];
///
/// let intents = engine.infer_intents(
///     &model_renames,
///     &model_moves,
///     &field_additions,
///     &field_renames
/// );
/// ```
#[derive(Debug, Clone)]
pub struct InferenceEngine {
	/// Inference rules
	rules: Vec<InferenceRule>,
	/// Change history for contextual analysis
	///
	/// The change tracker maintains a history of schema changes and can be used
	/// to improve inference accuracy by analyzing temporal patterns. To use:
	///
	/// 1. Record changes via `record_model_rename()`, `record_field_addition()`, etc.
	/// 2. Query patterns via `get_frequent_patterns()` or `analyze_cooccurrence()`
	/// 3. Use pattern analysis to boost confidence scores in inference rules
	///
	/// Example:
	/// ```rust,ignore
	/// use reinhardt_db::migrations::autodetector::InferenceEngine;
	/// let mut engine = InferenceEngine::new();
	/// // Record rename and field addition history
	/// engine.record_model_rename("blog", "BlogPost", "Post");
	/// engine.record_field_addition("blog", "Post", "slug");
	/// // Analyze co-occurrence within a 60-second window
	/// let _cooccurrences = engine.analyze_cooccurrence(std::time::Duration::from_secs(60));
	/// ```
	change_tracker: ChangeTracker,
}

impl Default for InferenceEngine {
	fn default() -> Self {
		Self::new()
	}
}

impl InferenceEngine {
	/// Create a new inference engine
	pub fn new() -> Self {
		Self {
			rules: Vec::new(),
			change_tracker: ChangeTracker::new(),
		}
	}

	/// Add a rule to the engine
	pub fn add_rule(&mut self, rule: InferenceRule) {
		self.rules.push(rule);
	}

	/// Add default inference rules
	pub fn add_default_rules(&mut self) {
		// Rule 1: Model refactoring (rename)
		self.add_rule(InferenceRule {
			name: "model_refactoring".to_string(),
			conditions: vec![RuleCondition::ModelRename {
				from_pattern: ".*".to_string(),
				to_pattern: ".*".to_string(),
			}],
			optional_conditions: vec![RuleCondition::MultipleModelRenames { min_count: 2 }],
			intent_type: "Refactoring: Model rename".to_string(),
			base_confidence: 0.7,
			confidence_boost_per_optional: 0.1,
		});

		// Rule 2: Timestamp tracking
		self.add_rule(InferenceRule {
			name: "add_timestamp_tracking".to_string(),
			conditions: vec![RuleCondition::FieldAddition {
				field_name_pattern: "created_at".to_string(),
			}],
			optional_conditions: vec![RuleCondition::FieldAddition {
				field_name_pattern: "updated_at".to_string(),
			}],
			intent_type: "Add timestamp tracking".to_string(),
			base_confidence: 0.8,
			confidence_boost_per_optional: 0.15,
		});

		// Rule 3: Cross-app model move
		self.add_rule(InferenceRule {
			name: "cross_app_move".to_string(),
			conditions: vec![RuleCondition::ModelMove {
				app_pattern: ".*".to_string(),
			}],
			optional_conditions: vec![],
			intent_type: "Cross-app model organization".to_string(),
			base_confidence: 0.75,
			confidence_boost_per_optional: 0.0,
		});

		// Rule 4: Field refactoring (rename)
		self.add_rule(InferenceRule {
			name: "field_refactoring".to_string(),
			conditions: vec![RuleCondition::FieldRename {
				from_pattern: ".*".to_string(),
				to_pattern: ".*".to_string(),
			}],
			optional_conditions: vec![RuleCondition::MultipleFieldAdditions {
				model_pattern: ".*".to_string(),
				min_count: 2,
			}],
			intent_type: "Refactoring: Field rename".to_string(),
			base_confidence: 0.65,
			confidence_boost_per_optional: 0.1,
		});

		// Rule 5: Model normalization
		self.add_rule(InferenceRule {
			name: "model_normalization".to_string(),
			conditions: vec![RuleCondition::MultipleFieldAdditions {
				model_pattern: ".*".to_string(),
				min_count: 3,
			}],
			optional_conditions: vec![],
			intent_type: "Schema normalization".to_string(),
			base_confidence: 0.6,
			confidence_boost_per_optional: 0.0,
		});
	}

	/// Match string against a pattern (supports regex)
	///
	/// Patterns can be:
	/// - Literal strings (exact match)
	/// - ".*" wildcard (matches anything)
	/// - Regular expressions (e.g., "User.*" matches "User", "UserProfile", etc.)
	fn matches_pattern(value: &str, pattern: &str) -> bool {
		// Wildcard pattern matches everything
		if pattern == ".*" {
			return true;
		}

		// Try exact match first
		if value == pattern {
			return true;
		}

		// Try regex match
		if let Ok(re) = Regex::new(pattern) {
			re.is_match(value)
		} else {
			// If regex is invalid, fall back to exact match
			false
		}
	}

	/// Get all rules
	pub fn rules(&self) -> &[InferenceRule] {
		&self.rules
	}

	/// Infer intents from detected changes
	pub fn infer_intents(
		&self,
		model_renames: &[(String, String, String, String)], // (from_app, from_model, to_app, to_model)
		model_moves: &[(String, String, String, String)],   // (from_app, from_model, to_app, to_model)
		field_additions: &[(String, String, String)],       // (app, model, field)
		field_renames: &[(String, String, String, String)], // (app, model, from_field, to_field)
	) -> Vec<InferredIntent> {
		let mut intents = Vec::new();

		for rule in &self.rules {
			let mut matches_required = true;
			let mut optional_matches = 0;
			let mut evidence = Vec::new();

			// Check required conditions
			for condition in &rule.conditions {
				match condition {
					RuleCondition::ModelRename {
						from_pattern,
						to_pattern,
					} => {
						if model_renames.is_empty() {
							matches_required = false;
							break;
						}

						// Check if any model rename matches the patterns
						let mut matched = false;
						for (from_app, from_model, to_app, to_model) in model_renames {
							let from_name = format!("{}.{}", from_app, from_model);
							let to_name = format!("{}.{}", to_app, to_model);

							if Self::matches_pattern(&from_name, from_pattern)
								&& Self::matches_pattern(&to_name, to_pattern)
							{
								evidence.push(format!(
									"Model renamed: {} → {} (pattern: {} → {})",
									from_name, to_name, from_pattern, to_pattern
								));
								matched = true;
								break;
							}
						}

						if !matched {
							matches_required = false;
							break;
						}
					}
					RuleCondition::ModelMove { app_pattern } => {
						if model_moves.is_empty() {
							matches_required = false;
							break;
						}

						// Check if any model move matches the app pattern
						let mut matched = false;
						for (from_app, from_model, to_app, to_model) in model_moves {
							if Self::matches_pattern(to_app, app_pattern) {
								evidence.push(format!(
									"Model moved: {}.{} → {}.{} (app pattern: {})",
									from_app, from_model, to_app, to_model, app_pattern
								));
								matched = true;
								break;
							}
						}

						if !matched {
							matches_required = false;
							break;
						}
					}
					RuleCondition::FieldAddition { field_name_pattern } => {
						let matching_fields: Vec<_> = field_additions
							.iter()
							.filter(|(_, _, field)| {
								Self::matches_pattern(field, field_name_pattern)
							})
							.collect();

						if matching_fields.is_empty() {
							matches_required = false;
							break;
						}
						evidence.push(format!(
							"Field added: {}.{}.{} (pattern: {})",
							matching_fields[0].0,
							matching_fields[0].1,
							matching_fields[0].2,
							field_name_pattern
						));
					}
					RuleCondition::FieldRename {
						from_pattern,
						to_pattern,
					} => {
						if field_renames.is_empty() {
							matches_required = false;
							break;
						}

						// Check if any field rename matches the patterns
						let mut matched = false;
						for (app, model, from_field, to_field) in field_renames {
							if Self::matches_pattern(from_field, from_pattern)
								&& Self::matches_pattern(to_field, to_pattern)
							{
								evidence.push(format!(
									"Field renamed: {}.{}.{} → {} (pattern: {} → {})",
									app, model, from_field, to_field, from_pattern, to_pattern
								));
								matched = true;
								break;
							}
						}

						if !matched {
							matches_required = false;
							break;
						}
					}
					RuleCondition::MultipleModelRenames { min_count } => {
						if model_renames.len() < *min_count {
							matches_required = false;
							break;
						}
						evidence.push(format!("Multiple model renames: {}", model_renames.len()));
					}
					RuleCondition::MultipleFieldAdditions {
						model_pattern,
						min_count,
					} => {
						let count = field_additions
							.iter()
							.filter(|(_, model, _)| Self::matches_pattern(model, model_pattern))
							.count();

						if count < *min_count {
							matches_required = false;
							break;
						}
						evidence.push(format!(
							"Multiple field additions: {} (pattern: {}, min: {})",
							count, model_pattern, min_count
						));
					}
				}
			}

			if !matches_required {
				continue;
			}

			// Check optional conditions
			for condition in &rule.optional_conditions {
				match condition {
					RuleCondition::FieldAddition { field_name_pattern } => {
						if field_additions
							.iter()
							.any(|(_, _, field)| field.contains(field_name_pattern.as_str()))
						{
							optional_matches += 1;
							evidence.push(format!("Optional field added: {}", field_name_pattern));
						}
					}
					RuleCondition::MultipleModelRenames { min_count } => {
						if model_renames.len() >= *min_count {
							optional_matches += 1;
							evidence.push(format!("Multiple renames: {}", model_renames.len()));
						}
					}
					_ => {}
				}
			}

			// Calculate confidence
			let confidence = rule.base_confidence
				+ (optional_matches as f64 * rule.confidence_boost_per_optional);
			let confidence = confidence.min(1.0);

			intents.push(InferredIntent {
				intent_type: rule.intent_type.clone(),
				confidence,
				description: format!("Detected: {}", rule.name),
				evidence,
				related_operations: Vec::new(),
			});
		}

		// Sort by confidence (highest first)
		intents.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

		intents
	}

	/// Infer intents from DetectedChanges
	///
	/// Extracts change operations from DetectedChanges and runs inference rules on them.
	///
	/// # Arguments
	/// * `changes` - Detected changes between two project states
	///
	/// # Returns
	/// Inferred intents sorted by confidence (highest first)
	pub fn infer_from_detected_changes(&self, changes: &DetectedChanges) -> Vec<InferredIntent> {
		// Extract model renames: (from_app, from_model, to_app, to_model)
		let model_renames: Vec<(String, String, String, String)> = changes
			.renamed_models
			.iter()
			.map(|(app, old_name, new_name)| {
				(app.clone(), old_name.clone(), app.clone(), new_name.clone())
			})
			.collect();

		// Extract model moves: (from_app, from_model, to_app, to_model)
		let model_moves: Vec<(String, String, String, String)> = changes
			.moved_models
			.iter()
			.map(|(from_app, to_app, model, _, _, _)| {
				(
					from_app.clone(),
					model.clone(),
					to_app.clone(),
					model.clone(),
				)
			})
			.collect();

		// Extract field additions: (app, model, field)
		let field_additions: Vec<(String, String, String)> = changes
			.added_fields
			.iter()
			.map(|(app, model, field)| (app.clone(), model.clone(), field.clone()))
			.collect();

		// Extract field renames: (app, model, from_field, to_field)
		let field_renames: Vec<(String, String, String, String)> = changes
			.renamed_fields
			.iter()
			.map(|(app, model, old_name, new_name)| {
				(
					app.clone(),
					model.clone(),
					old_name.clone(),
					new_name.clone(),
				)
			})
			.collect();

		// Run inference on extracted changes
		let mut intents = self.infer_intents(
			&model_renames,
			&model_moves,
			&field_additions,
			&field_renames,
		);

		// Post-process: populate related_operations for each intent based on evidence
		for intent in &mut intents {
			// Parse evidence to determine which operations are related
			// Evidence strings contain information about which changes triggered the intent
			for evidence_str in &intent.evidence {
				// Model rename evidence: "Model renamed: app.old → app.new ..."
				if evidence_str.starts_with("Model renamed:") {
					for (app, old_name, new_name) in &changes.renamed_models {
						intent.related_operations.push(OperationRef::RenamedModel {
							app_label: app.clone(),
							old_name: old_name.clone(),
							new_name: new_name.clone(),
						});
					}
				}
				// Model move evidence: "Model moved: from_app.model → to_app.model ..."
				else if evidence_str.starts_with("Model moved:") {
					for (from_app, to_app, model, _, _, _) in &changes.moved_models {
						intent.related_operations.push(OperationRef::MovedModel {
							from_app: from_app.clone(),
							to_app: to_app.clone(),
							model_name: model.clone(),
						});
					}
				}
				// Field added evidence: "Field added: app.model.field ..."
				else if evidence_str.starts_with("Field added:") {
					for (app, model, field) in &changes.added_fields {
						intent.related_operations.push(OperationRef::AddedField {
							app_label: app.clone(),
							model_name: model.clone(),
							field_name: field.clone(),
						});
					}
				}
				// Field renamed evidence: "Field renamed: app.model.old → new ..."
				else if evidence_str.starts_with("Field renamed:") {
					for (app, model, old_name, new_name) in &changes.renamed_fields {
						intent.related_operations.push(OperationRef::RenamedField {
							app_label: app.clone(),
							model_name: model.clone(),
							old_name: old_name.clone(),
							new_name: new_name.clone(),
						});
					}
				}
				// Multiple model renames evidence
				else if evidence_str.starts_with("Multiple model renames:") {
					for (app, old_name, new_name) in &changes.renamed_models {
						intent.related_operations.push(OperationRef::RenamedModel {
							app_label: app.clone(),
							old_name: old_name.clone(),
							new_name: new_name.clone(),
						});
					}
				}
				// Multiple field additions or optional field added evidence
				else if evidence_str.starts_with("Multiple field additions:")
					|| evidence_str.starts_with("Optional field added:")
				{
					for (app, model, field) in &changes.added_fields {
						intent.related_operations.push(OperationRef::AddedField {
							app_label: app.clone(),
							model_name: model.clone(),
							field_name: field.clone(),
						});
					}
				}
			}

			// Deduplicate related_operations
			intent
				.related_operations
				.sort_by(|a, b| format!("{:?}", a).cmp(&format!("{:?}", b)));
			intent.related_operations.dedup();
		}

		intents
	}

	/// Record a model rename in the change tracker
	///
	/// This enables contextual analysis for future migrations by tracking patterns.
	///
	/// # Arguments
	/// * `app_label` - App containing the model
	/// * `old_name` - Original model name
	/// * `new_name` - New model name
	pub fn record_model_rename(&mut self, app_label: &str, old_name: &str, new_name: &str) {
		self.change_tracker
			.record_model_rename(app_label, old_name, new_name);
	}

	/// Record a model move between apps
	///
	/// # Arguments
	/// * `from_app` - Source app label
	/// * `to_app` - Target app label
	/// * `model_name` - Name of the model being moved
	pub fn record_model_move(&mut self, from_app: &str, to_app: &str, model_name: &str) {
		self.change_tracker
			.record_model_move(from_app, to_app, model_name);
	}

	/// Record a field addition
	///
	/// # Arguments
	/// * `app_label` - App containing the model
	/// * `model_name` - Name of the model
	/// * `field_name` - Name of the field being added
	pub fn record_field_addition(&mut self, app_label: &str, model_name: &str, field_name: &str) {
		self.change_tracker
			.record_field_addition(app_label, model_name, field_name);
	}

	/// Record a field rename
	///
	/// # Arguments
	/// * `app_label` - App containing the model
	/// * `model_name` - Name of the model
	/// * `old_name` - Original field name
	/// * `new_name` - New field name
	pub fn record_field_rename(
		&mut self,
		app_label: &str,
		model_name: &str,
		old_name: &str,
		new_name: &str,
	) {
		self.change_tracker
			.record_field_rename(app_label, model_name, old_name, new_name);
	}

	/// Get frequent patterns from change history
	///
	/// Returns patterns that occur at least `min_frequency` times.
	/// This can be used to improve confidence scores for similar patterns.
	///
	/// # Arguments
	/// * `min_frequency` - Minimum number of occurrences to be considered frequent
	pub fn get_frequent_patterns(&self, min_frequency: usize) -> Vec<PatternFrequency> {
		self.change_tracker.get_frequent_patterns(min_frequency)
	}

	/// Get recent changes within the specified duration
	///
	/// # Arguments
	/// * `duration` - Time window for recent changes (e.g., last hour)
	pub fn get_recent_changes(&self, duration: std::time::Duration) -> Vec<&ChangeHistoryEntry> {
		self.change_tracker.get_recent_changes(duration)
	}

	/// Analyze co-occurring patterns in change history
	///
	/// Returns pairs of patterns that frequently appear together
	/// within a time window.
	///
	/// # Arguments
	/// * `window` - Time window for co-occurrence analysis (default: 1 hour)
	pub fn analyze_cooccurrence(
		&self,
		window: std::time::Duration,
	) -> HashMap<(String, String), usize> {
		self.change_tracker.analyze_cooccurrence(window)
	}
}

// ============================================================================
// Interactive UI for User Confirmation
// ============================================================================

/// Interactive prompt system for user confirmation of ambiguous changes
///
/// This module provides CLI-based prompts for:
/// - Ambiguous model/field renames
/// - Cross-app model moves
/// - Multiple possible intents with different confidence scores
///
/// Uses the `dialoguer` crate for rich terminal interactions.
pub struct MigrationPrompt {
	/// Minimum confidence threshold for auto-acceptance (0.0 - 1.0)
	/// Changes above this threshold are accepted without prompting
	auto_accept_threshold: f64,

	/// Theme for terminal styling
	theme: dialoguer::theme::ColorfulTheme,
}

impl std::fmt::Debug for MigrationPrompt {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("MigrationPrompt")
			.field("auto_accept_threshold", &self.auto_accept_threshold)
			.field("theme", &"ColorfulTheme")
			.finish()
	}
}

impl MigrationPrompt {
	/// Create a new prompt system with default settings
	pub fn new() -> Self {
		Self {
			auto_accept_threshold: 0.85,
			theme: dialoguer::theme::ColorfulTheme::default(),
		}
	}

	/// Create with custom auto-accept threshold
	pub fn with_threshold(threshold: f64) -> Self {
		Self {
			auto_accept_threshold: threshold,
			theme: dialoguer::theme::ColorfulTheme::default(),
		}
	}

	/// Get the auto-accept threshold
	pub fn auto_accept_threshold(&self) -> f64 {
		self.auto_accept_threshold
	}

	/// Confirm a single intent with the user
	///
	/// Returns true if the user confirms, false if they reject
	pub fn confirm_intent(
		&self,
		intent: &InferredIntent,
	) -> Result<bool, Box<dyn std::error::Error>> {
		// Auto-accept high-confidence changes
		if intent.confidence >= self.auto_accept_threshold {
			println!(
				"✓ Auto-accepting (confidence: {:.1}%): {}",
				intent.confidence * 100.0,
				intent.intent_type
			);
			return Ok(true);
		}

		// Build prompt message
		let message = format!(
			"Detected: {} (confidence: {:.1}%)\nDetails: {}\n\nAccept this change?",
			intent.intent_type,
			intent.confidence * 100.0,
			intent.description
		);

		// Show evidence
		if !intent.evidence.is_empty() {
			println!("\nEvidence:");
			for evidence in &intent.evidence {
				println!("  • {}", evidence);
			}
		}

		// Prompt user
		dialoguer::Confirm::with_theme(&self.theme)
			.with_prompt(message)
			.default(true)
			.interact()
			.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
	}

	/// Select one intent from multiple alternatives
	///
	/// Returns the index of the selected intent, or None if user cancels
	pub fn select_intent(
		&self,
		alternatives: &[InferredIntent],
		prompt: &str,
	) -> Result<Option<usize>, Box<dyn std::error::Error>> {
		if alternatives.is_empty() {
			return Ok(None);
		}

		// Single alternative - just confirm
		if alternatives.len() == 1 {
			let confirmed = self.confirm_intent(&alternatives[0])?;
			return Ok(if confirmed { Some(0) } else { None });
		}

		// Build selection items
		let items: Vec<String> = alternatives
			.iter()
			.map(|intent| {
				format!(
					"{} (confidence: {:.1}%) - {}",
					intent.intent_type,
					intent.confidence * 100.0,
					intent.description
				)
			})
			.collect();

		// Show prompt
		println!("\n{}", prompt);
		println!("Multiple possibilities detected:\n");

		// Add "None of the above" option
		let mut items_with_none = items.clone();
		items_with_none.push("None of the above / Skip".to_string());

		// Prompt user
		let selection = dialoguer::Select::with_theme(&self.theme)
			.items(&items_with_none)
			.default(0)
			.interact()
			.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

		// Return None if user selected "None of the above"
		if selection >= items.len() {
			Ok(None)
		} else {
			Ok(Some(selection))
		}
	}

	/// Multi-select intents from a list
	///
	/// Returns indices of selected intents
	pub fn multi_select_intents(
		&self,
		alternatives: &[InferredIntent],
		prompt: &str,
	) -> Result<Vec<usize>, Box<dyn std::error::Error>> {
		if alternatives.is_empty() {
			return Ok(Vec::new());
		}

		// Build selection items
		let items: Vec<String> = alternatives
			.iter()
			.map(|intent| {
				format!(
					"{} (confidence: {:.1}%) - {}",
					intent.intent_type,
					intent.confidence * 100.0,
					intent.description
				)
			})
			.collect();

		// Show prompt
		println!("\n{}", prompt);
		println!("Select all that apply:\n");

		// Prompt user with multi-select
		let selections = dialoguer::MultiSelect::with_theme(&self.theme)
			.items(&items)
			.interact()
			.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

		Ok(selections)
	}

	/// Confirm a model rename with details
	pub fn confirm_model_rename(
		&self,
		from_app: &str,
		from_model: &str,
		to_app: &str,
		to_model: &str,
		confidence: f64,
	) -> Result<bool, Box<dyn std::error::Error>> {
		// Auto-accept high-confidence changes
		if confidence >= self.auto_accept_threshold {
			println!(
				"✓ Auto-accepting model rename (confidence: {:.1}%): {}.{} → {}.{}",
				confidence * 100.0,
				from_app,
				from_model,
				to_app,
				to_model
			);
			return Ok(true);
		}

		let message = format!(
			"Rename model from {}.{} to {}.{}?\n(confidence: {:.1}%)",
			from_app,
			from_model,
			to_app,
			to_model,
			confidence * 100.0
		);

		dialoguer::Confirm::with_theme(&self.theme)
			.with_prompt(message)
			.default(true)
			.interact()
			.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
	}

	/// Confirm a field rename with details
	pub fn confirm_field_rename(
		&self,
		model: &str,
		from_field: &str,
		to_field: &str,
		confidence: f64,
	) -> Result<bool, Box<dyn std::error::Error>> {
		// Auto-accept high-confidence changes
		if confidence >= self.auto_accept_threshold {
			println!(
				"✓ Auto-accepting field rename (confidence: {:.1}%): {}.{} → {}.{}",
				confidence * 100.0,
				model,
				from_field,
				model,
				to_field
			);
			return Ok(true);
		}

		let message = format!(
			"Rename field in model {}:\n  {} → {}?\n(confidence: {:.1}%)",
			model,
			from_field,
			to_field,
			confidence * 100.0
		);

		dialoguer::Confirm::with_theme(&self.theme)
			.with_prompt(message)
			.default(true)
			.interact()
			.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
	}

	/// Show progress indicator for long operations
	pub fn with_progress<F, T>(
		&self,
		message: &str,
		total: u64,
		operation: F,
	) -> Result<T, Box<dyn std::error::Error>>
	where
		F: FnOnce(&indicatif::ProgressBar) -> Result<T, Box<dyn std::error::Error>>,
	{
		let pb = indicatif::ProgressBar::new(total);
		pb.set_style(
			indicatif::ProgressStyle::default_bar()
				.template("{msg} [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
				.expect("Failed to create progress bar template")
				.progress_chars("#>-"),
		);
		pb.set_message(message.to_string());

		let result = operation(&pb)?;

		pb.finish_with_message("Done");
		Ok(result)
	}
}

impl Default for MigrationPrompt {
	fn default() -> Self {
		Self::new()
	}
}

/// Extension trait for MigrationAutodetector with interactive prompts
pub trait InteractiveAutodetector {
	/// Detect changes with user prompts for ambiguous cases
	fn detect_changes_interactive(&self) -> Result<DetectedChanges, Box<dyn std::error::Error>>;

	/// Apply inferred intents with user confirmation
	fn apply_intents_interactive(
		&self,
		intents: Vec<InferredIntent>,
		changes: &mut DetectedChanges,
	) -> Result<(), Box<dyn std::error::Error>>;
}

impl InteractiveAutodetector for MigrationAutodetector {
	fn detect_changes_interactive(&self) -> Result<DetectedChanges, Box<dyn std::error::Error>> {
		let prompt = MigrationPrompt::new();
		let mut changes = self.detect_changes();

		// Build inference engine
		let mut engine = InferenceEngine::new();
		engine.add_default_rules();

		// Infer intents from detected changes
		let intents = engine.infer_from_detected_changes(&changes);

		// Filter high-confidence intents
		let ambiguous_intents: Vec<_> = intents
			.into_iter()
			.filter(|intent| intent.confidence < prompt.auto_accept_threshold)
			.collect();

		// Prompt for ambiguous changes
		if !ambiguous_intents.is_empty() {
			println!(
				"\n⚠️  Found {} ambiguous change(s) requiring confirmation:",
				ambiguous_intents.len()
			);

			for intent in &ambiguous_intents {
				let confirmed = prompt.confirm_intent(intent)?;

				if !confirmed {
					println!("✗ Skipped: {}", intent.description);
					// Remove the related operations from DetectedChanges
					// This prevents rejected intents from generating migration operations
					if !intent.related_operations.is_empty() {
						changes.remove_operations(&intent.related_operations);
						println!(
							"  → Removed {} related operation(s) from migration",
							intent.related_operations.len()
						);
					}
				}
			}
		}

		// Detect and order dependencies
		self.detect_model_dependencies(&mut changes);

		// Check for circular dependencies
		if let Err(cycle) = changes.check_circular_dependencies() {
			println!("\n⚠️  Warning: Circular dependency detected: {:?}", cycle);

			let should_continue = dialoguer::Confirm::new()
				.with_prompt("Continue anyway? (may require manual intervention)")
				.default(false)
				.interact()?;

			if !should_continue {
				return Err("Aborted due to circular dependency".into());
			}
		}

		Ok(changes)
	}

	fn apply_intents_interactive(
		&self,
		intents: Vec<InferredIntent>,
		_changes: &mut DetectedChanges,
	) -> Result<(), Box<dyn std::error::Error>> {
		let prompt = MigrationPrompt::new();

		// Group intents by confidence
		let mut high_confidence = Vec::new();
		let mut medium_confidence = Vec::new();
		let mut low_confidence = Vec::new();

		for intent in intents {
			if intent.confidence >= 0.85 {
				high_confidence.push(intent);
			} else if intent.confidence >= 0.65 {
				medium_confidence.push(intent);
			} else {
				low_confidence.push(intent);
			}
		}

		// Auto-apply high-confidence intents
		println!(
			"\n✓ Auto-applying {} high-confidence change(s):",
			high_confidence.len()
		);
		for intent in &high_confidence {
			println!(
				"  • {} (confidence: {:.1}%)",
				intent.description,
				intent.confidence * 100.0
			);
		}

		// Prompt for medium-confidence intents
		if !medium_confidence.is_empty() {
			println!(
				"\n⚠️  Review {} medium-confidence change(s):",
				medium_confidence.len()
			);

			for intent in &medium_confidence {
				let confirmed = prompt.confirm_intent(intent)?;
				if confirmed {
					println!("  ✓ Accepted: {}", intent.description);
				} else {
					println!("  ✗ Rejected: {}", intent.description);
				}
			}
		}

		// Prompt for low-confidence intents with multi-select
		if !low_confidence.is_empty() {
			let selections = prompt.multi_select_intents(
				&low_confidence,
				"⚠️  Select low-confidence changes to apply:",
			)?;

			for idx in selections {
				println!("  ✓ Accepted: {}", low_confidence[idx].description);
			}
		}

		Ok(())
	}
}

impl MigrationAutodetector {
	/// Create a new migration autodetector with default similarity config
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{MigrationAutodetector, ProjectState};
	///
	/// let from_state = ProjectState::new();
	/// let to_state = ProjectState::new();
	///
	/// let detector = MigrationAutodetector::new(from_state, to_state);
	/// ```
	pub fn new(from_state: ProjectState, to_state: ProjectState) -> Self {
		Self {
			from_state,
			to_state,
			similarity_config: SimilarityConfig::default(),
		}
	}

	/// Create a new migration autodetector with custom similarity config
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{MigrationAutodetector, ProjectState, SimilarityConfig};
	///
	/// let from_state = ProjectState::new();
	/// let to_state = ProjectState::new();
	/// let config = SimilarityConfig::new(0.75, 0.85).unwrap();
	///
	/// let detector = MigrationAutodetector::with_config(from_state, to_state, config);
	/// ```
	pub fn with_config(
		from_state: ProjectState,
		to_state: ProjectState,
		similarity_config: SimilarityConfig,
	) -> Self {
		Self {
			from_state,
			to_state,
			similarity_config,
		}
	}

	/// Detect all changes between from_state and to_state
	///
	/// Django equivalent: `_detect_changes()` in django/db/migrations/autodetector.py
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{MigrationAutodetector, ProjectState, ModelState};
	///
	/// let from_state = ProjectState::new();
	/// let mut to_state = ProjectState::new();
	///
	// Add a new model
	/// let model = ModelState::new("myapp", "User");
	/// to_state.add_model(model);
	///
	/// let detector = MigrationAutodetector::new(from_state, to_state);
	/// let changes = detector.detect_changes();
	///
	/// assert_eq!(changes.created_models.len(), 1);
	/// ```
	pub fn detect_changes(&self) -> DetectedChanges {
		let mut changes = DetectedChanges::default();

		// Detect model-level changes
		self.detect_created_models(&mut changes);
		self.detect_deleted_models(&mut changes);
		self.detect_renamed_models(&mut changes);

		// Detect field-level changes (only for models that exist in both states)
		self.detect_added_fields(&mut changes);
		self.detect_removed_fields(&mut changes);
		self.detect_altered_fields(&mut changes);
		self.detect_renamed_fields(&mut changes);

		// Detect index and constraint changes
		self.detect_added_indexes(&mut changes);
		self.detect_removed_indexes(&mut changes);
		self.detect_added_constraints(&mut changes);
		self.detect_removed_constraints(&mut changes);

		// Detect ManyToMany intermediate tables
		self.detect_created_many_to_many(&mut changes);

		// Detect model dependencies for operation ordering
		self.detect_model_dependencies(&mut changes);

		// Sort all changes to ensure deterministic ordering
		// This guarantees that the same model set always produces the same migration order
		changes.created_models.sort();
		changes.deleted_models.sort();
		changes.added_fields.sort();
		changes.removed_fields.sort();
		changes.altered_fields.sort();
		changes.renamed_models.sort();
		changes.renamed_fields.sort();

		// Sort by (app_label, model_name) for index and constraint changes
		changes
			.added_indexes
			.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
		changes.removed_indexes.sort();
		changes
			.added_constraints
			.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
		changes.removed_constraints.sort();
		changes
			.created_many_to_many
			.sort_by(|a, b| (&a.0, &a.1, &a.2).cmp(&(&b.0, &b.1, &b.2)));

		changes
	}

	/// Detect newly created models
	///
	/// Django reference: `generate_created_models()` in django/db/migrations/autodetector.py:800
	fn detect_created_models(&self, changes: &mut DetectedChanges) {
		for ((app_label, model_name), to_model) in &self.to_state.models {
			// Check if the model exists in from_state by table name
			if self
				.from_state
				.get_model_by_table_name(app_label, &to_model.table_name)
				.is_none()
			{
				changes
					.created_models
					.push((app_label.clone(), model_name.clone()));
			}
		}
	}

	/// Detect deleted models
	///
	/// Django reference: `generate_deleted_models()` in django/db/migrations/autodetector.py:900
	fn detect_deleted_models(&self, changes: &mut DetectedChanges) {
		for ((app_label, model_name), from_model) in &self.from_state.models {
			// Check if the model exists in to_state by table name
			if self
				.to_state
				.get_model_by_table_name(app_label, &from_model.table_name)
				.is_none()
			{
				changes
					.deleted_models
					.push((app_label.clone(), model_name.clone()));
			}
		}
	}

	/// Detect added fields
	///
	/// Django reference: `generate_added_fields()` in django/db/migrations/autodetector.py:1000
	fn detect_added_fields(&self, changes: &mut DetectedChanges) {
		for ((app_label, model_name), to_model) in &self.to_state.models {
			// Only check models that exist in both states (by table name)
			if let Some(from_model) = self
				.from_state
				.get_model_by_table_name(app_label, &to_model.table_name)
			{
				for field_name in to_model.fields.keys() {
					if !from_model.fields.contains_key(field_name) {
						changes.added_fields.push((
							app_label.clone(),
							model_name.clone(),
							field_name.clone(),
						));
					}
				}
			}
		}
	}

	/// Detect removed fields
	///
	/// Django reference: `generate_removed_fields()` in django/db/migrations/autodetector.py:1100
	fn detect_removed_fields(&self, changes: &mut DetectedChanges) {
		for ((app_label, model_name), from_model) in &self.from_state.models {
			// Only check models that exist in both states (by table name)
			if let Some(to_model) = self
				.to_state
				.get_model_by_table_name(app_label, &from_model.table_name)
			{
				for field_name in from_model.fields.keys() {
					if !to_model.fields.contains_key(field_name) {
						changes.removed_fields.push((
							app_label.clone(),
							model_name.clone(),
							field_name.clone(),
						));
					}
				}
			}
		}
	}

	/// Detect altered fields
	///
	/// Django reference: `generate_altered_fields()` in django/db/migrations/autodetector.py:1200
	fn detect_altered_fields(&self, changes: &mut DetectedChanges) {
		for ((app_label, model_name), to_model) in &self.to_state.models {
			// Only check models that exist in both states (by table name)
			if let Some(from_model) = self
				.from_state
				.get_model_by_table_name(app_label, &to_model.table_name)
			{
				for (field_name, to_field) in &to_model.fields {
					if let Some(from_field) = from_model.fields.get(field_name) {
						// Check if field definition has changed
						if self.has_field_changed(from_field, to_field) {
							changes.altered_fields.push((
								app_label.clone(),
								model_name.clone(),
								field_name.clone(),
							));
						}
					}
				}
			}
		}
	}

	/// Check if a field has changed
	fn has_field_changed(&self, from_field: &FieldState, to_field: &FieldState) -> bool {
		// Check if field type changed
		if from_field.field_type != to_field.field_type {
			return true;
		}

		// Check if nullable changed
		if from_field.nullable != to_field.nullable {
			return true;
		}

		// Check if params changed
		if from_field.params != to_field.params {
			return true;
		}

		false
	}

	/// Detect renamed models
	///
	/// This method attempts to detect model renames by comparing deleted and created models.
	/// It uses field similarity to determine if a model was renamed rather than deleted/created.
	///
	/// # Django Reference
	/// From: django/db/migrations/autodetector.py:620-750
	/// ```python
	/// def generate_renamed_models(self):
	///     # Find models that were deleted and created with similar fields
	///     for (app_label, old_model_name) in self.old_model_keys - self.new_model_keys:
	///         for (app_label, new_model_name) in self.new_model_keys - self.old_model_keys:
	///             if self._is_renamed_model(old_model_name, new_model_name):
	///                 self.add_operation(
	///                     app_label,
	///                     operations.RenameModel(
	///                         old_name=old_model_name,
	///                         new_name=new_model_name,
	///                     ),
	///                 )
	/// ```rust,ignore
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{MigrationAutodetector, ProjectState, ModelState, FieldState, FieldType};
	///
	/// let mut from_state = ProjectState::new();
	/// let mut old_model = ModelState::new("myapp", "OldUser");
	/// old_model.add_field(FieldState::new("id", FieldType::Integer, false));
	/// old_model.add_field(FieldState::new("name", FieldType::VarChar(255), false));
	/// from_state.add_model(old_model);
	///
	/// let mut to_state = ProjectState::new();
	/// let mut new_model = ModelState::new("myapp", "NewUser");
	/// new_model.add_field(FieldState::new("id", FieldType::Integer, false));
	/// new_model.add_field(FieldState::new("name", FieldType::VarChar(255), false));
	/// to_state.add_model(new_model);
	///
	/// let detector = MigrationAutodetector::new(from_state, to_state);
	/// let changes = detector.detect_changes();
	///
	// With high field similarity, should detect as rename
	/// assert!(changes.renamed_models.len() <= 1);
	/// ```
	fn detect_renamed_models(&self, changes: &mut DetectedChanges) {
		// Get deleted and created models
		let deleted: Vec<_> = self
			.from_state
			.models
			.keys()
			.filter(|k| !self.to_state.models.contains_key(k))
			.collect();

		let created: Vec<_> = self
			.to_state
			.models
			.keys()
			.filter(|k| !self.from_state.models.contains_key(k))
			.collect();

		// Use bipartite matching to find optimal model pairs
		// This supports both same-app renames and cross-app moves
		let matches = self.find_optimal_model_matches(&deleted, &created);

		for (deleted_key, created_key, _similarity) in matches {
			// Check if this is a cross-app move or same-app rename
			if deleted_key.0 == created_key.0 {
				// Same app: this is a rename operation
				changes
					.renamed_models
					.push((deleted_key.0, deleted_key.1, created_key.1));
			} else {
				// Different apps: this is a move operation
				// Determine if table needs to be renamed
				let old_table = format!("{}_{}", deleted_key.0, deleted_key.1.to_lowercase());
				let new_table = format!("{}_{}", created_key.0, created_key.1.to_lowercase());
				let rename_table = old_table != new_table || deleted_key.1 != created_key.1;

				changes.moved_models.push((
					deleted_key.0, // from_app
					created_key.0, // to_app
					created_key.1, // model_name (use new name)
					rename_table,
					if rename_table { Some(old_table) } else { None },
					if rename_table { Some(new_table) } else { None },
				));
			}
		}
	}

	/// Detect renamed fields
	///
	/// This method attempts to detect field renames by comparing removed and added fields.
	///
	/// # Django Reference
	/// From: django/db/migrations/autodetector.py:1300-1400
	/// ```python
	/// def generate_renamed_fields(self):
	///     for app_label, model_name in sorted(self.kept_model_keys):
	///         old_model_state = self.from_state.models[app_label, model_name]
	///         new_model_state = self.to_state.models[app_label, model_name]
	///
	///         # Find fields that were removed and added with same type
	///         for old_field_name, old_field in old_model_state.fields:
	///             for new_field_name, new_field in new_model_state.fields:
	///                 if self._is_renamed_field(old_field, new_field):
	///                     self.add_operation(...)
	/// ```rust,ignore
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{MigrationAutodetector, ProjectState, ModelState, FieldState, FieldType};
	///
	/// let mut from_state = ProjectState::new();
	/// let mut old_model = ModelState::new("myapp", "User");
	/// old_model.add_field(FieldState::new("old_email", FieldType::VarChar(255), false));
	/// from_state.add_model(old_model);
	///
	/// let mut to_state = ProjectState::new();
	/// let mut new_model = ModelState::new("myapp", "User");
	/// new_model.add_field(FieldState::new("new_email", FieldType::VarChar(255), false));
	/// to_state.add_model(new_model);
	///
	/// let detector = MigrationAutodetector::new(from_state, to_state);
	/// let changes = detector.detect_changes();
	///
	// With matching type, might detect as rename
	/// assert!(changes.renamed_fields.len() <= 1);
	/// ```
	fn detect_renamed_fields(&self, changes: &mut DetectedChanges) {
		// Only check models that exist in both states
		for ((app_label, model_name), from_model) in &self.from_state.models {
			if let Some(to_model) = self.to_state.get_model(app_label, model_name) {
				// Get removed and added fields for this model
				let removed_fields: Vec<_> = from_model
					.fields
					.iter()
					.filter(|(name, _)| !to_model.fields.contains_key(*name))
					.collect();

				let added_fields: Vec<_> = to_model
					.fields
					.iter()
					.filter(|(name, _)| !from_model.fields.contains_key(*name))
					.collect();

				// Try to match removed fields with added fields
				for (removed_name, removed_field) in &removed_fields {
					for (added_name, added_field) in &added_fields {
						// If field types match, consider it a rename
						if removed_field.field_type == added_field.field_type
							&& removed_field.nullable == added_field.nullable
						{
							changes.renamed_fields.push((
								app_label.clone(),
								model_name.clone(),
								removed_name.to_string(),
								added_name.to_string(),
							));
							break;
						}
					}
				}
			}
		}
	}

	/// Calculate similarity between two models using advanced field matching
	///
	/// # Algorithm: Weighted Bipartite Matching for Fields
	/// - Uses Jaro-Winkler for field name similarity
	/// - Time Complexity: O(n*m) where n,m are number of fields
	/// - Considers both exact matches and fuzzy matches
	///
	/// # Scoring:
	/// - Exact field name + type match: 1.0
	/// - Fuzzy field name + type match: Jaro-Winkler score (0.0-1.0)
	/// - No type match: 0.0
	///
	/// Returns a value between 0.0 and 1.0, where 1.0 means identical field sets.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{MigrationAutodetector, ProjectState, ModelState, FieldState, FieldType};
	///
	/// let mut from_state = ProjectState::new();
	/// let mut from_model = ModelState::new("myapp", "User");
	/// from_model.add_field(FieldState::new("user_id", FieldType::Integer, false));
	/// from_model.add_field(FieldState::new("user_email", FieldType::VarChar(255), false));
	/// from_state.add_model(from_model);
	///
	/// let mut to_state = ProjectState::new();
	/// let mut to_model = ModelState::new("auth", "User");
	/// to_model.add_field(FieldState::new("id", FieldType::Integer, false));
	/// to_model.add_field(FieldState::new("email", FieldType::VarChar(255), false));
	/// to_state.add_model(to_model);
	///
	/// let detector = MigrationAutodetector::new(from_state, to_state);
	/// // Similarity would be high due to fuzzy field name matching
	/// ```
	fn calculate_model_similarity(&self, from_model: &ModelState, to_model: &ModelState) -> f64 {
		if from_model.fields.is_empty() && to_model.fields.is_empty() {
			return 1.0;
		}

		if from_model.fields.is_empty() || to_model.fields.is_empty() {
			return 0.0;
		}

		let mut total_similarity = 0.0;
		let total_fields = from_model.fields.len().max(to_model.fields.len());

		// Use Hungarian algorithm concept: find best matching between fields
		let mut matched_to_fields = std::collections::HashSet::new();

		for (from_field_name, from_field) in &from_model.fields {
			let mut best_match_score = 0.0;
			let mut best_match_name = None;

			// Find best matching field in to_model
			for (to_field_name, to_field) in &to_model.fields {
				if matched_to_fields.contains(to_field_name) {
					continue;
				}

				let similarity = self.calculate_field_similarity(
					from_field_name,
					to_field_name,
					from_field,
					to_field,
				);

				if similarity > best_match_score {
					best_match_score = similarity;
					best_match_name = Some(to_field_name.clone());
				}
			}

			if let Some(matched_name) = best_match_name {
				matched_to_fields.insert(matched_name);
				total_similarity += best_match_score;
			}
		}

		total_similarity / total_fields as f64
	}

	/// Calculate field-level similarity using hybrid algorithm
	///
	/// This method combines Jaro-Winkler and Levenshtein distance to measure
	/// similarity between field names, providing better detection than either alone.
	///
	/// # Hybrid Algorithm
	/// - **Jaro-Winkler**: Best for prefix similarities (e.g., "UserEmail" vs "UserAddress")
	///   - Time Complexity: O(n)
	///   - Range: 0.0 to 1.0
	///   - Default weight: 70%
	/// - **Levenshtein**: Best for edit distance (e.g., "User" vs "Users")
	///   - Time Complexity: O(n*m)
	///   - Normalized to 0.0-1.0 range
	///   - Default weight: 30%
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{MigrationAutodetector, ProjectState, ModelState, FieldState, FieldType};
	///
	/// let from_state = ProjectState::new();
	/// let to_state = ProjectState::new();
	/// let detector = MigrationAutodetector::new(from_state, to_state);
	///
	/// let from_field = FieldState::new("user_email", FieldType::VarChar(255), false);
	/// let to_field = FieldState::new("email", FieldType::VarChar(255), false);
	///
	/// // High similarity (field name is similar and type matches)
	/// // Jaro-Winkler ≈ 0.81, Levenshtein normalized ≈ 0.45
	/// // Hybrid (0.7 * 0.81 + 0.3 * 0.45) ≈ 0.70
	/// ```
	fn calculate_field_similarity(
		&self,
		from_field_name: &str,
		to_field_name: &str,
		from_field: &FieldState,
		to_field: &FieldState,
	) -> f64 {
		// If types don't match, similarity is 0
		if from_field.field_type != to_field.field_type {
			return 0.0;
		}

		// Calculate Jaro-Winkler similarity (0.0 - 1.0)
		let jaro_winkler_sim = jaro_winkler(from_field_name, to_field_name);

		// Calculate Levenshtein distance and normalize to 0.0-1.0
		let lev_distance = levenshtein(from_field_name, to_field_name);
		let max_len = from_field_name.len().max(to_field_name.len()) as f64;
		let levenshtein_sim = if max_len > 0.0 {
			1.0 - (lev_distance as f64 / max_len)
		} else {
			1.0 // Both strings are empty
		};

		// Combine using configured weights
		let name_similarity = self.similarity_config.jaro_winkler_weight * jaro_winkler_sim
			+ self.similarity_config.levenshtein_weight * levenshtein_sim;

		// Boost similarity if nullability also matches
		let nullable_boost = if from_field.nullable == to_field.nullable {
			0.1
		} else {
			0.0
		};

		(name_similarity + nullable_boost).min(1.0)
	}

	/// Perform bipartite matching between deleted and created models
	///
	/// # Algorithm: Maximum Weight Bipartite Matching
	/// - Based on Hopcroft-Karp algorithm concept: O(n*m*√(n+m))
	/// - Uses petgraph for graph construction
	/// - Finds optimal matching considering all possible pairs
	///
	/// # Implementation Note
	/// This implementation uses a greedy approach with weighted edges sorted by
	/// similarity score. While not a full Hopcroft-Karp implementation, it provides
	/// good results with O(E log E) complexity where E = number of edges.
	///
	/// # Returns
	/// Vector of matches: (deleted_key, created_key, similarity_score)
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{MigrationAutodetector, ProjectState, ModelState, FieldState, FieldType};
	///
	/// let mut from_state = ProjectState::new();
	/// let mut old_model = ModelState::new("myapp", "User");
	/// old_model.add_field(FieldState::new("id", FieldType::Integer, false));
	/// from_state.add_model(old_model);
	///
	/// let mut to_state = ProjectState::new();
	/// let mut new_model = ModelState::new("auth", "User");
	/// new_model.add_field(FieldState::new("id", FieldType::Integer, false));
	/// to_state.add_model(new_model);
	///
	/// let detector = MigrationAutodetector::new(from_state, to_state);
	/// // Would detect cross-app model move from myapp.User to auth.User
	/// ```
	fn find_optimal_model_matches(
		&self,
		deleted: &[&(String, String)],
		created: &[&(String, String)],
	) -> Vec<ModelMatchResult> {
		let mut graph = Graph::<(), f64, Undirected>::new_undirected();
		let mut deleted_nodes = Vec::new();
		let mut created_nodes = Vec::new();

		// Create nodes for deleted models (left side of bipartite graph)
		for _ in deleted {
			deleted_nodes.push(graph.add_node(()));
		}

		// Create nodes for created models (right side of bipartite graph)
		for _ in created {
			created_nodes.push(graph.add_node(()));
		}

		// Add edges with similarity weights
		for (i, deleted_key) in deleted.iter().enumerate() {
			if let Some(from_model) = self.from_state.models.get(*deleted_key) {
				for (j, created_key) in created.iter().enumerate() {
					if let Some(to_model) = self.to_state.models.get(*created_key) {
						let similarity = self.calculate_model_similarity(from_model, to_model);

						// Only add edge if similarity exceeds threshold
						if similarity >= self.similarity_config.model_threshold() {
							graph.add_edge(deleted_nodes[i], created_nodes[j], similarity);
						}
					}
				}
			}
		}

		// Find maximum weight matching using greedy algorithm
		// (Full Hopcroft-Karp would require additional implementation)
		let mut matches = Vec::new();
		let mut used_deleted = std::collections::HashSet::new();
		let mut used_created = std::collections::HashSet::new();

		// Sort edges by weight (similarity) in descending order
		let mut weighted_edges: Vec<_> = graph
			.edge_references()
			.map(|e| (e.source(), e.target(), *e.weight()))
			.collect();
		weighted_edges.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

		// Greedy matching: pick highest weight edges first
		for (source, target, weight) in weighted_edges {
			let source_idx = deleted_nodes.iter().position(|&n| n == source);
			let target_idx = created_nodes.iter().position(|&n| n == target);

			if let (Some(i), Some(j)) = (source_idx, target_idx)
				&& !used_deleted.contains(&i)
				&& !used_created.contains(&j)
			{
				matches.push((deleted[i].clone(), created[j].clone(), weight));
				used_deleted.insert(i);
				used_created.insert(j);
			}
		}

		matches
	}

	/// Detect added indexes
	///
	/// # Django Reference
	/// From: django/db/migrations/autodetector.py:1500-1600
	fn detect_added_indexes(&self, changes: &mut DetectedChanges) {
		for ((app_label, model_name), to_model) in &self.to_state.models {
			if let Some(from_model) = self.from_state.get_model(app_label, model_name) {
				for to_index in &to_model.indexes {
					// Check if this index exists in from_model
					if !from_model
						.indexes
						.iter()
						.any(|idx| idx.name == to_index.name)
					{
						changes.added_indexes.push((
							app_label.clone(),
							model_name.clone(),
							to_index.clone(),
						));
					}
				}
			}
		}
	}

	/// Detect removed indexes
	///
	/// # Django Reference
	/// From: django/db/migrations/autodetector.py:1600-1700
	fn detect_removed_indexes(&self, changes: &mut DetectedChanges) {
		for ((app_label, model_name), from_model) in &self.from_state.models {
			if let Some(to_model) = self.to_state.get_model(app_label, model_name) {
				for from_index in &from_model.indexes {
					// Check if this index still exists in to_model
					if !to_model
						.indexes
						.iter()
						.any(|idx| idx.name == from_index.name)
					{
						changes.removed_indexes.push((
							app_label.clone(),
							model_name.clone(),
							from_index.name.clone(),
						));
					}
				}
			}
		}
	}

	/// Detect added constraints
	///
	/// # Django Reference
	/// From: django/db/migrations/autodetector.py:1700-1800
	fn detect_added_constraints(&self, changes: &mut DetectedChanges) {
		for ((app_label, model_name), to_model) in &self.to_state.models {
			if let Some(from_model) = self.from_state.get_model(app_label, model_name) {
				for to_constraint in &to_model.constraints {
					// Check if this constraint exists in from_model
					if !from_model
						.constraints
						.iter()
						.any(|c| c.name == to_constraint.name)
					{
						changes.added_constraints.push((
							app_label.clone(),
							model_name.clone(),
							to_constraint.clone(),
						));
					}
				}
			}
		}
	}

	/// Detect removed constraints
	///
	/// # Django Reference
	/// From: django/db/migrations/autodetector.py:1800-1900
	fn detect_removed_constraints(&self, changes: &mut DetectedChanges) {
		for ((app_label, model_name), from_model) in &self.from_state.models {
			if let Some(to_model) = self.to_state.get_model(app_label, model_name) {
				for from_constraint in &from_model.constraints {
					// Check if this constraint still exists in to_model
					if !to_model
						.constraints
						.iter()
						.any(|c| c.name == from_constraint.name)
					{
						changes.removed_constraints.push((
							app_label.clone(),
							model_name.clone(),
							from_constraint.name.clone(),
						));
					}
				}
			}
		}
	}

	/// Generate intermediate table operation for ManyToMany field
	///
	/// Creates a through table for ManyToMany relationships with:
	/// - id: BigInteger primary key with auto_increment
	/// - {source}_id: BigInteger foreign key to source model
	/// - {target}_id: BigInteger foreign key to target model
	/// - Unique constraint on (source_id, target_id)
	///
	/// # Arguments
	/// * `app_label` - The app label of the source model
	/// * `model_name` - The source model name
	/// * `field_name` - The ManyToMany field name
	/// * `to_model` - The target model reference (e.g., "app.Model")
	/// * `through_table` - Optional custom through table name
	///
	/// # Returns
	/// Optional CreateTable operation for the intermediate table
	fn generate_intermediate_table(
		&self,
		app_label: &str,
		model_name: &str,
		field_name: &str,
		to_model: &str,
		through_table: &Option<String>,
	) -> Option<super::Operation> {
		// Generate table name
		let table_name = if let Some(custom_name) = through_table {
			custom_name.clone()
		} else {
			// Auto-generate: {app}_{model}_{field_name}
			format!(
				"{}_{}_{}",
				to_snake_case(app_label),
				to_snake_case(model_name),
				to_snake_case(field_name)
			)
		};

		// Parse target model to get table name
		let (_target_app, target_model) = self.parse_model_reference(to_model, app_label)?;
		let target_table = to_snake_case(&target_model);

		// Handle self-referential relationships
		let (source_column, target_column) = if model_name == target_model {
			// Self-referential: use from_{model}_id and to_{model}_id
			(
				format!("from_{}_id", to_snake_case(model_name)),
				format!("to_{}_id", to_snake_case(model_name)),
			)
		} else {
			// Regular: use {source}_id and {target}_id
			(
				format!("{}_id", to_snake_case(model_name)),
				format!("{}_id", to_snake_case(&target_model)),
			)
		};

		// Create columns
		let columns = vec![
			// id column
			super::ColumnDefinition {
				name: "id".to_string(),
				type_definition: super::FieldType::BigInteger,
				not_null: true,
				unique: false,
				primary_key: true,
				auto_increment: true,
				default: None,
			},
			// source_id column
			super::ColumnDefinition {
				name: source_column.clone(),
				type_definition: super::FieldType::BigInteger,
				not_null: true,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
			},
			// target_id column
			super::ColumnDefinition {
				name: target_column.clone(),
				type_definition: super::FieldType::BigInteger,
				not_null: true,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
			},
		];

		// Create constraints
		let source_table = to_snake_case(model_name);
		let constraints = vec![
			// Foreign key to source table
			super::Constraint::ForeignKey {
				name: format!("fk_{}_{}", table_name, source_column),
				columns: vec![source_column.clone()],
				referenced_table: source_table.clone(),
				referenced_columns: vec!["id".to_string()],
				on_delete: super::ForeignKeyAction::Cascade,
				on_update: super::ForeignKeyAction::Cascade,
				deferrable: None,
			},
			// Foreign key to target table
			super::Constraint::ForeignKey {
				name: format!("fk_{}_{}", table_name, target_column),
				columns: vec![target_column.clone()],
				referenced_table: target_table.clone(),
				referenced_columns: vec!["id".to_string()],
				on_delete: super::ForeignKeyAction::Cascade,
				on_update: super::ForeignKeyAction::Cascade,
				deferrable: None,
			},
			// Unique constraint on (source_id, target_id)
			super::Constraint::Unique {
				name: format!(
					"uq_{}_{}_{}",
					table_name,
					source_column.replace("_id", ""),
					target_column.replace("_id", "")
				),
				columns: vec![source_column, target_column],
			},
		];

		Some(super::Operation::CreateTable {
			name: table_name,
			columns,
			constraints,
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		})
	}

	/// Generate operations from detected changes
	///
	/// Converts DetectedChanges into a list of Operation objects that can be
	/// executed to migrate the database schema.
	///
	/// # Django Reference
	/// From: django/db/migrations/autodetector.py:1063-1164
	/// ```python
	/// def generate_created_models(self):
	///     for app_label, model_name in sorted(self.new_model_keys):
	///         model_state = self.to_state.models[app_label, model_name]
	///         self.add_operation(
	///             app_label,
	///             operations.CreateModel(
	///                 name=model_name,
	///                 fields=model_state.fields,
	///                 options=model_state.options,
	///                 bases=model_state.bases,
	///             ),
	///         )
	/// ```rust,ignore
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{MigrationAutodetector, ProjectState, ModelState, FieldState, FieldType};
	///
	/// let mut from_state = ProjectState::new();
	/// let mut to_state = ProjectState::new();
	///
	// Add a new model to the target state
	/// let mut model = ModelState::new("myapp", "User");
	/// model.add_field(FieldState::new("id", FieldType::Integer, false));
	/// to_state.add_model(model);
	///
	/// let detector = MigrationAutodetector::new(from_state, to_state);
	/// let operations = detector.generate_operations();
	///
	/// assert!(!operations.is_empty());
	/// ```rust,ignore
	/// Sort operations by their dependencies to ensure correct execution order
	///
	/// This method reorders operations to prevent execution errors:
	/// 1. CreateTable operations first (tables must exist before modification)
	/// 2. AddColumn/AlterColumn operations next (field modifications)
	/// 3. Other operations last (indexes, constraints, etc.)
	fn sort_operations_by_dependency(
		&self,
		mut operations: Vec<super::Operation>,
	) -> Vec<super::Operation> {
		let mut sorted = Vec::new();

		// Extract CreateTable operations (must be first)
		let create_tables: Vec<_> = operations
			.iter()
			.filter(|op| matches!(op, super::Operation::CreateTable { .. }))
			.cloned()
			.collect();
		operations.retain(|op| !matches!(op, super::Operation::CreateTable { .. }));

		// Extract field operations (must be after CreateTable)
		let field_ops: Vec<_> = operations
			.iter()
			.filter(|op| {
				matches!(
					op,
					super::Operation::AddColumn { .. } | super::Operation::AlterColumn { .. }
				)
			})
			.cloned()
			.collect();
		operations.retain(|op| {
			!matches!(
				op,
				super::Operation::AddColumn { .. } | super::Operation::AlterColumn { .. }
			)
		});

		// Assemble in correct order
		sorted.extend(create_tables);
		sorted.extend(field_ops);
		sorted.extend(operations); // Remaining operations

		sorted
	}

	pub fn generate_operations(&self) -> Vec<super::Operation> {
		let changes = self.detect_changes();
		let mut operations = Vec::new();

		// Generate CreateTable operations for new models
		for (app_label, model_name) in &changes.created_models {
			if let Some(model) = self.to_state.get_model(app_label, model_name) {
				let mut columns = Vec::new();
				for (field_name, field_state) in &model.fields {
					let col_def =
						super::ColumnDefinition::from_field_state(field_name.clone(), field_state);
					columns.push(col_def);
				}

				// Convert model constraints to operation constraints
				let constraints: Vec<_> = model
					.constraints
					.iter()
					.map(|c| c.to_constraint())
					.collect();

				operations.push(super::Operation::CreateTable {
					name: model.table_name.clone(),
					columns,
					constraints,
					without_rowid: None,
					interleave_in_parent: None,
					partition: None,
				});
			}
		}

		// Generate intermediate tables for ManyToMany fields in new models
		for (app_label, model_name) in &changes.created_models {
			if let Some(model) = self.to_state.get_model(app_label, model_name) {
				for (field_name, field_state) in &model.fields {
					if let super::FieldType::ManyToMany { to, through } = &field_state.field_type
						&& let Some(operation) = self.generate_intermediate_table(
							app_label, model_name, field_name, to, through,
						) {
						operations.push(operation);
					}
				}
			}
		}

		// Generate AddColumn operations for new fields
		for (app_label, model_name, field_name) in &changes.added_fields {
			if let Some(model) = self.to_state.get_model(app_label, model_name)
				&& let Some(field) = model.get_field(field_name)
			{
				operations.push(super::Operation::AddColumn {
					table: model.name.clone(),
					column: super::ColumnDefinition::from_field_state(field_name.clone(), field),
					mysql_options: None,
				});
			}
		}

		// Generate intermediate tables for ManyToMany fields being added
		for (app_label, model_name, field_name) in &changes.added_fields {
			if let Some(model) = self.to_state.get_model(app_label, model_name)
				&& let Some(field) = model.get_field(field_name)
				&& let super::FieldType::ManyToMany { to, through } = &field.field_type
				&& let Some(operation) =
					self.generate_intermediate_table(app_label, model_name, field_name, to, through)
			{
				operations.push(operation);
			}
		}

		// Generate AlterColumn operations for changed fields
		for (app_label, model_name, field_name) in &changes.altered_fields {
			if let Some(model) = self.to_state.get_model(app_label, model_name)
				&& let Some(field) = model.get_field(field_name)
			{
				operations.push(super::Operation::AlterColumn {
					table: model.name.clone(),
					old_definition: None,
					column: field_name.clone(),
					new_definition: super::ColumnDefinition::from_field_state(
						field_name.clone(),
						field,
					),
					mysql_options: None,
				});
			}
		}

		// Generate DropColumn operations for removed fields
		for (app_label, model_name, field_name) in &changes.removed_fields {
			if let Some(model) = self.from_state.get_model(app_label, model_name) {
				operations.push(super::Operation::DropColumn {
					table: model.name.clone(),
					column: field_name.clone(),
				});
			}
		}

		// Generate DropTable operations for deleted models
		for (app_label, model_name) in &changes.deleted_models {
			if let Some(model) = self.from_state.get_model(app_label, model_name) {
				operations.push(super::Operation::DropTable {
					name: model.table_name.clone(),
				});
			}
		}

		// Note: MoveModel operations for cross-app moves are detected in moved_models
		// and handled in generate_migrations() using Operation::MoveModel variant.
		// The MoveModel variant was added to the Operation enum to support this use case.

		// Sort operations by dependency to ensure correct execution order

		self.sort_operations_by_dependency(operations)
	}

	/// Generate migrations from detected changes
	///
	/// Groups operations by app_label and creates Migration objects for each app.
	/// This is the final step in the migration autodetection process.
	///
	/// # Django Reference
	/// From: django/db/migrations/autodetector.py:95-141
	/// ```python
	/// def changes(self, graph, trim_to_apps=None, convert_apps=None, migration_name=None):
	///     # Generate operations
	///     self._generate_through_model_map()
	///     self.generate_renamed_models()
	///     # ... all other generate_* methods
	///
	///     # Group operations by app
	///     self.arrange_for_graph(changes, graph, trim_to_apps)
	///
	///     # Create Migration objects
	///     return changes
	/// ```rust,ignore
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{MigrationAutodetector, ProjectState, ModelState, FieldState, FieldType};
	///
	/// let mut from_state = ProjectState::new();
	/// let mut to_state = ProjectState::new();
	///
	// Add a new model
	/// let mut model = ModelState::new("blog", "Post");
	/// model.add_field(FieldState::new("title", FieldType::VarChar(255), false));
	/// to_state.add_model(model);
	///
	/// let detector = MigrationAutodetector::new(from_state, to_state);
	/// let migrations = detector.generate_migrations();
	///
	/// assert_eq!(migrations.len(), 1);
	/// assert_eq!(migrations[0].app_label, "blog");
	/// assert!(!migrations[0].operations.is_empty());
	/// ```
	pub fn generate_migrations(&self) -> Vec<super::Migration> {
		let changes = self.detect_changes();
		let mut migrations_by_app: std::collections::BTreeMap<String, Vec<super::Operation>> =
			std::collections::BTreeMap::new();

		// Group created models by app
		for (app_label, model_name) in &changes.created_models {
			if let Some(model) = self.to_state.get_model(app_label, model_name) {
				let mut columns = Vec::new();
				for (field_name, field_state) in &model.fields {
					columns.push(super::ColumnDefinition::from_field_state(
						field_name.clone(),
						field_state,
					));
				}

				// Convert ConstraintDefinition to operations::Constraint
				let constraints: Vec<super::operations::Constraint> = model
					.constraints
					.iter()
					.map(|c| c.to_constraint())
					.collect();

				migrations_by_app
					.entry(app_label.clone())
					.or_default()
					.push(super::Operation::CreateTable {
						name: model.table_name.clone(),
						columns,
						constraints,
						without_rowid: None,
						interleave_in_parent: None,
						partition: None,
					});
			}
		}

		// Group added fields by app
		for (app_label, model_name, field_name) in &changes.added_fields {
			if let Some(model) = self.to_state.get_model(app_label, model_name)
				&& let Some(field) = model.get_field(field_name)
			{
				migrations_by_app
					.entry(app_label.clone())
					.or_default()
					.push(super::Operation::AddColumn {
						table: model.table_name.clone(),
						column: super::ColumnDefinition::from_field_state(
							field_name.clone(),
							field,
						),
						mysql_options: None,
					});
			}
		}

		// Group altered fields by app
		for (app_label, model_name, field_name) in &changes.altered_fields {
			if let Some(model) = self.to_state.get_model(app_label, model_name)
				&& let Some(field) = model.get_field(field_name)
			{
				migrations_by_app
					.entry(app_label.clone())
					.or_default()
					.push(super::Operation::AlterColumn {
						table: model.table_name.clone(),
						column: field_name.clone(),
						old_definition: None,
						new_definition: super::ColumnDefinition::from_field_state(
							field_name.clone(),
							field,
						),
						mysql_options: None,
					});
			}
		}

		// Group removed fields by app
		for (app_label, model_name, field_name) in &changes.removed_fields {
			if let Some(model) = self.from_state.get_model(app_label, model_name) {
				migrations_by_app
					.entry(app_label.clone())
					.or_default()
					.push(super::Operation::DropColumn {
						table: model.table_name.clone(),
						column: field_name.clone(),
					});
			}
		}

		// Group deleted models by app
		for (app_label, model_name) in &changes.deleted_models {
			if let Some(model) = self.from_state.get_model(app_label, model_name) {
				migrations_by_app
					.entry(app_label.clone())
					.or_default()
					.push(super::Operation::DropTable {
						name: model.table_name.clone(),
					});
			}
		}

		// Generate intermediate tables for ManyToMany relationships
		for (app_label, model_name, through_table, m2m) in &changes.created_many_to_many {
			// Generate column names (Django-style naming convention)
			let source_model_lower = model_name.to_lowercase();
			let target_model_lower = m2m.to_model.to_lowercase();

			// Check for self-referencing ManyToMany (e.g., User follows User)
			let is_self_referencing = source_model_lower == target_model_lower;

			// For self-referencing ManyToMany, use from_/to_ prefixes to avoid column name collision
			// Django uses this convention: from_{model}_id and to_{model}_id
			let source_column = m2m.source_field.clone().unwrap_or_else(|| {
				if is_self_referencing {
					format!("from_{}_id", source_model_lower)
				} else {
					format!("{}_id", source_model_lower)
				}
			});
			let target_column = m2m.target_field.clone().unwrap_or_else(|| {
				if is_self_referencing {
					format!("to_{}_id", target_model_lower)
				} else {
					format!("{}_id", target_model_lower)
				}
			});

			// Get source table name
			let source_table = self
				.to_state
				.get_model(app_label, model_name)
				.map(|m| m.table_name.clone())
				.unwrap_or_else(|| format!("{}_{}", app_label, source_model_lower));

			// Get target table name
			// First try to_state, then fall back to global registry for cross-app references
			let target_table = self
				.find_model_app(&m2m.to_model)
				.and_then(|target_app| {
					// Try to_state first
					if let Some(model) = self.to_state.get_model(&target_app, &m2m.to_model) {
						return Some(model.table_name.clone());
					}
					// Fall back to global registry for cross-app references
					for model_meta in super::model_registry::global_registry().get_models() {
						if model_meta.app_label == target_app
							&& model_meta.model_name == m2m.to_model
						{
							return Some(model_meta.table_name.clone());
						}
					}
					None
				})
				.unwrap_or_else(|| m2m.to_model.to_lowercase());

			// Get source model's primary key type
			let source_pk_type = self.to_state.get_primary_key_type(app_label, model_name);

			// Get target model's primary key type
			// First extract target app_label from to_model (may be in "app.Model" format)
			let (target_app, target_model) = if m2m.to_model.contains('.') {
				let parts: Vec<&str> = m2m.to_model.split('.').collect();
				(parts[0].to_string(), parts[1].to_string())
			} else {
				// Same app reference
				(app_label.to_string(), m2m.to_model.clone())
			};

			let target_pk_type = self
				.to_state
				.get_primary_key_type(&target_app, &target_model);

			// Create intermediate table columns
			let columns = vec![
				super::ColumnDefinition {
					name: "id".to_string(),
					type_definition: super::FieldType::Integer,
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: true,
					default: None,
				},
				super::ColumnDefinition {
					name: source_column.clone(),
					type_definition: source_pk_type.clone(),
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				super::ColumnDefinition {
					name: target_column.clone(),
					type_definition: target_pk_type,
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
			];

			// Create FK constraints for the intermediate table
			let constraints = vec![
				super::operations::Constraint::ForeignKey {
					name: format!("fk_{}_{}", through_table, source_column),
					columns: vec![source_column.clone()],
					referenced_table: source_table.clone(),
					referenced_columns: vec!["id".to_string()],
					on_delete: ForeignKeyAction::Cascade,
					on_update: ForeignKeyAction::Cascade,
					deferrable: None,
				},
				super::operations::Constraint::ForeignKey {
					name: format!("fk_{}_{}", through_table, target_column),
					columns: vec![target_column.clone()],
					referenced_table: target_table,
					referenced_columns: vec!["id".to_string()],
					on_delete: ForeignKeyAction::Cascade,
					on_update: ForeignKeyAction::Cascade,
					deferrable: None,
				},
				// Add unique constraint on the combination of both FK columns
				super::operations::Constraint::Unique {
					name: format!("{}_unique", through_table),
					columns: vec![source_column, target_column],
				},
			];

			migrations_by_app
				.entry(app_label.clone())
				.or_default()
				.push(super::Operation::CreateTable {
					name: through_table.clone(),
					columns,
					constraints,
					without_rowid: None,
					interleave_in_parent: None,
					partition: None,
				});
		}

		// Handle model renames (same app)
		for (app_label, old_name, new_name) in &changes.renamed_models {
			if let Some(model) = self.to_state.get_model(app_label, new_name) {
				// Get the old table name from from_state
				let old_table_name = self
					.from_state
					.get_model(app_label, old_name)
					.map(|m| m.table_name.clone())
					.unwrap_or_else(|| format!("{}_{}", app_label, old_name.to_lowercase()));

				migrations_by_app
					.entry(app_label.clone())
					.or_default()
					.push(super::Operation::RenameTable {
						old_name: old_table_name,
						new_name: model.table_name.clone(),
					});
			}
		}

		// Handle cross-app model moves
		// MovedModelInfo: (from_app, to_app, model_name, rename_table, old_table, new_table)
		for (from_app, to_app, model_name, rename_table, old_table, new_table) in
			&changes.moved_models
		{
			// Get table names
			let old_table_name = old_table.clone().unwrap_or_else(|| {
				self.from_state
					.get_model(from_app, model_name)
					.map(|m| m.table_name.clone())
					.unwrap_or_else(|| format!("{}_{}", from_app, model_name.to_lowercase()))
			});

			let new_table_name = new_table.clone().unwrap_or_else(|| {
				self.to_state
					.get_model(to_app, model_name)
					.map(|m| m.table_name.clone())
					.unwrap_or_else(|| format!("{}_{}", to_app, model_name.to_lowercase()))
			});

			// Add MoveModel operation to the target app's migrations
			migrations_by_app.entry(to_app.clone()).or_default().push(
				super::Operation::MoveModel {
					model_name: model_name.clone(),
					from_app: from_app.clone(),
					to_app: to_app.clone(),
					rename_table: *rename_table,
					old_table_name: if *rename_table {
						Some(old_table_name)
					} else {
						None
					},
					new_table_name: if *rename_table {
						Some(new_table_name)
					} else {
						None
					},
				},
			);
		}

		// Create Migration objects for each app
		let mut migrations = Vec::new();
		for (app_label, operations) in migrations_by_app {
			// Generate a simple migration name based on the first operation
			let migration_name = if let Some(op) = operations.first() {
				match op {
					super::Operation::CreateTable { name, .. } => {
						format!("0001_initial_{}", name.to_lowercase())
					}
					super::Operation::AddColumn { table, column, .. } => {
						format!(
							"0001_add_{}_{}",
							column.name.to_lowercase(),
							table.to_lowercase()
						)
					}
					super::Operation::AlterColumn { table, column, .. } => format!(
						"0001_alter_{}_{}",
						column.to_lowercase(),
						table.to_lowercase()
					),
					super::Operation::DropColumn { table, column, .. } => {
						format!(
							"0001_remove_{}_{}",
							column.to_lowercase(),
							table.to_lowercase()
						)
					}
					super::Operation::DropTable { name, .. } => {
						format!("0001_delete_{}", name.to_lowercase())
					}
					super::Operation::RenameTable { old_name, new_name } => {
						format!(
							"0001_rename_{}_to_{}",
							old_name.to_lowercase(),
							new_name.to_lowercase()
						)
					}
					super::Operation::MoveModel {
						model_name,
						from_app,
						to_app,
						..
					} => {
						format!(
							"0001_move_{}_from_{}_to_{}",
							model_name.to_lowercase(),
							from_app.to_lowercase(),
							to_app.to_lowercase()
						)
					}
					_ => "0001_auto".to_string(),
				}
			} else {
				"0001_auto".to_string()
			};

			let mut migration = super::Migration::new(&migration_name, &app_label);
			for operation in operations {
				migration = migration.add_operation(operation);
			}
			migrations.push(migration);
		}

		migrations
	}

	/// Detect newly created ManyToMany relationships
	///
	/// This method compares ManyToMany fields between from_state and to_state
	/// to detect new relationships that require intermediate table creation.
	///
	/// # Detection Logic
	/// 1. Iterate through all models in to_state
	/// 2. For each ManyToMany field, check if it exists in from_state
	/// 3. If not, mark it as a newly created ManyToMany relationship
	///
	/// # Intermediate Table Naming
	/// Uses Django naming convention: `{app}_{model}_{field}`
	/// Custom through table names are supported via `through` option.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{MigrationAutodetector, ProjectState, ModelState, ManyToManyMetadata};
	///
	/// let from_state = ProjectState::new();
	/// let mut to_state = ProjectState::new();
	///
	/// // Create User model with ManyToMany to Group
	/// let mut user = ModelState::new("auth", "User");
	/// user.many_to_many_fields.push(ManyToManyMetadata {
	///     field_name: "groups".to_string(),
	///     to_model: "Group".to_string(),
	///     related_name: Some("users".to_string()),
	///     through: None,
	///     source_field: None,
	///     target_field: None,
	///     db_constraint_prefix: None,
	/// });
	/// to_state.add_model(user);
	///
	/// let detector = MigrationAutodetector::new(from_state, to_state);
	/// let changes = detector.detect_changes();
	///
	/// // Should detect created ManyToMany relationship
	/// assert_eq!(changes.created_many_to_many.len(), 1);
	/// assert_eq!(changes.created_many_to_many[0].2, "auth_user_groups");
	/// ```
	fn detect_created_many_to_many(&self, changes: &mut DetectedChanges) {
		for ((app_label, model_name), model_state) in &self.to_state.models {
			for m2m in &model_state.many_to_many_fields {
				// Check if this ManyToMany already exists in from_state
				let exists_in_from = self
					.from_state
					.get_model(app_label, model_name)
					.map(|from_model| {
						from_model
							.many_to_many_fields
							.iter()
							.any(|f| f.field_name == m2m.field_name)
					})
					.unwrap_or(false);

				if !exists_in_from {
					// Generate through table name (Django naming convention)
					let through_table = m2m.through.clone().unwrap_or_else(|| {
						format!(
							"{}_{}_{}",
							app_label.to_lowercase(),
							model_name.to_lowercase(),
							m2m.field_name.to_lowercase()
						)
					});

					// Add to created_many_to_many
					changes.created_many_to_many.push((
						app_label.clone(),
						model_name.clone(),
						through_table.clone(),
						m2m.clone(),
					));

					// Add model dependencies
					// The intermediate table depends on both source and target models
					let target_app = self
						.find_model_app(&m2m.to_model)
						.unwrap_or_else(|| app_label.clone());

					changes
						.model_dependencies
						.entry((app_label.clone(), through_table))
						.or_default()
						.extend(vec![
							(app_label.clone(), model_name.clone()),
							(target_app, m2m.to_model.clone()),
						]);
				}
			}
		}
	}

	/// Find the app_label for a given model name
	///
	/// Searches through to_state models to find the app that contains the model.
	/// If not found in to_state, falls back to the global registry for cross-app references.
	fn find_model_app(&self, model_name: &str) -> Option<String> {
		// First, search in to_state
		for (app_label, name) in self.to_state.models.keys() {
			if name == model_name {
				return Some(app_label.clone());
			}
		}

		// If not found, search in global registry for cross-app references
		// This is needed when generating migrations for one app that references models in another app
		for model_meta in super::model_registry::global_registry().get_models() {
			if model_meta.model_name == model_name {
				return Some(model_meta.app_label.clone());
			}
		}

		None
	}

	/// Detect model dependencies for proper migration ordering
	///
	/// This method analyzes ForeignKey relationships between models to ensure
	/// migrations are generated in the correct order. A model that references
	/// another model via ForeignKey depends on that model being created first.
	///
	/// # Django Reference
	/// Django's dependency detection is in `django/db/migrations/autodetector.py:1400`
	/// Function: `_generate_through_model_map` and dependency tracking
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{MigrationAutodetector, ProjectState, ModelState, FieldState, FieldType};
	///
	/// let mut from_state = ProjectState::new();
	/// let mut to_state = ProjectState::new();
	///
	/// // Create User model
	/// let mut user = ModelState::new("accounts", "User");
	/// user.add_field(FieldState::new("id", FieldType::Integer, false));
	/// to_state.add_model(user);
	///
	/// // Create Post model that references User
	/// let mut post = ModelState::new("blog", "Post");
	/// post.add_field(FieldState::new("id", FieldType::Integer, false));
	/// post.add_field(FieldState::new("author_id", FieldType::Custom("ForeignKey(accounts.User)".into()), false));
	/// to_state.add_model(post);
	///
	/// let detector = MigrationAutodetector::new(from_state, to_state);
	/// let changes = detector.detect_changes();
	///
	/// // blog.Post depends on accounts.User
	/// let post_deps = changes.model_dependencies.get(&("blog".to_string(), "Post".to_string()));
	/// assert!(post_deps.is_some());
	/// assert!(post_deps.unwrap().contains(&("accounts".to_string(), "User".to_string())));
	/// ```
	fn detect_model_dependencies(&self, changes: &mut DetectedChanges) {
		// Analyze all models in the final state
		for ((app_label, model_name), model) in &self.to_state.models {
			let mut dependencies = Vec::new();

			// Check each field for foreign key relationships
			for field in model.fields.values() {
				match &field.field_type {
					// Handle structured ForeignKey variant
					super::FieldType::ForeignKey { to_table, .. } => {
						// Find model by table name in the project state
						if let Some(dep) = self.find_model_by_table_name(to_table) {
							// Avoid self-reference unless intentional
							if dep != (app_label.clone(), model_name.clone()) {
								dependencies.push(dep);
							}
						}
					}
					// Handle structured OneToOne variant
					super::FieldType::OneToOne { to, .. } => {
						// Format: "app.Model" or "Model"
						if let Some(dep) = self.parse_model_reference(to, app_label)
							&& dep != (app_label.clone(), model_name.clone())
						{
							dependencies.push(dep);
						}
					}
					// Handle structured ManyToMany variant
					super::FieldType::ManyToMany { to, .. } => {
						// Format: "app.Model" or "Model"
						if let Some(dep) = self.parse_model_reference(to, app_label)
							&& dep != (app_label.clone(), model_name.clone())
						{
							dependencies.push(dep);
						}
					}
					// Handle legacy Custom string format
					super::FieldType::Custom(s) => {
						if let Some(referenced_model) = self.extract_related_model(s, app_label)
							&& referenced_model != (app_label.clone(), model_name.clone())
						{
							dependencies.push(referenced_model);
						}
					}
					// Skip other field types
					_ => {}
				}
			}

			// Only store if there are actual dependencies
			if !dependencies.is_empty() {
				changes
					.model_dependencies
					.insert((app_label.clone(), model_name.clone()), dependencies);
			}
		}
	}

	/// Extract related model from field type string
	///
	/// Parses field type strings like:
	/// - "ForeignKey(app.Model)" -> Some(("app", "Model"))
	/// - "ManyToManyField(app.Model)" -> Some(("app", "Model"))
	/// - "ForeignKey(Model)" -> Some((current_app, "Model"))
	/// - "CharField" -> None
	///
	/// # Arguments
	/// * `field_type` - Field type string (e.g., "ForeignKey(accounts.User)")
	/// * `current_app` - Current app label for resolving unqualified references
	///
	/// # Returns
	/// * `Some((app_label, model_name))` if field is a relation
	/// * `None` if field is not a relation
	fn extract_related_model(
		&self,
		field_type: &str,
		current_app: &str,
	) -> Option<(String, String)> {
		// Check for ForeignKey pattern
		if let Some(inner) = field_type
			.strip_prefix("ForeignKey(")
			.and_then(|s| s.strip_suffix(")"))
		{
			return self.parse_model_reference(inner, current_app);
		}

		// Check for ManyToManyField pattern
		if let Some(inner) = field_type
			.strip_prefix("ManyToManyField(")
			.and_then(|s| s.strip_suffix(")"))
		{
			return self.parse_model_reference(inner, current_app);
		}

		// Check for OneToOneField pattern
		if let Some(inner) = field_type
			.strip_prefix("OneToOneField(")
			.and_then(|s| s.strip_suffix(")"))
		{
			return self.parse_model_reference(inner, current_app);
		}

		None
	}

	/// Parse model reference string into (app_label, model_name)
	///
	/// Supports formats:
	/// - "app.Model" -> ("app", "Model")
	/// - "Model" -> (current_app, "Model") - Uses current app for unqualified references
	///
	/// # Arguments
	/// * `reference` - Model reference string (e.g., "accounts.User" or "User")
	/// * `current_app` - Current app label for resolving unqualified references
	///
	/// # Returns
	/// * `Some((app_label, model_name))` if parseable
	/// * `None` if format is invalid
	fn parse_model_reference(
		&self,
		reference: &str,
		current_app: &str,
	) -> Option<(String, String)> {
		let parts: Vec<&str> = reference.split('.').collect();
		match parts.as_slice() {
			// Fully qualified reference: "app.Model"
			[app, model] => Some((app.to_string(), model.to_string())),
			// Unqualified reference: "Model" - assume same app
			[model] => {
				// Use current app for same-app references
				Some((current_app.to_string(), model.to_string()))
			}
			// Invalid format
			_ => None,
		}
	}

	/// Find a model in the project state by its table name
	///
	/// This method searches through all models in both from_state and to_state
	/// to find a model whose table name matches the given table name.
	///
	/// Table name matching supports:
	/// - Django-style table names: "app_modelname" (e.g., "auth_user")
	/// - Simple model name match: "modelname" (lowercase, e.g., "user")
	///
	/// # Arguments
	/// * `table_name` - The table name to search for
	///
	/// # Returns
	/// * `Some((app_label, model_name))` if found
	/// * `None` if no matching model is found
	fn find_model_by_table_name(&self, table_name: &str) -> Option<(String, String)> {
		// Search in to_state (target state has priority)
		for (app_label, model_name) in self.to_state.models.keys() {
			// Check Django-style table name: app_modelname
			let django_table = format!("{}_{}", app_label, model_name.to_lowercase());
			if django_table == table_name {
				return Some((app_label.clone(), model_name.clone()));
			}

			// Check simple lowercase model name
			if model_name.to_lowercase() == table_name {
				return Some((app_label.clone(), model_name.clone()));
			}
		}

		// Fallback: search in from_state
		for (app_label, model_name) in self.from_state.models.keys() {
			let django_table = format!("{}_{}", app_label, model_name.to_lowercase());
			if django_table == table_name {
				return Some((app_label.clone(), model_name.clone()));
			}

			if model_name.to_lowercase() == table_name {
				return Some((app_label.clone(), model_name.clone()));
			}
		}

		None
	}
}

impl ModelState {
	/// Remove a field from this model
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{ModelState, FieldState, FieldType};
	///
	/// let mut model = ModelState::new("myapp", "User");
	/// let field = FieldState::new("email", FieldType::VarChar(255), false);
	/// model.add_field(field);
	/// assert!(model.has_field("email"));
	///
	/// model.remove_field("email");
	/// assert!(!model.has_field("email"));
	/// ```
	pub fn remove_field(&mut self, name: &str) {
		self.fields.remove(name);
	}

	/// Alter a field definition
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{ModelState, FieldState, FieldType};
	///
	/// let mut model = ModelState::new("myapp", "User");
	/// let field = FieldState::new("email", FieldType::VarChar(255), false);
	/// model.add_field(field);
	///
	/// let new_field = FieldState::new("email", FieldType::Text, true);
	/// model.alter_field("email", new_field);
	///
	/// let altered = model.get_field("email").unwrap();
	/// assert_eq!(altered.field_type, FieldType::Text);
	/// assert!(altered.nullable);
	/// ```
	pub fn alter_field(&mut self, name: &str, new_field: FieldState) {
		self.fields.insert(name.to_string(), new_field);
	}
}
