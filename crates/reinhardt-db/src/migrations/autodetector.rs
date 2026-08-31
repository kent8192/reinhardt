//! Migration autodetector

use petgraph::Undirected;
use petgraph::graph::Graph;
use petgraph::visit::EdgeRef;
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet, HashMap};
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
pub use crate::naming::to_snake_case;

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
#[derive(Debug, Clone, PartialEq)]
pub struct FieldState {
	/// The name.
	pub name: String,
	/// The field type.
	pub field_type: super::FieldType,
	/// The nullable.
	pub nullable: bool,
	/// The params.
	pub params: std::collections::HashMap<String, String>,
	/// Generated-column metadata.
	pub generated: Option<super::GeneratedColumnDefinition>,
	/// Structured database value domain.
	pub domain: Option<crate::field_domain::FieldDomain>,
	/// ForeignKey information if this field is a foreign key
	pub foreign_key: Option<ForeignKeyInfo>,
}

impl FieldState {
	/// Creates a new instance.
	pub fn new(name: impl Into<String>, field_type: super::FieldType, nullable: bool) -> Self {
		Self {
			name: name.into(),
			field_type,
			nullable,
			params: std::collections::HashMap::new(),
			generated: None,
			domain: None,
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
			generated: None,
			domain: None,
			foreign_key: Some(foreign_key),
		}
	}

	/// Sets structured database value domain metadata.
	pub fn with_domain(mut self, domain: crate::field_domain::FieldDomain) -> Self {
		self.domain = Some(domain.canonicalized());
		self
	}
}

/// Model state for migration detection
///
/// Django equivalent: `ModelState` in django/db/migrations/state.py
#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone)]
struct GeneratedColumnReplacementDependent {
	name: String,
	old_definition: super::ColumnDefinition,
	new_definition: Option<super::ColumnDefinition>,
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
	/// Predicate for a partial index.
	pub where_clause: Option<String>,
	/// Typed index method and options.
	#[cfg(feature = "pgvector")]
	pub index_type: Option<super::operations::IndexType>,
	/// PostgreSQL operator class.
	#[cfg(feature = "pgvector")]
	pub operator_class: Option<String>,
	/// Index expressions.
	#[cfg(feature = "pgvector")]
	pub expressions: Option<Vec<String>>,
}

const ADVANCED_INDEX_OPTION_PREFIX: &str = "__reinhardt_advanced_index__:";
const ADVANCED_INDEX_OPTION_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct AdvancedIndexOptionState {
	version: u8,
	#[serde(default)]
	advanced: bool,
	#[serde(default)]
	concurrently: bool,
	#[serde(default)]
	mysql_options: Option<super::operations::AlterTableOptions>,
}

impl Default for AdvancedIndexOptionState {
	fn default() -> Self {
		Self {
			version: ADVANCED_INDEX_OPTION_VERSION,
			advanced: false,
			concurrently: false,
			mysql_options: None,
		}
	}
}

impl AdvancedIndexOptionState {
	fn new(
		advanced: bool,
		concurrently: bool,
		mysql_options: Option<super::operations::AlterTableOptions>,
	) -> Self {
		Self {
			advanced,
			concurrently,
			mysql_options,
			..Self::default()
		}
	}

	fn should_store(self) -> bool {
		self.advanced || self.concurrently || self.mysql_options.is_some()
	}
}

impl IndexDefinition {
	/// Creates index metadata from its feature-independent fields.
	pub fn new(name: impl Into<String>, fields: Vec<String>, unique: bool) -> Self {
		Self {
			name: super::operations::truncate_identifier_with_hash(&name.into()),
			fields,
			unique,
			where_clause: None,
			#[cfg(feature = "pgvector")]
			index_type: None,
			#[cfg(feature = "pgvector")]
			operator_class: None,
			#[cfg(feature = "pgvector")]
			expressions: None,
		}
	}

	/// Returns the configured typed index method and options.
	pub fn index_type(&self) -> Option<super::operations::IndexType> {
		#[cfg(feature = "pgvector")]
		{
			self.index_type
		}
		#[cfg(not(feature = "pgvector"))]
		{
			None
		}
	}

	/// Returns the PostgreSQL operator class, when configured.
	pub fn operator_class(&self) -> Option<&String> {
		#[cfg(feature = "pgvector")]
		{
			self.operator_class.as_ref()
		}
		#[cfg(not(feature = "pgvector"))]
		{
			None
		}
	}

	/// Returns the index expressions, when configured.
	pub fn expressions(&self) -> Option<&Vec<String>> {
		#[cfg(feature = "pgvector")]
		{
			self.expressions.as_ref()
		}
		#[cfg(not(feature = "pgvector"))]
		{
			None
		}
	}

	fn create_operation(&self, model: &ModelState) -> super::Operation {
		self.create_operation_with_state(
			&model.table_name,
			advanced_index_option_state(model, self).unwrap_or_default(),
		)
	}

	fn create_operation_with_state(
		&self,
		table: &str,
		state: AdvancedIndexOptionState,
	) -> super::Operation {
		#[cfg(feature = "pgvector")]
		{
			super::Operation::CreateNamedIndex {
				table: table.to_string(),
				name: self.name.clone(),
				columns: self.fields.clone(),
				unique: self.unique,
				index_type: self.index_type(),
				where_clause: self.where_clause.clone(),
				concurrently: state.concurrently,
				expressions: self.expressions().cloned(),
				mysql_options: state.mysql_options,
				operator_class: self.operator_class().cloned(),
			}
		}
		#[cfg(not(feature = "pgvector"))]
		{
			super::Operation::CreateIndex {
				table: table.to_string(),
				columns: self.fields.clone(),
				unique: self.unique,
				index_type: None,
				where_clause: self.where_clause.clone(),
				concurrently: state.concurrently,
				expressions: None,
				mysql_options: state.mysql_options,
				operator_class: None,
			}
		}
	}

	fn drop_operation(&self, model: &ModelState) -> super::Operation {
		self.drop_operation_with_state(
			&model.table_name,
			advanced_index_option_state(model, self).unwrap_or_default(),
		)
	}

	fn drop_operation_with_state(
		&self,
		table: &str,
		state: AdvancedIndexOptionState,
	) -> super::Operation {
		#[cfg(feature = "pgvector")]
		{
			super::Operation::DropNamedIndex {
				table: table.to_string(),
				name: self.name.clone(),
				columns: self.fields.clone(),
				unique: self.unique,
				index_type: self.index_type(),
				where_clause: self.where_clause.clone(),
				concurrently: state.concurrently,
				expressions: self.expressions().cloned(),
				mysql_options: state.mysql_options,
				operator_class: self.operator_class().cloned(),
			}
		}
		#[cfg(not(feature = "pgvector"))]
		{
			super::Operation::DropNamedIndex {
				table: table.to_string(),
				name: self.name.clone(),
				columns: self.fields.clone(),
				unique: self.unique,
				index_type: None,
				where_clause: self.where_clause.clone(),
				concurrently: state.concurrently,
				expressions: None,
				mysql_options: state.mysql_options,
				operator_class: None,
			}
		}
	}

	fn create_named_operation_with_state(
		&self,
		table: &str,
		state: AdvancedIndexOptionState,
	) -> super::Operation {
		super::Operation::CreateIndexRepair {
			table: table.to_string(),
			name: Some(self.name.clone()),
			columns: self.fields.clone(),
			unique: self.unique,
			index_type: self.index_type(),
			where_clause: self.where_clause.clone(),
			concurrently: state.concurrently,
			expressions: self.expressions().cloned(),
			mysql_options: state.mysql_options,
			operator_class: self.operator_class().cloned(),
		}
	}

	fn create_replacement_operation_with_state(
		&self,
		table: &str,
		state: AdvancedIndexOptionState,
	) -> super::Operation {
		if self.index_type().is_some() {
			self.create_operation_with_state(table, state)
		} else {
			self.create_named_operation_with_state(table, state)
		}
	}
}

fn advanced_index_option_key(name: &str) -> String {
	format!("{ADVANCED_INDEX_OPTION_PREFIX}{name}")
}

fn decode_advanced_index_option_state(value: &str) -> AdvancedIndexOptionState {
	if value.trim() == "true" {
		return AdvancedIndexOptionState {
			advanced: true,
			..AdvancedIndexOptionState::default()
		};
	}

	serde_json::from_str::<AdvancedIndexOptionState>(value)
		.ok()
		.filter(|state| state.version == ADVANCED_INDEX_OPTION_VERSION)
		.unwrap_or_default()
}

fn advanced_index_option_state(
	model: &ModelState,
	index: &IndexDefinition,
) -> Option<AdvancedIndexOptionState> {
	model
		.options
		.get(&advanced_index_option_key(&index.name))
		.map(|value| decode_advanced_index_option_state(value))
}

fn set_advanced_index_option_state(
	model: &mut ModelState,
	index_name: &str,
	state: AdvancedIndexOptionState,
) {
	let key = advanced_index_option_key(index_name);
	if state.should_store() {
		let value = serde_json::to_string(&state)
			.expect("serializing advanced index option state must succeed");
		model.options.insert(key, value);
	} else {
		model.options.remove(&key);
	}
}

fn model_index_is_advanced(model: &ModelState, index: &IndexDefinition) -> bool {
	index.where_clause.is_some()
		|| index.index_type().is_some()
		|| index.expressions().is_some()
		|| index.operator_class().is_some()
		|| advanced_index_option_state(model, index).is_some_and(|state| state.advanced)
}

fn model_index_definitions_equivalent(
	left_model: &ModelState,
	left: &IndexDefinition,
	right_model: &ModelState,
	right: &IndexDefinition,
) -> bool {
	index_definitions_equivalent(left, right)
		&& (left_model.table_name != right_model.table_name || left.name == right.name)
		&& model_index_is_advanced(left_model, left) == model_index_is_advanced(right_model, right)
}

/// Build the physical index name used by `Operation::CreateIndex`.
pub(crate) fn default_index_name(table: &str, fields: &[String]) -> String {
	super::operations::generated_index_name(table, fields, None)
}

/// Compare indexes by schema semantics rather than generated names.
pub(crate) fn index_definitions_equivalent(
	left: &IndexDefinition,
	right: &IndexDefinition,
) -> bool {
	left.fields == right.fields
		&& left.unique == right.unique
		&& left.where_clause == right.where_clause
		&& left.index_type() == right.index_type()
		&& left.expressions() == right.expressions()
		&& left.operator_class() == right.operator_class()
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

/// Returns true when `c` is a single-column UNIQUE constraint.
///
/// "Single-column" means exactly one entry in `fields`; `constraint_type` is
/// compared case-insensitively against `"unique"` because the codebase has
/// historically mixed `"unique"` (model registry / autodetector) and
/// `"UNIQUE"` (`schema_diff::ConstraintSchema`) spellings for the same
/// concept. The two diff codepaths converge here, so accept either.
fn is_single_field_unique(c: &ConstraintDefinition) -> bool {
	c.constraint_type.eq_ignore_ascii_case("unique") && c.fields.len() == 1
}

/// Extracts the single column name from a constraint SQL of the form
/// `CONSTRAINT <name> UNIQUE (<column>)` and returns it when the body has no
/// comma (i.e. it really is a single-column UNIQUE).
///
/// Used by `MigrationAutodetector::dedup_redundant_unique_add_constraints`
/// to identify which `Operation::AddConstraint` operations are eligible for
/// the redundant-emission check (multi-column UNIQUE / non-UNIQUE
/// constraints are deliberately ignored).
fn parse_single_column_unique(constraint_sql: &str) -> Option<&str> {
	// Uppercase-only match: every emitter in this crate writes `UNIQUE` in
	// upper case (see `operations::Constraint`'s `Display` impl and
	// `schema_diff::constraint_schema_to_sql`).
	let after_unique = constraint_sql.split(" UNIQUE (").nth(1)?;
	let close = after_unique.find(')')?;
	let body = after_unique[..close].trim();
	if body.contains(',') || body.is_empty() {
		return None;
	}
	Some(
		body.strip_prefix('"')
			.and_then(|body| body.strip_suffix('"'))
			.unwrap_or(body),
	)
}

impl ConstraintDefinition {
	/// Creates a typed enum-domain constraint definition.
	pub fn enum_domain(
		name: impl Into<String>,
		column: impl Into<String>,
		domain: crate::field_domain::FieldDomain,
	) -> Self {
		let domain = domain.canonicalized();
		Self {
			name: name.into(),
			constraint_type: "enum_domain".to_string(),
			fields: vec![column.into()],
			expression: Some(
				serde_json::to_string(&domain).expect("FieldDomain serialization must succeed"),
			),
			foreign_key_info: None,
		}
	}

	/// Convert ConstraintDefinition to operations::Constraint
	pub fn to_constraint(&self) -> super::operations::Constraint {
		match self.constraint_type.as_str() {
			"enum_domain" => super::operations::Constraint::EnumDomain {
				name: self.name.clone(),
				column: self.fields.first().cloned().unwrap_or_default(),
				domain: serde_json::from_str::<crate::field_domain::FieldDomain>(
					self.expression.as_deref().unwrap_or(""),
				)
				.expect("enum-domain constraint metadata must contain a FieldDomain")
				.canonicalized(),
			},
			unique if unique.eq_ignore_ascii_case("unique") => {
				super::operations::Constraint::Unique {
					name: self.name.clone(),
					columns: self.fields.clone(),
				}
			}
			"primary_key" => super::operations::Constraint::PrimaryKey {
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
	/// assert_eq!(model.table_name, "myapp_user");
	/// assert_eq!(model.fields.len(), 0);
	/// ```
	pub fn new(app_label: impl Into<String>, name: impl Into<String>) -> Self {
		let app_label = app_label.into();
		let name_str = name.into();
		let table_name = format!("{}_{}", app_label, to_snake_case(&name_str));

		Self {
			app_label,
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
				name: crate::naming::foreign_key_constraint_name(&self.table_name, field_name),
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
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectState {
	/// Models: (app_label, model_name) -> ModelState
	pub models: std::collections::BTreeMap<(String, String), ModelState>,

	/// Whether prior migrations contain opaque SQL that this state cannot model.
	pub has_opaque_schema_operations: bool,
}

impl Default for ProjectState {
	fn default() -> Self {
		Self::new()
	}
}

impl ProjectState {
	/// Validates that physical index names are safe and unique in the schema namespace.
	pub fn validate_physical_index_names(&self) -> super::Result<()> {
		let mut owners = self
			.models
			.values()
			.map(|model| (model.table_name.clone(), model.table_name.clone()))
			.collect::<HashMap<_, _>>();
		for model in self.models.values() {
			for index in &model.indexes {
				if index.name.is_empty() {
					return Err(super::MigrationError::InvalidMigration(format!(
						"physical index name on table `{}` must not be empty",
						model.table_name
					)));
				}
				if index.name.contains('\0') {
					return Err(super::MigrationError::InvalidMigration(format!(
						"physical index name on table `{}` must not contain NUL",
						model.table_name
					)));
				}
				if let Some(previous_table) =
					owners.insert(index.name.clone(), model.table_name.clone())
				{
					return Err(super::MigrationError::InvalidMigration(format!(
						"physical index name `{}` conflicts with relation `{}` while declared by `{}`",
						index.name, previous_table, model.table_name
					)));
				}
			}
		}
		Ok(())
	}

	/// Converts to database schema.
	pub fn to_database_schema(&self) -> super::schema_diff::DatabaseSchema {
		let mut tables = BTreeMap::new();

		for model_state in self.models.values() {
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
						generated: field_state.generated.clone(),
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

			// Convert indexes from ModelState to IndexSchema
			let indexes: Vec<super::schema_diff::IndexSchema> = model_state
				.indexes
				.iter()
				.map(|idx| super::schema_diff::IndexSchema {
					name: idx.name.clone(),
					columns: idx.fields.clone(),
					unique: idx.unique,
					access_method: None,
					index_type: idx.index_type(),
					expressions: idx.expressions().cloned(),
					operator_class: idx.operator_class().cloned(),
				})
				.collect();

			let table_key = model_state.table_name.clone();
			tables.insert(
				table_key,
				super::schema_diff::TableSchema {
					name: model_state.table_name.clone(),
					columns,
					indexes,
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

		for ((this_app_label, _), model_state) in &self.models {
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
							generated: field_state.generated.clone(),
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

				// Convert indexes from ModelState to IndexSchema
				let indexes: Vec<super::schema_diff::IndexSchema> = model_state
					.indexes
					.iter()
					.map(|idx| super::schema_diff::IndexSchema {
						name: idx.name.clone(),
						columns: idx.fields.clone(),
						unique: idx.unique,
						access_method: None,
						index_type: idx.index_type(),
						expressions: idx.expressions().cloned(),
						operator_class: idx.operator_class().cloned(),
					})
					.collect();

				let table_key = model_state.table_name.clone();
				tables.insert(
					table_key,
					super::schema_diff::TableSchema {
						name: model_state.table_name.clone(),
						columns,
						indexes,
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
			has_opaque_schema_operations: false,
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

	/// Get a mutable model by its physical table name.
	pub fn get_model_by_table_mut(
		&mut self,
		app_label: &str,
		table_name: &str,
	) -> Option<&mut ModelState> {
		self.models.iter_mut().find_map(|((app, _), model)| {
			(app == app_label && model.table_name == table_name).then_some(model)
		})
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

	/// Remove a model from this project state by its physical table name.
	pub fn remove_model_by_table_name(
		&mut self,
		app_label: &str,
		table_name: &str,
	) -> Option<ModelState> {
		let key = self.models.iter().find_map(|((app, model_name), model)| {
			(app == app_label && model.table_name == table_name)
				.then(|| (app.clone(), model_name.clone()))
		})?;
		self.models.remove(&key)
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

	/// Rename a database table while preserving model identities and references.
	pub fn rename_table(&mut self, old_table_name: &str, new_table_name: &str) {
		if let Some(model) = self.find_model_by_table_mut(old_table_name) {
			model.table_name = new_table_name.to_string();
		}
		self.update_foreign_key_table_references(old_table_name, new_table_name);
	}

	/// Rename a database table within an application while preserving model identities.
	pub fn rename_table_in_app(
		&mut self,
		app_label: &str,
		old_table_name: &str,
		new_table_name: &str,
	) {
		if let Some(model) = self
			.models
			.iter_mut()
			.find(|((app, _), model)| app == app_label && model.table_name == old_table_name)
			.map(|(_, model)| model)
		{
			model.table_name = new_table_name.to_string();
		}

		self.update_foreign_key_table_references_in_app(app_label, old_table_name, new_table_name);
	}

	fn update_foreign_key_table_references_in_app(
		&mut self,
		app_label: &str,
		old_table_name: &str,
		new_table_name: &str,
	) {
		for ((model_app_label, _), model) in &mut self.models {
			for field in model.fields.values_mut() {
				let targets_renamed_model = field
					.params
					.get("fk_target_app")
					.map_or(model_app_label == app_label, |target_app| {
						target_app == app_label
					});
				if targets_renamed_model
					&& let Some(foreign_key) = &mut field.foreign_key
					&& foreign_key.referenced_table == old_table_name
				{
					foreign_key.referenced_table = new_table_name.to_string();
				}
			}
			for constraint in &mut model.constraints {
				if let Some(foreign_key) = &mut constraint.foreign_key_info
					&& foreign_key.referenced_table == old_table_name
				{
					foreign_key.referenced_table = new_table_name.to_string();
				}
			}
		}
	}

	fn update_foreign_key_table_references(&mut self, old_table_name: &str, new_table_name: &str) {
		for model in self.models.values_mut() {
			for field in model.fields.values_mut() {
				if let Some(foreign_key) = &mut field.foreign_key
					&& foreign_key.referenced_table == old_table_name
				{
					foreign_key.referenced_table = new_table_name.to_string();
				}
			}
			for constraint in &mut model.constraints {
				if let Some(foreign_key) = &mut constraint.foreign_key_info
					&& foreign_key.referenced_table == old_table_name
				{
					foreign_key.referenced_table = new_table_name.to_string();
				}
			}
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
	/// // state will contain all models registered in the global registry
	/// ```
	pub fn from_global_registry() -> Self {
		use super::model_registry::global_registry;

		Self::from_model_metadata(global_registry().get_models())
	}

	/// Load ProjectState from the global model registry, reporting a poisoned lock.
	pub fn try_from_global_registry() -> super::Result<Self> {
		let models = super::model_registry::global_registry().try_get_models()?;
		Ok(Self::from_model_metadata(models))
	}

	fn from_model_metadata(models_metadata: Vec<super::model_registry::ModelMetadata>) -> Self {
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
	/// - Foreign key to source model: `{source_table}_id` (or
	///   `from_{source_table}_id` for self-referencing M2M)
	/// - Foreign key to target model: `{target_table}_id` (or
	///   `to_{target_table}_id` for self-referencing M2M)
	/// - Foreign key constraints with CASCADE
	/// - Unique constraint on (source_id, target_id)
	fn create_intermediate_table_for_m2m(
		&self,
		source_app_label: &str,
		source_model_name: &str,
		source_table_name: &str,
		m2m: &super::model_registry::ManyToManyMetadata,
	) -> ModelState {
		// Default through-table and column names come from the canonical
		// convention in `crate::m2m_naming` (also re-exported as
		// `crate::migrations::naming`) so this site cannot drift from
		// `detect_created_many_to_many` (lookup) or `ManyToManyAccessor`
		// (runtime). See issue #4665.
		let table_name = m2m.through.clone().unwrap_or_else(|| {
			crate::m2m_naming::default_through_table(source_table_name, &m2m.field_name)
		});

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

		// Determine the primary key type for the source and target from the registry
		let source_pk_type = self.get_primary_key_type(source_app_label, source_model_name);
		// Extract target app_label from to_model (may be in "app.Model" format)
		let (target_app, target_model) =
			self.resolve_model_reference(&m2m.to_model, source_app_label);

		let target_pk_type = self.get_primary_key_type(&target_app, &target_model);

		// Resolve the target table name from ProjectState, falling back to the
		// `app_label`-prefixed snake_case form as a last-resort heuristic.
		let target_table_name = self
			.get_model(&target_app, &target_model)
			.map(|m| m.table_name.clone())
			.unwrap_or_else(|| format!("{}_{}", target_app, to_snake_case(&target_model)));

		// Default FK column names come from the canonical convention in
		// `crate::m2m_naming::default_m2m_columns` (single source of truth,
		// see issue #4665): `{table}_id` for non-self-referential relations,
		// and `from_/to_` prefixes when source and target tables match.
		// Keying off the *actual* table names (not the struct identifiers)
		// matches the ORM accessor fallback in
		// `crates/reinhardt-db/src/orm/many_to_many_accessor.rs` and the
		// on-disk schema produced by initial migrations (#4659).
		let (default_source_col, default_target_col) =
			crate::m2m_naming::default_m2m_columns(source_table_name, &target_table_name);
		let source_field_name = m2m.source_field.clone().unwrap_or(default_source_col);
		let target_field_name = m2m.target_field.clone().unwrap_or(default_target_col);

		// Add foreign key to source model
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

		// Add foreign key to target model
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

	fn resolve_model_reference(&self, reference: &str, current_app: &str) -> (String, String) {
		let parts: Vec<&str> = reference.split('.').collect();
		match parts.as_slice() {
			[app, model] => (app.to_string(), model.to_string()),
			[model] => {
				let model = model.to_string();
				if self.get_model(current_app, &model).is_some() {
					(current_app.to_string(), model)
				} else {
					let app = self
						.models
						.keys()
						.find_map(|(app_label, model_name)| {
							(model_name == &model).then(|| app_label.clone())
						})
						.or_else(|| {
							super::model_registry::global_registry()
								.get_models()
								.iter()
								.find(|metadata| metadata.model_name == model)
								.map(|metadata| metadata.app_label.clone())
						})
						.unwrap_or_else(|| current_app.to_string());
					(app, model)
				}
			}
			_ => (current_app.to_string(), reference.to_string()),
		}
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
	/// - CreateIndex/DropIndex: Tracks model indexes
	/// - Other operations are logged but not applied to state
	pub fn apply_migration_operations(
		&mut self,
		operations: &[super::operations::Operation],
		app_label: &str,
	) {
		use super::operations::Operation;

		for op in operations {
			match op {
				Operation::RunSQL { .. } => {
					self.has_opaque_schema_operations = true;
				}
				Operation::CreateTable {
					name,
					columns,
					constraints,
					..
				} => {
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
					for constraint in constraints {
						model
							.constraints
							.push(Self::constraint_to_definition(constraint));
					}

					self.add_model(model);
				}
				Operation::CreateIndex {
					table,
					columns,
					unique,
					index_type,
					where_clause,
					concurrently,
					expressions,
					mysql_options,
					operator_class,
				} => {
					let name = super::operations::generated_index_name(
						table,
						columns,
						expressions.as_deref(),
					);
					let mut index = IndexDefinition::new(name, columns.clone(), *unique);
					index.where_clause = where_clause.clone();
					#[cfg(feature = "pgvector")]
					{
						index.index_type = *index_type;
						index.expressions = expressions.clone();
						index.operator_class = operator_class.clone();
					}
					#[cfg(not(feature = "pgvector"))]
					let _ = (index_type, expressions, operator_class);
					let is_advanced = where_clause.is_some()
						|| index_type.is_some()
						|| expressions.is_some()
						|| operator_class.is_some();
					let state =
						AdvancedIndexOptionState::new(is_advanced, *concurrently, *mysql_options);
					if let Some(model) = self.find_model_by_table_mut(table) {
						let state_index_name = model
							.indexes
							.iter()
							.find(|existing| {
								index_definitions_equivalent(existing, &index)
									&& model_index_is_advanced(model, existing) == is_advanced
							})
							.map(|existing| existing.name.clone())
							.unwrap_or_else(|| {
								model.indexes.push(index.clone());
								index.name.clone()
							});
						set_advanced_index_option_state(model, &state_index_name, state);
					}
				}
				Operation::CreateIndexRepair {
					table,
					name,
					columns,
					unique,
					index_type,
					where_clause,
					concurrently,
					expressions,
					mysql_options,
					operator_class,
				} => {
					let name = name.clone().unwrap_or_else(|| {
						super::operations::generated_index_name(
							table,
							columns,
							expressions.as_deref(),
						)
					});
					let mut index = IndexDefinition::new(name, columns.clone(), *unique);
					index.where_clause = where_clause.clone();
					#[cfg(feature = "pgvector")]
					{
						index.index_type = *index_type;
						index.expressions = expressions.clone();
						index.operator_class = operator_class.clone();
					}
					#[cfg(not(feature = "pgvector"))]
					let _ = (index_type, expressions, operator_class);
					let is_advanced = where_clause.is_some()
						|| index_type.is_some()
						|| expressions.is_some()
						|| operator_class.is_some();
					let state =
						AdvancedIndexOptionState::new(is_advanced, *concurrently, *mysql_options);
					if let Some(model) = self.find_model_by_table_mut(table) {
						let state_index_name = model
							.indexes
							.iter()
							.find(|existing| {
								index_definitions_equivalent(existing, &index)
									&& model_index_is_advanced(model, existing) == is_advanced
							})
							.map(|existing| existing.name.clone())
							.unwrap_or_else(|| {
								model.indexes.push(index.clone());
								index.name.clone()
							});
						set_advanced_index_option_state(model, &state_index_name, state);
					}
				}
				#[cfg(feature = "pgvector")]
				Operation::CreateNamedIndex {
					table,
					name,
					columns,
					unique,
					index_type,
					where_clause,
					concurrently,
					expressions,
					mysql_options,
					operator_class,
				} => {
					let mut index = IndexDefinition::new(name.clone(), columns.clone(), *unique);
					index.where_clause = where_clause.clone();
					index.index_type = *index_type;
					index.expressions = expressions.clone();
					index.operator_class = operator_class.clone();
					let state = AdvancedIndexOptionState::new(
						where_clause.is_some()
							|| index_type.is_some()
							|| expressions.is_some()
							|| operator_class.is_some(),
						*concurrently,
						*mysql_options,
					);
					if let Some(model) = self.find_model_by_table_mut(table) {
						model.indexes.retain(|existing| existing.name != *name);
						model.indexes.push(index.clone());
						set_advanced_index_option_state(model, &index.name, state);
					}
				}
				Operation::DropIndex { table, columns } => {
					if let Some(model) = self.find_model_by_table_mut(table) {
						let generated_name =
							super::operations::generated_index_name(table, columns, None);
						model.indexes.retain(|index| index.name != generated_name);
						model
							.options
							.remove(&advanced_index_option_key(&generated_name));
					}
				}
				Operation::DropNamedIndex { table, name, .. } => {
					if let Some(model) = self.find_model_by_table_mut(table) {
						model.indexes.retain(|index| index.name != *name);
						model.options.remove(&advanced_index_option_key(name));
					}
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
				Operation::DropColumn { table, column, .. } => {
					// Find the model and remove the field
					if let Some(model) = self.find_model_by_table_mut(table) {
						let removed_names: Vec<_> = model
							.indexes
							.iter()
							.filter(|index| Self::index_definition_references_column(index, column))
							.map(|index| index.name.clone())
							.collect();
						model.fields.remove(column);
						model.indexes.retain(|index| {
							!Self::index_definition_references_column(index, column)
						});
						for name in removed_names {
							model.options.remove(&advanced_index_option_key(&name));
						}
						model.constraints.retain(|constraint| {
							!constraint.fields.iter().any(|field| field == column)
						});
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
					if let Some(model) = self
						.models
						.iter_mut()
						.find(|((app, _), model)| app == app_label && model.table_name == *old_name)
						.map(|(_, model)| model)
					{
						let index_option_renames: Vec<_> = model
							.indexes
							.iter()
							.filter_map(|index| {
								let value = model
									.options
									.get(&advanced_index_option_key(&index.name))?
									.clone();
								let old_default = default_index_name(old_name, &index.fields);
								let old_legacy =
									format!("{}_{}_idx", old_name, index.fields.join("_"));
								let renamed_index_name =
									if index.name == old_default || index.name == old_legacy {
										default_index_name(new_name, &index.fields)
									} else {
										index.name.clone()
									};
								Some((index.name.clone(), renamed_index_name, value))
							})
							.collect();
						for index in &mut model.indexes {
							let old_default = default_index_name(old_name, &index.fields);
							let old_legacy = format!("{}_{}_idx", old_name, index.fields.join("_"));
							if index.name == old_default || index.name == old_legacy {
								index.name = default_index_name(new_name, &index.fields);
							}
						}
						for (old_index_name, new_index_name, value) in index_option_renames {
							model
								.options
								.remove(&advanced_index_option_key(&old_index_name));
							model
								.options
								.insert(advanced_index_option_key(&new_index_name), value);
						}
					}
					self.rename_table_in_app(app_label, old_name, new_name);
				}
				Operation::RenameColumn {
					table,
					old_name,
					new_name,
				} => {
					// Find the model and rename the field
					if let Some(model) = self.find_model_by_table_mut(table) {
						let index_option_renames: Vec<_> = model
							.indexes
							.iter()
							.filter_map(|index| {
								let value = model
									.options
									.get(&advanced_index_option_key(&index.name))?
									.clone();
								let old_fields = index.fields.clone();
								let old_default = default_index_name(table, &old_fields);
								let old_legacy = format!("{}_{}_idx", table, old_fields.join("_"));
								let mut new_fields = old_fields.clone();
								for field in &mut new_fields {
									if field == old_name {
										*field = new_name.clone();
									}
								}
								let new_index_name =
									if index.name == old_default || index.name == old_legacy {
										default_index_name(table, &new_fields)
									} else {
										index.name.clone()
									};
								Some((index.name.clone(), new_index_name, value))
							})
							.collect();
						model.rename_field(old_name, new_name.to_string());
						for index in &mut model.indexes {
							if !index.fields.iter().any(|field| field == old_name) {
								continue;
							}
							let old_fields = index.fields.clone();
							let old_default = default_index_name(table, &old_fields);
							let old_legacy = format!("{}_{}_idx", table, old_fields.join("_"));
							for field in &mut index.fields {
								if field == old_name {
									*field = new_name.to_string();
								}
							}
							if index.name == old_default || index.name == old_legacy {
								index.name = default_index_name(table, &index.fields);
							}
						}
						for (old_index_name, new_index_name, value) in index_option_renames {
							model
								.options
								.remove(&advanced_index_option_key(&old_index_name));
							model
								.options
								.insert(advanced_index_option_key(&new_index_name), value);
						}
						for constraint in &mut model.constraints {
							for field in &mut constraint.fields {
								if field == old_name {
									*field = new_name.to_string();
								}
							}
						}
					}
				}
				Operation::AddConstraint {
					table,
					constraint_sql,
				}
				| Operation::AddConstraintRepair {
					table,
					constraint_sql,
				} => {
					if let Some(model) = self.find_model_by_table_mut(table)
						&& let Some(constraint) =
							Self::constraint_definition_from_sql(constraint_sql)
						&& !model.constraints.iter().any(|c| c.name == constraint.name)
					{
						model.constraints.push(constraint);
					}
				}
				Operation::AddConstraintDefinition { table, constraint } => {
					if let Some(model) = self.find_model_by_table_mut(table) {
						if let super::Constraint::EnumDomain { column, domain, .. } = constraint
							&& let Some(field) = model.fields.get_mut(column)
						{
							field.domain = Some(domain.clone().canonicalized());
						}
						let constraint = Self::constraint_to_definition(constraint);
						if !model
							.constraints
							.iter()
							.any(|existing| existing.name == constraint.name)
						{
							model.constraints.push(constraint);
						}
					}
				}
				Operation::DropConstraint {
					table,
					constraint_name,
				} => {
					if let Some(model) = self.find_model_by_table_mut(table) {
						model
							.constraints
							.retain(|constraint| constraint.name != *constraint_name);
					}
				}
				Operation::DropConstraintDefinition { table, constraint } => {
					if let Some(model) = self.find_model_by_table_mut(table) {
						if let super::Constraint::EnumDomain { column, .. } = constraint
							&& let Some(field) = model.fields.get_mut(column)
						{
							field.domain = None;
						}
						model
							.constraints
							.retain(|definition| definition.name != constraint.name());
					}
				}
				// Other operations don't affect the schema state in ways we track.
				_ => {}
			}
		}
	}

	fn index_definition_references_column(index: &IndexDefinition, column: &str) -> bool {
		index.fields.iter().any(|field| field == column)
			|| index.expressions().is_some_and(|expressions| {
				expressions
					.iter()
					.any(|expression| Self::expression_references_column(expression, column))
			}) || index
			.where_clause
			.as_deref()
			.is_some_and(|where_clause| Self::expression_references_column(where_clause, column))
	}

	fn expression_references_column(expression: &str, column: &str) -> bool {
		let mut token = String::new();
		let mut in_string = false;
		let mut characters = expression.chars().peekable();

		while let Some(character) = characters.next() {
			if character == '\'' {
				in_string = !in_string;
				if !in_string {
					token.clear();
				}
				continue;
			}
			if in_string {
				continue;
			}
			if character.is_ascii_alphanumeric() || character == '_' {
				token.push(character);
			} else if !token.is_empty() {
				let is_function_call = character == '('
					|| (character.is_ascii_whitespace()
						&& characters
							.clone()
							.find(|next| !next.is_ascii_whitespace())
							.is_some_and(|next| next == '('));
				if !is_function_call && token.eq_ignore_ascii_case(column) {
					return true;
				}
				token.clear();
			}
		}

		!token.is_empty() && token.eq_ignore_ascii_case(column)
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

	pub(crate) fn constraint_to_definition(
		constraint: &super::operations::Constraint,
	) -> ConstraintDefinition {
		match constraint {
			super::operations::Constraint::PrimaryKey { name, columns } => ConstraintDefinition {
				name: name.clone(),
				constraint_type: "primary_key".to_string(),
				fields: columns.clone(),
				expression: None,
				foreign_key_info: None,
			},
			super::operations::Constraint::ForeignKey {
				name,
				columns,
				referenced_table,
				referenced_columns,
				on_delete,
				on_update,
				..
			} => ConstraintDefinition {
				name: name.clone(),
				constraint_type: "foreign_key".to_string(),
				fields: columns.clone(),
				expression: None,
				foreign_key_info: Some(ForeignKeyConstraintInfo {
					referenced_table: referenced_table.clone(),
					referenced_columns: referenced_columns.clone(),
					on_delete: *on_delete,
					on_update: *on_update,
				}),
			},
			super::operations::Constraint::Unique { name, columns } => ConstraintDefinition {
				name: name.clone(),
				constraint_type: "unique".to_string(),
				fields: columns.clone(),
				expression: None,
				foreign_key_info: None,
			},
			super::operations::Constraint::Check { name, expression } => ConstraintDefinition {
				name: name.clone(),
				constraint_type: "check".to_string(),
				fields: Vec::new(),
				expression: Some(expression.clone()),
				foreign_key_info: None,
			},
			super::operations::Constraint::EnumDomain {
				name,
				column,
				domain,
			} => ConstraintDefinition::enum_domain(name.clone(), column.clone(), domain.clone()),
			super::operations::Constraint::OneToOne {
				name,
				column,
				referenced_table,
				referenced_column,
				on_delete,
				on_update,
				..
			} => ConstraintDefinition {
				name: name.clone(),
				constraint_type: "one_to_one".to_string(),
				fields: vec![column.clone()],
				expression: None,
				foreign_key_info: Some(ForeignKeyConstraintInfo {
					referenced_table: referenced_table.clone(),
					referenced_columns: vec![referenced_column.clone()],
					on_delete: *on_delete,
					on_update: *on_update,
				}),
			},
			super::operations::Constraint::ManyToMany {
				name,
				source_column,
				target_column,
				..
			} => ConstraintDefinition {
				name: name.clone(),
				constraint_type: "many_to_many".to_string(),
				fields: vec![source_column.clone(), target_column.clone()],
				expression: None,
				foreign_key_info: None,
			},
			super::operations::Constraint::Exclude { name, elements, .. } => ConstraintDefinition {
				name: name.clone(),
				constraint_type: "exclude".to_string(),
				fields: elements.iter().map(|(field, _)| field.clone()).collect(),
				expression: None,
				foreign_key_info: None,
			},
		}
	}

	fn trim_sql_identifier(identifier: &str) -> String {
		let trimmed = identifier.trim();
		let Some(quote) = trimmed.chars().next() else {
			return String::new();
		};
		let stripped = match quote {
			'"' => trimmed
				.strip_prefix('"')
				.and_then(|value| value.strip_suffix('"')),
			'`' => trimmed
				.strip_prefix('`')
				.and_then(|value| value.strip_suffix('`')),
			'\'' => trimmed
				.strip_prefix('\'')
				.and_then(|value| value.strip_suffix('\'')),
			_ => None,
		};
		if let Some(stripped) = stripped {
			stripped.replace(&format!("{quote}{quote}"), &quote.to_string())
		} else {
			trimmed.to_string()
		}
	}

	fn parse_constraint_identifier_list(identifier_list: &str) -> Vec<String> {
		let mut identifiers = Vec::new();
		let mut current = String::new();
		let mut quote = None;
		let mut depth = 0usize;
		let mut chars = identifier_list.chars().peekable();

		while let Some(character) = chars.next() {
			if let Some(quote_char) = quote {
				current.push(character);
				if character == quote_char {
					if chars.peek() == Some(&quote_char) {
						current.push(chars.next().expect("peeked quote must exist"));
					} else {
						quote = None;
					}
				}
				continue;
			}

			match character {
				'\'' | '"' | '`' => {
					quote = Some(character);
					current.push(character);
				}
				'(' => {
					depth += 1;
					current.push(character);
				}
				')' => {
					depth = depth.saturating_sub(1);
					current.push(character);
				}
				',' if depth == 0 => {
					let identifier = Self::trim_sql_identifier(&current);
					if !identifier.is_empty() {
						identifiers.push(identifier);
					}
					current.clear();
				}
				_ => current.push(character),
			}
		}

		let identifier = Self::trim_sql_identifier(&current);
		if !identifier.is_empty() {
			identifiers.push(identifier);
		}
		identifiers
	}

	fn extract_sql_parenthesized_expression(sql: &str, open: usize) -> Option<(&str, usize)> {
		if !sql.is_char_boundary(open) || sql[open..].chars().next()? != '(' {
			return None;
		}

		let mut depth = 0usize;
		let mut quote = None;
		let mut chars = sql[open..].char_indices().peekable();
		while let Some((relative_index, character)) = chars.next() {
			let index = open + relative_index;
			if let Some(quote_char) = quote {
				if character == quote_char {
					if chars.peek().is_some_and(|(_, next)| *next == quote_char) {
						chars.next();
					} else {
						quote = None;
					}
				}
				continue;
			}

			match character {
				'\'' | '"' | '`' => quote = Some(character),
				'(' => depth += 1,
				')' => {
					depth = depth.checked_sub(1)?;
					if depth == 0 {
						return Some((&sql[open + 1..index], index));
					}
				}
				_ => {}
			}
		}

		None
	}

	fn foreign_key_action_from_clause(clauses: &str, clause: &str) -> Option<ForeignKeyAction> {
		let upper_clauses = clauses.to_ascii_uppercase();
		let tail = upper_clauses.split_once(clause)?.1.trim_start();
		if tail.starts_with("SET NULL") {
			Some(ForeignKeyAction::SetNull)
		} else if tail.starts_with("SET DEFAULT") {
			Some(ForeignKeyAction::SetDefault)
		} else if tail.starts_with("NO ACTION") {
			Some(ForeignKeyAction::NoAction)
		} else if tail.starts_with("CASCADE") {
			Some(ForeignKeyAction::Cascade)
		} else if tail.starts_with("RESTRICT") {
			Some(ForeignKeyAction::Restrict)
		} else {
			None
		}
	}

	fn constraint_definition_from_sql(constraint_sql: &str) -> Option<ConstraintDefinition> {
		let rest = constraint_sql.trim().strip_prefix("CONSTRAINT ")?;
		let (name, body) = rest.split_once(' ')?;
		let body = body.trim();
		let upper_body = body.to_ascii_uppercase();
		if upper_body.starts_with("UNIQUE (") {
			let open = body.find('(')?;
			let (identifier_list, _) = Self::extract_sql_parenthesized_expression(body, open)?;
			let fields = Self::parse_constraint_identifier_list(identifier_list);
			if fields.is_empty() {
				return None;
			}
			return Some(ConstraintDefinition {
				name: name.trim_matches('"').to_string(),
				constraint_type: "unique".to_string(),
				fields,
				expression: None,
				foreign_key_info: None,
			});
		}
		if upper_body.starts_with("CHECK (") {
			let open = body.find('(')?;
			let close = body.rfind(')')?;
			return Some(ConstraintDefinition {
				name: name.trim_matches('"').to_string(),
				constraint_type: "check".to_string(),
				fields: Vec::new(),
				expression: Some(body[open + 1..close].trim().to_string()),
				foreign_key_info: None,
			});
		}
		if upper_body.starts_with("FOREIGN KEY") {
			let open = body.find('(')?;
			let (identifier_list, close) = Self::extract_sql_parenthesized_expression(body, open)?;
			let fields = Self::parse_constraint_identifier_list(identifier_list);
			if fields.is_empty() {
				return None;
			}

			let after_fields = body[close + 1..].trim_start();
			if !after_fields.to_ascii_uppercase().starts_with("REFERENCES") {
				return None;
			}
			let after_references = after_fields["REFERENCES".len()..].trim_start();
			let referenced_open = after_references.find('(')?;
			let referenced_table = Self::trim_sql_identifier(&after_references[..referenced_open]);
			if referenced_table.is_empty() {
				return None;
			}

			let (referenced_identifier_list, referenced_close) =
				Self::extract_sql_parenthesized_expression(after_references, referenced_open)?;
			let referenced_columns =
				Self::parse_constraint_identifier_list(referenced_identifier_list);
			if referenced_columns.is_empty() {
				return None;
			}

			let clauses = &after_references[referenced_close + 1..];
			return Some(ConstraintDefinition {
				name: name.trim_matches('"').to_string(),
				constraint_type: "foreign_key".to_string(),
				fields,
				expression: None,
				foreign_key_info: Some(ForeignKeyConstraintInfo {
					referenced_table,
					referenced_columns,
					on_delete: Self::foreign_key_action_from_clause(clauses, "ON DELETE")
						.unwrap_or(ForeignKeyAction::NoAction),
					on_update: Self::foreign_key_action_from_clause(clauses, "ON UPDATE")
						.unwrap_or(ForeignKeyAction::NoAction),
				}),
			});
		}
		None
	}

	/// Helper: Convert ColumnDefinition to FieldState
	fn column_def_to_field_state(&self, col: &super::operations::ColumnDefinition) -> FieldState {
		let (mut params, default) =
			super::operations::decode_file_field_metadata(col.default.as_deref());

		if col.primary_key {
			params.insert("primary_key".to_string(), "true".to_string());
		}
		if col.auto_increment {
			params.insert("auto_increment".to_string(), "true".to_string());
		}
		if col.unique {
			params.insert("unique".to_string(), "true".to_string());
		}
		if let Some(default) = default {
			params.insert("default".to_string(), default);
		}

		FieldState {
			name: col.name.to_string(),
			field_type: col.type_definition.clone(),
			nullable: !col.not_null,
			params,
			generated: col.generated.clone(),
			domain: col.domain.clone(),
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
#[non_exhaustive]
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
/// // Add a new model to to_state
/// let mut model = ModelState::new("myapp", "User");
/// model.add_field(FieldState::new("id", FieldType::Integer, false));
/// to_state.add_model(model);
///
/// let detector = MigrationAutodetector::new(from_state, to_state);
/// let changes = detector.detect_changes();
///
/// // Should detect the new model creation
/// assert_eq!(changes.created_models.len(), 1);
/// ```
pub struct MigrationAutodetector {
	from_state: ProjectState,
	to_state: ProjectState,
	similarity_config: SimilarityConfig,
}

/// A structured warning produced while comparing migration states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutodetectorWarning {
	/// Existing rows may not satisfy the replacement enum-domain constraint.
	EnumDomainDataMigrationRequired {
		/// Database table containing the enum-backed column.
		table: String,
		/// Resolved database column name.
		column: String,
		/// Domain enforced before the migration.
		old_domain: crate::field_domain::FieldDomain,
		/// Domain enforced after the migration.
		new_domain: crate::field_domain::FieldDomain,
	},
}

/// Migration generation output including actionable schema-change warnings.
#[derive(Debug, Clone)]
pub struct GeneratedMigrations {
	/// Generated migrations grouped by application.
	pub migrations: Vec<super::Migration>,
	/// Warnings discovered while comparing the migration states.
	pub warnings: Vec<AutodetectorWarning>,
}

impl std::fmt::Display for AutodetectorWarning {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::EnumDomainDataMigrationRequired {
				table,
				column,
				old_domain,
				new_domain,
			} => {
				let crate::field_domain::FieldDomain::Enum {
					values: old_values, ..
				} = old_domain;
				let crate::field_domain::FieldDomain::Enum {
					values: new_values, ..
				} = new_domain;
				let removed_values = old_values
					.iter()
					.filter(|value| !new_values.contains(value))
					.map(|value| match value {
						crate::field_domain::ModelEnumValue::String(value) => value.clone(),
						crate::field_domain::ModelEnumValue::I32(value) => value.to_string(),
					})
					.collect::<Vec<_>>()
					.join(", ");
				write!(
					formatter,
					"enum domain change for {table}.{column} removes or re-encodes values [{removed_values}]; place a data migration before the new constraint"
				)
			}
		}
	}
}

/// Type alias for moved model information:
/// (from_app, from_model, to_app, to_model, rename_table, old_table, new_table)
type MovedModelInfo = (
	String,
	String,
	String,
	String,
	bool,
	Option<String>,
	Option<String>,
);

/// Operations needed to preserve a ManyToMany through table when convention-
/// derived table or column names change.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ManyToManyArtifactRename {
	old_table: String,
	old_source_column: String,
	old_target_column: String,
	new_table: String,
	new_source_column: String,
	new_target_column: String,
}

/// Type alias for model match result: ((deleted_app, deleted_model), (created_app, created_model), similarity_score)
type ModelMatchResult = ((String, String), (String, String), f64);

/// Detected changes between two project states
#[derive(Debug, Clone, Default)]
pub struct DetectedChanges {
	/// Actionable warnings discovered while comparing schema metadata.
	pub warnings: Vec<AutodetectorWarning>,
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
	/// Tables renamed without changing model identity: (app_label, model_name, old_table, new_table)
	pub renamed_tables: Vec<(String, String, String, String)>,
	/// Models that were moved between apps: (from_app, from_model, to_app, to_model, rename_table, old_table, new_table)
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
	/// Composite primary keys added: (app_label, model_name, ConstraintDefinition)
	pub added_composite_primary_keys: Vec<(String, String, ConstraintDefinition)>,
	/// Composite primary keys removed due to modification (same name, different fields): (app_label, model_name, constraint_name)
	pub removed_composite_primary_keys: Vec<(String, String, String)>,
	/// Auto-increment sequence resets: (app_label, model_name, column_name, value)
	pub auto_increment_resets: Vec<(String, String, String, i64)>,
	/// Model dependencies for ordering operations
	/// Maps (app_label, model_name) -> `Vec<(dependent_app, dependent_model)>`
	/// A model depends on another if it has ForeignKey or ManyToMany fields pointing to it
	pub model_dependencies: std::collections::BTreeMap<(String, String), Vec<(String, String)>>,
	/// ManyToMany intermediate tables that were created
	/// Contains (app_label, source_model, through_table, ManyToManyMetadata)
	pub created_many_to_many: Vec<(String, String, String, ManyToManyMetadata)>,
}

/// Deterministic topological sort of `(app_label, model_name)` keys.
///
/// Only `nodes` participate in the graph. Edges that point at models outside
/// that set (already-existing tables, or models belonging to another
/// migration) are ignored so they cannot inject extra CreateTable operations
/// or leave a created model with a non-zero in-degree that never drains.
///
/// Ready nodes are stored in a `BTreeSet` so equal-in-degree ties break
/// lexicographically. Cycles fall back to lexicographic order of the
/// remaining nodes after a warning, rather than `HashSet` iteration order.
fn topological_sort_model_keys(
	nodes: &[(String, String)],
	dependencies: &std::collections::BTreeMap<(String, String), Vec<(String, String)>>,
) -> Vec<(String, String)> {
	use std::collections::{BTreeMap, BTreeSet};

	let node_set: BTreeSet<(String, String)> = nodes.iter().cloned().collect();
	let mut in_degree: BTreeMap<(String, String), usize> =
		node_set.iter().cloned().map(|node| (node, 0)).collect();
	let mut dependents: BTreeMap<(String, String), BTreeSet<(String, String)>> = BTreeMap::new();

	for (dependent, deps) in dependencies {
		if !node_set.contains(dependent) {
			continue;
		}
		for dependency in deps {
			if dependent == dependency || !node_set.contains(dependency) {
				continue;
			}
			*in_degree.entry(dependent.clone()).or_insert(0) += 1;
			dependents
				.entry(dependency.clone())
				.or_default()
				.insert(dependent.clone());
		}
	}

	let mut ready: BTreeSet<(String, String)> = in_degree
		.iter()
		.filter(|(_, degree)| **degree == 0)
		.map(|(node, _)| node.clone())
		.collect();
	let mut ordered = Vec::with_capacity(node_set.len());

	while let Some(node) = ready.iter().next().cloned() {
		ready.remove(&node);
		ordered.push(node.clone());
		if let Some(children) = dependents.get(&node) {
			for child in children {
				if let Some(degree) = in_degree.get_mut(child) {
					*degree = degree.saturating_sub(1);
					if *degree == 0 {
						ready.insert(child.clone());
					}
				}
			}
		}
	}

	if ordered.len() < node_set.len() {
		let mut remaining: Vec<(String, String)> = node_set
			.into_iter()
			.filter(|node| !ordered.contains(node))
			.collect();
		remaining.sort();
		eprintln!(
			"⚠️  Warning: Circular dependency detected in models: [{}]",
			remaining
				.iter()
				.map(|(app, name)| format!("{app}.{name}"))
				.collect::<Vec<_>>()
				.join(", ")
		);
		eprintln!(
			"    Falling back to lexicographic order for remaining models. Migration operations may need manual reordering."
		);
		ordered.extend(remaining);
	}

	ordered
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
		let mut nodes = self.created_models.clone();
		for moved in &self.moved_models {
			nodes.push((moved.2.clone(), moved.3.clone()));
		}
		topological_sort_model_keys(&nodes, &self.model_dependencies)
	}

	/// Order `created_models` by foreign-key dependencies.
	///
	/// Only models that are themselves being created participate in the graph.
	/// References to already-existing (or cross-app) tables do not inject extra
	/// CreateTable operations and do not delay the dependent model.
	pub fn order_created_models_by_dependency(&self) -> Vec<(String, String)> {
		topological_sort_model_keys(&self.created_models, &self.model_dependencies)
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
					// MovedModelInfo is (from_app, from_model, to_app, to_model, rename_table, old_table, new_table)
					self.moved_models.retain(|info| {
						!(&info.0 == from_app
							&& &info.2 == to_app && (&info.1 == model_name || &info.3 == model_name))
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

		patterns.sort_by_key(|pattern| std::cmp::Reverse(pattern.frequency));
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
		/// The source model name pattern.
		from_pattern: String,
		/// The target model name pattern.
		to_pattern: String,
	},
	/// Model move pattern
	ModelMove {
		/// The application label pattern.
		app_pattern: String,
	},
	/// Field addition pattern
	FieldAddition {
		/// The field name pattern.
		field_name_pattern: String,
	},
	/// Field rename pattern
	FieldRename {
		/// The source field name pattern.
		from_pattern: String,
		/// The target field name pattern.
		to_pattern: String,
	},
	/// Multiple model renames
	MultipleModelRenames {
		/// The minimum count of renames.
		min_count: usize,
	},
	/// Multiple field additions
	MultipleFieldAdditions {
		/// The model name pattern.
		model_pattern: String,
		/// The minimum count of additions.
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
		/// The app label.
		app_label: String,
		/// The old name.
		old_name: String,
		/// The new name.
		new_name: String,
	},
	/// Reference to a moved model: (from_app, to_app, model_name)
	MovedModel {
		/// The from app.
		from_app: String,
		/// The to app.
		to_app: String,
		/// The model name.
		model_name: String,
	},
	/// Reference to an added field: (app_label, model_name, field_name)
	AddedField {
		/// The app label.
		app_label: String,
		/// The model name.
		model_name: String,
		/// The field name.
		field_name: String,
	},
	/// Reference to a renamed field: (app_label, model_name, old_name, new_name)
	RenamedField {
		/// The app label.
		app_label: String,
		/// The model name.
		model_name: String,
		/// The old name.
		old_name: String,
		/// The new name.
		new_name: String,
	},
	/// Reference to a removed field: (app_label, model_name, field_name)
	RemovedField {
		/// The app label.
		app_label: String,
		/// The model name.
		model_name: String,
		/// The field name.
		field_name: String,
	},
	/// Reference to an altered field: (app_label, model_name, field_name)
	AlteredField {
		/// The app label.
		app_label: String,
		/// The model name.
		model_name: String,
		/// The field name.
		field_name: String,
	},
	/// Reference to a created model: (app_label, model_name)
	CreatedModel {
		/// The app label.
		app_label: String,
		/// The model name.
		model_name: String,
	},
	/// Reference to a deleted model: (app_label, model_name)
	DeletedModel {
		/// The app label.
		app_label: String,
		/// The model name.
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
					RuleCondition::MultipleModelRenames { min_count }
						if model_renames.len() >= *min_count =>
					{
						optional_matches += 1;
						evidence.push(format!("Multiple renames: {}", model_renames.len()));
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
		intents.sort_by(|a, b| {
			b.confidence
				.partial_cmp(&a.confidence)
				.unwrap_or(std::cmp::Ordering::Equal)
		});

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
			.map(|(from_app, from_model, to_app, to_model, _, _, _)| {
				(
					from_app.clone(),
					from_model.clone(),
					to_app.clone(),
					to_model.clone(),
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
					for (from_app, _from_model, to_app, to_model, _, _, _) in &changes.moved_models
					{
						intent.related_operations.push(OperationRef::MovedModel {
							from_app: from_app.clone(),
							to_app: to_app.clone(),
							model_name: to_model.clone(),
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
	/// // Add a new model
	/// let model = ModelState::new("myapp", "User");
	/// to_state.add_model(model);
	///
	/// let detector = MigrationAutodetector::new(from_state, to_state);
	/// let changes = detector.detect_changes();
	///
	/// assert_eq!(changes.created_models.len(), 1);
	/// ```
	pub fn detect_changes(&self) -> DetectedChanges {
		self.detect_changes_internal(false)
			.expect("non-strict autodetection must not fail")
	}

	/// Detect changes and fail when a compatible field rename is ambiguous.
	///
	/// This is the safer entry point for `makemigrations`: when the
	/// autodetector sees add/drop candidates that could be field renames, it
	/// must either emit `RenameColumn` for one-to-one compatible pairs or stop
	/// instead of silently generating destructive add/drop operations.
	pub fn try_detect_changes(&self) -> super::Result<DetectedChanges> {
		self.to_state.validate_physical_index_names()?;
		self.detect_changes_internal(true)
	}

	/// Validate physical table-name collisions caused by model or table renames.
	///
	/// This performs only the model-level detection needed to find rename
	/// destinations. It intentionally does not inspect field changes, allowing
	/// callers that operate on one app to validate cross-app table ownership
	/// without applying field-rename ambiguity checks to unrelated apps.
	pub fn validate_table_rename_destinations(&self) -> super::Result<()> {
		let mut changes = DetectedChanges::default();

		self.detect_created_models(&mut changes);
		self.detect_deleted_models(&mut changes);
		self.detect_renamed_models(&mut changes);
		self.detect_renamed_tables(&mut changes);
		self.detect_deleted_rename_target_owners(&mut changes);
		self.validate_rename_destinations(&changes)
	}

	fn detect_changes_internal(
		&self,
		strict_rename_ambiguity: bool,
	) -> super::Result<DetectedChanges> {
		let mut changes = DetectedChanges::default();

		// Detect model-level changes
		self.detect_created_models(&mut changes);
		self.detect_deleted_models(&mut changes);
		self.detect_renamed_models(&mut changes);
		self.detect_renamed_tables(&mut changes);
		self.detect_deleted_rename_target_owners(&mut changes);
		if strict_rename_ambiguity {
			self.validate_rename_destinations(&changes)?;
		}

		// Detect field-level changes (only for models that exist in both states)
		self.detect_added_fields(&mut changes);
		self.detect_removed_fields(&mut changes);
		self.detect_altered_fields(&mut changes);
		self.detect_enum_domain_warnings(&mut changes);
		self.detect_renamed_fields(&mut changes, strict_rename_ambiguity)?;

		// Detect index and constraint changes
		self.detect_added_indexes(&mut changes);
		self.detect_removed_indexes(&mut changes);
		self.detect_added_constraints(&mut changes);
		self.detect_removed_constraints(&mut changes);
		self.detect_composite_pk_changes(&mut changes);
		self.detect_auto_increment_resets(&mut changes);

		// Detect ManyToMany intermediate tables
		self.detect_created_many_to_many(&mut changes);

		// Detect model dependencies for operation ordering
		self.detect_model_dependencies(&mut changes);

		// Order newly created models by foreign-key dependencies so CreateTable
		// operations are emitted with referenced tables first. Lexicographic
		// sort is the wrong default here: `auth_api_keys` sorts before
		// `auth_users` even when the former has an inline FK to the latter.
		let created_set: std::collections::BTreeSet<_> =
			changes.created_models.iter().cloned().collect();
		changes.created_models = changes
			.order_created_models_by_dependency()
			.into_iter()
			.filter(|model| created_set.contains(model))
			.collect();

		// Sort remaining change lists for deterministic output
		changes.deleted_models.sort();
		changes.added_fields.sort();
		changes.removed_fields.sort();
		changes.altered_fields.sort();
		changes.renamed_models.sort();
		changes.renamed_tables.sort();
		changes.renamed_fields.sort();
		changes.warnings.sort_by(|left, right| match (left, right) {
			(
				AutodetectorWarning::EnumDomainDataMigrationRequired {
					table: left_table,
					column: left_column,
					..
				},
				AutodetectorWarning::EnumDomainDataMigrationRequired {
					table: right_table,
					column: right_column,
					..
				},
			) => (left_table, left_column).cmp(&(right_table, right_column)),
		});

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
			.added_composite_primary_keys
			.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
		changes.removed_composite_primary_keys.sort();
		changes.auto_increment_resets.sort();
		changes
			.created_many_to_many
			.sort_by(|a, b| (&a.0, &a.1, &a.2).cmp(&(&b.0, &b.1, &b.2)));

		Ok(changes)
	}

	fn validate_rename_destinations(&self, changes: &DetectedChanges) -> super::Result<()> {
		let mut renamed_destinations = BTreeSet::new();
		for (app_label, _model_name, _old_name, new_name) in &changes.renamed_tables {
			renamed_destinations.insert((app_label.clone(), new_name.clone()));
		}
		for (app_label, _old_model_name, new_model_name) in &changes.renamed_models {
			if let Some(model) = self.to_state.get_model(app_label, new_model_name) {
				renamed_destinations.insert((app_label.clone(), model.table_name.clone()));
			}
		}
		for (_from_app, _from_model, to_app, to_model, rename_table, _old_table, new_table) in
			&changes.moved_models
		{
			if !rename_table {
				continue;
			}
			let new_table_name = new_table.clone().or_else(|| {
				self.to_state
					.get_model(to_app, to_model)
					.map(|model| model.table_name.clone())
			});
			if let Some(new_table_name) = new_table_name {
				renamed_destinations.insert((to_app.clone(), new_table_name));
			}
		}
		for (_app_label, table_name) in renamed_destinations {
			let claims = self
				.to_state
				.models
				.values()
				.filter(|model| model.table_name == table_name)
				.count();
			if claims > 1 {
				return Err(super::MigrationError::InvalidMigration(format!(
					"cannot rename a table to `{table_name}` because multiple target models claim that table name"
				)));
			}
		}
		Ok(())
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
			// Only check models that exist in both states. Model renames are
			// resolved through `changes.renamed_models` so simultaneous
			// RenameTable + AddColumn cases are preserved.
			if let Some(from_model) =
				self.matching_from_model_for_to_model(app_label, model_name, to_model, changes)
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
			// Only check models that exist in both states. Model renames are
			// resolved through `changes.renamed_models` so simultaneous
			// RenameTable + DropColumn cases are preserved.
			if let Some(to_model) =
				self.matching_to_model_for_from_model(app_label, model_name, from_model, changes)
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
			// Only check models that exist in both states. Model renames are
			// resolved through `changes.renamed_models` so simultaneous
			// RenameTable + AlterColumn cases are preserved.
			if let Some(from_model) =
				self.matching_from_model_for_to_model(app_label, model_name, to_model, changes)
			{
				for (field_name, to_field) in &to_model.fields {
					if let Some(from_field) = from_model.fields.get(field_name) {
						// Check if field definition has changed
						if self.has_field_changed_in_model_context(
							field_name, from_model, to_model, from_field, to_field,
						) {
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

	fn detect_enum_domain_warnings(&self, changes: &mut DetectedChanges) {
		for ((app_label, model_name), to_model) in &self.to_state.models {
			let Some(from_model) =
				self.matching_from_model_for_to_model(app_label, model_name, to_model, changes)
			else {
				continue;
			};
			for (column, to_field) in &to_model.fields {
				let Some(from_field) = from_model.fields.get(column) else {
					continue;
				};
				let (old_domain, new_domain) = match (&from_field.domain, &to_field.domain) {
					(Some(old_domain), Some(new_domain)) => (
						old_domain.clone().canonicalized(),
						new_domain.clone().canonicalized(),
					),
					(None, Some(new_domain @ crate::field_domain::FieldDomain::Enum { .. })) => {
						let new_domain = new_domain.clone().canonicalized();
						changes.warnings.push(
							AutodetectorWarning::EnumDomainDataMigrationRequired {
								table: to_model.table_name.clone(),
								column: column.clone(),
								old_domain: new_domain.clone(),
								new_domain,
							},
						);
						continue;
					}
					_ => continue,
				};
				if old_domain == new_domain {
					continue;
				}
				let crate::field_domain::FieldDomain::Enum {
					repr: old_repr,
					values: old_values,
				} = &old_domain;
				let crate::field_domain::FieldDomain::Enum {
					repr: new_repr,
					values: new_values,
				} = &new_domain;
				if old_repr != new_repr
					|| old_values.iter().any(|value| !new_values.contains(value))
				{
					changes
						.warnings
						.push(AutodetectorWarning::EnumDomainDataMigrationRequired {
							table: to_model.table_name.clone(),
							column: column.clone(),
							old_domain,
							new_domain,
						});
				}
			}
		}
	}

	fn matching_from_model_for_to_model<'a>(
		&'a self,
		app_label: &str,
		to_model_name: &str,
		to_model: &ModelState,
		changes: &DetectedChanges,
	) -> Option<&'a ModelState> {
		if changes
			.renamed_tables
			.iter()
			.any(|(app, model, _old_table, new_table)| {
				app == app_label && model == to_model_name && new_table == &to_model.table_name
			}) {
			return self.from_state.get_model(app_label, to_model_name);
		}
		if let Some((_app, old_name, _new_name)) = changes
			.renamed_models
			.iter()
			.find(|(app, _old_name, new_name)| app == app_label && new_name == to_model_name)
		{
			return self.from_state.get_model(app_label, old_name);
		}
		self.from_state
			.get_model_by_table_name(app_label, &to_model.table_name)
			.or_else(|| {
				changes
					.moved_models
					.iter()
					.find(|(_from_app, _from_model, to_app, to_model, _, _, _)| {
						to_app == app_label && to_model == to_model_name
					})
					.and_then(|(from_app, from_model, _to_app, _to_model, _, _, _)| {
						self.from_state.get_model(from_app, from_model)
					})
			})
	}

	fn matching_to_model_for_from_model<'a>(
		&'a self,
		app_label: &str,
		from_model_name: &str,
		from_model: &ModelState,
		changes: &DetectedChanges,
	) -> Option<&'a ModelState> {
		if changes
			.renamed_tables
			.iter()
			.any(|(app, model, old_table, _new_table)| {
				app == app_label && model == from_model_name && old_table == &from_model.table_name
			}) {
			return self.to_state.get_model(app_label, from_model_name);
		}
		self.to_state
			.get_model_by_table_name(app_label, &from_model.table_name)
			.or_else(|| {
				changes
					.renamed_models
					.iter()
					.find(|(app, old_name, _new_name)| {
						app == app_label && old_name == from_model_name
					})
					.and_then(|(_app, _old_name, new_name)| {
						self.to_state.get_model(app_label, new_name)
					})
			})
			.or_else(|| {
				changes
					.moved_models
					.iter()
					.find(|(from_app, from_model, _to_app, _to_model, _, _, _)| {
						from_app == app_label && from_model == from_model_name
					})
					.and_then(|(_from_app, _from_model, to_app, to_model, _, _, _)| {
						self.to_state.get_model(to_app, to_model)
					})
			})
	}

	fn has_field_changed_in_model_context(
		&self,
		field_name: &str,
		from_model: &ModelState,
		to_model: &ModelState,
		from_field: &FieldState,
		to_field: &FieldState,
	) -> bool {
		let from_constraint_managed =
			Self::single_field_unique_constraint_present(from_model, field_name);
		let to_constraint_managed =
			Self::single_field_unique_constraint_present(to_model, field_name);
		let constraint_managed = from_constraint_managed || to_constraint_managed;
		let from_inline_unique = Self::field_has_inline_unique(from_model, field_name);
		let to_inline_unique = Self::field_has_inline_unique(to_model, field_name);
		let from_unique = Some(if constraint_managed {
			from_inline_unique && !to_constraint_managed
		} else {
			Self::single_field_unique_column_already_present(from_model, field_name)
		});
		let to_unique = Some(if constraint_managed {
			to_inline_unique && !from_constraint_managed
		} else {
			Self::single_field_unique_column_already_present(to_model, field_name)
		});
		self.has_field_changed_with_unique(field_name, from_field, to_field, from_unique, to_unique)
	}

	fn field_has_inline_unique(model: &ModelState, field_name: &str) -> bool {
		model
			.fields
			.get(field_name)
			.and_then(|field| field.params.get("unique"))
			.map(String::as_str)
			== Some("true")
	}

	fn has_field_changed_with_unique(
		&self,
		field_name: &str,
		from_field: &FieldState,
		to_field: &FieldState,
		from_unique: Option<bool>,
		to_unique: Option<bool>,
	) -> bool {
		// Schema-affecting bits are compared via the canonical
		// `ColumnDefinition` form to absorb asymmetric param populations.
		let from_def = Self::normalized_column_definition(field_name, from_field, from_unique);
		let to_def = Self::normalized_column_definition(field_name, to_field, to_unique);
		// File fields persist only a logical path, so changes to their upload
		// policy must remain visible to migration-state comparison even when the
		// bounded character column itself is unchanged. PostgreSQL's physical
		// `storage` parameter is only encoded for file fields; generic fields do
		// not persist that parameter in `ColumnDefinition`.
		let is_file_field = from_field
			.params
			.get("model_field_type")
			.map(String::as_str)
			.is_some_and(|value| matches!(value, "file" | "image"))
			|| to_field
				.params
				.get("model_field_type")
				.map(String::as_str)
				.is_some_and(|value| matches!(value, "file" | "image"));
		let semantic_param_changed = [
			"model_field_type",
			"upload_to",
			"file_storage",
			"cleanup",
			"max_width",
			"max_height",
		]
		.iter()
		.any(|key| {
			if *key == "cleanup" && is_file_field {
				from_field.params.get(*key).map_or("false", String::as_str)
					!= to_field.params.get(*key).map_or("false", String::as_str)
			} else {
				from_field.params.get(*key) != to_field.params.get(*key)
			}
		}) || (is_file_field
			&& from_field.params.get("storage") != to_field.params.get("storage"));
		from_def.type_definition != to_def.type_definition
			|| from_def.not_null != to_def.not_null
			|| from_def.primary_key != to_def.primary_key
			|| from_def.auto_increment != to_def.auto_increment
			|| from_def.unique != to_def.unique
			|| from_def.default != to_def.default
			|| from_def.generated != to_def.generated
			|| semantic_param_changed
	}

	fn field_change_touches_generated(
		field_name: &str,
		from_field: &FieldState,
		to_field: &FieldState,
		from_unique: Option<bool>,
		to_unique: Option<bool>,
	) -> bool {
		let from_def = Self::normalized_column_definition(field_name, from_field, from_unique);
		let to_def = Self::normalized_column_definition(field_name, to_field, to_unique);
		from_def.generated != to_def.generated
	}

	fn emit_generated_column_replacement_dependencies(
		operations: &mut Vec<super::Operation>,
		from_model: &ModelState,
		to_model: &ModelState,
		affected_columns: &BTreeSet<String>,
	) {
		for index in to_model.indexes.iter().filter(|index| {
			index
				.fields
				.iter()
				.any(|field| affected_columns.contains(field))
				&& from_model
					.indexes
					.iter()
					.any(|from_index| from_index == *index)
		}) {
			let state = advanced_index_option_state(to_model, index)
				.or_else(|| {
					from_model
						.indexes
						.iter()
						.find(|from_index| *from_index == index)
						.and_then(|from_index| advanced_index_option_state(from_model, from_index))
				})
				.unwrap_or_default();
			operations.push(index.create_named_operation_with_state(&to_model.table_name, state));
		}

		for constraint in to_model.constraints.iter().filter(|constraint| {
			Self::constraint_references_any_column(constraint, affected_columns)
				&& from_model
					.constraints
					.iter()
					.any(|from_constraint| from_constraint == *constraint)
		}) {
			operations.push(super::Operation::AddConstraintRepair {
				table: to_model.table_name.clone(),
				constraint_sql: constraint.to_constraint().to_string(),
			});
		}
	}

	fn emit_generated_column_replacement_dependency_rollbacks(
		operations: &mut Vec<super::Operation>,
		from_model: &ModelState,
		to_model: &ModelState,
		affected_columns: &BTreeSet<String>,
	) {
		for index in from_model.indexes.iter().filter(|index| {
			index
				.fields
				.iter()
				.any(|field| affected_columns.contains(field))
				&& to_model.indexes.iter().any(|to_index| to_index == *index)
		}) {
			let state = advanced_index_option_state(from_model, index).unwrap_or_default();
			operations.push(super::Operation::RestoreIndexOnRollback {
				table: from_model.table_name.clone(),
				name: Some(index.name.clone()),
				columns: index.fields.clone(),
				unique: index.unique,
				index_type: index.index_type(),
				where_clause: index.where_clause.clone(),
				concurrently: state.concurrently,
				expressions: index.expressions().cloned(),
				mysql_options: state.mysql_options,
				operator_class: index.operator_class().cloned(),
			});
		}

		for constraint in from_model.constraints.iter().filter(|constraint| {
			Self::constraint_references_any_column(constraint, affected_columns)
				&& to_model
					.constraints
					.iter()
					.any(|to_constraint| to_constraint == *constraint)
		}) {
			operations.push(super::Operation::RestoreConstraintOnRollback {
				table: from_model.table_name.clone(),
				constraint_sql: constraint.to_constraint().to_string(),
			});
		}
	}

	fn constraint_references_any_column(
		constraint: &ConstraintDefinition,
		columns: &BTreeSet<String>,
	) -> bool {
		constraint
			.fields
			.iter()
			.any(|field| columns.contains(field))
			|| constraint.expression.as_ref().is_some_and(|expression| {
				columns
					.iter()
					.any(|column| Self::expression_text_references_column(expression, column))
			})
	}

	fn generated_replacement_fields_for_model(
		generated_replacement_columns: &BTreeSet<(String, String, String)>,
		app_label: &str,
		model_name: &str,
	) -> BTreeSet<String> {
		generated_replacement_columns
			.iter()
			.filter(|(app, model, _)| app == app_label && model == model_name)
			.map(|(_, _, field)| field.clone())
			.collect()
	}

	fn generated_column_replacement_dependents(
		from_model: &ModelState,
		to_model: &ModelState,
		field_name: &str,
	) -> Vec<GeneratedColumnReplacementDependent> {
		let mut affected_columns = BTreeSet::from([field_name.to_string()]);
		let mut dependents = Vec::new();

		loop {
			let mut added = false;
			for (dependent_name, from_field) in &from_model.fields {
				if affected_columns.contains(dependent_name) {
					continue;
				}

				let references_affected_column =
					from_field.generated.as_ref().is_some_and(|generated| {
						affected_columns.iter().any(|column| {
							Self::generated_column_references_column(generated, column)
						})
					});
				let to_field = to_model.get_field(dependent_name);
				let to_references_affected_column = to_field.is_some_and(|to_field| {
					to_field.generated.as_ref().is_some_and(|generated| {
						affected_columns.iter().any(|column| {
							Self::generated_column_references_column(generated, column)
						})
					})
				});

				if references_affected_column || to_references_affected_column {
					affected_columns.insert(dependent_name.clone());
					dependents.push(GeneratedColumnReplacementDependent {
						name: dependent_name.clone(),
						old_definition: super::ColumnDefinition::from_field_state(
							dependent_name.clone(),
							from_field,
						),
						new_definition: to_field.map(|to_field| {
							super::ColumnDefinition::from_field_state(
								dependent_name.clone(),
								to_field,
							)
						}),
					});
					added = true;
				}
			}

			if !added {
				break;
			}
		}

		dependents
	}

	fn generated_field_references_column(
		from_field: &FieldState,
		to_field: &FieldState,
		column: &str,
	) -> bool {
		from_field
			.generated
			.as_ref()
			.is_some_and(|generated| Self::generated_column_references_column(generated, column))
			|| to_field.generated.as_ref().is_some_and(|generated| {
				Self::generated_column_references_column(generated, column)
			})
	}

	fn generated_column_references_column(
		generated: &super::GeneratedColumnDefinition,
		column: &str,
	) -> bool {
		generated
			.typed_expr()
			.as_ref()
			.is_some_and(|expr| Self::schema_expr_references_column(expr, column))
			|| generated
				.raw_sql
				.as_deref()
				.is_some_and(|sql| Self::expression_text_references_column(sql, column))
			|| generated
				.expr_tokens
				.as_deref()
				.is_some_and(|tokens| Self::expression_text_references_column(tokens, column))
	}

	fn schema_expr_references_column(expr: &super::SchemaExpr, column: &str) -> bool {
		match expr {
			super::SchemaExpr::Column(identifier) => identifier.to_string() == column,
			super::SchemaExpr::Value(_) => false,
			super::SchemaExpr::Binary { left, right, .. } => {
				Self::schema_expr_references_column(left, column)
					|| Self::schema_expr_references_column(right, column)
			}
			super::SchemaExpr::Function { args, .. } => args
				.iter()
				.any(|arg| Self::schema_expr_references_column(arg, column)),
			super::SchemaExpr::Cast { expr, .. } => {
				Self::schema_expr_references_column(expr, column)
			}
			_ => false,
		}
	}

	fn expression_text_references_column(text: &str, column: &str) -> bool {
		text.split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
			.any(|token| token.eq_ignore_ascii_case(column))
	}

	fn normalized_column_definition(
		field_name: &str,
		field: &FieldState,
		unique: Option<bool>,
	) -> super::ColumnDefinition {
		let mut normalized_field = field.clone();
		if normalized_field
			.params
			.get("model_field_type")
			.is_some_and(|value| matches!(value.as_str(), "file" | "image"))
		{
			normalized_field
				.params
				.entry("cleanup".to_owned())
				.or_insert_with(|| "false".to_owned());
		}
		let mut def = super::ColumnDefinition::from_field_state(field_name, &normalized_field);
		def.auto_increment =
			Self::canonical_auto_increment(&def.type_definition, def.auto_increment);
		if let Some(unique) = unique {
			def.unique = unique;
		}
		def
	}

	fn canonical_auto_increment(field_type: &super::FieldType, auto_increment: bool) -> bool {
		auto_increment
			&& matches!(
				field_type,
				super::FieldType::BigInteger
					| super::FieldType::Integer
					| super::FieldType::SmallInteger
					| super::FieldType::TinyInt
					| super::FieldType::MediumInt
			)
	}

	fn order_added_fields_by_generated_dependencies(
		&self,
		added_fields: &[(String, String, String)],
	) -> Vec<(String, String, String)> {
		let mut remaining = added_fields.to_vec();
		let mut ordered = Vec::with_capacity(remaining.len());

		while !remaining.is_empty() {
			let next_index = remaining
				.iter()
				.enumerate()
				.find(|(_, (app_label, model_name, field_name))| {
					let Some(model) = self.to_state.get_model(app_label, model_name) else {
						return true;
					};
					let Some(field) = model.get_field(field_name) else {
						return true;
					};
					let Some(generated) = field.generated.as_ref() else {
						return true;
					};

					!remaining.iter().any(
						|(other_app_label, other_model_name, other_field_name)| {
							other_app_label == app_label
								&& other_model_name == model_name
								&& other_field_name != field_name
								&& Self::generated_column_references_column(
									generated,
									other_field_name,
								)
						},
					)
				})
				.map(|(index, _)| index);

			if let Some(index) = next_index {
				ordered.push(remaining.remove(index));
			} else {
				ordered.extend(remaining);
				break;
			}
		}

		ordered
	}

	fn order_model_fields_by_generated_dependencies(
		model: &ModelState,
	) -> Vec<(&String, &FieldState)> {
		let mut remaining: Vec<_> = model.fields.iter().collect();
		let mut ordered = Vec::with_capacity(remaining.len());

		while !remaining.is_empty() {
			let next_index = remaining
				.iter()
				.enumerate()
				.find(|(_, (field_name, field))| {
					let Some(generated) = field.generated.as_ref() else {
						return true;
					};

					!remaining.iter().any(|(other_field_name, _)| {
						other_field_name.as_str() != field_name.as_str()
							&& Self::generated_column_references_column(
								generated,
								other_field_name.as_str(),
							)
					})
				})
				.map(|(index, _)| index);

			if let Some(index) = next_index {
				ordered.push(remaining.remove(index));
			} else {
				ordered.extend(remaining);
				break;
			}
		}

		ordered
	}

	fn added_generated_field_references_altered_source(
		&self,
		altered_fields: &[(String, String, String)],
		renamed_fields: &[(String, String, String, String)],
		app_label: &str,
		model_name: &str,
		field_name: &str,
	) -> bool {
		let Some(model) = self.to_state.get_model(app_label, model_name) else {
			return false;
		};
		let Some(field) = model.get_field(field_name) else {
			return false;
		};
		let Some(generated) = field.generated.as_ref() else {
			return false;
		};

		let references_altered_source =
			altered_fields
				.iter()
				.any(|(other_app_label, other_model_name, other_field_name)| {
					other_app_label == app_label
						&& other_model_name == model_name
						&& other_field_name != field_name
						&& Self::generated_column_references_column(generated, other_field_name)
				});
		let references_renamed_source = renamed_fields.iter().any(
			|(other_app_label, other_model_name, _old_name, new_name)| {
				other_app_label == app_label
					&& other_model_name == model_name
					&& new_name != field_name
					&& Self::generated_column_references_column(generated, new_name)
			},
		);

		references_altered_source || references_renamed_source
	}

	fn push_add_column_operation(
		&self,
		by_app: &mut std::collections::BTreeMap<String, Vec<super::Operation>>,
		app_label: &str,
		model_name: &str,
		field_name: &str,
	) {
		if let Some(model) = self.to_state.get_model(app_label, model_name)
			&& let Some(field) = model.get_field(field_name)
		{
			by_app
				.entry(app_label.to_string())
				.or_default()
				.push(super::Operation::AddColumn {
					table: model.table_name.clone(),
					column: super::ColumnDefinition::from_field_state(
						field_name.to_string(),
						field,
					),
					mysql_options: None,
				});
		}
	}

	fn order_removed_fields_by_generated_dependencies(
		&self,
		removed_fields: &[(String, String, String)],
	) -> Vec<(String, String, String)> {
		let mut remaining = removed_fields.to_vec();
		let mut ordered = Vec::with_capacity(remaining.len());

		while !remaining.is_empty() {
			let next_index = remaining
				.iter()
				.enumerate()
				.find(|(_, (app_label, model_name, field_name))| {
					!remaining.iter().any(
						|(other_app_label, other_model_name, other_field_name)| {
							if other_app_label != app_label
								|| other_model_name != model_name
								|| other_field_name == field_name
							{
								return false;
							}
							self.from_state
								.get_model(other_app_label, other_model_name)
								.and_then(|model| model.get_field(other_field_name))
								.and_then(|field| field.generated.as_ref())
								.is_some_and(|generated| {
									Self::generated_column_references_column(generated, field_name)
								})
						},
					)
				})
				.map(|(index, _)| index);

			if let Some(index) = next_index {
				ordered.push(remaining.remove(index));
			} else {
				ordered.extend(remaining);
				break;
			}
		}

		ordered
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
	/// // With high field similarity, should detect as rename
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
				let app_label = deleted_key.0.clone();
				let old_model_name = deleted_key.1.clone();
				let new_model_name = created_key.1.clone();
				// Same app: check if table names actually differ
				// Struct-only renames (same table name) are not schema changes
				let old_table = self
					.from_state
					.get_model(&app_label, &old_model_name)
					.map(|m| m.table_name.as_str());
				let new_table = self
					.to_state
					.get_model(&app_label, &new_model_name)
					.map(|m| m.table_name.as_str());

				if old_table != new_table {
					changes.renamed_models.push((
						app_label.clone(),
						old_model_name.clone(),
						new_model_name.clone(),
					));
					changes
						.created_models
						.retain(|(app, model)| !(app == &app_label && model == &new_model_name));
					changes
						.deleted_models
						.retain(|(app, model)| !(app == &app_label && model == &old_model_name));
				}
			} else {
				let from_app = deleted_key.0.clone();
				let to_app = created_key.0.clone();
				let model_name = created_key.1.clone();
				let deleted_model_name = deleted_key.1.clone();
				// Different apps: this is a move operation
				// Determine if table needs to be renamed
				let old_table = self
					.from_state
					.get_model(&from_app, &deleted_model_name)
					.map(|model| model.table_name.clone())
					.unwrap_or_else(|| {
						format!("{}_{}", from_app, deleted_model_name.to_lowercase())
					});
				let new_table = self
					.to_state
					.get_model(&to_app, &model_name)
					.map(|model| model.table_name.clone())
					.unwrap_or_else(|| format!("{}_{}", to_app, model_name.to_lowercase()));
				let rename_table = old_table != new_table;

				changes.moved_models.push((
					from_app.clone(),
					deleted_model_name.clone(),
					to_app.clone(),
					model_name.clone(),
					rename_table,
					if rename_table { Some(old_table) } else { None },
					if rename_table { Some(new_table) } else { None },
				));
				changes
					.created_models
					.retain(|(app, model)| !(app == &to_app && model == &model_name));
				changes
					.deleted_models
					.retain(|(app, model)| !(app == &from_app && model == &deleted_model_name));
			}
		}
	}

	/// Detect table-name changes for models whose app label and model name are unchanged.
	fn detect_renamed_tables(&self, changes: &mut DetectedChanges) {
		for ((app_label, model_name), to_model) in &self.to_state.models {
			let Some(from_model) = self.from_state.get_model(app_label, model_name) else {
				continue;
			};
			if from_model.table_name == to_model.table_name {
				continue;
			}

			changes.renamed_tables.push((
				app_label.clone(),
				model_name.clone(),
				from_model.table_name.clone(),
				to_model.table_name.clone(),
			));
			changes
				.created_models
				.retain(|(app, model)| !(app == app_label && model == model_name));
			changes
				.deleted_models
				.retain(|(app, model)| !(app == app_label && model == model_name));

			if let Some((reused_app, reused_model)) = self
				.to_state
				.get_model_by_table_name(app_label, &from_model.table_name)
				.map(|model| (app_label.to_string(), model.name.clone()))
				.filter(|(_, reused_model)| reused_model != model_name)
			{
				let reused_model_exists_in_from_state = self
					.from_state
					.get_model(&reused_app, &reused_model)
					.is_some();
				let reused_model_is_renamed =
					changes
						.renamed_models
						.iter()
						.any(|(app, _old_model, new_model)| {
							app == &reused_app && new_model == &reused_model
						});
				let reused_model_is_moved = changes.moved_models.iter().any(
					|(_from_app, _from_model, to_app, to_model, _, _, _)| {
						to_app == &reused_app && to_model == &reused_model
					},
				);
				if !reused_model_exists_in_from_state
					&& !reused_model_is_renamed
					&& !reused_model_is_moved
				{
					changes.created_models.push((reused_app, reused_model));
				}
			}
		}
	}

	/// Detect models deleted while owning a table newly claimed by a rename.
	///
	/// Table-name based deletion detection intentionally retains a source model
	/// when its table exists in the target state. A same-app table rename can
	/// otherwise mask a distinct deleted model that owns the rename target.
	fn detect_deleted_rename_target_owners(&self, changes: &mut DetectedChanges) {
		let mut renamed_table_targets: Vec<(String, String, String)> = changes
			.renamed_tables
			.iter()
			.map(|(app_label, model_name, _old_name, new_name)| {
				(app_label.clone(), model_name.clone(), new_name.clone())
			})
			.collect();
		for (app_label, old_model_name, new_model_name) in &changes.renamed_models {
			let Some(new_table_name) = self
				.to_state
				.get_model(app_label, new_model_name)
				.map(|model| model.table_name.clone())
			else {
				continue;
			};
			renamed_table_targets.push((app_label.clone(), old_model_name.clone(), new_table_name));
		}
		for (_from_app, _from_model, to_app, to_model, _rename_table, _old_table, new_table) in
			&changes.moved_models
		{
			let Some(new_table_name) = new_table.clone().or_else(|| {
				self.to_state
					.get_model(to_app, to_model)
					.map(|model| model.table_name.clone())
			}) else {
				continue;
			};
			renamed_table_targets.push((to_app.clone(), to_model.clone(), new_table_name));
		}

		for (app_label, renamed_model_name, new_name) in renamed_table_targets {
			for ((owner_app_label, owner_model_name), owner_model) in &self.from_state.models {
				if owner_app_label != &app_label
					|| owner_model_name == &renamed_model_name
					|| owner_model.table_name != new_name
					|| self
						.to_state
						.get_model(owner_app_label, owner_model_name)
						.is_some()
				{
					continue;
				}
				if !changes
					.deleted_models
					.contains(&(owner_app_label.clone(), owner_model_name.clone()))
				{
					changes
						.deleted_models
						.push((owner_app_label.clone(), owner_model_name.clone()));
				}
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
	/// // With matching type, might detect as rename
	/// assert!(changes.renamed_fields.len() <= 1);
	/// ```
	fn detect_renamed_fields(
		&self,
		changes: &mut DetectedChanges,
		strict_rename_ambiguity: bool,
	) -> super::Result<()> {
		let mut confirmed_renames = Vec::new();
		let mut ambiguous_groups = Vec::new();

		for ((app_label, model_name), to_model) in &self.to_state.models {
			let Some(from_model) =
				self.matching_from_model_for_to_model(app_label, model_name, to_model, changes)
			else {
				continue;
			};

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

			if removed_fields.is_empty() || added_fields.is_empty() {
				continue;
			}

			let mut old_to_new: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
			let mut new_to_old: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

			for (removed_name, removed_field) in &removed_fields {
				for (added_name, added_field) in &added_fields {
					if Self::field_definitions_match_for_rename(
						removed_name,
						removed_field,
						added_name,
						added_field,
						Self::single_field_unique_column_already_present(from_model, removed_name),
						Self::single_field_unique_column_already_present(to_model, added_name),
					) {
						old_to_new
							.entry((*removed_name).clone())
							.or_default()
							.insert((*added_name).clone());
						new_to_old
							.entry((*added_name).clone())
							.or_default()
							.insert((*removed_name).clone());
					}
				}
			}

			if old_to_new.is_empty() {
				continue;
			}

			for (old_name, new_names) in &old_to_new {
				if new_names.len() == 1 {
					let new_name = new_names.iter().next().expect("one candidate");
					if new_to_old
						.get(new_name)
						.is_some_and(|old_names| old_names.len() == 1)
					{
						confirmed_renames.push((
							app_label.clone(),
							model_name.clone(),
							from_model.name.clone(),
							to_model.table_name.clone(),
							old_name.clone(),
							new_name.clone(),
						));
						continue;
					}
				}

				ambiguous_groups.push(format!(
					"{}.{} (table {}): old [{}] -> new [{}]",
					app_label,
					model_name,
					to_model.table_name,
					old_name,
					new_names.iter().cloned().collect::<Vec<_>>().join(", ")
				));
			}

			for (new_name, old_names) in &new_to_old {
				if old_names.len() > 1 {
					ambiguous_groups.push(format!(
						"{}.{} (table {}): old [{}] -> new [{}]",
						app_label,
						model_name,
						to_model.table_name,
						old_names.iter().cloned().collect::<Vec<_>>().join(", "),
						new_name
					));
				}
			}
		}

		ambiguous_groups.sort();
		ambiguous_groups.dedup();
		if strict_rename_ambiguity && !ambiguous_groups.is_empty() {
			return Err(super::MigrationError::InvalidMigration(format!(
				"Ambiguous field rename candidates detected. \
				 Reinhardt will not emit destructive AddColumn + DropColumn operations for \
				 rename-like changes. Split the change or make the rename intent explicit. \
				 Candidates: {}",
				ambiguous_groups.join("; ")
			)));
		}

		for (app_label, model_name, from_model_name, table_name, old_name, new_name) in
			confirmed_renames
		{
			changes.renamed_fields.push((
				app_label.clone(),
				model_name.clone(),
				old_name.clone(),
				new_name.clone(),
			));
			changes.added_fields.retain(|(app, model, field)| {
				!(app == &app_label && model == &model_name && field == &new_name)
			});
			changes.removed_fields.retain(|(app, model, field)| {
				!(app == &app_label
					&& field == &old_name
					&& (model == &from_model_name
						|| self
							.from_state
							.get_model(app, model)
							.is_some_and(|from_model| from_model.table_name == table_name)))
			});
			changes.altered_fields.retain(|(app, model, field)| {
				!(app == &app_label && model == &model_name && field == &new_name)
			});
		}

		Ok(())
	}

	fn field_definitions_match_for_rename(
		from_name: &str,
		from_field: &FieldState,
		to_name: &str,
		to_field: &FieldState,
		from_unique: bool,
		to_unique: bool,
	) -> bool {
		if from_field.field_type != to_field.field_type
			|| from_field.nullable != to_field.nullable
			|| from_field.foreign_key != to_field.foreign_key
		{
			return false;
		}

		let mut from_def = Self::normalized_column_definition(from_name, from_field, None);
		let mut to_def = Self::normalized_column_definition(to_name, to_field, None);
		from_def.name = "__renamed_field__".to_string();
		to_def.name = "__renamed_field__".to_string();
		from_def.unique = from_unique;
		to_def.unique = to_unique;
		from_def == to_def
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
			if let Some(from_model) =
				self.matching_from_model_for_to_model(app_label, model_name, to_model, changes)
			{
				for to_index in &to_model.indexes {
					// Check if this index exists in from_model
					if !from_model.indexes.iter().any(|idx| {
						model_index_definitions_equivalent(from_model, idx, to_model, to_index)
					}) {
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
			if let Some(to_model) =
				self.matching_to_model_for_from_model(app_label, model_name, from_model, changes)
			{
				for from_index in &from_model.indexes {
					// Check if this index still exists in to_model
					if !to_model.indexes.iter().any(|idx| {
						model_index_definitions_equivalent(from_model, from_index, to_model, idx)
					}) {
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
	/// A single-field UNIQUE constraint on the to-side is treated as
	/// already-present on the from-side when any of the following is true:
	///
	/// 1. From-state has a constraint with the same name (legacy behaviour).
	/// 2. From-state has any single-field UNIQUE constraint covering the same
	///    column — same semantics, different name. This handles the
	///    DB-introspection case where SQLite auto-generates names like
	///    `sqlite_autoindex_users_1`, which never match the to-state's
	///    `{table}_{field}_uniq`.
	/// 3. From-state has a `FieldState` for that column with
	///    `params["unique"] == "true"`. This handles the file-based
	///    reconstruction path: `apply_migration_operations` translates
	///    `ColumnDefinition.unique = true` into an inline field param but
	///    never synthesises a peer `ConstraintDefinition`, while
	///    `ModelMetadata::to_model_state()` on the to-side does synthesise
	///    one. Without this branch the autodetector keeps emitting a
	///    redundant `AddConstraint` every time `makemigrations` runs against
	///    a model whose `#[field(unique = true)]` column already shipped in
	///    `0001_initial.rs` (see reinhardt-web#4448).
	///
	/// # Django Reference
	/// From: django/db/migrations/autodetector.py:1700-1800
	fn detect_added_constraints(&self, changes: &mut DetectedChanges) {
		for ((app_label, model_name), to_model) in &self.to_state.models {
			if let Some(from_model) =
				self.matching_from_model_for_to_model(app_label, model_name, to_model, changes)
			{
				for to_constraint in &to_model.constraints {
					if from_model.constraints.iter().any(|c| {
						c.name == to_constraint.name
							&& Self::constraint_definitions_match(c, to_constraint)
					}) {
						continue;
					}
					if Self::single_field_unique_already_present(to_constraint, from_model) {
						continue;
					}
					if Self::added_single_field_unique_preserved_by_rename(
						changes,
						app_label,
						model_name,
						to_constraint,
						from_model,
					) {
						continue;
					}
					changes.added_constraints.push((
						app_label.clone(),
						model_name.clone(),
						to_constraint.clone(),
					));
				}
			}
		}
	}

	/// Detect removed constraints
	///
	/// Symmetric to [`Self::detect_added_constraints`]: a from-side
	/// single-field UNIQUE constraint is NOT reported as removed when the
	/// to-side carries an equivalent shape — either another single-field
	/// UNIQUE constraint over the same column, or a `FieldState` for that
	/// column with `params["unique"] == "true"`. Without this guard the
	/// asymmetric shape-match in `detect_added_constraints` would simply move
	/// the redundancy from `AddConstraint` into a spurious `DropConstraint`
	/// when the column was originally introduced as a separately-named
	/// UNIQUE constraint but is now declared via inline `#[field(unique =
	/// true)]` (or vice versa). See reinhardt-web#4448.
	///
	/// # Django Reference
	/// From: django/db/migrations/autodetector.py:1800-1900
	fn detect_removed_constraints(&self, changes: &mut DetectedChanges) {
		for ((app_label, model_name), from_model) in &self.from_state.models {
			if let Some(to_model) =
				self.matching_to_model_for_from_model(app_label, model_name, from_model, changes)
			{
				for from_constraint in &from_model.constraints {
					if to_model.constraints.iter().any(|c| {
						c.name == from_constraint.name
							&& Self::constraint_definitions_match(c, from_constraint)
					}) {
						continue;
					}
					if Self::single_field_unique_already_present(from_constraint, to_model) {
						continue;
					}
					if Self::removed_single_field_unique_preserved_by_rename(
						changes,
						app_label,
						model_name,
						to_model,
						from_constraint,
					) {
						continue;
					}
					changes.removed_constraints.push((
						app_label.clone(),
						model_name.clone(),
						from_constraint.name.clone(),
					));
				}
			}
		}
	}

	fn constraint_definitions_match(
		left: &ConstraintDefinition,
		right: &ConstraintDefinition,
	) -> bool {
		left.constraint_type
			.eq_ignore_ascii_case(&right.constraint_type)
			&& left.fields == right.fields
			&& left.expression == right.expression
			&& left.foreign_key_info == right.foreign_key_info
	}

	fn renamed_single_field_unique_constraints(
		from_model: &ModelState,
		to_model: &ModelState,
	) -> Vec<(ConstraintDefinition, ConstraintDefinition)> {
		from_model
			.constraints
			.iter()
			.filter(|from_constraint| is_single_field_unique(from_constraint))
			.filter_map(|from_constraint| {
				to_model
					.constraints
					.iter()
					.find(|to_constraint| {
						to_constraint.name != from_constraint.name
							&& is_single_field_unique(to_constraint)
							&& Self::constraint_definitions_match(from_constraint, to_constraint)
					})
					.map(|to_constraint| (from_constraint.clone(), to_constraint.clone()))
			})
			.collect()
	}

	/// Returns true when `candidate` is a single-field UNIQUE constraint and
	/// the same column on `other_side` is already covered by either:
	/// - any single-field UNIQUE constraint over the same column, or
	/// - a field whose `params["unique"] == "true"`.
	///
	/// Used by both `detect_added_constraints` and
	/// `detect_removed_constraints` to recognise inline `column.unique = true`
	/// and a separately-named single-field `UNIQUE` constraint as
	/// semantically identical, so a name mismatch alone does not trigger a
	/// redundant `AddConstraint` / `DropConstraint` (reinhardt-web#4448).
	fn single_field_unique_already_present(
		candidate: &ConstraintDefinition,
		other_side: &ModelState,
	) -> bool {
		if !is_single_field_unique(candidate) {
			return false;
		}
		let column = &candidate.fields[0];
		Self::single_field_unique_column_already_present(other_side, column)
	}

	fn single_field_unique_column_already_present(model: &ModelState, column: &str) -> bool {
		if Self::single_field_unique_constraint_present(model, column) {
			return true;
		}
		model
			.fields
			.get(column)
			.and_then(|f| f.params.get("unique"))
			.map(String::as_str)
			== Some("true")
	}

	fn single_field_unique_constraint_present(model: &ModelState, column: &str) -> bool {
		model
			.constraints
			.iter()
			.any(|constraint| is_single_field_unique(constraint) && constraint.fields[0] == column)
	}

	fn added_single_field_unique_preserved_by_rename(
		changes: &DetectedChanges,
		app_label: &str,
		model_name: &str,
		to_constraint: &ConstraintDefinition,
		from_model: &ModelState,
	) -> bool {
		if !is_single_field_unique(to_constraint) {
			return false;
		}
		let new_column = &to_constraint.fields[0];
		changes.renamed_fields.iter().any(|(app, model, old, new)| {
			app == app_label
				&& model == model_name
				&& new == new_column
				&& !Self::single_field_unique_constraint_present(from_model, old)
				&& Self::single_field_unique_column_already_present(from_model, old)
		})
	}

	fn removed_single_field_unique_preserved_by_rename(
		changes: &DetectedChanges,
		app_label: &str,
		from_model_name: &str,
		to_model: &ModelState,
		from_constraint: &ConstraintDefinition,
	) -> bool {
		if !is_single_field_unique(from_constraint) {
			return false;
		}
		let old_column = &from_constraint.fields[0];
		changes.renamed_fields.iter().any(|(app, model, old, new)| {
			app == app_label
				&& (model == from_model_name || model == &to_model.name)
				&& old == old_column
				&& !Self::single_field_unique_constraint_present(to_model, new)
				&& Self::single_field_unique_column_already_present(to_model, new)
		})
	}

	/// Final-pass dedup: drop redundant single-column `AddConstraint UNIQUE`
	/// operations whose column is already declared unique elsewhere in the
	/// same migration.
	///
	/// This is the second of two layers that protect against the bug in
	/// reinhardt-web#4448. The primary fix is `detect_added_constraints`'s
	/// shape-match, which compares from-state and to-state. This pass
	/// inspects the *generated* operation list and is the safety net for
	/// any future codepath that produces both an `AddColumn { column.unique
	/// = true }` and a peer `AddConstraint` for the same single column —
	/// for example, a column being added in the same migration as the
	/// model registry synthesises its `{table}_{field}_uniq`
	/// constraint.
	///
	/// Coverage rules (per `(table, column)`):
	/// - `Operation::CreateTable { name, columns, constraints }` —
	///   any column with `unique = true` or any `Constraint::Unique` over a
	///   single column counts the column as already unique.
	/// - `Operation::AddColumn { table, column }` — `column.unique = true`
	///   counts.
	/// - A previously-emitted `Operation::AddConstraint` whose SQL is a
	///   single-column UNIQUE on the same column also counts, so duplicate
	///   `AddConstraint`s for the same column in the same op list collapse
	///   to one.
	///
	/// Multi-column UNIQUE (`unique_together`) is intentionally not touched
	/// — its semantics differ from a single-column UNIQUE.
	fn dedup_redundant_unique_add_constraints(
		by_app: &mut std::collections::BTreeMap<String, Vec<super::Operation>>,
	) {
		use std::collections::HashSet;

		for operations in by_app.values_mut() {
			// (table, column) pairs already known to be UNIQUE in this migration.
			let mut covered: HashSet<(String, String)> = HashSet::new();
			let mut keep = Vec::with_capacity(operations.len());
			for op in operations.drain(..) {
				match &op {
					super::Operation::CreateTable {
						name,
						columns,
						constraints,
						..
					} => {
						for col in columns {
							if col.unique {
								covered.insert((name.clone(), col.name.clone()));
							}
						}
						for c in constraints {
							if let super::operations::Constraint::Unique { columns, .. } = c
								&& columns.len() == 1
							{
								covered.insert((name.clone(), columns[0].clone()));
							}
						}
						keep.push(op);
					}
					super::Operation::AddColumn { table, column, .. } => {
						if column.unique {
							covered.insert((table.clone(), column.name.clone()));
						}
						keep.push(op);
					}
					super::Operation::AddConstraint {
						table,
						constraint_sql,
					} => {
						if let Some(col) = parse_single_column_unique(constraint_sql) {
							let key = (table.clone(), col.to_string());
							if covered.contains(&key) {
								// Redundant — drop it.
								continue;
							}
							covered.insert(key);
						}
						keep.push(op);
					}
					_ => keep.push(op),
				}
			}
			*operations = keep;
		}
	}

	/// Detect added and modified composite primary keys (2+ columns).
	///
	/// A composite PK is represented as a `ConstraintDefinition` with
	/// `constraint_type == "primary_key"` and `fields.len() >= 2`.
	///
	/// Three cases are handled:
	/// - Added: no constraint with the same name existed in from_state → emit CreateCompositePrimaryKey
	/// - Modified: same constraint name exists but fields differ → emit DropConstraint + CreateCompositePrimaryKey
	/// - Unchanged: same constraint name and identical fields → no operation
	fn detect_composite_pk_changes(&self, changes: &mut DetectedChanges) {
		for ((app_label, model_name), to_model) in &self.to_state.models {
			let from_model =
				self.matching_from_model_for_to_model(app_label, model_name, to_model, changes);
			for constraint in &to_model.constraints {
				if constraint.constraint_type != "primary_key" || constraint.fields.len() < 2 {
					continue;
				}
				let from_pk = from_model
					.and_then(|m| m.constraints.iter().find(|c| c.name == constraint.name));
				match from_pk {
					Some(existing) if existing.fields == constraint.fields => {
						// Unchanged — no operation needed
					}
					Some(_) => {
						// Modified (same name, different fields) — drop old then create new
						changes.removed_composite_primary_keys.push((
							app_label.clone(),
							model_name.clone(),
							constraint.name.clone(),
						));
						changes.added_composite_primary_keys.push((
							app_label.clone(),
							model_name.clone(),
							constraint.clone(),
						));
					}
					None => {
						// Added — create new
						changes.added_composite_primary_keys.push((
							app_label.clone(),
							model_name.clone(),
							constraint.clone(),
						));
					}
				}
			}
		}
	}

	/// Detect auto-increment sequence resets driven by `sequence_reset` model option.
	///
	/// When `ModelState.options["sequence_reset"]` is added or changed, emit a
	/// `SetAutoIncrementValue` operation targeting the model's auto-increment column.
	fn detect_auto_increment_resets(&self, changes: &mut DetectedChanges) {
		for ((app_label, model_name), to_model) in &self.to_state.models {
			let Some(value_str) = to_model.options.get("sequence_reset") else {
				continue;
			};
			let from_value = self
				.from_state
				.get_model(app_label, model_name)
				.and_then(|m| m.options.get("sequence_reset"))
				.map(String::as_str);
			if from_value == Some(value_str.as_str()) {
				continue;
			}
			let Ok(value) = value_str.parse::<i64>() else {
				eprintln!(
					"Invalid sequence_reset value for {}.{}: {:?}. Expected an integer.",
					app_label, model_name, value_str
				);
				continue;
			};
			let Some(column) = to_model
				.fields
				.iter()
				.find(|(_, f)| f.params.get("auto_increment").is_some_and(|v| v == "true"))
				.map(|(name, _)| name.clone())
			else {
				continue;
			};
			changes.auto_increment_resets.push((
				app_label.clone(),
				model_name.clone(),
				column,
				value,
			));
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
		// Resolve the source table name from to_state. The source model is
		// guaranteed to be in to_state because this function is called from
		// `generate_operations` while iterating `to_state.models`.
		let source_table = self
			.to_state
			.get_model(app_label, model_name)
			.map(|m| m.table_name.clone())
			.unwrap_or_else(|| {
				format!("{}_{}", to_snake_case(app_label), to_snake_case(model_name))
			});

		// Parse target model to get its app and table name
		let (target_app, target_model) = self.parse_model_reference(to_model, app_label)?;
		let target_table = self
			.to_state
			.get_model(&target_app, &target_model)
			.map(|m| m.table_name.clone())
			.or_else(|| {
				super::model_registry::global_registry()
					.get_models()
					.iter()
					.find(|m| m.app_label == target_app && m.model_name == target_model)
					.map(|m| m.table_name.clone())
			})
			.unwrap_or_else(|| format!("{}_{}", target_app, to_snake_case(&target_model)));

		// Generate through-table name: prefer explicit `through`, otherwise
		// derive from the source table name (matches
		// `create_intermediate_table_for_m2m` and the ORM accessor's
		// `default_through_table`, which both lowercase the source table
		// before composition — see #4659).
		let table_name = if let Some(custom_name) = through_table {
			custom_name.clone()
		} else {
			format!(
				"{}_{}",
				source_table.to_lowercase(),
				to_snake_case(field_name)
			)
		};

		// Derive column names from the resolved *table* names so the
		// autodetector matches the ORM accessor convention
		// (`format!("{}_id", T::table_name().to_lowercase())` in
		// `crates/reinhardt-db/src/orm/many_to_many_accessor.rs`). Compare by
		// table identity for self-reference (#4659 follow-up).
		let source_table_lower = source_table.to_lowercase();
		let target_table_lower = target_table.to_lowercase();
		let (source_column, target_column) = if source_table_lower == target_table_lower {
			(
				format!("from_{}_id", source_table_lower),
				format!("to_{}_id", target_table_lower),
			)
		} else {
			(
				format!("{}_id", source_table_lower),
				format!("{}_id", target_table_lower),
			)
		};

		// Resolve real PK types on both sides so the junction table matches
		// what `create_intermediate_table_for_m2m` / `generate_migrations`
		// produce. Without this, FK columns would hard-code `BigInteger`
		// even when either side uses a different PK type.
		let source_pk_type = self.to_state.get_primary_key_type(app_label, model_name);
		let target_pk_type = self
			.to_state
			.get_primary_key_type(&target_app, &target_model);

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
				generated: None,
				domain: None,
			},
			// source_id column
			super::ColumnDefinition {
				name: source_column.clone(),
				type_definition: source_pk_type,
				not_null: true,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
				generated: None,
				domain: None,
			},
			// target_id column
			super::ColumnDefinition {
				name: target_column.clone(),
				type_definition: target_pk_type,
				not_null: true,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
				generated: None,
				domain: None,
			},
		];

		// Create constraints (use the resolved table names)
		let constraints = vec![
			// Foreign key to source table
			super::Constraint::ForeignKey {
				name: crate::naming::foreign_key_constraint_name(&table_name, &source_column),
				columns: vec![source_column.clone()],
				referenced_table: source_table.clone(),
				referenced_columns: vec!["id".to_string()],
				on_delete: super::ForeignKeyAction::Cascade,
				on_update: super::ForeignKeyAction::Cascade,
				deferrable: None,
			},
			// Foreign key to target table
			super::Constraint::ForeignKey {
				name: crate::naming::foreign_key_constraint_name(&table_name, &target_column),
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
	/// // Add a new model to the target state
	/// let mut model = ModelState::new("myapp", "User");
	/// model.add_field(FieldState::new("id", FieldType::Integer, false));
	/// to_state.add_model(model);
	///
	/// let detector = MigrationAutodetector::new(from_state, to_state);
	/// let operations = detector.generate_operations();
	///
	/// assert!(!operations.is_empty());
	/// ```rust,ignore
	/// Sort operations by their dependencies to ensure correct execution order.
	///
	/// CreateTable operations must run before table modifications unless they
	/// depend on a table produced by a rename. The remaining emission order
	/// carries dependency information. In particular, generated-column repair
	/// and rollback marker operations must stay around the drop/add replacement
	/// pair that they protect.
	fn sort_operations_by_dependency(
		&self,
		mut operations: Vec<super::Operation>,
	) -> Vec<super::Operation> {
		let mut sorted = Vec::new();

		// Extract CreateTable operations (must be first)
		let renamed_table_sources: BTreeSet<_> = operations
			.iter()
			.filter_map(|operation| match operation {
				super::Operation::RenameTable { old_name, .. } => Some(old_name.clone()),
				super::Operation::MoveModel {
					rename_table: true,
					old_table_name: Some(old_name),
					..
				} => Some(old_name.clone()),
				_ => None,
			})
			.collect();
		let renamed_table_destinations: BTreeSet<_> = operations
			.iter()
			.filter_map(|operation| match operation {
				super::Operation::RenameTable { new_name, .. } => Some(new_name.clone()),
				super::Operation::MoveModel {
					rename_table: true,
					new_table_name: Some(new_name),
					..
				} => Some(new_name.clone()),
				_ => None,
			})
			.collect();
		let create_tables: Vec<_> = operations
			.iter()
			.filter(|operation| {
				matches!(
					operation,
					super::Operation::CreateTable { name, .. }
						if !renamed_table_sources.contains(name)
				) && !renamed_table_destinations
					.iter()
					.any(|table_name| Self::operation_references_table(operation, table_name))
			})
			.cloned()
			.collect();
		operations.retain(|operation| {
			!matches!(
			operation,
			super::Operation::CreateTable { name, .. }
				if !renamed_table_sources.contains(name)
			) || renamed_table_destinations
				.iter()
				.any(|table_name| Self::operation_references_table(operation, table_name))
		});

		// Assemble in correct order. CreateTable ops are topologically ordered
		// so inline foreign keys do not reference tables created later in the
		// same migration.
		sorted.extend(Self::topological_sort_create_tables(create_tables));
		sorted.extend(operations);

		sorted
	}

	fn operation_targets_table(operation: &super::Operation, table_name: &str) -> bool {
		match operation {
			super::Operation::AddColumn { table, .. }
			| super::Operation::AlterColumn { table, .. }
			| super::Operation::DropColumn { table, .. }
			| super::Operation::RenameColumn { table, .. }
			| super::Operation::AddConstraint { table, .. }
			| super::Operation::AddConstraintDefinition { table, .. }
			| super::Operation::AddConstraintRepair { table, .. }
			| super::Operation::RestoreConstraintOnRollback { table, .. }
			| super::Operation::DropConstraint { table, .. }
			| super::Operation::DropConstraintDefinition { table, .. }
			| super::Operation::CreateIndex { table, .. }
			| super::Operation::CreateIndexRepair { table, .. }
			| super::Operation::RestoreIndexOnRollback { table, .. }
			| super::Operation::DropIndex { table, .. }
			| super::Operation::DropNamedIndex { table, .. }
			| super::Operation::CreateCompositePrimaryKey { table, .. }
			| super::Operation::SetAutoIncrementValue { table, .. } => table == table_name,
			#[cfg(feature = "pgvector")]
			super::Operation::CreateNamedIndex { table, .. } => table == table_name,
			super::Operation::CreateTable { name, .. } | super::Operation::DropTable { name } => {
				name == table_name
			}
			super::Operation::RenameTable { old_name, new_name } => {
				old_name == table_name || new_name == table_name
			}
			_ => false,
		}
	}

	fn constraint_references_table(constraint: &super::Constraint, table_name: &str) -> bool {
		match constraint {
			super::Constraint::ForeignKey {
				referenced_table, ..
			}
			| super::Constraint::OneToOne {
				referenced_table, ..
			} => referenced_table == table_name,
			super::Constraint::ManyToMany {
				target_table,
				through_table,
				..
			} => target_table == table_name || through_table == table_name,
			super::Constraint::PrimaryKey { .. }
			| super::Constraint::Unique { .. }
			| super::Constraint::Check { .. }
			| super::Constraint::EnumDomain { .. }
			| super::Constraint::Exclude { .. } => false,
		}
	}

	fn reference_tail_starts_with_table(tail: &str, table_name: &str) -> bool {
		let tail = tail.trim_start();
		let Some(first_char) = tail.chars().next() else {
			return false;
		};

		let (referenced_table, rest) = if first_char == '"' {
			let Some(end_quote) = tail[1..].find('"') else {
				return false;
			};
			(&tail[1..=end_quote], &tail[end_quote + 2..])
		} else {
			let end = tail
				.find(|ch: char| ch == '(' || ch.is_whitespace())
				.unwrap_or(tail.len());
			(&tail[..end], &tail[end..])
		};

		referenced_table == table_name
			&& (rest.is_empty()
				|| rest.starts_with('(')
				|| rest.chars().next().is_some_and(char::is_whitespace))
	}

	fn constraint_sql_references_table(constraint_sql: &str, table_name: &str) -> bool {
		let mut rest = constraint_sql;
		while let Some(index) = rest.find("REFERENCES ") {
			let tail = &rest[index + "REFERENCES ".len()..];
			if Self::reference_tail_starts_with_table(tail, table_name) {
				return true;
			}
			rest = tail;
		}
		false
	}

	fn operation_references_table(operation: &super::Operation, table_name: &str) -> bool {
		match operation {
			super::Operation::CreateTable { constraints, .. } => constraints
				.iter()
				.any(|constraint| Self::constraint_references_table(constraint, table_name)),
			super::Operation::AddConstraint { constraint_sql, .. }
			| super::Operation::AddConstraintRepair { constraint_sql, .. }
			| super::Operation::RestoreConstraintOnRollback { constraint_sql, .. } => {
				Self::constraint_sql_references_table(constraint_sql, table_name)
			}
			super::Operation::AddConstraintDefinition { constraint, .. } => {
				Self::constraint_references_table(constraint, table_name)
			}
			_ => false,
		}
	}

	fn operation_needs_table_after_rename(
		operation: &super::Operation,
		new_table_name: &str,
	) -> bool {
		Self::operation_targets_table(operation, new_table_name)
			|| Self::operation_references_table(operation, new_table_name)
	}

	fn table_rename_names(operation: &super::Operation) -> Option<(String, String)> {
		match operation {
			super::Operation::RenameTable { old_name, new_name } => {
				Some((old_name.clone(), new_name.clone()))
			}
			super::Operation::MoveModel {
				rename_table: true,
				old_table_name: Some(old_name),
				new_table_name: Some(new_name),
				..
			} => Some((old_name.clone(), new_name.clone())),
			_ => None,
		}
	}

	fn order_renamed_table_operations(operations: &mut Vec<super::Operation>) {
		let rename_sources: BTreeSet<_> = operations
			.iter()
			.filter_map(|operation| {
				Self::table_rename_names(operation).map(|(old_name, _)| old_name)
			})
			.collect();
		let swap_destinations: BTreeSet<_> = operations
			.iter()
			.filter_map(|operation| {
				Self::table_rename_names(operation).map(|(_, new_name)| new_name)
			})
			.filter(|new_name| rename_sources.contains(new_name))
			.collect();
		let mut index = 0;
		while index < operations.len() {
			let (old_name, new_name) = match Self::table_rename_names(&operations[index]) {
				Some(names) => names,
				_ => {
					index += 1;
					continue;
				}
			};

			let rename_operation = operations.remove(index);
			let mut before_rename = Vec::new();
			let mut after_rename = Vec::new();

			for (candidate_index, operation) in std::mem::take(operations).into_iter().enumerate() {
				if candidate_index < index && Self::table_rename_names(&operation).is_some() {
					before_rename.push(operation);
				} else if Self::table_rename_names(&operation).is_some()
					|| matches!(
						&operation,
						super::Operation::CreateTable { name, .. } if name == &old_name
					) {
					after_rename.push(operation);
				} else if matches!(
					&operation,
					super::Operation::DropTable { name } if name == &new_name
				) || Self::operation_targets_table(&operation, &old_name)
				{
					before_rename.push(operation);
				} else if swap_destinations.iter().any(|destination| {
					Self::operation_needs_table_after_rename(&operation, destination)
				}) || Self::operation_needs_table_after_rename(&operation, &new_name)
				{
					after_rename.push(operation);
				} else if candidate_index < index {
					before_rename.push(operation);
				} else {
					after_rename.push(operation);
				}
			}

			let next_index = before_rename.len() + 1;
			before_rename.push(rename_operation);
			before_rename.append(&mut after_rename);
			*operations = before_rename;
			index = next_index;
		}
	}

	fn operation_index_references_column(
		operation: &super::Operation,
		table_name: &str,
		column: &str,
	) -> bool {
		match operation {
			super::Operation::CreateIndex {
				table,
				columns,
				expressions,
				where_clause,
				..
			}
			| super::Operation::CreateIndexRepair {
				table,
				columns,
				expressions,
				where_clause,
				..
			}
			| super::Operation::DropNamedIndex {
				table,
				columns,
				expressions,
				where_clause,
				..
			} => {
				table == table_name
					&& (columns.iter().any(|field| field == column)
						|| expressions.as_deref().is_some_and(|expressions| {
							expressions.iter().any(|expression| {
								ProjectState::expression_references_column(expression, column)
							})
						}) || where_clause.as_deref().is_some_and(|where_clause| {
						ProjectState::expression_references_column(where_clause, column)
					}))
			}
			super::Operation::DropIndex { table, columns } => {
				table == table_name && columns.iter().any(|field| field == column)
			}
			_ => false,
		}
	}

	fn typed_constraint_references_column(constraint: &super::Constraint, column: &str) -> bool {
		match constraint {
			super::Constraint::PrimaryKey { columns, .. }
			| super::Constraint::ForeignKey { columns, .. }
			| super::Constraint::Unique { columns, .. } => {
				columns.iter().any(|candidate| candidate == column)
			}
			super::Constraint::Check { expression, .. } => {
				Self::expression_text_references_column(expression, column)
			}
			super::Constraint::EnumDomain {
				column: constrained_column,
				..
			}
			| super::Constraint::OneToOne {
				column: constrained_column,
				..
			} => constrained_column == column,
			super::Constraint::ManyToMany {
				source_column,
				target_column,
				..
			} => source_column == column || target_column == column,
			super::Constraint::Exclude {
				elements,
				where_clause,
				..
			} => {
				elements.iter().any(|(expression, _)| {
					expression == column
						|| Self::expression_text_references_column(expression, column)
				}) || where_clause.as_deref().is_some_and(|expression| {
					Self::expression_text_references_column(expression, column)
				})
			}
		}
	}

	fn order_renamed_column_operations(operations: &mut Vec<super::Operation>) {
		let mut index = 0;
		while index < operations.len() {
			let (table, old_name, new_name) = match &operations[index] {
				super::Operation::RenameColumn {
					table,
					old_name,
					new_name,
				} => (table.clone(), old_name.clone(), new_name.clone()),
				_ => {
					index += 1;
					continue;
				}
			};

			let mut remaining = std::mem::take(operations);
			let rename_operation = remaining.remove(index);
			let prefix = remaining.drain(..index).collect::<Vec<_>>();
			let mut before_rename = Vec::new();
			let mut after_rename = Vec::new();
			let mut prefix_without_recreated_indexes = Vec::new();
			for operation in prefix {
				match &operation {
					super::Operation::CreateIndex { .. }
					| super::Operation::CreateIndexRepair { .. }
						if Self::operation_index_references_column(
							&operation, &table, &new_name,
						) =>
					{
						after_rename.push(operation);
					}
					_ => prefix_without_recreated_indexes.push(operation),
				}
			}
			for operation in remaining {
				match &operation {
					super::Operation::DropConstraintDefinition {
						table: constraint_table,
						constraint,
					} if constraint_table == &table
						&& Self::typed_constraint_references_column(constraint, &old_name) =>
					{
						before_rename.push(operation);
					}
					super::Operation::DropIndex { .. }
					| super::Operation::DropNamedIndex { .. }
						if Self::operation_index_references_column(
							&operation, &table, &old_name,
						) =>
					{
						before_rename.push(operation);
					}
					super::Operation::CreateIndex { .. }
					| super::Operation::CreateIndexRepair { .. }
						if Self::operation_index_references_column(
							&operation, &table, &new_name,
						) =>
					{
						after_rename.push(operation);
					}
					_ => after_rename.push(operation),
				}
			}

			let rename_position = prefix_without_recreated_indexes.len() + before_rename.len();
			let mut reordered = prefix_without_recreated_indexes;
			reordered.extend(before_rename);
			reordered.push(rename_operation);
			reordered.extend(after_rename);
			*operations = reordered;
			index = rename_position + 1;
		}
	}

	/// Generates operations, failing fast when migration metadata is invalid.
	///
	/// # Panics
	///
	/// Panics when checked autodetection fails. Use
	/// [`Self::try_generate_operations`] to handle validation errors.
	pub fn generate_operations(&self) -> Vec<super::Operation> {
		self.try_generate_operations().expect(
			"migration operation generation failed; use try_generate_operations() to handle invalid metadata or ambiguous renames",
		)
	}

	/// Performs operation generation and fails on ambiguous rename-like changes.
	pub fn try_generate_operations(&self) -> super::Result<Vec<super::Operation>> {
		let changes = self.try_detect_changes()?;
		Ok(self.generate_operations_from_changes(&changes))
	}

	fn generate_operations_from_changes(&self, changes: &DetectedChanges) -> Vec<super::Operation> {
		let mut by_app: BTreeMap<String, Vec<super::Operation>> = BTreeMap::new();

		// Shared per-app emissions (CreateTable, column ops, constraint ops,
		// auto-increment resets). This is the single source of truth shared
		// with `generate_migrations()` so the two paths cannot diverge again
		// (issue #4040).
		self.emit_shared_per_app_operations(changes, &mut by_app);
		self.emit_table_rename_operations(changes, &mut by_app);
		for (from_app, from_model, to_app, to_model, rename_table, old_table, new_table) in
			&changes.moved_models
		{
			if *rename_table {
				let old_name = old_table.clone().or_else(|| {
					self.from_state
						.get_model(from_app, from_model)
						.map(|model| model.table_name.clone())
				});
				let new_name = new_table.clone().or_else(|| {
					self.to_state
						.get_model(to_app, to_model)
						.map(|model| model.table_name.clone())
				});
				if let (Some(old_name), Some(new_name)) = (old_name, new_name) {
					by_app
						.entry(to_app.clone())
						.or_default()
						.push(super::Operation::RenameTable { old_name, new_name });
				}
			}
		}
		self.preserve_many_to_many_artifact_renames(changes, &mut by_app);

		// `generate_operations()`-specific extra: walk ManyToMany fields on
		// new and added models and emit intermediate `CreateTable`s via
		// `generate_intermediate_table`. This complements the shared
		// emissions above. (`generate_migrations()` covers the same
		// ground via `created_many_to_many` with PK-type-resolved logic.)
		for (app_label, model_name) in &changes.created_models {
			if let Some(model) = self.to_state.get_model(app_label, model_name) {
				for (field_name, field_state) in &model.fields {
					if let super::FieldType::ManyToMany { to, through } = &field_state.field_type
						&& let Some(operation) = self.generate_intermediate_table(
							app_label, model_name, field_name, to, through,
						) {
						by_app.entry(app_label.clone()).or_default().push(operation);
					}
				}
			}
		}
		for (app_label, model_name, field_name) in &changes.added_fields {
			if let Some(model) = self.to_state.get_model(app_label, model_name)
				&& let Some(field) = model.get_field(field_name)
				&& let super::FieldType::ManyToMany { to, through } = &field.field_type
				&& let Some(operation) =
					self.generate_intermediate_table(app_label, model_name, field_name, to, through)
			{
				by_app.entry(app_label.clone()).or_default().push(operation);
			}
		}

		// MoveModel operations remain migration-only because they carry app
		// metadata. Table renames are emitted above because they are database
		// schema operations that direct callers must apply as well.

		// Second-line defence against redundant single-column `AddConstraint
		// UNIQUE` operations. The primary fix lives in
		// `detect_added_constraints` (shape-match), but this pass also catches
		// cases where the column is being added in the same migration with
		// `column.unique = true` *and* a peer `AddConstraint` is emitted for
		// it. See reinhardt-web#4448.
		Self::dedup_redundant_unique_add_constraints(&mut by_app);
		for operations in by_app.values_mut() {
			Self::order_renamed_column_operations(operations);
		}

		// Flatten and sort by dependency to ensure correct execution order.
		for operations in by_app.values_mut() {
			Self::order_renamed_table_operations(operations);
		}
		let operations: Vec<super::Operation> = by_app.into_values().flatten().collect();
		self.sort_operations_by_dependency(operations)
	}

	fn emit_table_rename_operations(
		&self,
		changes: &DetectedChanges,
		by_app: &mut BTreeMap<String, Vec<super::Operation>>,
	) {
		let mut post_rename_by_app: BTreeMap<String, Vec<super::Operation>> = BTreeMap::new();
		let mut registered_renames = BTreeSet::new();
		{
			let mut register_recreated_names =
				|app_label: &str, old_model: &ModelState, new_model: &ModelState| {
					let key = (
						app_label.to_string(),
						old_model.table_name.clone(),
						new_model.table_name.clone(),
					);
					if old_model.table_name == new_model.table_name
						|| !registered_renames.insert(key)
					{
						return;
					}

					let renamed_constraints =
						Self::renamed_single_field_unique_constraints(old_model, new_model);
					let renamed_indexes = old_model
						.indexes
						.iter()
						.filter_map(|old_index| {
							new_model
								.indexes
								.iter()
								.find(|new_index| {
									model_index_definitions_equivalent(
										old_model, old_index, new_model, new_index,
									)
								})
								.filter(|new_index| old_index.name != new_index.name)
								.map(|new_index| {
									let old_state =
										advanced_index_option_state(old_model, old_index)
											.unwrap_or_default();
									let new_state =
										advanced_index_option_state(new_model, new_index)
											.unwrap_or(old_state);
									(old_index.clone(), new_index.clone(), old_state, new_state)
								})
						})
						.collect::<Vec<_>>();

					let before = by_app.entry(app_label.to_string()).or_default();
					let after = post_rename_by_app.entry(app_label.to_string()).or_default();
					for (old_constraint, new_constraint) in renamed_constraints {
						before.push(super::Operation::DropConstraintDefinition {
							table: old_model.table_name.clone(),
							constraint: old_constraint.to_constraint(),
						});
						after.push(super::Operation::AddConstraint {
							table: new_model.table_name.clone(),
							constraint_sql: new_constraint.to_constraint().to_string(),
						});
					}
					for (old_index, new_index, old_state, new_state) in renamed_indexes {
						before.push(
							old_index.drop_operation_with_state(&old_model.table_name, old_state),
						);
						after.push(
							new_index.create_named_operation_with_state(
								&new_model.table_name,
								new_state,
							),
						);
					}
				};

			for (app_label, model_name, _, _) in &changes.renamed_tables {
				if let (Some(old_model), Some(new_model)) = (
					self.from_state.get_model(app_label, model_name),
					self.to_state.get_model(app_label, model_name),
				) {
					register_recreated_names(app_label, old_model, new_model);
				}
			}
			for (app_label, old_name, new_name) in &changes.renamed_models {
				if let (Some(old_model), Some(new_model)) = (
					self.from_state.get_model(app_label, old_name),
					self.to_state.get_model(app_label, new_name),
				) {
					register_recreated_names(app_label, old_model, new_model);
				}
			}
		}

		let mut pending_by_app: BTreeMap<_, Vec<_>> = BTreeMap::new();
		for (app_label, _model_name, old_name, new_name) in &changes.renamed_tables {
			pending_by_app
				.entry(app_label.clone())
				.or_default()
				.push((old_name.clone(), new_name.clone()));
		}
		for (app_label, old_model, new_model) in &changes.renamed_models {
			if let (Some(old), Some(new)) = (
				self.from_state.get_model(app_label, old_model),
				self.to_state.get_model(app_label, new_model),
			) && old.table_name != new.table_name
			{
				pending_by_app
					.entry(app_label.clone())
					.or_default()
					.push((old.table_name.clone(), new.table_name.clone()));
			}
		}

		for (app_label, mut pending) in pending_by_app {
			let mut reserved_names: BTreeSet<_> = pending
				.iter()
				.flat_map(|(old_name, new_name)| [old_name.clone(), new_name.clone()])
				.collect();
			reserved_names.extend(
				self.from_state
					.models
					.values()
					.map(|model| model.table_name.clone()),
			);
			reserved_names.extend(
				self.to_state
					.models
					.values()
					.map(|model| model.table_name.clone()),
			);
			while !pending.is_empty() {
				let source_names: BTreeSet<_> = pending
					.iter()
					.map(|(old_name, _)| old_name.clone())
					.collect();
				if let Some(index) = pending
					.iter()
					.position(|(_, new_name)| !source_names.contains(new_name))
				{
					let (old_name, new_name) = pending.remove(index);
					by_app
						.entry(app_label.clone())
						.or_default()
						.push(super::Operation::RenameTable { old_name, new_name });
					continue;
				}

				let (old_name, _) = &mut pending[0];
				let base = format!("__reinhardt_rename_tmp_{old_name}");
				let mut temporary_name = base.clone();
				let mut suffix = 1;
				while reserved_names.contains(&temporary_name) {
					temporary_name = format!("{base}_{suffix}");
					suffix += 1;
				}
				reserved_names.insert(temporary_name.clone());
				by_app
					.entry(app_label.clone())
					.or_default()
					.push(super::Operation::RenameTable {
						old_name: old_name.clone(),
						new_name: temporary_name.clone(),
					});
				*old_name = temporary_name;
			}
			if let Some(mut operations) = post_rename_by_app.remove(&app_label) {
				by_app.entry(app_label).or_default().append(&mut operations);
			}
		}
	}

	/// Emit per-app operations shared by `generate_operations()` and
	/// `generate_migrations()`.
	///
	/// This is the single source of truth for emissions that previously had
	/// to be duplicated between the two methods. Issue #4040 was caused by
	/// PR #3998 updating only `generate_operations()` while
	/// `generate_migrations()` (the CLI entry point) was left silently
	/// divergent. Centralizing the shared emissions here makes that class of
	/// drift impossible.
	///
	/// Method-specific extras (M2M field-walking for `generate_operations()`;
	/// `created_many_to_many` / `renamed_models` / `moved_models` for
	/// `generate_migrations()`) are added by the callers after this helper
	/// returns.
	fn emit_shared_per_app_operations(
		&self,
		changes: &DetectedChanges,
		by_app: &mut std::collections::BTreeMap<String, Vec<super::Operation>>,
	) {
		// CreateTable for new models.
		for (app_label, model_name) in &changes.created_models {
			if let Some(model) = self.to_state.get_model(app_label, model_name) {
				let mut columns = Vec::new();
				for (field_name, field_state) in
					Self::order_model_fields_by_generated_dependencies(model)
				{
					columns.push(super::ColumnDefinition::from_field_state(
						field_name.clone(),
						field_state,
					));
				}

				let constraints: Vec<super::operations::Constraint> = model
					.constraints
					.iter()
					.map(|c| c.to_constraint())
					.collect();

				by_app
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

				for index in &model.indexes {
					by_app
						.entry(app_label.clone())
						.or_default()
						.push(index.create_operation(model));
				}
			}
		}

		// AddColumn for new fields.
		//
		// Use `model.table_name` (not `model.name`) so the executor's
		// `find_model_by_table_mut(table)` path resolves correctly. The
		// previous `generate_operations()` body used `model.name` here, which
		// was a latent bug that did not surface because that path was rarely
		// exercised against table-name-keyed state.
		let ordered_added_fields =
			self.order_added_fields_by_generated_dependencies(&changes.added_fields);
		let mut deferred_added_fields = Vec::new();
		for (app_label, model_name, field_name) in ordered_added_fields {
			if self.added_generated_field_references_altered_source(
				&changes.altered_fields,
				&changes.renamed_fields,
				&app_label,
				&model_name,
				&field_name,
			) {
				deferred_added_fields.push((app_label, model_name, field_name));
			} else {
				self.push_add_column_operation(by_app, &app_label, &model_name, &field_name);
			}
		}

		#[cfg(feature = "pgvector")]
		let mut altered_index_replacements: Vec<(String, String, IndexDefinition)> = Vec::new();
		#[cfg(not(feature = "pgvector"))]
		let altered_index_replacements: Vec<(String, String, IndexDefinition)> = Vec::new();
		#[cfg(feature = "pgvector")]
		let mut replacement_index_keys = BTreeSet::new();
		#[cfg(feature = "pgvector")]
		for (app_label, model_name, field_name) in &changes.altered_fields {
			let Some(from_model) = self.from_state.get_model(app_label, model_name) else {
				continue;
			};
			let Some(to_model) = self.to_state.get_model(app_label, model_name) else {
				continue;
			};
			let vector_dimensions_changed = matches!(
				(
					from_model.get_field(field_name).map(|field| &field.field_type),
					to_model.get_field(field_name).map(|field| &field.field_type),
				),
				(
					Some(super::FieldType::Vector {
						dimensions: from_dimensions,
					}),
					Some(super::FieldType::Vector {
						dimensions: to_dimensions,
					}),
				) if from_dimensions != to_dimensions
			);
			if !vector_dimensions_changed {
				continue;
			}
			for index in &from_model.indexes {
				let is_approximate = matches!(
					index.index_type(),
					Some(
						super::operations::IndexType::Hnsw { .. }
							| super::operations::IndexType::Ivfflat { .. }
					)
				);
				if is_approximate
					&& (index.fields.iter().any(|field| field == field_name)
						|| index.expressions().is_some_and(|expressions| {
							expressions.iter().any(|expression| {
								Self::expression_text_references_column(expression, field_name)
							})
						})) && to_model.indexes.iter().any(|to_index| to_index == index)
					&& replacement_index_keys.insert((
						app_label.clone(),
						model_name.clone(),
						index.name.clone(),
					)) {
					altered_index_replacements.push((
						app_label.clone(),
						model_name.clone(),
						index.clone(),
					));
				}
			}
		}

		for (app_label, model_name, index_name) in &changes.removed_indexes {
			let Some(model) = self.from_state.get_model(app_label, model_name) else {
				continue;
			};
			let Some(index) = model.indexes.iter().find(|index| index.name == *index_name) else {
				continue;
			};
			by_app
				.entry(app_label.clone())
				.or_default()
				.push(index.drop_operation(model));
		}
		for (app_label, model_name, index) in &altered_index_replacements {
			let Some(model) = self.from_state.get_model(app_label, model_name) else {
				continue;
			};
			by_app
				.entry(app_label.clone())
				.or_default()
				.push(index.drop_operation(model));
		}

		// RenameColumn for confirmed field renames.
		for (app_label, model_name, old_name, new_name) in &changes.renamed_fields {
			if let Some(model) = self.to_state.get_model(app_label, model_name) {
				by_app
					.entry(app_label.clone())
					.or_default()
					.push(super::Operation::RenameColumn {
						table: model.table_name.clone(),
						old_name: old_name.clone(),
						new_name: new_name.clone(),
					});
			}
		}

		// Enum-domain checks must be removed before a storage-type alteration.
		// The replacement constraint is emitted after column changes below.
		let mut retained_enum_constraints = Vec::new();
		for (app_label, model_name, constraint_name) in &changes.removed_constraints {
			let Some(from_model) = self.from_state.get_model(app_label, model_name) else {
				continue;
			};
			let Some(constraint) = from_model
				.constraints
				.iter()
				.find(|constraint| constraint.name == *constraint_name)
			else {
				continue;
			};
			if constraint.constraint_type == "enum_domain" {
				by_app.entry(app_label.clone()).or_default().push(
					super::Operation::DropConstraintDefinition {
						table: from_model.table_name.clone(),
						constraint: constraint.to_constraint(),
					},
				);
			}
		}
		for (app_label, model_name, field_name) in &changes.altered_fields {
			let Some(from_model) = self.from_state.get_model(app_label, model_name) else {
				continue;
			};
			let Some(to_model) = self.to_state.get_model(app_label, model_name) else {
				continue;
			};
			for constraint in from_model.constraints.iter().filter(|constraint| {
				constraint.constraint_type == "enum_domain"
					&& constraint.fields.as_slice() == std::slice::from_ref(field_name)
					&& to_model
						.constraints
						.iter()
						.any(|to_constraint| to_constraint == *constraint)
			}) {
				by_app.entry(app_label.clone()).or_default().push(
					super::Operation::DropConstraintDefinition {
						table: from_model.table_name.clone(),
						constraint: constraint.to_constraint(),
					},
				);
				retained_enum_constraints.push((
					app_label.clone(),
					model_name.clone(),
					constraint.clone(),
				));
			}
		}

		// AlterColumn for changed fields.
		let mut generated_replacement_columns: BTreeSet<(String, String, String)> = BTreeSet::new();
		for (app_label, model_name, field_name) in &changes.altered_fields {
			let replacement_key = (app_label.clone(), model_name.clone(), field_name.clone());
			if generated_replacement_columns.contains(&replacement_key) {
				continue;
			}
			if let Some(model) = self.to_state.get_model(app_label, model_name)
				&& let Some(field) = model.get_field(field_name)
			{
				let from_model = self.from_state.get_model(app_label, model_name);
				let old_definition = from_model
					.and_then(|from_model| from_model.get_field(field_name))
					.map(|from_field| {
						super::ColumnDefinition::from_field_state(field_name.clone(), from_field)
					});
				let new_definition =
					super::ColumnDefinition::from_field_state(field_name.clone(), field);
				let generated_change = from_model
					.and_then(|from_model| {
						from_model
							.get_field(field_name)
							.map(|from_field| (from_model, from_field))
					})
					.is_some_and(|(from_model, from_field)| {
						let from_unique = Self::single_field_unique_column_already_present(
							from_model, field_name,
						);
						let to_unique =
							Self::single_field_unique_column_already_present(model, field_name);
						Self::field_change_touches_generated(
							field_name,
							from_field,
							field,
							Some(from_unique),
							Some(to_unique),
						)
					});

				if generated_change {
					let is_dependent_of_other_altered_field = from_model
						.and_then(|from_model| {
							from_model
								.get_field(field_name)
								.map(|from_field| (from_model, from_field))
						})
						.is_some_and(|(from_model, from_field)| {
							changes.altered_fields.iter().any(
								|(other_app_label, other_model_name, other_field_name)| {
									if other_app_label != app_label
										|| other_model_name != model_name
										|| other_field_name == field_name
									{
										return false;
									}

									from_model.get_field(other_field_name).is_some()
										&& model.get_field(other_field_name).is_some()
										&& Self::generated_field_references_column(
											from_field,
											field,
											other_field_name,
										)
								},
							)
						});
					if is_dependent_of_other_altered_field {
						continue;
					}
				}

				let operations = by_app.entry(app_label.clone()).or_default();
				let dependent_generated_columns = from_model
					.map(|from_model| {
						Self::generated_column_replacement_dependents(from_model, model, field_name)
					})
					.unwrap_or_default();
				let source_change_requires_dependent_recreation =
					!generated_change && !dependent_generated_columns.is_empty();
				if generated_change || source_change_requires_dependent_recreation {
					let dependent_column_names: BTreeSet<String> = dependent_generated_columns
						.iter()
						.map(|dependent| dependent.name.clone())
						.collect();
					let affected_columns: BTreeSet<String> = if generated_change {
						std::iter::once(field_name.clone())
							.chain(dependent_column_names.iter().cloned())
							.collect()
					} else {
						dependent_column_names.clone()
					};
					generated_replacement_columns.insert(replacement_key);
					for dependent_name in &dependent_column_names {
						generated_replacement_columns.insert((
							app_label.clone(),
							model_name.clone(),
							dependent_name.clone(),
						));
					}
					if let Some(from_model) = from_model {
						Self::emit_generated_column_replacement_dependency_rollbacks(
							operations,
							from_model,
							model,
							&affected_columns,
						);
					}
					for dependent in dependent_generated_columns.iter().rev() {
						operations.push(super::Operation::DropColumn {
							table: model.table_name.clone(),
							column: dependent.name.clone(),
							old_definition: Some(dependent.old_definition.clone()),
						});
					}
					if generated_change {
						operations.push(super::Operation::DropColumn {
							table: model.table_name.clone(),
							column: field_name.clone(),
							old_definition: old_definition.clone(),
						});
						operations.push(super::Operation::AddColumn {
							table: model.table_name.clone(),
							column: new_definition,
							mysql_options: None,
						});
					} else {
						operations.push(super::Operation::AlterColumn {
							table: model.table_name.clone(),
							old_definition,
							column: field_name.clone(),
							new_definition,
							mysql_options: None,
						});
					}
					for dependent in dependent_generated_columns {
						if let Some(new_dependent_definition) = dependent.new_definition {
							operations.push(super::Operation::AddColumn {
								table: model.table_name.clone(),
								column: new_dependent_definition,
								mysql_options: None,
							});
						}
					}
					if let Some(from_model) = from_model {
						Self::emit_generated_column_replacement_dependencies(
							operations,
							from_model,
							model,
							&affected_columns,
						);
					}
				} else {
					operations.push(super::Operation::AlterColumn {
						table: model.table_name.clone(),
						old_definition,
						column: field_name.clone(),
						new_definition,
						mysql_options: None,
					});
				}
			}
		}

		for (app_label, model_name, field_name) in &deferred_added_fields {
			self.push_add_column_operation(by_app, app_label, model_name, field_name);
		}

		// Drop non-PK constraints before DropColumn while preserving typed
		// enum-domain and generated-column replacement handling.
		for (app_label, model_name, constraint_name) in &changes.removed_constraints {
			let Some(from_model) = self.from_state.get_model(app_label, model_name) else {
				continue;
			};
			let is_composite_pk = from_model
				.constraints
				.iter()
				.find(|c| &c.name == constraint_name)
				.is_some_and(|c| c.constraint_type == "primary_key" && c.fields.len() >= 2);
			if is_composite_pk {
				continue;
			}
			let is_enum_domain = from_model
				.constraints
				.iter()
				.find(|constraint| constraint.name == *constraint_name)
				.is_some_and(|constraint| constraint.constraint_type == "enum_domain");
			if is_enum_domain {
				continue;
			}
			let replaced_fields = Self::generated_replacement_fields_for_model(
				&generated_replacement_columns,
				app_label,
				model_name,
			);
			if !replaced_fields.is_empty()
				&& from_model
					.constraints
					.iter()
					.find(|constraint| &constraint.name == constraint_name)
					.is_some_and(|constraint| {
						Self::constraint_references_any_column(constraint, &replaced_fields)
					}) {
				continue;
			}
			let operation = from_model
				.constraints
				.iter()
				.find(|constraint| constraint.name == *constraint_name)
				.map_or_else(
					|| super::Operation::DropConstraint {
						table: from_model.table_name.clone(),
						constraint_name: constraint_name.clone(),
					},
					|constraint| super::Operation::DropConstraintDefinition {
						table: from_model.table_name.clone(),
						constraint: constraint.to_constraint(),
					},
				);
			by_app.entry(app_label.clone()).or_default().push(operation);
		}

		// DropColumn for removed fields.
		let ordered_removed_fields =
			self.order_removed_fields_by_generated_dependencies(&changes.removed_fields);
		for (app_label, model_name, field_name) in &ordered_removed_fields {
			let replacement_key = (app_label.clone(), model_name.clone(), field_name.clone());
			if generated_replacement_columns.contains(&replacement_key) {
				continue;
			}
			if let Some(model) = self.from_state.get_model(app_label, model_name) {
				by_app
					.entry(app_label.clone())
					.or_default()
					.push(super::Operation::DropColumn {
						table: model.table_name.clone(),
						column: field_name.clone(),
						old_definition: model.get_field(field_name).map(|field| {
							super::ColumnDefinition::from_field_state(field_name.clone(), field)
						}),
					});
			}
		}

		// DropTable for deleted models.
		for (app_label, model_name) in &changes.deleted_models {
			if let Some(model) = self.from_state.get_model(app_label, model_name) {
				by_app
					.entry(app_label.clone())
					.or_default()
					.push(super::Operation::DropTable {
						name: model.table_name.clone(),
					});
			}
		}

		// DropConstraint for modified composite PKs (drop before recreate).
		for (app_label, model_name, constraint_name) in &changes.removed_composite_primary_keys {
			if let Some(model) = self.from_state.get_model(app_label, model_name) {
				let operation = model
					.constraints
					.iter()
					.find(|constraint| constraint.name == *constraint_name)
					.map_or_else(
						|| super::Operation::DropConstraint {
							table: model.table_name.clone(),
							constraint_name: constraint_name.clone(),
						},
						|constraint| super::Operation::DropConstraintDefinition {
							table: model.table_name.clone(),
							constraint: constraint.to_constraint(),
						},
					);
				by_app.entry(app_label.clone()).or_default().push(operation);
			}
		}

		// CreateCompositePrimaryKey for composite PK additions.
		for (app_label, model_name, constraint) in &changes.added_composite_primary_keys {
			if let Some(model) = self.to_state.get_model(app_label, model_name) {
				by_app.entry(app_label.clone()).or_default().push(
					super::Operation::CreateCompositePrimaryKey {
						table: model.table_name.clone(),
						columns: constraint.fields.clone(),
						constraint_name: Some(constraint.name.clone()),
					},
				);
			}
		}

		// AddConstraint for non-PK constraints added to existing tables.
		//
		// Covers `unique_together`, `Check`, `ForeignKey`, and `OneToOne`
		// constraints declared on a model that already exists in
		// `from_state`. Composite primary keys are emitted via
		// `added_composite_primary_keys` using `CreateCompositePrimaryKey`
		// and must be skipped here to avoid duplicate emission. The
		// constraint SQL is rendered through the existing
		// `ConstraintDefinition::to_constraint()` -> `Constraint: Display`
		// path, which mirrors the SQL produced for the same constraint when
		// emitted as part of a `CreateTable` operation, so the on-disk
		// schema for a "create + add later" sequence stays equivalent to a
		// "create with constraint" sequence.
		for (app_label, model_name, constraint) in &changes.added_constraints {
			if constraint.constraint_type == "primary_key" && constraint.fields.len() >= 2 {
				continue;
			}
			let Some(to_model) = self.to_state.get_model(app_label, model_name) else {
				continue;
			};
			let operation = if constraint.constraint_type == "enum_domain" {
				super::Operation::AddConstraintDefinition {
					table: to_model.table_name.clone(),
					constraint: constraint.to_constraint(),
				}
			} else {
				super::Operation::AddConstraint {
					table: to_model.table_name.clone(),
					constraint_sql: constraint.to_constraint().to_string(),
				}
			};
			by_app.entry(app_label.clone()).or_default().push(operation);
		}

		for (app_label, model_name, constraint) in retained_enum_constraints {
			let Some(to_model) = self.to_state.get_model(&app_label, &model_name) else {
				continue;
			};
			by_app
				.entry(app_label)
				.or_default()
				.push(super::Operation::AddConstraintDefinition {
					table: to_model.table_name.clone(),
					constraint: constraint.to_constraint(),
				});
		}

		for (app_label, model_name, index) in &changes.added_indexes {
			let Some(model) = self.to_state.get_model(app_label, model_name) else {
				continue;
			};
			let replaced_index_state = changes.removed_indexes.iter().find_map(
				|(removed_app, removed_model, removed_name)| {
					if removed_app != app_label || removed_model != model_name {
						return None;
					}
					self.from_state
						.get_model(removed_app, removed_model)
						.and_then(|from_model| {
							from_model
								.indexes
								.iter()
								.find(|removed| removed.name == *removed_name)
								.filter(|removed| {
									removed.name == index.name
										|| index_definitions_equivalent(removed, index)
								})
								.map(|removed| {
									advanced_index_option_state(from_model, removed)
										.unwrap_or_default()
								})
						})
				},
			);
			let state = advanced_index_option_state(model, index)
				.or(replaced_index_state)
				.unwrap_or_default();
			let operation = if replaced_index_state.is_some() {
				index.create_replacement_operation_with_state(&model.table_name, state)
			} else {
				index.create_operation_with_state(&model.table_name, state)
			};
			by_app.entry(app_label.clone()).or_default().push(operation);
		}
		for (app_label, model_name, index) in &altered_index_replacements {
			let Some(model) = self.to_state.get_model(app_label, model_name) else {
				continue;
			};
			let state = advanced_index_option_state(model, index)
				.or_else(|| {
					self.from_state
						.get_model(app_label, model_name)
						.and_then(|from_model| {
							from_model
								.indexes
								.iter()
								.find(|from_index| *from_index == index)
								.and_then(|from_index| {
									advanced_index_option_state(from_model, from_index)
								})
						})
				})
				.unwrap_or_default();
			by_app
				.entry(app_label.clone())
				.or_default()
				.push(index.create_replacement_operation_with_state(&model.table_name, state));
		}

		// SetAutoIncrementValue for detected sequence resets.
		for (app_label, model_name, column, value) in &changes.auto_increment_resets {
			if let Some(model) = self.to_state.get_model(app_label, model_name) {
				by_app.entry(app_label.clone()).or_default().push(
					super::Operation::SetAutoIncrementValue {
						table: model.table_name.clone(),
						column: column.clone(),
						value: *value,
					},
				);
			}
		}
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
	/// // Add a new model
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
	///
	/// # Panics
	///
	/// Panics when checked autodetection fails. Use
	/// [`Self::try_generate_migrations`] to handle validation errors.
	pub fn generate_migrations(&self) -> Vec<super::Migration> {
		self.try_generate_migrations().expect(
			"migration generation failed; use try_generate_migrations() to handle invalid metadata or ambiguous renames",
		)
	}

	/// Generate migrations together with actionable schema-change warnings.
	///
	/// # Panics
	///
	/// Panics when checked autodetection fails. Use
	/// [`Self::try_generate_migrations_with_warnings`] to handle validation
	/// errors.
	pub fn generate_migrations_with_warnings(&self) -> GeneratedMigrations {
		self.try_generate_migrations_with_warnings().expect(
			"migration generation failed; use try_generate_migrations_with_warnings() to handle invalid metadata or ambiguous renames",
		)
	}

	/// Generate migrations and fail on ambiguous rename-like changes.
	pub fn try_generate_migrations(&self) -> super::Result<Vec<super::Migration>> {
		Ok(self.try_generate_migrations_with_warnings()?.migrations)
	}

	/// Generate migrations and warnings, failing on ambiguous rename-like changes.
	pub fn try_generate_migrations_with_warnings(&self) -> super::Result<GeneratedMigrations> {
		let changes = self.try_detect_changes()?;
		Ok(GeneratedMigrations {
			migrations: self.generate_migrations_from_changes(&changes),
			warnings: changes.warnings,
		})
	}

	fn generate_migrations_from_changes(&self, changes: &DetectedChanges) -> Vec<super::Migration> {
		let mut migrations_by_app: BTreeMap<String, Vec<super::Operation>> = BTreeMap::new();

		// Shared per-app emissions (CreateTable, column ops, constraint ops,
		// auto-increment resets). Single source of truth shared with
		// `generate_operations()` — see `emit_shared_per_app_operations` and
		// issue #4040.
		self.emit_shared_per_app_operations(changes, &mut migrations_by_app);
		self.emit_table_rename_operations(changes, &mut migrations_by_app);

		// Generate intermediate tables for ManyToMany relationships
		for (app_label, model_name, through_table, m2m) in &changes.created_many_to_many {
			// Resolve source table name from the to_state model. The
			// `table_name` (user-set via `#[model(table_name = "...")]` or
			// derived by the macro) is the canonical identifier — never
			// the `struct` identifier (see #4659).
			let source_table = self
				.to_state
				.get_model(app_label, model_name)
				.map(|m| m.table_name.clone())
				.unwrap_or_else(|| format!("{}_{}", app_label, model_name.to_lowercase()));

			// Parse the target reference up-front so qualified names like
			// "app.Model" resolve correctly throughout the rest of this
			// block (table lookup, PK type lookup, and the snake-case
			// fallback). Without this, lookups would use the literal
			// "app.Model" string as the model name, miss every
			// to_state/registry entry, and produce defaults like
			// "app.model_id".
			let (parsed_target_app, parsed_target_model) =
				self.resolve_model_reference(&m2m.to_model, app_label);

			// Resolve target table name: prefer to_state, then global registry,
			// finally fall back to the canonical `{app}_{model_snake_case}` form
			// (mirroring the source-table fallback above). The fallback must
			// include the parsed app label — emitting only the canonicalized
			// model name would lose the app prefix that `#[model]` writes
			// into the real `table_name`, so FK constraints would point at a
			// table that does not exist.
			let target_table = self
				.to_state
				.get_model(&parsed_target_app, &parsed_target_model)
				.map(|model| model.table_name.clone())
				.or_else(|| {
					super::model_registry::global_registry()
						.get_models()
						.iter()
						.find(|m| {
							m.app_label == parsed_target_app && m.model_name == parsed_target_model
						})
						.map(|m| m.table_name.clone())
				})
				.unwrap_or_else(|| {
					format!(
						"{}_{}",
						parsed_target_app,
						to_snake_case(&parsed_target_model)
					)
				});

			// Default FK column names come from `crate::m2m_naming::default_m2m_columns`,
			// the single source of truth shared with `create_intermediate_table_for_m2m`
			// and the ORM accessor (issue #4665). The helper keys off the
			// *actual* table names (not struct identifiers) and applies
			// `from_/to_` prefixes only for self-referential M2M (#4659).
			let (default_source_col, default_target_col) =
				crate::m2m_naming::default_m2m_columns(&source_table, &target_table);
			let source_column = m2m.source_field.clone().unwrap_or(default_source_col);
			let target_column = m2m.target_field.clone().unwrap_or(default_target_col);

			// Get source model's primary key type
			let source_pk_type = self.to_state.get_primary_key_type(app_label, model_name);

			// Get target model's primary key type. Reuse the values parsed
			// from `m2m.to_model` at the top of this block so the lookup
			// agrees with how `target_table` was resolved (#4659 follow-up).
			let target_pk_type = self
				.to_state
				.get_primary_key_type(&parsed_target_app, &parsed_target_model);

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
					generated: None,
					domain: None,
				},
				super::ColumnDefinition {
					name: source_column.clone(),
					type_definition: source_pk_type.clone(),
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
					generated: None,
					domain: None,
				},
				super::ColumnDefinition {
					name: target_column.clone(),
					type_definition: target_pk_type,
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
					generated: None,
					domain: None,
				},
			];

			// Create FK constraints for the intermediate table
			let constraints = vec![
				super::operations::Constraint::ForeignKey {
					name: crate::naming::foreign_key_constraint_name(
						through_table,
						&source_column,
					),
					columns: vec![source_column.clone()],
					referenced_table: source_table.clone(),
					referenced_columns: vec!["id".to_string()],
					on_delete: ForeignKeyAction::Cascade,
					on_update: ForeignKeyAction::Cascade,
					deferrable: None,
				},
				super::operations::Constraint::ForeignKey {
					name: crate::naming::foreign_key_constraint_name(
						through_table,
						&target_column,
					),
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

		// Handle cross-app model moves
		// MovedModelInfo: (from_app, from_model, to_app, to_model, rename_table, old_table, new_table)
		for (
			from_app,
			from_model_name,
			to_app,
			to_model_name,
			rename_table,
			old_table,
			new_table,
		) in &changes.moved_models
		{
			// Get table names
			let old_table_name = old_table.clone().unwrap_or_else(|| {
				self.from_state
					.get_model(from_app, from_model_name)
					.map(|m| m.table_name.clone())
					.unwrap_or_else(|| format!("{}_{}", from_app, from_model_name.to_lowercase()))
			});

			let new_table_name = new_table.clone().unwrap_or_else(|| {
				self.to_state
					.get_model(to_app, to_model_name)
					.map(|m| m.table_name.clone())
					.unwrap_or_else(|| format!("{}_{}", to_app, to_model_name.to_lowercase()))
			});

			let (renamed_constraints, renamed_indexes) = if *rename_table {
				match (
					self.from_state.get_model(from_app, from_model_name),
					self.to_state.get_model(to_app, to_model_name),
				) {
					(Some(old_model), Some(new_model)) => (
						Self::renamed_single_field_unique_constraints(old_model, new_model),
						old_model
							.indexes
							.iter()
							.filter_map(|old_index| {
								new_model
									.indexes
									.iter()
									.find(|new_index| {
										model_index_definitions_equivalent(
											old_model, old_index, new_model, new_index,
										)
									})
									.filter(|new_index| old_index.name != new_index.name)
									.map(|new_index| {
										let old_state =
											advanced_index_option_state(old_model, old_index)
												.unwrap_or_default();
										let new_state =
											advanced_index_option_state(new_model, new_index)
												.unwrap_or(old_state);
										(old_index.clone(), new_index.clone(), old_state, new_state)
									})
							})
							.collect::<Vec<_>>(),
					),
					_ => (Vec::new(), Vec::new()),
				}
			} else {
				(Vec::new(), Vec::new())
			};
			let operations = migrations_by_app.entry(to_app.clone()).or_default();
			for (old_constraint, _) in &renamed_constraints {
				operations.push(super::Operation::DropConstraintDefinition {
					table: old_table_name.clone(),
					constraint: old_constraint.to_constraint(),
				});
			}
			for (old_index, _, old_state, _) in &renamed_indexes {
				operations.push(old_index.drop_operation_with_state(&old_table_name, *old_state));
			}
			// Add MoveModel operation to the target app's migrations
			operations.push(super::Operation::MoveModel {
				model_name: from_model_name.clone(),
				from_app: from_app.clone(),
				to_app: to_app.clone(),
				rename_table: *rename_table,
				old_table_name: if *rename_table {
					Some(old_table_name)
				} else {
					None
				},
				new_table_name: if *rename_table {
					Some(new_table_name.clone())
				} else {
					None
				},
			});
			for (_, new_constraint) in renamed_constraints {
				operations.push(super::Operation::AddConstraint {
					table: new_table_name.clone(),
					constraint_sql: new_constraint.to_constraint().to_string(),
				});
			}
			for (_, new_index, _, new_state) in renamed_indexes {
				operations
					.push(new_index.create_named_operation_with_state(&new_table_name, new_state));
			}
		}

		// Preserve ManyToMany through tables when source or target models adopt
		// new table names. Rename convention-derived artifacts so existing
		// relationship rows remain intact (#5673).
		for ((app_label, model_name), model_state) in &self.to_state.models {
			for m2m in &model_state.many_to_many_fields {
				let Some(through_rename) =
					self.find_many_to_many_artifact_rename(changes, app_label, model_name, m2m)
				else {
					continue;
				};

				let operations = migrations_by_app.entry(app_label.clone()).or_default();
				let table_rename_exists = operations.iter().any(|operation| {
					matches!(
						operation,
						super::Operation::RenameTable { old_name, new_name }
							if old_name == &through_rename.old_table
								&& new_name == &through_rename.new_table
					)
				});
				if through_rename.old_table != through_rename.new_table && !table_rename_exists {
					operations.push(super::Operation::RenameTable {
						old_name: through_rename.old_table.clone(),
						new_name: through_rename.new_table.clone(),
					});
				}
				if through_rename.old_source_column != through_rename.new_source_column {
					Self::remove_many_to_many_column_replacement_operations(
						operations,
						&through_rename,
						&through_rename.old_source_column,
						&through_rename.new_source_column,
					);
					let column_rename_exists = operations.iter().any(|operation| {
						matches!(
							operation,
							super::Operation::RenameColumn {
								table,
								old_name,
								new_name
							}
								if table == &through_rename.new_table
									&& old_name == &through_rename.old_source_column
									&& new_name == &through_rename.new_source_column
						)
					});
					if !column_rename_exists {
						operations.push(super::Operation::RenameColumn {
							table: through_rename.new_table.clone(),
							old_name: through_rename.old_source_column.clone(),
							new_name: through_rename.new_source_column.clone(),
						});
					}
					Self::defer_many_to_many_constraint_replacements(
						operations,
						&through_rename,
						&through_rename.old_source_column,
						&through_rename.new_source_column,
					);
				}
				if through_rename.old_target_column != through_rename.new_target_column {
					Self::remove_many_to_many_column_replacement_operations(
						operations,
						&through_rename,
						&through_rename.old_target_column,
						&through_rename.new_target_column,
					);
					let column_rename_exists = operations.iter().any(|operation| {
						matches!(
							operation,
							super::Operation::RenameColumn {
								table,
								old_name,
								new_name
							}
								if table == &through_rename.new_table
									&& old_name == &through_rename.old_target_column
									&& new_name == &through_rename.new_target_column
						)
					});
					if !column_rename_exists {
						operations.push(super::Operation::RenameColumn {
							table: through_rename.new_table.clone(),
							old_name: through_rename.old_target_column.clone(),
							new_name: through_rename.new_target_column.clone(),
						});
					}
					Self::defer_many_to_many_constraint_replacements(
						operations,
						&through_rename,
						&through_rename.old_target_column,
						&through_rename.new_target_column,
					);
				}
			}
		}

		// Second-line defence against redundant single-column `AddConstraint
		// UNIQUE` operations. The primary fix lives in
		// `detect_added_constraints` (shape-match); this pass also catches
		// cases where the column is being added in the same migration with
		// `column.unique = true` *and* a peer `AddConstraint` is emitted for
		// it. See reinhardt-web#4448.
		Self::dedup_redundant_unique_add_constraints(&mut migrations_by_app);
		for operations in migrations_by_app.values_mut() {
			Self::order_create_tables_by_foreign_keys(operations);
			Self::order_renamed_table_operations(operations);
			Self::order_renamed_column_operations(operations);
		}

		// Create Migration objects for each app
		let mut migrations = Vec::new();
		for (app_label, operations) in migrations_by_app {
			// Placeholder name; the final migration name is generated by
			// MakeMigrationsCommand using MigrationNamer::generate_name().
			let migration_name = "autodetected".to_string();

			let mut migration = super::Migration::new(&migration_name, &app_label);
			for operation in operations {
				migration = migration.add_operation(operation);
			}
			migrations.push(migration);
		}

		migrations
	}

	fn remove_many_to_many_column_replacement_operations(
		operations: &mut Vec<super::Operation>,
		rename: &ManyToManyArtifactRename,
		old_column: &str,
		new_column: &str,
	) {
		operations.retain(|operation| match operation {
			super::Operation::AddColumn { table, column, .. } => {
				!([&rename.old_table, &rename.new_table].contains(&table)
					&& column.name == new_column)
			}
			super::Operation::DropColumn { table, column, .. } => {
				!([&rename.old_table, &rename.new_table].contains(&table) && column == old_column)
			}
			_ => true,
		});
	}

	fn defer_many_to_many_constraint_replacements(
		operations: &mut Vec<super::Operation>,
		rename: &ManyToManyArtifactRename,
		old_column: &str,
		new_column: &str,
	) {
		let mut deferred = Vec::new();
		operations.retain(|operation| {
			let generated = match operation {
				super::Operation::AddConstraint {
					table,
					constraint_sql,
				} => {
					[&rename.old_table, &rename.new_table].contains(&table)
						&& Self::is_generated_many_to_many_constraint(
							constraint_sql,
							rename,
							old_column,
							new_column,
						)
				}
				super::Operation::DropConstraint {
					table,
					constraint_name,
				} => {
					[&rename.old_table, &rename.new_table].contains(&table)
						&& Self::is_generated_many_to_many_constraint(
							constraint_name,
							rename,
							old_column,
							new_column,
						)
				}
				_ => false,
			};
			if generated {
				deferred.push(operation.clone());
			}
			!generated
		});
		operations.extend(deferred);
	}

	fn is_generated_many_to_many_constraint(
		constraint: &str,
		rename: &ManyToManyArtifactRename,
		old_column: &str,
		new_column: &str,
	) -> bool {
		[
			crate::naming::foreign_key_constraint_name(&rename.old_table, old_column),
			crate::naming::foreign_key_constraint_name(&rename.new_table, new_column),
			format!("{}_unique", rename.old_table),
			format!("{}_unique", rename.new_table),
		]
		.iter()
		.any(|name| {
			constraint == name
				|| constraint
					.strip_prefix("CONSTRAINT ")
					.is_some_and(|definition| definition.starts_with(name))
		})
	}

	fn find_many_to_many_artifact_rename(
		&self,
		changes: &DetectedChanges,
		app_label: &str,
		model_name: &str,
		m2m: &ManyToManyMetadata,
	) -> Option<ManyToManyArtifactRename> {
		let new_source_model = self.to_state.get_model(app_label, model_name)?;
		let new_source_table = &new_source_model.table_name;
		let old_source_table = self
			.matching_from_model_for_to_model(app_label, model_name, new_source_model, changes)
			.or_else(|| self.from_state.get_model(app_label, model_name))
			.map(|model| model.table_name.as_str())
			.unwrap_or(new_source_table);
		let (target_app, target_model) = self.resolve_model_reference(&m2m.to_model, app_label);

		let new_target_model = self.to_state.get_model(&target_app, &target_model);
		let new_target_table = new_target_model
			.map(|model| model.table_name.clone())
			.or_else(|| {
				super::model_registry::global_registry()
					.get_model(&target_app, &target_model)
					.map(|model| model.table_name)
			})
			.unwrap_or_else(|| format!("{}_{}", target_app, to_snake_case(&target_model)));
		let old_target_table = new_target_model
			.and_then(|target| {
				self.matching_from_model_for_to_model(&target_app, &target_model, target, changes)
			})
			.map(|model| model.table_name.clone())
			.or_else(|| {
				self.from_state
					.get_model(&target_app, &target_model)
					.map(|model| model.table_name.clone())
			})
			.unwrap_or_else(|| new_target_table.clone());

		let old_table = m2m.through.clone().unwrap_or_else(|| {
			crate::m2m_naming::default_through_table(old_source_table, &m2m.field_name)
		});
		let new_table = m2m.through.clone().unwrap_or_else(|| {
			crate::m2m_naming::default_through_table(new_source_table, &m2m.field_name)
		});
		let (old_default_source_column, old_default_target_column) =
			crate::m2m_naming::default_m2m_columns(old_source_table, &old_target_table);
		let (new_default_source_column, new_default_target_column) =
			crate::m2m_naming::default_m2m_columns(new_source_table, &new_target_table);

		let rename = ManyToManyArtifactRename {
			old_table,
			old_source_column: m2m
				.source_field
				.clone()
				.unwrap_or(old_default_source_column),
			old_target_column: m2m
				.target_field
				.clone()
				.unwrap_or(old_default_target_column),
			new_table,
			new_source_column: m2m
				.source_field
				.clone()
				.unwrap_or(new_default_source_column),
			new_target_column: m2m
				.target_field
				.clone()
				.unwrap_or(new_default_target_column),
		};
		let old_through_model = self.from_state.find_model_by_table(&rename.old_table)?;
		if !old_through_model
			.fields
			.contains_key(&rename.old_source_column)
			|| !old_through_model
				.fields
				.contains_key(&rename.old_target_column)
		{
			return None;
		}
		(rename.old_table != rename.new_table
			|| rename.old_source_column != rename.new_source_column
			|| rename.old_target_column != rename.new_target_column)
			.then_some(rename)
	}

	/// Preserve convention-derived many-to-many artifacts for direct operation
	/// generation.
	fn preserve_many_to_many_artifact_renames(
		&self,
		changes: &DetectedChanges,
		by_app: &mut BTreeMap<String, Vec<super::Operation>>,
	) {
		for ((app_label, model_name), model_state) in &self.to_state.models {
			for m2m in &model_state.many_to_many_fields {
				let Some(through_rename) =
					self.find_many_to_many_artifact_rename(changes, app_label, model_name, m2m)
				else {
					continue;
				};

				let operations = by_app.entry(app_label.clone()).or_default();
				let table_rename_exists = operations.iter().any(|operation| {
					matches!(
						operation,
						super::Operation::RenameTable { old_name, new_name }
							if old_name == &through_rename.old_table
								&& new_name == &through_rename.new_table
					)
				});
				if through_rename.old_table != through_rename.new_table && !table_rename_exists {
					operations.push(super::Operation::RenameTable {
						old_name: through_rename.old_table.clone(),
						new_name: through_rename.new_table.clone(),
					});
				}
				for (old_column, new_column) in [
					(
						&through_rename.old_source_column,
						&through_rename.new_source_column,
					),
					(
						&through_rename.old_target_column,
						&through_rename.new_target_column,
					),
				] {
					if old_column == new_column {
						continue;
					}
					Self::remove_many_to_many_column_replacement_operations(
						operations,
						&through_rename,
						old_column,
						new_column,
					);
					let column_rename_exists = operations.iter().any(|operation| {
						matches!(
							operation,
							super::Operation::RenameColumn { table, old_name, new_name }
								if table == &through_rename.new_table
									&& old_name == old_column && new_name == new_column
						)
					});
					if !column_rename_exists {
						operations.push(super::Operation::RenameColumn {
							table: through_rename.new_table.clone(),
							old_name: old_column.clone(),
							new_name: new_column.clone(),
						});
					}
					Self::defer_many_to_many_constraint_replacements(
						operations,
						&through_rename,
						old_column,
						new_column,
					);
				}
			}
		}
	}

	/// Detect newly created ManyToMany relationships
	///
	/// This method compares ManyToMany fields between from_state and to_state
	/// to detect new relationships that require intermediate table creation.
	///
	/// # Detection Logic
	/// 1. Iterate through all models in to_state
	/// 2. For each ManyToMany field, check whether its convention-derived
	///    through table already exists in either state
	/// 3. If the source table was renamed and the old implicit through table is
	///    present, preserve it for a table and convention-derived column rename
	/// 4. Otherwise, if the through table is absent from both states, mark it as
	///    a newly created ManyToMany relationship
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
				// Generate the canonical through-table name from the source
				// model's actual `table_name`. Using `table_name` (not the
				// struct identifier) is required for two reasons:
				//   1. The ORM accessor's fallback derives the through-table
				//      and the source column from `S::table_name()` (see
				//      `crates/reinhardt-db/src/orm/many_to_many_accessor.rs`).
				//      The autodetector must agree on the same naming so that
				//      generated migrations match runtime expectations.
				//   2. `from_state` reconstructed from on-disk migrations only
				//      preserves `table_name`s (the original struct identifiers
				//      are lost — see #4659), so any subsequent existence check
				//      must use a `table_name`-derived key.
				// Route through `crate::m2m_naming::default_through_table`
				// so this existence-check key cannot drift from the
				// migration-emitting site (`create_intermediate_table_for_m2m`)
				// or the ORM accessor's fallback (#4659, #4665).
				let through_table = m2m.through.clone().unwrap_or_else(|| {
					crate::m2m_naming::default_through_table(
						&model_state.table_name,
						&m2m.field_name,
					)
				});

				// Check whether the through-table already exists in
				// `from_state`. The previous implementation looked at
				// `from_state.get_model(app_label, model_name).many_to_many_fields`,
				// but the M2M metadata is not stored in on-disk
				// `Operation::CreateTable`, so reconstructed `from_state`
				// always reported `many_to_many_fields.is_empty()` and
				// `exists_in_from` was always `false`. As a result, the
				// intermediate `CreateTable` was re-emitted on every
				// incremental `makemigrations` run (#4659).
				let exists_in_from = self
					.from_state
					.find_model_by_table(&through_table)
					.is_some();
				let exists_in_to = self.to_state.find_model_by_table(&through_table).is_some();

				if let Some(through_rename) =
					self.find_many_to_many_artifact_rename(changes, app_label, model_name, m2m)
				{
					// The old convention-derived through table is still present in
					// from_state. Keep it alive so migration generation can rename it
					// instead of dropping and recreating the table (#5673). This also
					// covers the path where to_state already contains the newly
					// materialized synthetic through model.
					if let Some((old_app, old_model)) = self
						.from_state
						.models
						.iter()
						.find(|(_, model)| model.table_name == through_rename.old_table)
						.map(|((app, model), _)| (app.clone(), model.clone()))
					{
						changes
							.deleted_models
							.retain(|(app, model)| app != &old_app || model != &old_model);
					}
					if let Some((new_app, new_model)) = self
						.to_state
						.models
						.iter()
						.find(|(_, model)| model.table_name == through_rename.new_table)
						.map(|((app, model), _)| (app.clone(), model.clone()))
					{
						changes
							.created_models
							.retain(|(app, model)| app != &new_app || model != &new_model);
					}
				} else if !exists_in_from && !exists_in_to {
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
	/// Detect foreign-key and relation dependencies between models.
	///
	/// Registered foreign-key ID columns typically keep a scalar `field_type`
	/// (for example `FieldType::Integer`) and store the relationship on
	/// `FieldState.foreign_key`. Detecting only relationship-shaped
	/// `FieldType` variants therefore leaves `model_dependencies` empty for
	/// the common inline-FK case. This method also inspects
	/// `FieldState.foreign_key` and `ModelState.constraints`, and resolves
	/// referenced tables through `ProjectState::find_model_by_table` so
	/// custom `table_name` values such as `auth_users` are found.
	fn detect_model_dependencies(&self, changes: &mut DetectedChanges) {
		use std::collections::BTreeSet;

		for ((app_label, model_name), model) in &self.to_state.models {
			let current = (app_label.clone(), model_name.clone());
			let mut dependencies = Vec::new();
			let mut seen = BTreeSet::new();

			for field in model.fields.values() {
				match &field.field_type {
					super::FieldType::ForeignKey { to_table, .. } => {
						self.record_table_dependency(
							to_table,
							&current,
							&mut dependencies,
							&mut seen,
						);
					}
					super::FieldType::OneToOne { to, .. } => {
						if let Some(dep) = self.parse_model_reference(to, app_label) {
							self.record_model_dependency(
								dep,
								&current,
								&mut dependencies,
								&mut seen,
							);
						}
					}
					super::FieldType::ManyToMany { to, .. } => {
						if let Some(dep) = self.parse_model_reference(to, app_label) {
							self.record_model_dependency(
								dep,
								&current,
								&mut dependencies,
								&mut seen,
							);
						}
					}
					super::FieldType::Custom(s) => {
						if let Some(dep) = self.extract_related_model(s, app_label) {
							self.record_model_dependency(
								dep,
								&current,
								&mut dependencies,
								&mut seen,
							);
						}
					}
					_ => {}
				}

				if let Some(fk) = &field.foreign_key {
					self.record_table_dependency(
						&fk.referenced_table,
						&current,
						&mut dependencies,
						&mut seen,
					);
				}
			}

			for constraint in &model.constraints {
				if (constraint
					.constraint_type
					.eq_ignore_ascii_case("foreign_key")
					|| constraint
						.constraint_type
						.eq_ignore_ascii_case("one_to_one"))
					&& let Some(fk_info) = &constraint.foreign_key_info
				{
					self.record_table_dependency(
						&fk_info.referenced_table,
						&current,
						&mut dependencies,
						&mut seen,
					);
				}
			}

			if !dependencies.is_empty() {
				changes.model_dependencies.insert(current, dependencies);
			}
		}
	}

	fn record_table_dependency(
		&self,
		table_name: &str,
		current: &(String, String),
		dependencies: &mut Vec<(String, String)>,
		seen: &mut std::collections::BTreeSet<(String, String)>,
	) {
		if let Some(dep) = self.resolve_model_by_table(table_name) {
			self.record_model_dependency(dep, current, dependencies, seen);
		}
	}

	fn record_model_dependency(
		&self,
		dep: (String, String),
		current: &(String, String),
		dependencies: &mut Vec<(String, String)>,
		seen: &mut std::collections::BTreeSet<(String, String)>,
	) {
		if dep != *current && seen.insert(dep.clone()) {
			dependencies.push(dep);
		}
	}

	fn resolve_model_by_table(&self, table_name: &str) -> Option<(String, String)> {
		if let Some(model) = self.to_state.find_model_by_table(table_name) {
			return Some((model.app_label.clone(), model.name.clone()));
		}
		if let Some(model) = self.from_state.find_model_by_table(table_name) {
			return Some((model.app_label.clone(), model.name.clone()));
		}
		self.find_model_by_table_name(table_name)
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

	fn resolve_model_reference(&self, reference: &str, current_app: &str) -> (String, String) {
		let parts: Vec<&str> = reference.split('.').collect();
		match parts.as_slice() {
			[app, model] => (app.to_string(), model.to_string()),
			[model] => {
				let model = model.to_string();
				if self.to_state.get_model(current_app, &model).is_some() {
					(current_app.to_string(), model)
				} else {
					(
						self.find_model_app(&model)
							.unwrap_or_else(|| current_app.to_string()),
						model,
					)
				}
			}
			_ => (current_app.to_string(), reference.to_string()),
		}
	}

	/// Apps whose tables are referenced by foreign keys in `operations`.
	///
	/// Used by `makemigrations` to attach cross-app migration dependencies so
	/// a fresh database applies provider-app initials before dependents.
	pub fn foreign_key_provider_apps(
		to_state: &ProjectState,
		operations: &[super::Operation],
		current_app: &str,
	) -> Vec<String> {
		use std::collections::BTreeSet;

		let mut apps = BTreeSet::new();
		for operation in operations {
			for table in Self::referenced_tables_in_operation(operation) {
				if let Some(model) = to_state.find_model_by_table(&table)
					&& model.app_label != current_app
				{
					apps.insert(model.app_label.clone());
				}
			}
			if let super::Operation::MoveModel { from_app, .. } = operation
				&& from_app != current_app
			{
				apps.insert(from_app.clone());
			}
		}
		apps.into_iter().collect()
	}

	fn referenced_tables_in_operation(operation: &super::Operation) -> Vec<String> {
		match operation {
			super::Operation::CreateTable { constraints, .. } => {
				Self::referenced_tables_from_constraints(constraints)
			}
			super::Operation::AddConstraint { constraint_sql, .. } => {
				Self::referenced_tables_from_constraint_sql(constraint_sql)
			}
			_ => Vec::new(),
		}
	}

	fn referenced_tables_from_constraints(
		constraints: &[super::operations::Constraint],
	) -> Vec<String> {
		let mut tables = Vec::new();
		for constraint in constraints {
			match constraint {
				super::operations::Constraint::ForeignKey {
					referenced_table, ..
				}
				| super::operations::Constraint::OneToOne {
					referenced_table, ..
				} => {
					tables.push(referenced_table.clone());
				}
				super::operations::Constraint::ManyToMany { target_table, .. } => {
					tables.push(target_table.clone());
				}
				_ => {}
			}
		}
		tables
	}

	fn referenced_tables_from_constraint_sql(constraint_sql: &str) -> Vec<String> {
		let mut tables = Vec::new();
		let mut rest = constraint_sql;
		while let Some(index) = rest.find("REFERENCES ") {
			let tail = rest[index + "REFERENCES ".len()..].trim_start();
			if tail.is_empty() {
				break;
			}
			let (table, remaining) = if let Some(stripped) = tail.strip_prefix('"') {
				match stripped.find('"') {
					Some(end) => (&stripped[..end], &stripped[end + 1..]),
					None => break,
				}
			} else {
				let end = tail
					.find(|ch: char| ch == '(' || ch.is_whitespace())
					.unwrap_or(tail.len());
				(&tail[..end], &tail[end..])
			};
			if !table.is_empty() {
				tables.push(table.to_string());
			}
			rest = remaining;
		}
		tables
	}

	fn order_create_tables_by_foreign_keys(operations: &mut [super::Operation]) {
		let create_indices: Vec<usize> = operations
			.iter()
			.enumerate()
			.filter(|(_, op)| matches!(op, super::Operation::CreateTable { .. }))
			.map(|(i, _)| i)
			.collect();
		if create_indices.len() <= 1 {
			return;
		}
		let create_ops: Vec<super::Operation> = create_indices
			.iter()
			.map(|&i| operations[i].clone())
			.collect();
		let sorted = Self::topological_sort_create_tables(create_ops);
		for (slot, op) in create_indices.into_iter().zip(sorted) {
			operations[slot] = op;
		}
	}

	fn topological_sort_create_tables(
		create_tables: Vec<super::Operation>,
	) -> Vec<super::Operation> {
		use std::collections::{BTreeMap, BTreeSet};

		if create_tables.len() <= 1 {
			return create_tables;
		}

		let names: Vec<String> = create_tables
			.iter()
			.filter_map(|op| match op {
				super::Operation::CreateTable { name, .. } => Some(name.clone()),
				_ => None,
			})
			.collect();
		let name_set: BTreeSet<String> = names.iter().cloned().collect();
		let mut in_degree: BTreeMap<String, usize> =
			names.iter().cloned().map(|name| (name, 0)).collect();
		let mut dependents: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

		for op in &create_tables {
			let super::Operation::CreateTable {
				name, constraints, ..
			} = op
			else {
				continue;
			};
			for referenced in Self::referenced_tables_from_constraints(constraints) {
				if referenced == *name || !name_set.contains(&referenced) {
					continue;
				}
				*in_degree.entry(name.clone()).or_insert(0) += 1;
				dependents
					.entry(referenced)
					.or_default()
					.insert(name.clone());
			}
		}

		let mut ready: BTreeSet<String> = in_degree
			.iter()
			.filter(|(_, degree)| **degree == 0)
			.map(|(name, _)| name.clone())
			.collect();
		let mut ordered_names = Vec::with_capacity(names.len());
		while let Some(name) = ready.iter().next().cloned() {
			ready.remove(&name);
			ordered_names.push(name.clone());
			if let Some(children) = dependents.get(&name) {
				for child in children {
					if let Some(degree) = in_degree.get_mut(child) {
						*degree = degree.saturating_sub(1);
						if *degree == 0 {
							ready.insert(child.clone());
						}
					}
				}
			}
		}

		if ordered_names.len() < names.len() {
			let mut remaining: Vec<String> = names
				.iter()
				.filter(|name| !ordered_names.contains(name))
				.cloned()
				.collect();
			remaining.sort();
			eprintln!(
				"⚠️  Warning: Circular foreign-key dependency detected among CreateTable operations: [{}]",
				remaining.join(", ")
			);
			ordered_names.extend(remaining);
		}

		let mut by_name: BTreeMap<String, super::Operation> = BTreeMap::new();
		let mut extras = Vec::new();
		for op in create_tables {
			match &op {
				super::Operation::CreateTable { name, .. } => {
					by_name.insert(name.clone(), op);
				}
				_ => extras.push(op),
			}
		}

		let mut sorted: Vec<super::Operation> = ordered_names
			.into_iter()
			.filter_map(|name| by_name.remove(&name))
			.collect();
		sorted.extend(by_name.into_values());
		sorted.extend(extras);
		sorted
	}

	/// Find a model in the project state by its table name
	///
	/// Resolves against the model's actual `table_name` first. Django-style
	/// `{app}_{model}` and lowercase model-name heuristics remain as fallbacks
	/// for callers that pass a relation target without a registered table.
	fn find_model_by_table_name(&self, table_name: &str) -> Option<(String, String)> {
		if let Some(model) = self.to_state.find_model_by_table(table_name) {
			return Some((model.app_label.clone(), model.name.clone()));
		}
		if let Some(model) = self.from_state.find_model_by_table(table_name) {
			return Some((model.app_label.clone(), model.name.clone()));
		}

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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::migrations::FieldType;
	use rstest::rstest;

	/// Helper to build a ProjectState with given models
	fn build_project_state(models: Vec<((String, String), ModelState)>) -> ProjectState {
		let mut state = ProjectState::new();
		for (key, model) in models {
			state.models.insert(key, model);
		}
		state
	}

	/// Helper to build a minimal ModelState
	fn build_model_state(
		app_label: &str,
		name: &str,
		fields: Vec<FieldState>,
		indexes: Vec<IndexDefinition>,
		constraints: Vec<ConstraintDefinition>,
	) -> ModelState {
		let mut field_map = std::collections::BTreeMap::new();
		for f in fields {
			field_map.insert(f.name.clone(), f);
		}
		ModelState {
			app_label: app_label.to_string(),
			name: name.to_string(),
			table_name: format!("{}_{}", app_label, name.to_lowercase()),
			fields: field_map,
			options: std::collections::HashMap::new(),
			base_model: None,
			inheritance_type: None,
			discriminator_column: None,
			indexes,
			constraints,
			many_to_many_fields: Vec::new(),
		}
	}

	#[cfg(not(feature = "pgvector"))]
	#[test]
	fn ordinary_index_addition_uses_legacy_create_index_operation() {
		let key = ("catalog".to_string(), "Product".to_string());
		let source = build_project_state(vec![(
			key.clone(),
			build_model_state("catalog", "Product", Vec::new(), Vec::new(), Vec::new()),
		)]);
		let target = build_project_state(vec![(
			key,
			build_model_state(
				"catalog",
				"Product",
				Vec::new(),
				vec![IndexDefinition::new(
					"product_sku_idx",
					vec!["sku".to_string()],
					false,
				)],
				Vec::new(),
			),
		)]);

		let operations = MigrationAutodetector::new(source, target).generate_operations();

		assert_eq!(
			operations,
			vec![super::super::Operation::CreateIndex {
				table: "catalog_product".to_string(),
				columns: vec!["sku".to_string()],
				unique: false,
				index_type: None,
				where_clause: None,
				concurrently: false,
				expressions: None,
				mysql_options: None,
				operator_class: None,
			}]
		);
	}

	#[cfg(not(feature = "pgvector"))]
	#[test]
	fn partial_index_addition_preserves_where_clause() {
		let key = ("auth".to_string(), "Token".to_string());
		let source = build_project_state(vec![(
			key.clone(),
			build_model_state("auth", "Token", Vec::new(), Vec::new(), Vec::new()),
		)]);
		let mut index = IndexDefinition::new(
			"auth_tokens_user_id_idx",
			vec!["user_id".to_string()],
			false,
		);
		index.where_clause = Some("consumed_at IS NULL".to_string());
		let target = build_project_state(vec![(
			key,
			build_model_state("auth", "Token", Vec::new(), vec![index], Vec::new()),
		)]);

		let operations = MigrationAutodetector::new(source, target).generate_operations();

		assert_eq!(
			operations,
			vec![super::super::Operation::CreateIndex {
				table: "auth_token".to_string(),
				columns: vec!["user_id".to_string()],
				unique: false,
				index_type: None,
				where_clause: Some("consumed_at IS NULL".to_string()),
				concurrently: false,
				expressions: None,
				mysql_options: None,
				operator_class: None,
			}]
		);
	}

	#[cfg(not(feature = "pgvector"))]
	#[test]
	fn apply_migration_operations_replays_legacy_partial_index_lifecycle() {
		let create_table = super::super::Operation::CreateTable {
			name: "auth_token".to_string(),
			columns: Vec::new(),
			constraints: Vec::new(),
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		};
		let create_index = super::super::Operation::CreateIndex {
			table: "auth_token".to_string(),
			columns: vec!["user_id".to_string()],
			unique: false,
			index_type: None,
			where_clause: Some("consumed_at IS NULL".to_string()),
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		};
		let drop_index = super::super::Operation::DropIndex {
			table: "auth_token".to_string(),
			columns: vec!["user_id".to_string()],
		};
		let mut state = ProjectState::new();

		state.apply_migration_operations(&[create_table, create_index], "auth");
		let model = state
			.find_model_by_table("auth_token")
			.expect("legacy index replay should create the model index");
		assert_eq!(model.indexes.len(), 1);
		assert_eq!(
			model.indexes[0].where_clause.as_deref(),
			Some("consumed_at IS NULL")
		);

		state.apply_migration_operations(&[drop_index], "auth");
		assert!(
			state
				.find_model_by_table("auth_token")
				.expect("model should remain after dropping its index")
				.indexes
				.is_empty()
		);
	}

	#[cfg(not(feature = "pgvector"))]
	#[test]
	fn replay_drop_index_removes_only_the_legacy_physical_index() {
		// Arrange
		let mut model = ModelState::new("blog", "Post");
		model.table_name = "blog_posts".to_string();
		model.indexes.push(IndexDefinition::new(
			"idx_blog_posts_author_id",
			vec!["author_id".to_string()],
			false,
		));
		model.indexes.push(IndexDefinition::new(
			"author_id_custom_idx",
			vec!["author_id".to_string()],
			true,
		));
		let mut state = ProjectState::new();
		state.add_model(model);
		let drop_index = super::super::Operation::DropIndex {
			table: "blog_posts".to_string(),
			columns: vec!["author_id".to_string()],
		};

		// Act
		state.apply_migration_operations(&[drop_index], "blog");

		// Assert
		let model = state
			.find_model_by_table("blog_posts")
			.expect("replayed model should exist");
		let names: Vec<_> = model
			.indexes
			.iter()
			.map(|index| index.name.as_str())
			.collect();
		assert_eq!(names, vec!["author_id_custom_idx"]);
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn partial_index_addition_preserves_where_clause_with_pgvector_enabled() {
		let key = ("auth".to_string(), "Token".to_string());
		let source = build_project_state(vec![(
			key.clone(),
			build_model_state("auth", "Token", Vec::new(), Vec::new(), Vec::new()),
		)]);
		let mut index = IndexDefinition::new(
			"auth_tokens_user_id_idx",
			vec!["user_id".to_string()],
			false,
		);
		index.where_clause = Some("consumed_at IS NULL".to_string());
		let target = build_project_state(vec![(
			key,
			build_model_state("auth", "Token", Vec::new(), vec![index], Vec::new()),
		)]);

		let operations = MigrationAutodetector::new(source, target).generate_operations();

		assert_eq!(
			operations,
			vec![super::super::Operation::CreateNamedIndex {
				table: "auth_token".to_string(),
				name: "auth_tokens_user_id_idx".to_string(),
				columns: vec!["user_id".to_string()],
				unique: false,
				index_type: None,
				where_clause: Some("consumed_at IS NULL".to_string()),
				concurrently: false,
				expressions: None,
				mysql_options: None,
				operator_class: None,
			}]
		);
	}

	#[cfg(not(feature = "pgvector"))]
	#[test]
	fn ordinary_index_removal_preserves_definition_for_rollback() {
		let key = ("catalog".to_string(), "Product".to_string());
		let source = build_project_state(vec![(
			key.clone(),
			build_model_state(
				"catalog",
				"Product",
				Vec::new(),
				vec![IndexDefinition::new(
					"product_sku_idx",
					vec!["sku".to_string()],
					false,
				)],
				Vec::new(),
			),
		)]);
		let target = build_project_state(vec![(
			key,
			build_model_state("catalog", "Product", Vec::new(), Vec::new(), Vec::new()),
		)]);

		let operations = MigrationAutodetector::new(source, target).generate_operations();

		assert_eq!(
			operations,
			vec![super::super::Operation::DropNamedIndex {
				table: "catalog_product".to_string(),
				name: "product_sku_idx".to_string(),
				columns: vec!["sku".to_string()],
				unique: false,
				index_type: None,
				where_clause: None,
				concurrently: false,
				expressions: None,
				mysql_options: None,
				operator_class: None,
			}]
		);
	}

	#[cfg(feature = "pgvector")]
	fn vector_index(
		index_type: crate::migrations::operations::IndexType,
		operator_class: &str,
	) -> IndexDefinition {
		IndexDefinition {
			name: "documents_embedding_ann".to_string(),
			fields: vec!["embedding".to_string()],
			unique: false,
			where_clause: None,
			index_type: Some(index_type),
			operator_class: Some(operator_class.to_string()),
			expressions: None,
		}
	}

	#[cfg(feature = "pgvector")]
	fn vector_model(dimensions: usize, indexes: Vec<IndexDefinition>) -> ModelState {
		build_model_state(
			"search",
			"Document",
			vec![FieldState::new(
				"embedding",
				super::super::FieldType::Vector { dimensions },
				false,
			)],
			indexes,
			Vec::new(),
		)
	}

	#[cfg(feature = "pgvector")]
	fn vector_project_state(model: ModelState) -> ProjectState {
		build_project_state(vec![(
			("search".to_string(), "Document".to_string()),
			model,
		)])
	}

	#[cfg(feature = "pgvector")]
	fn duplicate_vector_index_detector() -> MigrationAutodetector {
		let first = vector_model(
			1536,
			vec![vector_index(
				crate::migrations::operations::IndexType::Hnsw {
					m: Some(16),
					ef_construction: Some(64),
				},
				"vector_cosine_ops",
			)],
		);
		let second = build_model_state(
			"billing",
			"Invoice",
			vec![FieldState::new(
				"embedding",
				super::super::FieldType::Vector { dimensions: 1536 },
				false,
			)],
			vec![vector_index(
				crate::migrations::operations::IndexType::Ivfflat { lists: Some(100) },
				"vector_l2_ops",
			)],
			Vec::new(),
		);
		MigrationAutodetector::new(
			ProjectState::new(),
			build_project_state(vec![
				(("search".to_string(), "Document".to_string()), first),
				(("billing".to_string(), "Invoice".to_string()), second),
			]),
		)
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	fn vector_index_duplicate_physical_names_across_models_are_rejected_before_sql() {
		// Arrange
		let detector = duplicate_vector_index_detector();

		// Act
		let error = detector
			.try_generate_operations()
			.expect_err("duplicate physical index names must fail before SQL generation");

		// Assert
		assert!(matches!(
			error,
			super::super::MigrationError::InvalidMigration(message)
				if message.contains("documents_embedding_ann")
					&& message.contains("search_document")
					&& message.contains("billing_invoice")
		));
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn vector_index_name_cannot_collide_with_model_table() {
		// Arrange
		let mut document = vector_model(3, Vec::new());
		document.indexes.push(IndexDefinition {
			name: "billing_invoice".to_string(),
			fields: vec!["embedding".to_string()],
			unique: false,
			where_clause: None,
			index_type: Some(crate::migrations::operations::IndexType::Hnsw {
				m: Some(16),
				ef_construction: Some(64),
			}),
			operator_class: Some("vector_l2_ops".to_string()),
			expressions: None,
		});
		let invoice = build_model_state(
			"billing",
			"Invoice",
			vec![FieldState::new(
				"id",
				super::super::FieldType::BigInteger,
				false,
			)],
			Vec::new(),
			Vec::new(),
		);
		let detector = MigrationAutodetector::new(
			ProjectState::new(),
			build_project_state(vec![
				(("search".to_string(), "Document".to_string()), document),
				(("billing".to_string(), "Invoice".to_string()), invoice),
			]),
		);

		// Act
		let error = detector
			.try_generate_operations()
			.expect_err("index names must not collide with table relations");

		// Assert
		assert!(matches!(
			error,
			super::super::MigrationError::InvalidMigration(message)
				if message.contains("billing_invoice")
		));
	}

	#[cfg(feature = "pgvector")]
	#[test]
	#[should_panic(expected = "use try_generate_operations()")]
	fn infallible_generate_operations_cannot_emit_duplicate_physical_names() {
		duplicate_vector_index_detector().generate_operations();
	}

	#[cfg(feature = "pgvector")]
	#[test]
	#[should_panic(expected = "use try_generate_migrations()")]
	fn infallible_generate_migrations_cannot_emit_duplicate_physical_names() {
		duplicate_vector_index_detector().generate_migrations();
	}

	#[cfg(feature = "pgvector")]
	#[test]
	#[should_panic(expected = "use try_generate_migrations_with_warnings()")]
	fn infallible_generate_migrations_with_warnings_cannot_emit_duplicate_physical_names() {
		duplicate_vector_index_detector().generate_migrations_with_warnings();
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn checked_generate_migrations_rejects_duplicate_physical_names() {
		let error = duplicate_vector_index_detector()
			.try_generate_migrations_with_warnings()
			.expect_err("checked migration generation must reject duplicate physical names");

		assert!(matches!(
			error,
			super::super::MigrationError::InvalidMigration(message)
				if message.contains("documents_embedding_ann")
					&& message.contains("search_document")
					&& message.contains("billing_invoice")
		));
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	fn vector_index_addition_emits_one_create_operation() {
		// Arrange
		let index = vector_index(
			crate::migrations::operations::IndexType::Hnsw {
				m: Some(16),
				ef_construction: Some(64),
			},
			"vector_cosine_ops",
		);
		let target_state = vector_project_state(vector_model(1536, vec![index]));
		let detector = MigrationAutodetector::new(
			vector_project_state(vector_model(1536, Vec::new())),
			target_state.clone(),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert_eq!(operations.len(), 1);
		assert!(matches!(
			&operations[0],
			super::super::Operation::CreateNamedIndex {
				name,
				table,
				columns,
				index_type: Some(crate::migrations::operations::IndexType::Hnsw {
					m: Some(16),
					ef_construction: Some(64),
				}),
				operator_class: Some(operator_class),
				..
			} if name == "documents_embedding_ann"
				&& table == "search_document"
				&& columns == &vec!["embedding".to_string()]
				&& operator_class == "vector_cosine_ops"
		));
		let sql = operations[0]
			.try_to_sql(&crate::migrations::operations::SqlDialect::Postgres)
			.expect("named vector index must render as PostgreSQL SQL");
		assert_eq!(
			sql,
			"CREATE INDEX documents_embedding_ann ON search_document USING hnsw (embedding vector_cosine_ops) WITH (m = 16, ef_construction = 64);"
		);
		assert_eq!(
			operations[0]
				.to_reverse_sql(
					&crate::migrations::operations::SqlDialect::Postgres,
					&ProjectState::new(),
				)
				.expect("named vector index must render reverse SQL"),
			Some(vec!["DROP INDEX documents_embedding_ann;".to_string()])
		);
		let mut replayed_state = vector_project_state(vector_model(1536, Vec::new()));
		replayed_state.apply_migration_operations(&operations, "search");
		assert_eq!(
			replayed_state
				.get_model("search", "Document")
				.expect("replayed model")
				.indexes,
			target_state
				.get_model("search", "Document")
				.expect("target model")
				.indexes
		);
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	fn vector_index_removal_emits_one_drop_operation() {
		// Arrange
		let index = vector_index(
			crate::migrations::operations::IndexType::Ivfflat { lists: Some(100) },
			"vector_l2_ops",
		);
		let detector = MigrationAutodetector::new(
			vector_project_state(vector_model(1536, vec![index])),
			vector_project_state(vector_model(1536, Vec::new())),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert_eq!(operations.len(), 1);
		assert!(matches!(
			&operations[0],
			super::super::Operation::DropNamedIndex { table, name, .. }
				if table == "search_document"
					&& name == "documents_embedding_ann"
		));
		let sql = operations[0]
			.try_to_sql(&crate::migrations::operations::SqlDialect::Postgres)
			.expect("named vector index drop must render as PostgreSQL SQL");
		assert_eq!(sql, "DROP INDEX documents_embedding_ann;");
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	#[case(
		crate::migrations::operations::IndexType::Hnsw {
			m: Some(16),
			ef_construction: Some(64),
		},
		"vector_l2_ops",
		crate::migrations::operations::IndexType::Ivfflat { lists: Some(100) },
		"vector_l2_ops"
	)]
	#[case(
		crate::migrations::operations::IndexType::Hnsw {
			m: Some(16),
			ef_construction: Some(64),
		},
		"vector_l2_ops",
		crate::migrations::operations::IndexType::Hnsw {
			m: Some(16),
			ef_construction: Some(64),
		},
		"vector_cosine_ops"
	)]
	#[case(
		crate::migrations::operations::IndexType::Hnsw {
			m: Some(16),
			ef_construction: Some(64),
		},
		"vector_l2_ops",
		crate::migrations::operations::IndexType::Hnsw {
			m: Some(24),
			ef_construction: Some(64),
		},
		"vector_l2_ops"
	)]
	#[case(
		crate::migrations::operations::IndexType::Hnsw {
			m: Some(16),
			ef_construction: Some(64),
		},
		"vector_l2_ops",
		crate::migrations::operations::IndexType::Hnsw {
			m: Some(16),
			ef_construction: Some(96),
		},
		"vector_l2_ops"
	)]
	#[case(
		crate::migrations::operations::IndexType::Ivfflat { lists: Some(100) },
		"vector_l2_ops",
		crate::migrations::operations::IndexType::Ivfflat { lists: Some(200) },
		"vector_l2_ops"
	)]
	fn vector_index_property_change_emits_drop_then_create(
		#[case] from_type: crate::migrations::operations::IndexType,
		#[case] from_opclass: &str,
		#[case] to_type: crate::migrations::operations::IndexType,
		#[case] to_opclass: &str,
	) {
		// Arrange
		let detector = MigrationAutodetector::new(
			vector_project_state(vector_model(
				1536,
				vec![vector_index(from_type, from_opclass)],
			)),
			vector_project_state(vector_model(1536, vec![vector_index(to_type, to_opclass)])),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert_eq!(operations.len(), 2);
		assert!(matches!(
			&operations[0],
			super::super::Operation::DropNamedIndex { .. }
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::CreateNamedIndex {
				index_type: Some(actual_type),
				operator_class: Some(actual_opclass),
				..
			} if actual_type == &to_type && actual_opclass == to_opclass
		));
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	fn vector_index_identical_metadata_emits_no_operation() {
		// Arrange
		let index = vector_index(
			crate::migrations::operations::IndexType::Hnsw {
				m: Some(16),
				ef_construction: Some(64),
			},
			"vector_ip_ops",
		);
		let detector = MigrationAutodetector::new(
			vector_project_state(vector_model(1536, vec![index.clone()])),
			vector_project_state(vector_model(1536, vec![index])),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert!(operations.is_empty());
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	fn vector_index_dimension_change_replaces_index_around_alter_column() {
		// Arrange
		let index = vector_index(
			crate::migrations::operations::IndexType::Hnsw {
				m: Some(16),
				ef_construction: Some(64),
			},
			"vector_cosine_ops",
		);
		let detector = MigrationAutodetector::new(
			vector_project_state(vector_model(768, vec![index.clone()])),
			vector_project_state(vector_model(1536, vec![index])),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert_eq!(operations.len(), 3);
		assert!(matches!(
			&operations[0],
			super::super::Operation::DropNamedIndex { .. }
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::AlterColumn {
				column,
				new_definition,
				..
			} if column == "embedding"
				&& new_definition.type_definition
					== (super::super::FieldType::Vector { dimensions: 1536 })
		));
		assert!(matches!(
			&operations[2],
			super::super::Operation::CreateNamedIndex {
				index_type: Some(crate::migrations::operations::IndexType::Hnsw {
					m: Some(16),
					ef_construction: Some(64),
				}),
				operator_class: Some(operator_class),
				..
			} if operator_class == "vector_cosine_ops"
		));
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	fn vector_dimension_change_replaces_expression_index_around_alter_column() {
		// Arrange
		let mut index = vector_index(
			crate::migrations::operations::IndexType::Hnsw {
				m: Some(16),
				ef_construction: Some(64),
			},
			"vector_cosine_ops",
		);
		index.fields.clear();
		index.expressions = Some(vec!["normalize(embedding)".to_string()]);
		let detector = MigrationAutodetector::new(
			vector_project_state(vector_model(768, vec![index.clone()])),
			vector_project_state(vector_model(1536, vec![index])),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert_eq!(operations.len(), 3);
		assert!(matches!(
			&operations[0],
			super::super::Operation::DropNamedIndex {
				expressions: Some(expressions),
				..
			} if expressions == &vec!["normalize(embedding)".to_string()]
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::AlterColumn { column, .. } if column == "embedding"
		));
		assert!(matches!(
			&operations[2],
			super::super::Operation::CreateNamedIndex {
				expressions: Some(expressions),
				..
			} if expressions == &vec!["normalize(embedding)".to_string()]
		));
	}

	#[rstest]
	fn altered_enum_column_recreates_unchanged_domain_constraint() {
		// Arrange
		let from_field = FieldState::new("status", super::super::FieldType::VarChar(16), false);
		let to_field = FieldState::new("status", super::super::FieldType::VarChar(32), false);
		let domain = crate::field_domain::FieldDomain::Enum {
			repr: crate::field_domain::ModelEnumRepr::String,
			values: vec![
				crate::field_domain::ModelEnumValue::String("queued".to_string()),
				crate::field_domain::ModelEnumValue::String("completed".to_string()),
			],
		};
		let constraint =
			ConstraintDefinition::enum_domain("ck_accounts_user_status_enum", "status", domain);
		let from_model = build_model_state(
			"accounts",
			"User",
			vec![from_field],
			Vec::new(),
			vec![constraint.clone()],
		);
		let to_model = build_model_state(
			"accounts",
			"User",
			vec![to_field],
			Vec::new(),
			vec![constraint],
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				to_model,
			)]),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert_eq!(operations.len(), 3, "unexpected operations: {operations:?}");
		assert!(matches!(
			&operations[0],
			super::super::Operation::DropConstraintDefinition { table, constraint }
				if table == "accounts_user"
					&& constraint.name() == "ck_accounts_user_status_enum"
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::AlterColumn { table, column, .. }
				if table == "accounts_user" && column == "status"
		));
		assert!(matches!(
			&operations[2],
			super::super::Operation::AddConstraintDefinition { table, constraint }
				if table == "accounts_user"
					&& constraint.name() == "ck_accounts_user_status_enum"
		));
	}

	#[rstest]
	fn apply_migration_operations_replays_foreign_key_add_constraint() {
		// Arrange
		let create_posts = super::super::Operation::CreateTable {
			name: "blog_posts".to_string(),
			columns: vec![
				super::super::ColumnDefinition {
					name: "id".to_string(),
					type_definition: super::super::FieldType::BigInteger,
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: true,
					default: None,
					generated: None,
					domain: None,
				},
				super::super::ColumnDefinition {
					name: "user_id".to_string(),
					type_definition: super::super::FieldType::BigInteger,
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
					generated: None,
					domain: None,
				},
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		};
		let add_user_fk = super::super::Operation::AddConstraint {
				table: "blog_posts".to_string(),
				constraint_sql: "CONSTRAINT blog_posts_user_id_fk FOREIGN KEY (user_id) REFERENCES auth_users(id) ON DELETE CASCADE ON UPDATE NO ACTION".to_string(),
			};
		let mut state = ProjectState::new();

		// Act
		state.apply_migration_operations(&[create_posts, add_user_fk], "blog");

		// Assert
		let model = state
			.find_model_by_table("blog_posts")
			.expect("blog_posts model should be reconstructed");
		let constraint = model
			.constraints
			.iter()
			.find(|constraint| constraint.name == "blog_posts_user_id_fk")
			.expect("foreign key constraint should be reconstructed");
		assert_eq!(constraint.constraint_type, "foreign_key");
		assert_eq!(constraint.fields, vec!["user_id".to_string()]);
		let fk_info = constraint
			.foreign_key_info
			.as_ref()
			.expect("foreign key metadata should be reconstructed");
		assert_eq!(fk_info.referenced_table, "auth_users");
		assert_eq!(fk_info.referenced_columns, vec!["id".to_string()]);
		assert_eq!(fk_info.on_delete, ForeignKeyAction::Cascade);
		assert_eq!(fk_info.on_update, ForeignKeyAction::NoAction);
	}

	#[test]
	fn generated_indexes_are_replayed_without_second_migration() {
		// Arrange
		let mut target = ProjectState::new();
		let mut post = ModelState::new("blog", "Post");
		post.table_name = "blog_posts".to_string();
		post.add_field(FieldState::new("id", FieldType::Integer, false));
		post.add_field(FieldState::new("author_id", FieldType::Uuid, false));
		post.indexes.push(IndexDefinition::new(
			"idx_blog_posts_author_id",
			vec!["author_id".to_string()],
			false,
		));
		target.add_model(post);

		// Act
		let operations =
			MigrationAutodetector::new(ProjectState::new(), target.clone()).generate_operations();
		let mut replayed = ProjectState::new();
		replayed.apply_migration_operations(&operations, "blog");
		let second_run = MigrationAutodetector::new(replayed.clone(), target).generate_operations();

		// Assert
		assert_eq!(
			operations
				.iter()
				.filter(|operation| match operation {
					super::super::Operation::CreateIndex { .. }
					| super::super::Operation::CreateIndexRepair { .. } => true,
					#[cfg(feature = "pgvector")]
					super::super::Operation::CreateNamedIndex { .. } => true,
					_ => false,
				})
				.count(),
			1
		);
		assert_eq!(
			replayed
				.find_model_by_table("blog_posts")
				.unwrap()
				.indexes
				.len(),
			1
		);
		assert!(
			second_run.is_empty(),
			"unexpected second migration: {second_run:?}"
		);
	}

	#[test]
	fn replays_advanced_indexes_for_replacement_and_removal() {
		// Arrange
		let create_advanced_index = super::super::Operation::CreateIndex {
			table: "blog_posts".to_string(),
			columns: vec!["slug".to_string()],
			unique: false,
			index_type: None,
			where_clause: Some("published = TRUE".to_string()),
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		};
		let mut replayed = ProjectState::new();
		let mut old_model = ModelState::new("blog", "Post");
		old_model.table_name = "blog_posts".to_string();
		old_model.add_field(FieldState::new("slug", FieldType::VarChar(255), false));
		replayed.add_model(old_model);
		replayed.apply_migration_operations(&[create_advanced_index], "blog");
		let mut replacement_model = ModelState::new("blog", "Post");
		replacement_model.table_name = "blog_posts".to_string();
		replacement_model.add_field(FieldState::new("slug", FieldType::VarChar(255), false));
		replacement_model.indexes.push(IndexDefinition::new(
			"idx_blog_posts_slug",
			vec!["slug".to_string()],
			false,
		));
		let mut replacement_target = ProjectState::new();
		replacement_target.add_model(replacement_model);

		// Act
		let replacement_operations =
			MigrationAutodetector::new(replayed.clone(), replacement_target).generate_operations();
		let mut removal_target = ProjectState::new();
		let mut removal_model = ModelState::new("blog", "Post");
		removal_model.table_name = "blog_posts".to_string();
		removal_model.add_field(FieldState::new("slug", FieldType::VarChar(255), false));
		removal_target.add_model(removal_model);
		let removal_operations =
			MigrationAutodetector::new(replayed, removal_target).generate_operations();

		// Assert
		let drop_position = replacement_operations
			.iter()
			.position(|operation| {
				matches!(operation, super::super::Operation::DropNamedIndex { .. })
			})
			.expect("advanced index replacement should drop the old index");
		let create_position = replacement_operations
			.iter()
			.position(|operation| {
				matches!(
					operation,
					super::super::Operation::CreateIndex { .. }
						| super::super::Operation::CreateIndexRepair { .. }
				)
			})
			.expect("advanced index replacement should create the ordinary index");
		assert!(drop_position < create_position);
		assert_eq!(
			removal_operations
				.iter()
				.filter(|operation| {
					matches!(operation, super::super::Operation::DropNamedIndex { .. })
				})
				.count(),
			1
		);
	}

	#[cfg(not(feature = "pgvector"))]
	#[test]
	fn create_index_replay_preserves_concurrently_through_reverse_sql() {
		// Arrange
		let create_index = super::super::Operation::CreateIndex {
			table: "blog_posts".to_string(),
			columns: vec!["slug".to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: true,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		};
		let mut replayed = ProjectState::new();
		let mut replayed_model = ModelState::new("blog", "Post");
		replayed_model.table_name = "blog_posts".to_string();
		replayed_model.add_field(FieldState::new("slug", FieldType::VarChar(255), false));
		replayed.add_model(replayed_model);
		replayed.apply_migration_operations(&[create_index], "blog");

		let mut removal_target = ProjectState::new();
		let mut removal_model = ModelState::new("blog", "Post");
		removal_model.table_name = "blog_posts".to_string();
		removal_model.add_field(FieldState::new("slug", FieldType::VarChar(255), false));
		removal_target.add_model(removal_model);

		// Act
		let removal_operations =
			MigrationAutodetector::new(replayed.clone(), removal_target).generate_operations();
		let reverse = removal_operations[0]
			.to_reverse_operation(&replayed)
			.expect("named index removal should be reversible")
			.expect("named index removal should restore its definition");

		// Assert
		assert!(matches!(
			removal_operations.as_slice(),
			[super::super::Operation::DropNamedIndex {
				concurrently: true,
				..
			}]
		));
		assert!(matches!(
			&reverse,
			super::super::Operation::CreateIndexRepair {
				concurrently: true,
				..
			}
		));
		let sql = reverse.to_sql(&super::super::operations::SqlDialect::Postgres);
		assert!(
			sql.starts_with("CREATE INDEX CONCURRENTLY "),
			"reverse SQL must preserve CONCURRENTLY: {sql}"
		);
	}

	#[cfg(not(feature = "pgvector"))]
	#[test]
	fn create_index_repair_replay_preserves_mysql_options_through_reverse_sql() {
		// Arrange
		let mysql_options = super::super::operations::AlterTableOptions::new()
			.with_algorithm(super::super::operations::MySqlAlgorithm::Inplace)
			.with_lock(super::super::operations::MySqlLock::None);
		let create_index = super::super::Operation::CreateIndexRepair {
			table: "blog_posts".to_string(),
			name: Some("custom_slug_idx".to_string()),
			columns: vec!["slug".to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: Some(mysql_options),
			operator_class: None,
		};
		let mut replayed = ProjectState::new();
		let mut replayed_model = ModelState::new("blog", "Post");
		replayed_model.table_name = "blog_posts".to_string();
		replayed_model.add_field(FieldState::new("slug", FieldType::VarChar(255), false));
		replayed.add_model(replayed_model);
		replayed.apply_migration_operations(&[create_index], "blog");

		let mut removal_target = ProjectState::new();
		let mut removal_model = ModelState::new("blog", "Post");
		removal_model.table_name = "blog_posts".to_string();
		removal_model.add_field(FieldState::new("slug", FieldType::VarChar(255), false));
		removal_target.add_model(removal_model);

		// Act
		let removal_operations =
			MigrationAutodetector::new(replayed.clone(), removal_target).generate_operations();
		let reverse = removal_operations[0]
			.to_reverse_operation(&replayed)
			.expect("named index removal should be reversible")
			.expect("named index removal should restore its definition");

		// Assert
		assert!(matches!(
			removal_operations.as_slice(),
			[super::super::Operation::DropNamedIndex {
				mysql_options: Some(options),
				..
			}] if *options == mysql_options
		));
		assert!(matches!(
			&reverse,
			super::super::Operation::CreateIndexRepair {
				mysql_options: Some(options),
				..
			} if *options == mysql_options
		));
		let sql = reverse.to_sql(&super::super::operations::SqlDialect::Mysql);
		assert!(
			sql.ends_with(", ALGORITHM=INPLACE, LOCK=NONE;"),
			"reverse SQL must preserve MySQL index options: {sql}"
		);
	}

	#[test]
	fn legacy_advanced_index_sidecar_defaults_execution_hints() {
		// Arrange
		let index = IndexDefinition::new("custom_slug_idx", vec!["slug".to_string()], false);
		let mut from_model = ModelState::new("blog", "Post");
		from_model.table_name = "blog_posts".to_string();
		from_model.add_field(FieldState::new("slug", FieldType::VarChar(255), false));
		from_model
			.options
			.insert(advanced_index_option_key(&index.name), "true".to_string());
		from_model.indexes.push(index);
		let mut from_state = ProjectState::new();
		from_state.add_model(from_model);

		let mut to_model = ModelState::new("blog", "Post");
		to_model.table_name = "blog_posts".to_string();
		to_model.add_field(FieldState::new("slug", FieldType::VarChar(255), false));
		let mut to_state = ProjectState::new();
		to_state.add_model(to_model);

		// Act
		let operations = MigrationAutodetector::new(from_state, to_state).generate_operations();

		// Assert
		assert!(matches!(
			operations.as_slice(),
			[super::super::Operation::DropNamedIndex {
				concurrently: false,
				mysql_options: None,
				..
			}]
		));
	}

	#[test]
	fn equivalent_index_replay_attaches_execution_state_to_existing_name() {
		// Arrange
		let mut replayed_model = ModelState::new("blog", "Post");
		replayed_model.table_name = "blog_posts".to_string();
		replayed_model.add_field(FieldState::new("slug", FieldType::VarChar(255), false));
		replayed_model.indexes.push(IndexDefinition::new(
			"custom_slug_idx",
			vec!["slug".to_string()],
			false,
		));
		let mut replayed = ProjectState::new();
		replayed.add_model(replayed_model);
		let create_index = super::super::Operation::CreateIndex {
			table: "blog_posts".to_string(),
			columns: vec!["slug".to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: true,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		};

		// Act
		replayed.apply_migration_operations(&[create_index], "blog");
		let mut removal_model = ModelState::new("blog", "Post");
		removal_model.table_name = "blog_posts".to_string();
		removal_model.add_field(FieldState::new("slug", FieldType::VarChar(255), false));
		let mut removal_target = ProjectState::new();
		removal_target.add_model(removal_model);
		let operations = MigrationAutodetector::new(replayed, removal_target).generate_operations();

		// Assert
		assert!(matches!(
			operations.as_slice(),
			[super::super::Operation::DropNamedIndex {
				name,
				concurrently: true,
				..
			}] if name == "custom_slug_idx"
		));
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn replays_expression_index_name_and_definition_for_removal() {
		// Arrange
		let create_expression_index = super::super::Operation::CreateIndex {
			table: "blog_posts".to_string(),
			columns: vec!["slug".to_string()],
			unique: true,
			index_type: Some(super::super::operations::IndexType::BTree),
			where_clause: Some("published = TRUE".to_string()),
			concurrently: false,
			expressions: Some(vec!["LOWER(slug)".to_string()]),
			mysql_options: None,
			operator_class: None,
		};
		let mut replayed = ProjectState::new();
		let mut old_model = ModelState::new("blog", "Post");
		old_model.table_name = "blog_posts".to_string();
		old_model.add_field(FieldState::new("slug", FieldType::VarChar(255), false));
		replayed.add_model(old_model);
		replayed.apply_migration_operations(&[create_expression_index], "blog");
		let replayed_index = &replayed
			.find_model_by_table("blog_posts")
			.expect("replayed model")
			.indexes[0];
		assert_eq!(replayed_index.name, "idx_blog_posts_expr");
		assert_eq!(
			replayed_index.expressions,
			Some(vec!["LOWER(slug)".to_string()])
		);

		let mut removal_target = ProjectState::new();
		let mut target_model = ModelState::new("blog", "Post");
		target_model.table_name = "blog_posts".to_string();
		target_model.add_field(FieldState::new("slug", FieldType::VarChar(255), false));
		removal_target.add_model(target_model);

		// Act
		let removal_operations =
			MigrationAutodetector::new(replayed, removal_target).generate_operations();

		// Assert
		assert!(matches!(
			removal_operations.as_slice(),
			[super::super::Operation::DropNamedIndex {
				name,
				unique: true,
				where_clause: Some(predicate),
				expressions: Some(expressions),
				..
			}] if name == "idx_blog_posts_expr"
				&& predicate == "published = TRUE"
				&& expressions == &["LOWER(slug)".to_string()]
		));
	}

	#[test]
	fn detects_same_table_index_name_changes() {
		// Arrange
		let index = |name: &str| IndexDefinition::new(name, vec!["email".to_string()], false);
		let from_model = build_model_state(
			"blog",
			"Post",
			vec![FieldState::new("email", FieldType::VarChar(255), false)],
			vec![index("old_email_idx")],
			Vec::new(),
		);
		let to_model = build_model_state(
			"blog",
			"Post",
			vec![FieldState::new("email", FieldType::VarChar(255), false)],
			vec![index("new_email_idx")],
			Vec::new(),
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(("blog".to_string(), "Post".to_string()), from_model)]),
			build_project_state(vec![(("blog".to_string(), "Post".to_string()), to_model)]),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert!(matches!(
			operations.as_slice(),
			[
				super::super::Operation::DropNamedIndex { name: old, .. },
				super::super::Operation::CreateIndexRepair { name: Some(new), .. },
			] if old == "old_email_idx" && new == "new_email_idx"
		));
	}

	#[test]
	fn replays_drop_index_only_removes_generated_name() {
		// Arrange
		let mut model = ModelState::new("blog", "Post");
		model.table_name = "blog_posts".to_string();
		model.add_field(FieldState::new("email", FieldType::VarChar(255), false));
		model.indexes = vec![
			IndexDefinition::new("idx_blog_posts_email", vec!["email".to_string()], false),
			IndexDefinition::new("custom_email_idx", vec!["email".to_string()], true),
		];
		let mut state = ProjectState::new();
		state.add_model(model);
		let drop = super::super::Operation::DropIndex {
			table: "blog_posts".to_string(),
			columns: vec!["email".to_string()],
		};

		// Act
		state.apply_migration_operations(&[drop], "blog");

		// Assert
		let indexes = &state
			.find_model_by_table("blog_posts")
			.expect("replayed model")
			.indexes;
		assert_eq!(indexes.len(), 1);
		assert_eq!(indexes[0].name, "custom_email_idx");
	}

	#[test]
	fn replays_drop_column_predicate_and_ignores_function_name() {
		// Arrange
		assert!(!ProjectState::expression_references_column(
			"LOWER(email)",
			"lower"
		));
		assert!(ProjectState::expression_references_column(
			"LOWER(email)",
			"email"
		));
		let mut model = ModelState::new("blog", "Post");
		model.table_name = "blog_posts".to_string();
		model.add_field(FieldState::new("email", FieldType::VarChar(255), false));
		model.add_field(FieldState::new("active", FieldType::Boolean, false));
		let mut state = ProjectState::new();
		state.add_model(model);
		let create = super::super::Operation::CreateIndexRepair {
			table: "blog_posts".to_string(),
			name: Some("active_email_idx".to_string()),
			columns: vec!["email".to_string()],
			unique: false,
			index_type: None,
			where_clause: Some("active = TRUE".to_string()),
			concurrently: false,
			expressions: Some(vec!["LOWER(email)".to_string()]),
			mysql_options: None,
			operator_class: None,
		};

		// Act
		state.apply_migration_operations(&[create], "blog");
		state.apply_migration_operations(
			&[super::super::Operation::DropColumn {
				table: "blog_posts".to_string(),
				column: "active".to_string(),
				old_definition: None,
			}],
			"blog",
		);

		// Assert
		assert!(
			state
				.find_model_by_table("blog_posts")
				.expect("replayed model")
				.indexes
				.is_empty()
		);
	}

	#[test]
	fn generate_migrations_recreates_generated_indexes_around_cross_app_move() {
		// Arrange
		let old_index =
			IndexDefinition::new("idx_legacy_user_email", vec!["email".to_string()], true);
		let new_index = IndexDefinition {
			name: "idx_accounts_user_email".to_string(),
			..old_index.clone()
		};
		let old_constraint = ConstraintDefinition {
			name: "legacy_user_email_uniq".to_string(),
			constraint_type: "unique".to_string(),
			fields: vec!["email".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let new_constraint = ConstraintDefinition {
			name: "accounts_user_email_uniq".to_string(),
			..old_constraint.clone()
		};
		let old_model = build_model_state(
			"legacy",
			"User",
			vec![
				FieldState::new("id", FieldType::Integer, false),
				FieldState::new("email", FieldType::VarChar(255), false),
			],
			vec![old_index],
			vec![old_constraint.clone()],
		);
		let new_model = build_model_state(
			"accounts",
			"User",
			vec![
				FieldState::new("id", FieldType::Integer, false),
				FieldState::new("email", FieldType::VarChar(255), false),
			],
			vec![new_index],
			vec![new_constraint],
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("legacy".to_string(), "User".to_string()),
				old_model,
			)]),
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				new_model,
			)]),
		);

		// Act
		let migrations = detector
			.try_generate_migrations()
			.expect("cross-app move should generate a migration");

		// Assert
		let operations = &migrations[0].operations;
		assert!(matches!(
			operations.as_slice(),
			[
				super::super::Operation::DropConstraintDefinition {
					table: drop_table,
					constraint: super::super::Constraint::Unique {
						name: old_constraint_name,
						columns: old_columns,
					},
				},
				super::super::Operation::DropNamedIndex { name, .. },
				super::super::Operation::MoveModel { .. },
				super::super::Operation::AddConstraint {
					table: add_table,
					constraint_sql,
				},
				create,
			] if drop_table == "legacy_user"
				&& old_constraint_name == "legacy_user_email_uniq"
				&& old_columns == &["email"]
				&& name == "idx_legacy_user_email"
				&& add_table == "accounts_user"
				&& constraint_sql == "CONSTRAINT accounts_user_email_uniq UNIQUE (email)"
				&& matches!(
					create,
					super::super::Operation::CreateIndex {
						table,
						columns,
						unique: true,
						..
					} | super::super::Operation::CreateIndexRepair {
						table,
						columns,
						unique: true,
						..
					}
					if table == "accounts_user" && columns == &["email".to_string()]
				)
		));
		assert_eq!(
			operations[0]
				.to_reverse_operation(&ProjectState::new())
				.expect("typed unique constraint drop should be reversible"),
			Some(super::super::Operation::AddConstraintDefinition {
				table: "legacy_user".to_string(),
				constraint: old_constraint.to_constraint(),
			})
		);
	}

	#[test]
	fn reorders_index_recreation_around_column_rename() {
		// Arrange
		let create_index = |columns: &[&str]| super::super::Operation::CreateIndex {
			table: "blog_posts".to_string(),
			columns: columns.iter().map(|column| (*column).to_string()).collect(),
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		};
		let mut operations = vec![
			create_index(&["slug_new"]),
			super::super::Operation::RenameColumn {
				table: "blog_posts".to_string(),
				old_name: "slug".to_string(),
				new_name: "slug_new".to_string(),
			},
			super::super::Operation::DropIndex {
				table: "blog_posts".to_string(),
				columns: vec!["slug".to_string()],
			},
		];

		// Act
		MigrationAutodetector::order_renamed_column_operations(&mut operations);

		// Assert
		assert!(matches!(
			&operations[..],
			[
				super::super::Operation::DropIndex { .. },
				super::super::Operation::RenameColumn { .. },
				super::super::Operation::CreateIndex { .. },
			]
		));
	}

	#[test]
	fn reorders_predicate_index_drop_before_column_rename() {
		// Arrange
		let mut operations = vec![
			super::super::Operation::RenameColumn {
				table: "blog_posts".to_string(),
				old_name: "active_old".to_string(),
				new_name: "active_new".to_string(),
			},
			super::super::Operation::DropNamedIndex {
				table: "blog_posts".to_string(),
				name: "active_email_idx".to_string(),
				columns: vec!["email".to_string()],
				unique: false,
				index_type: None,
				where_clause: Some("active_old = TRUE".to_string()),
				concurrently: false,
				expressions: None,
				mysql_options: None,
				operator_class: None,
			},
		];

		// Act
		MigrationAutodetector::order_renamed_column_operations(&mut operations);

		// Assert
		assert!(matches!(
			&operations[..],
			[
				super::super::Operation::DropNamedIndex { .. },
				super::super::Operation::RenameColumn { .. },
			]
		));
	}

	#[test]
	fn drops_replaced_index_before_creating_new_definition() {
		// Arrange
		let old_model = build_model_state(
			"blog",
			"Post",
			vec![FieldState::new("email", FieldType::VarChar(255), false)],
			vec![IndexDefinition::new(
				"idx_blog_post_email",
				vec!["email".to_string()],
				false,
			)],
			Vec::new(),
		);
		let new_model = build_model_state(
			"blog",
			"Post",
			vec![FieldState::new("email", FieldType::VarChar(255), false)],
			vec![IndexDefinition::new(
				"idx_blog_post_email",
				vec!["email".to_string()],
				true,
			)],
			Vec::new(),
		);
		let from_state =
			build_project_state(vec![(("blog".to_string(), "Post".to_string()), old_model)]);
		let to_state =
			build_project_state(vec![(("blog".to_string(), "Post".to_string()), new_model)]);

		// Act
		let operations = MigrationAutodetector::new(from_state, to_state).generate_operations();

		// Assert
		let drop_position = operations
			.iter()
			.position(|operation| {
				matches!(operation, super::super::Operation::DropNamedIndex { .. })
			})
			.expect("replacing an index should drop the previous definition");
		let create_position = operations
			.iter()
			.position(|operation| {
				matches!(
					operation,
					super::super::Operation::CreateIndex { .. }
						| super::super::Operation::CreateIndexRepair { .. }
				)
			})
			.expect("replacing an index should create the new definition");
		assert!(drop_position < create_position);
	}

	#[rstest]
	fn apply_migration_operations_replays_foreign_key_add_constraint_repair() {
		// Arrange
		let create_posts = super::super::Operation::CreateTable {
			name: "blog_posts".to_string(),
			columns: vec![
				super::super::ColumnDefinition {
					name: "id".to_string(),
					type_definition: super::super::FieldType::BigInteger,
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: true,
					default: None,
					generated: None,
					domain: None,
				},
				super::super::ColumnDefinition {
					name: "user_id".to_string(),
					type_definition: super::super::FieldType::BigInteger,
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
					generated: None,
					domain: None,
				},
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		};
		let repair_user_fk = super::super::Operation::AddConstraintRepair {
				table: "blog_posts".to_string(),
				constraint_sql: "CONSTRAINT blog_posts_user_id_fk FOREIGN KEY (user_id) REFERENCES auth_users(id) ON DELETE CASCADE ON UPDATE NO ACTION".to_string(),
			};
		let mut state = ProjectState::new();

		// Act
		state.apply_migration_operations(&[create_posts, repair_user_fk], "blog");

		// Assert
		let model = state
			.find_model_by_table("blog_posts")
			.expect("blog_posts model should be reconstructed");
		let constraint = model
			.constraints
			.iter()
			.find(|constraint| constraint.name == "blog_posts_user_id_fk")
			.expect("foreign key repair constraint should be reconstructed");
		assert_eq!(constraint.constraint_type, "foreign_key");
		assert_eq!(constraint.fields, vec!["user_id".to_string()]);
	}

	#[rstest]
	fn apply_migration_operations_replays_omitted_foreign_key_actions_as_no_action() {
		// Arrange
		let create_posts = super::super::Operation::CreateTable {
			name: "blog_posts".to_string(),
			columns: vec![
				super::super::ColumnDefinition {
					name: "id".to_string(),
					type_definition: super::super::FieldType::BigInteger,
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: true,
					default: None,
					generated: None,
					domain: None,
				},
				super::super::ColumnDefinition {
					name: "user_id".to_string(),
					type_definition: super::super::FieldType::BigInteger,
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
					generated: None,
					domain: None,
				},
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		};
		let add_user_fk = super::super::Operation::AddConstraint {
			table: "blog_posts".to_string(),
			constraint_sql:
				"CONSTRAINT blog_posts_user_id_fk FOREIGN KEY (user_id) REFERENCES auth_users(id)"
					.to_string(),
		};
		let mut state = ProjectState::new();

		// Act
		state.apply_migration_operations(&[create_posts, add_user_fk], "blog");

		// Assert
		let model = state
			.find_model_by_table("blog_posts")
			.expect("blog_posts model should be reconstructed");
		let fk_info = model
			.constraints
			.iter()
			.find(|constraint| constraint.name == "blog_posts_user_id_fk")
			.and_then(|constraint| constraint.foreign_key_info.as_ref())
			.expect("foreign key metadata should be reconstructed");
		assert_eq!(fk_info.on_delete, ForeignKeyAction::NoAction);
		assert_eq!(fk_info.on_update, ForeignKeyAction::NoAction);
	}

	#[rstest]
	fn generate_operations_emits_rename_column_for_unambiguous_field_rename() {
		let from_model = build_model_state(
			"deployments",
			"Deployment",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				FieldState::new("app_name", super::super::FieldType::VarChar(255), false),
			],
			Vec::new(),
			Vec::new(),
		);
		let to_model = build_model_state(
			"deployments",
			"Deployment",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				FieldState::new("project_name", super::super::FieldType::VarChar(255), false),
			],
			Vec::new(),
			Vec::new(),
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("deployments".to_string(), "Deployment".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("deployments".to_string(), "Deployment".to_string()),
				to_model,
			)]),
		);

		let operations = detector
			.try_generate_operations()
			.expect("unambiguous rename should generate operations");

		assert_eq!(operations.len(), 1, "unexpected operations: {operations:?}");
		assert!(matches!(
			&operations[0],
			super::super::Operation::RenameColumn {
				table,
				old_name,
				new_name
			} if table == "deployments_deployment"
				&& old_name == "app_name"
				&& new_name == "project_name"
		));
	}

	#[rstest]
	fn generate_operations_renames_unique_column_with_constraint_name_change() {
		let id_field = FieldState::new("id", super::super::FieldType::Integer, false);
		let old_slug_field =
			FieldState::new("old_slug", super::super::FieldType::VarChar(255), false);
		let new_slug_field = FieldState::new("slug", super::super::FieldType::VarChar(255), false);
		let from_unique = ConstraintDefinition {
			name: "deployments_deployment_old_slug_uniq".to_string(),
			constraint_type: "unique".to_string(),
			fields: vec!["old_slug".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let to_unique = ConstraintDefinition {
			name: "deployments_deployment_slug_uniq".to_string(),
			constraint_type: "unique".to_string(),
			fields: vec!["slug".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let from_model = build_model_state(
			"deployments",
			"Deployment",
			vec![id_field.clone(), old_slug_field],
			Vec::new(),
			vec![from_unique],
		);
		let to_model = build_model_state(
			"deployments",
			"Deployment",
			vec![id_field, new_slug_field],
			Vec::new(),
			vec![to_unique],
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("deployments".to_string(), "Deployment".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("deployments".to_string(), "Deployment".to_string()),
				to_model,
			)]),
		);

		let operations = detector
			.try_generate_operations()
			.expect("unique column rename should generate operations");

		assert_eq!(operations.len(), 3, "unexpected operations: {operations:?}");
		assert!(matches!(
			&operations[0],
			super::super::Operation::DropConstraintDefinition {
				table,
				constraint: super::super::Constraint::Unique { name, columns },
			} if table == "deployments_deployment"
				&& name == "deployments_deployment_old_slug_uniq"
				&& columns == &["old_slug"]
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::RenameColumn {
				table,
				old_name,
				new_name
			} if table == "deployments_deployment"
				&& old_name == "old_slug"
				&& new_name == "slug"
		));
		assert!(matches!(
			&operations[2],
			super::super::Operation::AddConstraint { constraint_sql, .. }
				if constraint_sql == "CONSTRAINT deployments_deployment_slug_uniq UNIQUE (slug)"
		));

		let state = ProjectState::new();
		let reverse_operations = operations
			.iter()
			.rev()
			.map(|operation| {
				operation
					.to_reverse_operation(&state)
					.expect("unique rename operation should be reversible")
					.expect("unique rename operation should have a reverse operation")
			})
			.collect::<Vec<_>>();
		assert!(matches!(
			reverse_operations.as_slice(),
			[
				super::super::Operation::DropConstraint {
					table: drop_table,
					constraint_name,
				},
				super::super::Operation::RenameColumn {
					table: rename_table,
					old_name,
					new_name,
				},
				super::super::Operation::AddConstraintDefinition {
					table: add_table,
					constraint: super::super::Constraint::Unique { name, columns },
				},
			] if drop_table == "deployments_deployment"
				&& constraint_name == "deployments_deployment_slug_uniq"
				&& rename_table == "deployments_deployment"
				&& old_name == "slug"
				&& new_name == "old_slug"
				&& add_table == "deployments_deployment"
				&& name == "deployments_deployment_old_slug_uniq"
				&& columns == &["old_slug"]
		));

		let reverse_sql = operations
			.iter()
			.rev()
			.flat_map(|operation| {
				operation
					.to_reverse_sql(&super::super::operations::SqlDialect::Postgres, &state)
					.expect("unique rename operation should render reverse SQL")
					.expect("unique rename operation should have reverse SQL")
			})
			.collect::<Vec<_>>();
		assert_eq!(
			reverse_sql,
			vec![
				"ALTER TABLE deployments_deployment DROP CONSTRAINT deployments_deployment_slug_uniq;",
				"ALTER TABLE deployments_deployment RENAME COLUMN \"slug\" TO \"old_slug\";",
				"ALTER TABLE deployments_deployment ADD CONSTRAINT \"deployments_deployment_old_slug_uniq\" UNIQUE (\"old_slug\");",
			]
		);
	}

	#[rstest]
	fn generate_operations_renames_unique_column_from_inline_to_model_constraint() {
		// Arrange
		let id_field = FieldState::new("id", super::super::FieldType::Integer, false);
		let mut old_email_field =
			FieldState::new("old_email", super::super::FieldType::VarChar(255), false);
		old_email_field
			.params
			.insert("unique".to_string(), "true".to_string());
		let new_email_field =
			FieldState::new("email", super::super::FieldType::VarChar(255), false);
		let new_unique = ConstraintDefinition {
			name: "accounts_account_email_uniq".to_string(),
			constraint_type: "unique".to_string(),
			fields: vec!["email".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let from_model = build_model_state(
			"accounts",
			"Account",
			vec![id_field.clone(), old_email_field],
			Vec::new(),
			Vec::new(),
		);
		let to_model = build_model_state(
			"accounts",
			"Account",
			vec![id_field, new_email_field],
			Vec::new(),
			vec![new_unique],
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("accounts".to_string(), "Account".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("accounts".to_string(), "Account".to_string()),
				to_model,
			)]),
		);

		// Act
		let operations = detector
			.try_generate_operations()
			.expect("unique field rename should generate operations");

		// Assert
		assert_eq!(operations.len(), 1, "unexpected operations: {operations:?}");
		assert!(matches!(
			&operations[0],
			super::super::Operation::RenameColumn {
				table,
				old_name,
				new_name
			} if table == "accounts_account"
				&& old_name == "old_email"
				&& new_name == "email"
		));
	}

	#[rstest]
	fn generate_operations_detects_field_rename_from_offline_table_keyed_state() {
		let mut from_model = build_model_state(
			"deployments",
			"DeploymentsDeployment",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				FieldState::new("reinhardt_app_yaml", super::super::FieldType::Text, false),
			],
			Vec::new(),
			Vec::new(),
		);
		from_model.table_name = "deployments_deployment".to_string();
		let to_model = build_model_state(
			"deployments",
			"Deployment",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				FieldState::new("project_yaml", super::super::FieldType::Text, false),
			],
			Vec::new(),
			Vec::new(),
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				(
					"deployments".to_string(),
					"DeploymentsDeployment".to_string(),
				),
				from_model,
			)]),
			build_project_state(vec![(
				("deployments".to_string(), "Deployment".to_string()),
				to_model,
			)]),
		);

		let migrations = detector
			.try_generate_migrations()
			.expect("table-name matched state should detect rename");
		let operations: Vec<_> = migrations
			.iter()
			.flat_map(|migration| migration.operations.iter())
			.collect();

		assert_eq!(operations.len(), 1, "unexpected operations: {operations:?}");
		assert!(matches!(
			operations[0],
			super::super::Operation::RenameColumn {
				table,
				old_name,
				new_name
			} if table == "deployments_deployment"
				&& old_name == "reinhardt_app_yaml"
				&& new_name == "project_yaml"
		));
	}

	#[rstest]
	fn generate_migrations_renames_field_with_renamed_model() {
		let from_model = build_model_state(
			"deployments",
			"Deployment",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				FieldState::new("created_at", super::super::FieldType::DateTime, false),
				FieldState::new("app_name", super::super::FieldType::VarChar(255), false),
			],
			Vec::new(),
			Vec::new(),
		);
		let to_model = build_model_state(
			"deployments",
			"Project",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				FieldState::new("created_at", super::super::FieldType::DateTime, false),
				FieldState::new("project_name", super::super::FieldType::VarChar(255), false),
			],
			Vec::new(),
			Vec::new(),
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("deployments".to_string(), "Deployment".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("deployments".to_string(), "Project".to_string()),
				to_model,
			)]),
		);

		let migrations = detector
			.try_generate_migrations()
			.expect("model and field rename should generate migrations");
		assert_eq!(migrations.len(), 1, "unexpected migrations: {migrations:?}");
		let operations = &migrations[0].operations;

		assert_eq!(operations.len(), 2, "unexpected operations: {operations:?}");
		assert!(
			matches!(
				&operations[0],
				super::super::Operation::RenameTable { old_name, new_name }
					if old_name == "deployments_deployment"
						&& new_name == "deployments_project"
			),
			"RenameTable must precede new-table field operations: {operations:?}"
		);
		assert!(matches!(
			&operations[1],
			super::super::Operation::RenameColumn {
				table,
				old_name,
				new_name
			} if table == "deployments_project"
				&& old_name == "app_name"
				&& new_name == "project_name"
		));
		assert!(
			operations.iter().all(|operation| {
				!matches!(
					operation,
					super::super::Operation::AddColumn { .. }
						| super::super::Operation::DropColumn { .. }
				)
			}),
			"field rename on a renamed model must not degrade to AddColumn/DropColumn: {operations:?}"
		);
	}

	#[rstest]
	fn generate_migrations_adds_field_after_renaming_model_table() {
		let from_model = build_model_state(
			"deployments",
			"Deployment",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				FieldState::new("created_at", super::super::FieldType::DateTime, false),
				FieldState::new("updated_at", super::super::FieldType::DateTime, false),
			],
			Vec::new(),
			Vec::new(),
		);
		let to_model = build_model_state(
			"deployments",
			"Project",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				FieldState::new("created_at", super::super::FieldType::DateTime, false),
				FieldState::new("updated_at", super::super::FieldType::DateTime, false),
				FieldState::new("project_name", super::super::FieldType::VarChar(255), true),
			],
			Vec::new(),
			Vec::new(),
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("deployments".to_string(), "Deployment".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("deployments".to_string(), "Project".to_string()),
				to_model,
			)]),
		);

		let migrations = detector
			.try_generate_migrations()
			.expect("model rename with added field should generate migrations");
		assert_eq!(migrations.len(), 1, "unexpected migrations: {migrations:?}");
		let operations = &migrations[0].operations;

		assert_eq!(operations.len(), 2, "unexpected operations: {operations:?}");
		assert!(
			matches!(
				&operations[0],
				super::super::Operation::RenameTable { old_name, new_name }
					if old_name == "deployments_deployment"
						&& new_name == "deployments_project"
			),
			"RenameTable must precede new-table field operations: {operations:?}"
		);
		assert!(matches!(
			&operations[1],
			super::super::Operation::AddColumn { table, column, .. }
				if table == "deployments_project" && column.name == "project_name"
		));
	}

	#[rstest]
	fn generate_migrations_keeps_old_table_drop_before_renaming_model_table() {
		let from_model = build_model_state(
			"deployments",
			"Deployment",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				FieldState::new("created_at", super::super::FieldType::DateTime, false),
				FieldState::new("updated_at", super::super::FieldType::DateTime, false),
				FieldState::new("legacy_payload", super::super::FieldType::Text, true),
			],
			Vec::new(),
			Vec::new(),
		);
		let to_model = build_model_state(
			"deployments",
			"Project",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				FieldState::new("created_at", super::super::FieldType::DateTime, false),
				FieldState::new("updated_at", super::super::FieldType::DateTime, false),
				FieldState::new("retry_count", super::super::FieldType::Integer, false),
			],
			Vec::new(),
			Vec::new(),
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("deployments".to_string(), "Deployment".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("deployments".to_string(), "Project".to_string()),
				to_model,
			)]),
		);

		let migrations = detector
			.try_generate_migrations()
			.expect("model rename with old-table drop should generate migrations");
		assert_eq!(migrations.len(), 1, "unexpected migrations: {migrations:?}");
		let operations = &migrations[0].operations;

		assert_eq!(operations.len(), 3, "unexpected operations: {operations:?}");
		assert!(matches!(
			&operations[0],
			super::super::Operation::DropColumn { table, column, .. }
				if table == "deployments_deployment" && column == "legacy_payload"
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::RenameTable { old_name, new_name }
				if old_name == "deployments_deployment" && new_name == "deployments_project"
		));
		assert!(matches!(
			&operations[2],
			super::super::Operation::AddColumn { table, column, .. }
				if table == "deployments_project" && column.name == "retry_count"
		));
	}

	#[rstest]
	fn generate_migrations_adds_constraint_after_renaming_model_table() {
		let from_model = build_model_state(
			"deployments",
			"Deployment",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				FieldState::new("project_name", super::super::FieldType::VarChar(255), false),
			],
			Vec::new(),
			Vec::new(),
		);
		let to_model = build_model_state(
			"deployments",
			"Project",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				FieldState::new("project_name", super::super::FieldType::VarChar(255), false),
			],
			Vec::new(),
			vec![ConstraintDefinition {
				name: "deployments_project_project_name_not_empty".to_string(),
				constraint_type: "check".to_string(),
				fields: vec!["project_name".to_string()],
				expression: Some("project_name <> ''".to_string()),
				foreign_key_info: None,
			}],
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("deployments".to_string(), "Deployment".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("deployments".to_string(), "Project".to_string()),
				to_model,
			)]),
		);

		let migrations = detector
			.try_generate_migrations()
			.expect("model rename with added constraint should generate migrations");
		assert_eq!(migrations.len(), 1, "unexpected migrations: {migrations:?}");
		let operations = &migrations[0].operations;

		assert_eq!(operations.len(), 2, "unexpected operations: {operations:?}");
		assert!(matches!(
			&operations[0],
			super::super::Operation::RenameTable { old_name, new_name }
				if old_name == "deployments_deployment" && new_name == "deployments_project"
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::AddConstraint {
				table,
				constraint_sql
			} if table == "deployments_project"
				&& constraint_sql.contains("deployments_project_project_name_not_empty")
				&& constraint_sql.contains("CHECK")
				&& constraint_sql.contains("project_name")
		));
	}

	#[rstest]
	fn generate_migrations_preserves_field_changes_for_cross_app_move() {
		let from_model = build_model_state(
			"legacy",
			"Deployment",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				FieldState::new("created_at", super::super::FieldType::DateTime, false),
				FieldState::new("updated_at", super::super::FieldType::DateTime, false),
				FieldState::new("status", super::super::FieldType::VarChar(32), false),
			],
			Vec::new(),
			Vec::new(),
		);
		let to_model = build_model_state(
			"deployments",
			"Project",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				FieldState::new("created_at", super::super::FieldType::DateTime, false),
				FieldState::new("updated_at", super::super::FieldType::DateTime, false),
				FieldState::new("status", super::super::FieldType::VarChar(32), false),
				FieldState::new("project_name", super::super::FieldType::VarChar(255), true),
			],
			Vec::new(),
			Vec::new(),
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("legacy".to_string(), "Deployment".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("deployments".to_string(), "Project".to_string()),
				to_model,
			)]),
		);

		let migrations = detector
			.try_generate_migrations()
			.expect("cross-app model move with added field should generate migrations");
		assert_eq!(migrations.len(), 1, "unexpected migrations: {migrations:?}");
		assert_eq!(migrations[0].app_label, "deployments");
		let operations = &migrations[0].operations;

		assert_eq!(operations.len(), 2, "unexpected operations: {operations:?}");
		assert!(matches!(
			&operations[0],
			super::super::Operation::MoveModel {
				model_name,
				from_app,
				to_app,
				rename_table: true,
				old_table_name: Some(old_table),
				new_table_name: Some(new_table)
			} if model_name == "Deployment"
				&& from_app == "legacy"
				&& to_app == "deployments"
				&& old_table == "legacy_deployment"
				&& new_table == "deployments_project"
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::AddColumn { table, column, .. }
				if table == "deployments_project" && column.name == "project_name"
		));
	}

	#[rstest]
	fn generate_migrations_orders_referenced_table_constraint_after_rename() {
		let from_account = build_model_state(
			"crm",
			"User",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				FieldState::new("email", super::super::FieldType::VarChar(255), false),
			],
			Vec::new(),
			Vec::new(),
		);
		let from_profile = build_model_state(
			"crm",
			"Profile",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				FieldState::new("account_id", super::super::FieldType::Integer, false),
			],
			Vec::new(),
			Vec::new(),
		);
		let to_account = build_model_state(
			"crm",
			"Account",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				FieldState::new("email", super::super::FieldType::VarChar(255), false),
			],
			Vec::new(),
			Vec::new(),
		);
		let to_profile = build_model_state(
			"crm",
			"Profile",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				FieldState::new("account_id", super::super::FieldType::Integer, false),
			],
			Vec::new(),
			vec![ConstraintDefinition {
				name: "crm_profile_account_id_fk".to_string(),
				constraint_type: "foreign_key".to_string(),
				fields: vec!["account_id".to_string()],
				expression: None,
				foreign_key_info: Some(ForeignKeyConstraintInfo {
					referenced_table: "crm_account".to_string(),
					referenced_columns: vec!["id".to_string()],
					on_delete: ForeignKeyAction::Cascade,
					on_update: ForeignKeyAction::Cascade,
				}),
			}],
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![
				(("crm".to_string(), "User".to_string()), from_account),
				(("crm".to_string(), "Profile".to_string()), from_profile),
			]),
			build_project_state(vec![
				(("crm".to_string(), "Account".to_string()), to_account),
				(("crm".to_string(), "Profile".to_string()), to_profile),
			]),
		);

		let migrations = detector
			.try_generate_migrations()
			.expect("referenced table rename with added FK should generate migrations");
		assert_eq!(migrations.len(), 1, "unexpected migrations: {migrations:?}");
		let operations = &migrations[0].operations;

		assert_eq!(operations.len(), 2, "unexpected operations: {operations:?}");
		assert!(matches!(
			&operations[0],
			super::super::Operation::RenameTable { old_name, new_name }
				if old_name == "crm_user" && new_name == "crm_account"
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::AddConstraint {
				table,
				constraint_sql
			} if table == "crm_profile"
				&& constraint_sql.contains("crm_profile_account_id_fk")
				&& constraint_sql.contains("REFERENCES crm_account")
		));
	}

	#[rstest]
	fn try_generate_operations_rejects_ambiguous_field_rename_candidates() {
		let from_model = build_model_state(
			"projects",
			"Project",
			vec![
				FieldState::new("old_code", super::super::FieldType::VarChar(255), false),
				FieldState::new("legacy_code", super::super::FieldType::VarChar(255), false),
			],
			Vec::new(),
			Vec::new(),
		);
		let to_model = build_model_state(
			"projects",
			"Project",
			vec![FieldState::new(
				"project_code",
				super::super::FieldType::VarChar(255),
				false,
			)],
			Vec::new(),
			Vec::new(),
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("projects".to_string(), "Project".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("projects".to_string(), "Project".to_string()),
				to_model,
			)]),
		);

		let error = detector
			.try_generate_operations()
			.expect_err("ambiguous rename candidates must fail");
		let message = error.to_string();

		assert!(
			message.contains("Ambiguous field rename candidates"),
			"unexpected error: {message}"
		);
		assert!(
			message.contains("legacy_code")
				&& message.contains("old_code")
				&& message.contains("project_code"),
			"error should name candidate fields: {message}"
		);
	}

	#[rstest]
	fn try_generate_operations_preserves_unrelated_add_and_drop() {
		let from_model = build_model_state(
			"projects",
			"Project",
			vec![FieldState::new(
				"legacy_payload",
				super::super::FieldType::Text,
				true,
			)],
			Vec::new(),
			Vec::new(),
		);
		let to_model = build_model_state(
			"projects",
			"Project",
			vec![FieldState::new(
				"retry_count",
				super::super::FieldType::Integer,
				false,
			)],
			Vec::new(),
			Vec::new(),
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("projects".to_string(), "Project".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("projects".to_string(), "Project".to_string()),
				to_model,
			)]),
		);

		let operations = detector
			.try_generate_operations()
			.expect("unrelated add/drop should remain valid");

		assert_eq!(operations.len(), 2, "unexpected operations: {operations:?}");
		assert!(
			operations.iter().any(|op| matches!(
				op,
				super::super::Operation::AddColumn { column, .. } if column.name == "retry_count"
			)),
			"expected AddColumn, got: {operations:?}"
		);
		assert!(
			operations.iter().any(|op| matches!(
				op,
				super::super::Operation::DropColumn { column, .. } if column == "legacy_payload"
			)),
			"expected DropColumn, got: {operations:?}"
		);
		assert!(
			operations
				.iter()
				.all(|op| !matches!(op, super::super::Operation::RenameColumn { .. })),
			"unrelated add/drop must not be collapsed into RenameColumn: {operations:?}"
		);
	}

	#[rstest]
	fn to_database_schema_uses_physical_table_name_as_table_key() {
		// Arrange
		let model = build_model_state_with_table_name(
			"routing",
			"HttpRoute",
			"routing_http_route",
			vec![FieldState::new(
				"id",
				super::super::FieldType::Integer,
				false,
			)],
		);
		let state = build_project_state(vec![(
			("routing".to_string(), "HttpRoute".to_string()),
			model,
		)]);

		// Act
		let schema = state.to_database_schema();

		// Assert
		assert_eq!(schema.tables.len(), 1);
		assert!(
			schema.tables.contains_key("routing_http_route"),
			"schema diff keys must match physical table names"
		);
		let table = &schema.tables["routing_http_route"];
		assert_eq!(table.name, "routing_http_route");
	}

	#[rstest]
	fn to_database_schema_prevents_cross_app_collision() {
		// Arrange
		// Two different apps with identically named models
		let blog_user = build_model_state(
			"blog",
			"User",
			vec![FieldState::new(
				"id",
				super::super::FieldType::Integer,
				false,
			)],
			Vec::new(),
			Vec::new(),
		);
		let auth_user = build_model_state(
			"auth",
			"User",
			vec![FieldState::new(
				"id",
				super::super::FieldType::Integer,
				false,
			)],
			Vec::new(),
			Vec::new(),
		);
		let state = build_project_state(vec![
			(("blog".to_string(), "User".to_string()), blog_user),
			(("auth".to_string(), "User".to_string()), auth_user),
		]);

		// Act
		let schema = state.to_database_schema();

		// Assert
		assert_eq!(schema.tables.len(), 2);
		assert!(schema.tables.contains_key("blog_user"));
		assert!(schema.tables.contains_key("auth_user"));
	}

	#[rstest]
	fn to_database_schema_propagates_indexes() {
		// Arrange
		let indexes = vec![
			IndexDefinition {
				name: "idx_title".to_string(),
				fields: vec!["title".to_string()],
				unique: false,
				where_clause: None,
				#[cfg(feature = "pgvector")]
				index_type: None,
				#[cfg(feature = "pgvector")]
				operator_class: None,
				#[cfg(feature = "pgvector")]
				expressions: None,
			},
			IndexDefinition {
				name: "idx_slug_unique".to_string(),
				fields: vec!["slug".to_string()],
				unique: true,
				where_clause: None,
				#[cfg(feature = "pgvector")]
				index_type: None,
				#[cfg(feature = "pgvector")]
				operator_class: None,
				#[cfg(feature = "pgvector")]
				expressions: None,
			},
		];
		let model = build_model_state(
			"blog",
			"Post",
			vec![
				FieldState::new("title", super::super::FieldType::VarChar(255), false),
				FieldState::new("slug", super::super::FieldType::VarChar(100), false),
			],
			indexes,
			Vec::new(),
		);
		let state = build_project_state(vec![(("blog".to_string(), "Post".to_string()), model)]);

		// Act
		let schema = state.to_database_schema();

		// Assert
		let table = &schema.tables["blog_post"];
		assert_eq!(table.indexes.len(), 2);
		assert_eq!(table.indexes[0].name, "idx_title");
		assert_eq!(table.indexes[0].columns, vec!["title".to_string()]);
		assert!(!table.indexes[0].unique);
		assert_eq!(table.indexes[1].name, "idx_slug_unique");
		assert!(table.indexes[1].unique);
	}

	#[rstest]
	fn to_database_schema_propagates_constraints() {
		// Arrange
		let constraints = vec![ConstraintDefinition {
			name: "uq_email".to_string(),
			constraint_type: "unique".to_string(),
			fields: vec!["email".to_string()],
			expression: None,
			foreign_key_info: None,
		}];
		let model = build_model_state(
			"auth",
			"Account",
			vec![FieldState::new(
				"email",
				super::super::FieldType::VarChar(255),
				false,
			)],
			Vec::new(),
			constraints,
		);
		let state = build_project_state(vec![(("auth".to_string(), "Account".to_string()), model)]);

		// Act
		let schema = state.to_database_schema();

		// Assert
		let table = &schema.tables["auth_account"];
		assert_eq!(table.constraints.len(), 1);
		assert_eq!(table.constraints[0].name, "uq_email");
		assert_eq!(table.constraints[0].constraint_type, "unique");
		assert_eq!(table.constraints[0].definition, "email");
	}

	#[rstest]
	fn to_database_schema_maps_field_params() {
		// Arrange
		let mut field = FieldState::new("id", super::super::FieldType::Integer, false);
		field
			.params
			.insert("primary_key".to_string(), "true".to_string());
		field
			.params
			.insert("auto_increment".to_string(), "true".to_string());
		field.params.insert("default".to_string(), "0".to_string());

		let mut nullable_field = FieldState::new("bio", super::super::FieldType::Text, true);
		nullable_field
			.params
			.insert("default".to_string(), "''".to_string());

		let model = build_model_state(
			"users",
			"Profile",
			vec![field, nullable_field],
			Vec::new(),
			Vec::new(),
		);
		let state =
			build_project_state(vec![(("users".to_string(), "Profile".to_string()), model)]);

		// Act
		let schema = state.to_database_schema();

		// Assert
		let table = &schema.tables["users_profile"];
		let id_col = &table.columns["id"];
		assert!(id_col.primary_key);
		assert!(id_col.auto_increment);
		assert_eq!(id_col.default, Some("0".to_string()));
		assert!(!id_col.nullable);

		let bio_col = &table.columns["bio"];
		assert!(!bio_col.primary_key);
		assert!(!bio_col.auto_increment);
		assert!(bio_col.nullable);
		assert_eq!(bio_col.default, Some("''".to_string()));
	}

	#[rstest]
	fn to_database_schema_for_app_filters_by_app_label() {
		// Arrange
		let blog_post = build_model_state(
			"blog",
			"Post",
			vec![FieldState::new(
				"id",
				super::super::FieldType::Integer,
				false,
			)],
			Vec::new(),
			Vec::new(),
		);
		let auth_user = build_model_state(
			"auth",
			"User",
			vec![FieldState::new(
				"id",
				super::super::FieldType::Integer,
				false,
			)],
			Vec::new(),
			Vec::new(),
		);
		let state = build_project_state(vec![
			(("blog".to_string(), "Post".to_string()), blog_post),
			(("auth".to_string(), "User".to_string()), auth_user),
		]);

		// Act
		let blog_schema = state.to_database_schema_for_app("blog");
		let auth_schema = state.to_database_schema_for_app("auth");
		let empty_schema = state.to_database_schema_for_app("nonexistent");

		// Assert
		assert_eq!(blog_schema.tables.len(), 1);
		assert!(blog_schema.tables.contains_key("blog_post"));

		assert_eq!(auth_schema.tables.len(), 1);
		assert!(auth_schema.tables.contains_key("auth_user"));

		assert_eq!(empty_schema.tables.len(), 0);
	}

	#[rstest]
	fn to_database_schema_for_app_propagates_indexes_and_constraints() {
		// Arrange
		let indexes = vec![IndexDefinition {
			name: "idx_created".to_string(),
			fields: vec!["created_at".to_string()],
			unique: false,
			where_clause: None,
			#[cfg(feature = "pgvector")]
			index_type: None,
			#[cfg(feature = "pgvector")]
			operator_class: None,
			#[cfg(feature = "pgvector")]
			expressions: None,
		}];
		let constraints = vec![ConstraintDefinition {
			name: "ck_status".to_string(),
			constraint_type: "check".to_string(),
			fields: vec!["status".to_string()],
			expression: Some("status IN ('draft', 'published')".to_string()),
			foreign_key_info: None,
		}];
		let model = build_model_state(
			"blog",
			"Post",
			vec![
				FieldState::new("created_at", super::super::FieldType::DateTime, false),
				FieldState::new("status", super::super::FieldType::VarChar(20), false),
			],
			indexes,
			constraints,
		);
		let state = build_project_state(vec![(("blog".to_string(), "Post".to_string()), model)]);

		// Act
		let schema = state.to_database_schema_for_app("blog");

		// Assert
		let table = &schema.tables["blog_post"];
		assert_eq!(table.indexes.len(), 1);
		assert_eq!(table.indexes[0].name, "idx_created");
		assert_eq!(table.indexes[0].columns, vec!["created_at".to_string()]);

		assert_eq!(table.constraints.len(), 1);
		assert_eq!(table.constraints[0].name, "ck_status");
		assert_eq!(table.constraints[0].constraint_type, "check");
		assert_eq!(table.constraints[0].definition, "status");
	}

	/// Helper to build a ModelState with a custom table name
	fn build_model_state_with_table_name(
		app_label: &str,
		name: &str,
		table_name: &str,
		fields: Vec<FieldState>,
	) -> ModelState {
		let mut field_map = std::collections::BTreeMap::new();
		for f in fields {
			field_map.insert(f.name.clone(), f);
		}
		ModelState {
			app_label: app_label.to_string(),
			name: name.to_string(),
			table_name: table_name.to_string(),
			fields: field_map,
			options: std::collections::HashMap::new(),
			base_model: None,
			inheritance_type: None,
			discriminator_column: None,
			indexes: Vec::new(),
			constraints: Vec::new(),
			many_to_many_fields: Vec::new(),
		}
	}

	#[rstest]
	fn to_database_schema_respects_custom_table_name() {
		// Arrange
		let model = build_model_state_with_table_name(
			"blog",
			"Post",
			"custom_posts_table",
			vec![FieldState::new(
				"id",
				super::super::FieldType::Integer,
				false,
			)],
		);
		let state = build_project_state(vec![(("blog".to_string(), "Post".to_string()), model)]);

		// Act
		let schema = state.to_database_schema();

		// Assert
		assert!(schema.tables.contains_key("custom_posts_table"));
		let table = &schema.tables["custom_posts_table"];
		assert_eq!(table.name, "custom_posts_table");
	}

	#[rstest]
	fn to_database_schema_for_app_respects_custom_table_name() {
		// Arrange
		let model = build_model_state_with_table_name(
			"blog",
			"Post",
			"custom_posts_table",
			vec![FieldState::new(
				"id",
				super::super::FieldType::Integer,
				false,
			)],
		);
		let state = build_project_state(vec![(("blog".to_string(), "Post".to_string()), model)]);

		// Act
		let schema = state.to_database_schema_for_app("blog");

		// Assert
		assert!(schema.tables.contains_key("custom_posts_table"));
		let table = &schema.tables["custom_posts_table"];
		assert_eq!(table.name, "custom_posts_table");
	}

	// --- Tests for #3204: no-op migration detection ---

	/// Build fields commonly used in #3204 tests
	fn sample_fields() -> Vec<FieldState> {
		vec![
			FieldState::new("id", super::super::FieldType::Integer, false),
			FieldState::new("name", super::super::FieldType::VarChar(255), false),
		]
	}

	fn many_to_many_foreign_key_field(name: &str, referenced_table: &str) -> FieldState {
		let mut field = FieldState::new(name, super::super::FieldType::Integer, false);
		field.foreign_key = Some(ForeignKeyInfo {
			referenced_table: referenced_table.to_string(),
			referenced_column: "id".to_string(),
			on_delete: ForeignKeyAction::Cascade,
			on_update: ForeignKeyAction::Cascade,
		});
		field
	}

	/// Regression test for issue #4659.
	///
	/// Scenario: a model struct is renamed (`Room` -> `DMRoom`) but the
	/// `table_name` stays the same (`dm_room`), and an M2M through-table
	/// (`dm_room_members`) already exists on disk. The state reconstructed
	/// from on-disk migrations loses the original struct identifier (it
	/// derives a `RoomMembers`-style approximation via
	/// `table_name_to_model_name`) and the M2M metadata is gone — only the
	/// raw through-table survives.
	///
	/// Before the fix, `detect_created_many_to_many` keyed its existence
	/// check on `(app_label, model_name)` and inspected
	/// `many_to_many_fields`, which is always empty on reconstructed
	/// states, so the through-table was spuriously re-created on every
	/// incremental `makemigrations` run.
	#[rstest]
	fn detect_created_many_to_many_recognises_existing_through_table_by_table_name() {
		use super::super::model_registry::ManyToManyMetadata;

		// Arrange: from_state mimics the on-disk reconstruction.
		// `table_name_to_model_name` produces `Room` / `RoomMembers` for
		// tables `dm_room` / `dm_room_members`. The through table appears
		// as an ordinary model with no M2M metadata attached.
		let from_room = build_model_state_with_table_name("dm", "Room", "dm_room", sample_fields());
		let from_through = build_model_state_with_table_name(
			"dm",
			"RoomMembers",
			"dm_room_members",
			sample_fields(),
		);
		let from_state = build_project_state(vec![
			(("dm".to_string(), "Room".to_string()), from_room),
			(("dm".to_string(), "RoomMembers".to_string()), from_through),
		]);

		// to_state: the renamed struct (`DMRoom`) re-declares the same
		// M2M field. The synthetic through-table model (`DMRoomMembers`)
		// the macro emits keeps the canonical `dm_room_members` table
		// name.
		let mut to_room =
			build_model_state_with_table_name("dm", "DMRoom", "dm_room", sample_fields());
		to_room
			.many_to_many_fields
			.push(ManyToManyMetadata::new("members", "User"));
		let to_through = build_model_state_with_table_name(
			"dm",
			"DMRoomMembers",
			"dm_room_members",
			sample_fields(),
		);
		let to_state = build_project_state(vec![
			(("dm".to_string(), "DMRoom".to_string()), to_room),
			(("dm".to_string(), "DMRoomMembers".to_string()), to_through),
		]);

		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let changes = detector.detect_changes();

		// Assert: the through table is already present in from_state, so
		// the autodetector must not re-create it (#4659).
		assert!(
			changes.created_many_to_many.is_empty(),
			"M2M through table already exists in from_state; expected no \
			 created_many_to_many, got {:?}",
			changes.created_many_to_many
		);
	}

	#[rstest]
	fn generate_migrations_resolves_unqualified_many_to_many_target_across_apps() {
		use super::super::Operation;
		use super::super::model_registry::ManyToManyMetadata;
		use super::super::operations::Constraint;

		let auth_user =
			build_model_state_with_table_name("auth", "User", "auth_user", sample_fields());
		let mut dm_room =
			build_model_state_with_table_name("dm", "DMRoom", "dm_room", sample_fields());
		dm_room
			.many_to_many_fields
			.push(ManyToManyMetadata::new("members", "User"));
		let to_state = build_project_state(vec![
			(("auth".to_string(), "User".to_string()), auth_user),
			(("dm".to_string(), "DMRoom".to_string()), dm_room),
		]);
		let detector = MigrationAutodetector::new(ProjectState::new(), to_state);

		let migrations = detector.generate_migrations();
		let dm_migration = migrations
			.iter()
			.find(|migration| migration.app_label == "dm")
			.expect("dm migration should be generated");
		let Operation::CreateTable {
			name, constraints, ..
		} = dm_migration
			.operations
			.iter()
			.find(|operation| {
				matches!(
					operation,
					Operation::CreateTable { name, .. } if name == "dm_room_members"
				)
			})
			.expect("dm_room_members through table should be generated")
		else {
			panic!("expected CreateTable operation");
		};
		assert_eq!(name, "dm_room_members");

		let target_fk = constraints
			.iter()
			.find_map(|constraint| match constraint {
				Constraint::ForeignKey {
					columns,
					referenced_table,
					..
				} if columns == &vec!["auth_user_id".to_string()] => Some(referenced_table),
				_ => None,
			})
			.expect("auth_user_id foreign key should be generated");
		assert_eq!(target_fk, "auth_user");
	}

	#[rstest]
	fn detect_created_many_to_many_skips_existing_to_state_through_table() {
		use super::super::model_registry::ManyToManyMetadata;

		let auth_user =
			build_model_state_with_table_name("auth", "User", "auth_user", sample_fields());
		let mut dm_room =
			build_model_state_with_table_name("dm", "DMRoom", "dm_room", sample_fields());
		dm_room
			.many_to_many_fields
			.push(ManyToManyMetadata::new("members", "User"));
		let dm_room_members = build_model_state_with_table_name(
			"dm",
			"DMRoomMembers",
			"dm_room_members",
			sample_fields(),
		);
		let to_state = build_project_state(vec![
			(("auth".to_string(), "User".to_string()), auth_user),
			(("dm".to_string(), "DMRoom".to_string()), dm_room),
			(
				("dm".to_string(), "DMRoomMembers".to_string()),
				dm_room_members,
			),
		]);
		let detector = MigrationAutodetector::new(ProjectState::new(), to_state);

		let changes = detector.detect_changes();

		assert!(
			changes.created_many_to_many.is_empty(),
			"to_state already contains the through table model; expected no \
			 synthetic created_many_to_many, got {:?}",
			changes.created_many_to_many
		);
	}

	#[rstest]
	fn generate_migrations_renames_implicit_many_to_many_table_with_source_table() {
		// Arrange
		let from_user =
			build_model_state_with_table_name("users", "User", "users", sample_fields());
		let from_group =
			build_model_state_with_table_name("users", "Group", "groups", sample_fields());
		let from_through = build_model_state_with_table_name(
			"users",
			"UsersGroups",
			"users_groups",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				many_to_many_foreign_key_field("users_id", "users"),
				many_to_many_foreign_key_field("groups_id", "groups"),
			],
		);
		let from_state = build_project_state(vec![
			(("users".to_string(), "User".to_string()), from_user),
			(("users".to_string(), "Group".to_string()), from_group),
			(
				("users".to_string(), "UsersGroups".to_string()),
				from_through,
			),
		]);

		let mut to_user =
			build_model_state_with_table_name("users", "User", "user", sample_fields());
		to_user
			.many_to_many_fields
			.push(ManyToManyMetadata::new("groups", "Group"));
		let to_group =
			build_model_state_with_table_name("users", "Group", "groups", sample_fields());
		let to_through = build_model_state_with_table_name(
			"users",
			"UserGroups",
			"user_groups",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				many_to_many_foreign_key_field("user_id", "user"),
				many_to_many_foreign_key_field("groups_id", "groups"),
			],
		);
		let to_state = build_project_state(vec![
			(("users".to_string(), "User".to_string()), to_user),
			(("users".to_string(), "Group".to_string()), to_group),
			(("users".to_string(), "UserGroups".to_string()), to_through),
		]);
		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let changes = detector.detect_changes();
		let migrations = detector.generate_migrations();
		let operations = &migrations
			.iter()
			.find(|migration| migration.app_label == "users")
			.expect("users migration should be generated")
			.operations;

		// Assert
		assert!(
			changes.created_many_to_many.is_empty(),
			"existing through table must not be recreated: {:?}",
			changes.created_many_to_many
		);
		assert!(
			!changes
				.deleted_models
				.contains(&("users".to_string(), "UsersGroups".to_string())),
			"existing through table must not be dropped: {:?}",
			changes.deleted_models
		);
		assert!(operations.iter().any(|operation| matches!(
			operation,
			super::super::Operation::RenameTable { old_name, new_name }
				if old_name == "users_groups" && new_name == "user_groups"
		)));
		assert!(operations.iter().any(|operation| matches!(
			operation,
			super::super::Operation::RenameColumn {
				table,
				old_name,
				new_name
			}
				if table == "user_groups"
					&& old_name == "users_id"
					&& new_name == "user_id"
		)));
		assert!(
			operations.iter().all(|operation| !matches!(
				operation,
				super::super::Operation::CreateTable { name, .. }
					if name == "user_groups"
			)),
			"through table must be renamed rather than recreated: {operations:?}"
		);
		assert!(
			operations.iter().all(|operation| !matches!(
				operation,
				super::super::Operation::DropTable { name }
					if name == "users_groups"
			)),
			"through table must be renamed rather than dropped: {operations:?}"
		);
	}

	#[rstest]
	fn generate_migrations_does_not_rename_unrelated_table_matching_old_m2m_name() {
		let from_user =
			build_model_state_with_table_name("users", "User", "users", sample_fields());
		let from_group =
			build_model_state_with_table_name("users", "Group", "groups", sample_fields());
		let unrelated = build_model_state_with_table_name(
			"users",
			"UsersGroupsArchive",
			"users_groups",
			vec![FieldState::new(
				"id",
				super::super::FieldType::Integer,
				false,
			)],
		);
		let from_state = build_project_state(vec![
			(("users".to_string(), "User".to_string()), from_user),
			(("users".to_string(), "Group".to_string()), from_group),
			(
				("users".to_string(), "UsersGroupsArchive".to_string()),
				unrelated,
			),
		]);

		let mut to_user =
			build_model_state_with_table_name("users", "User", "user", sample_fields());
		to_user
			.many_to_many_fields
			.push(ManyToManyMetadata::new("groups", "Group"));
		let to_group =
			build_model_state_with_table_name("users", "Group", "groups", sample_fields());
		let to_state = build_project_state(vec![
			(("users".to_string(), "User".to_string()), to_user),
			(("users".to_string(), "Group".to_string()), to_group),
		]);
		let detector = MigrationAutodetector::new(from_state, to_state);

		let operations = &detector.generate_migrations()[0].operations;

		assert!(operations.iter().any(|operation| matches!(
			operation,
			super::super::Operation::CreateTable { name, .. } if name == "user_groups"
		)));
		assert!(operations.iter().all(|operation| !matches!(
			operation,
			super::super::Operation::RenameTable { old_name, new_name }
				if old_name == "users_groups" && new_name == "user_groups"
		)));
	}

	#[test]
	fn defers_m2m_constraint_replacements_until_after_renamed_columns() {
		let rename = ManyToManyArtifactRename {
			old_table: "users_groups".to_string(),
			old_source_column: "users_id".to_string(),
			old_target_column: "groups_id".to_string(),
			new_table: "user_groups".to_string(),
			new_source_column: "user_id".to_string(),
			new_target_column: "groups_id".to_string(),
		};
		let mut operations = vec![
			super::super::Operation::AddConstraint {
				table: "user_groups".to_string(),
				constraint_sql:
					"CONSTRAINT fk_user_groups_user_id FOREIGN KEY (user_id) REFERENCES user(id)"
						.to_string(),
			},
			super::super::Operation::DropConstraint {
				table: "users_groups".to_string(),
				constraint_name: "fk_users_groups_users_id".to_string(),
			},
			super::super::Operation::AddConstraint {
				table: "user_groups".to_string(),
				constraint_sql: "CONSTRAINT user_groups_check CHECK (rank > 0)".to_string(),
			},
		];

		MigrationAutodetector::remove_many_to_many_column_replacement_operations(
			&mut operations,
			&rename,
			"users_id",
			"user_id",
		);

		assert_eq!(operations.len(), 3, "unexpected operations: {operations:?}");
		operations.push(super::super::Operation::RenameColumn {
			table: "user_groups".to_string(),
			old_name: "users_id".to_string(),
			new_name: "user_id".to_string(),
		});
		MigrationAutodetector::defer_many_to_many_constraint_replacements(
			&mut operations,
			&rename,
			"users_id",
			"user_id",
		);

		assert!(matches!(
			&operations[0],
			super::super::Operation::AddConstraint { constraint_sql, .. }
				if constraint_sql == "CONSTRAINT user_groups_check CHECK (rank > 0)"
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::RenameColumn { old_name, new_name, .. }
				if old_name == "users_id" && new_name == "user_id"
		));
	}

	#[test]
	fn retains_unrelated_m2m_constraints_that_mention_renamed_columns() {
		let rename = ManyToManyArtifactRename {
			old_table: "users_groups".to_string(),
			old_source_column: "users_id".to_string(),
			old_target_column: "groups_id".to_string(),
			new_table: "user_groups".to_string(),
			new_source_column: "user_id".to_string(),
			new_target_column: "groups_id".to_string(),
		};
		let mut operations = vec![
			super::super::Operation::AddConstraint {
				table: "user_groups".to_string(),
				constraint_sql: "CONSTRAINT user_groups_rank_unique UNIQUE (user_id, rank)"
					.to_string(),
			},
			super::super::Operation::AddConstraint {
				table: "user_groups".to_string(),
				constraint_sql:
					"CONSTRAINT fk_user_groups_user_id FOREIGN KEY (user_id) REFERENCES user(id)"
						.to_string(),
			},
		];

		MigrationAutodetector::remove_many_to_many_column_replacement_operations(
			&mut operations,
			&rename,
			"users_id",
			"user_id",
		);

		assert_eq!(operations.len(), 2, "unexpected operations: {operations:?}");
		assert!(matches!(
			&operations[0],
			super::super::Operation::AddConstraint { constraint_sql, .. }
				if constraint_sql == "CONSTRAINT user_groups_rank_unique UNIQUE (user_id, rank)"
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::AddConstraint { constraint_sql, .. }
				if constraint_sql == "CONSTRAINT fk_user_groups_user_id FOREIGN KEY (user_id) REFERENCES user(id)"
		));
	}

	#[rstest]
	fn generate_migrations_renames_implicit_many_to_many_target_column_with_target_table() {
		// Arrange
		let from_user =
			build_model_state_with_table_name("users", "User", "users", sample_fields());
		let from_group =
			build_model_state_with_table_name("users", "Group", "groups", sample_fields());
		let from_through = build_model_state_with_table_name(
			"users",
			"UsersGroups",
			"users_groups",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				many_to_many_foreign_key_field("users_id", "users"),
				many_to_many_foreign_key_field("groups_id", "groups"),
			],
		);
		let from_state = build_project_state(vec![
			(("users".to_string(), "User".to_string()), from_user),
			(("users".to_string(), "Group".to_string()), from_group),
			(
				("users".to_string(), "UsersGroups".to_string()),
				from_through,
			),
		]);

		let mut to_user =
			build_model_state_with_table_name("users", "User", "users", sample_fields());
		to_user
			.many_to_many_fields
			.push(ManyToManyMetadata::new("groups", "Group"));
		let to_group =
			build_model_state_with_table_name("users", "Group", "group", sample_fields());
		let to_through = build_model_state_with_table_name(
			"users",
			"UsersGroups",
			"users_groups",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				many_to_many_foreign_key_field("users_id", "users"),
				many_to_many_foreign_key_field("group_id", "group"),
			],
		);
		let to_state = build_project_state(vec![
			(("users".to_string(), "User".to_string()), to_user),
			(("users".to_string(), "Group".to_string()), to_group),
			(("users".to_string(), "UsersGroups".to_string()), to_through),
		]);
		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let migrations = detector.generate_migrations();
		let operations = &migrations
			.iter()
			.find(|migration| migration.app_label == "users")
			.expect("users migration should be generated")
			.operations;

		// Assert
		assert!(operations.iter().any(|operation| matches!(
			operation,
			super::super::Operation::RenameColumn { table, old_name, new_name }
				if table == "users_groups"
					&& old_name == "groups_id"
					&& new_name == "group_id"
		)));
		assert!(operations.iter().all(|operation| !matches!(
			operation,
			super::super::Operation::AddColumn { table, column, .. }
				if table == "users_groups" && column.name == "group_id"
		)));
		assert!(operations.iter().all(|operation| !matches!(
			operation,
			super::super::Operation::DropColumn { table, column, .. }
				if table == "users_groups" && column == "groups_id"
		)));
	}

	#[rstest]
	fn generate_migrations_renames_default_columns_for_explicit_many_to_many_through_table() {
		// Arrange
		let from_user =
			build_model_state_with_table_name("users", "User", "users", sample_fields());
		let from_group =
			build_model_state_with_table_name("users", "Group", "groups", sample_fields());
		let from_through = build_model_state_with_table_name(
			"users",
			"Membership",
			"memberships",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				many_to_many_foreign_key_field("users_id", "users"),
				many_to_many_foreign_key_field("groups_id", "groups"),
			],
		);
		let from_state = build_project_state(vec![
			(("users".to_string(), "User".to_string()), from_user),
			(("users".to_string(), "Group".to_string()), from_group),
			(
				("users".to_string(), "Membership".to_string()),
				from_through,
			),
		]);

		let mut to_user =
			build_model_state_with_table_name("users", "User", "user", sample_fields());
		to_user
			.many_to_many_fields
			.push(ManyToManyMetadata::new("groups", "Group").with_through("memberships"));
		let to_group =
			build_model_state_with_table_name("users", "Group", "groups", sample_fields());
		let to_through = build_model_state_with_table_name(
			"users",
			"Membership",
			"memberships",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				many_to_many_foreign_key_field("user_id", "user"),
				many_to_many_foreign_key_field("groups_id", "groups"),
			],
		);
		let to_state = build_project_state(vec![
			(("users".to_string(), "User".to_string()), to_user),
			(("users".to_string(), "Group".to_string()), to_group),
			(("users".to_string(), "Membership".to_string()), to_through),
		]);
		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let migrations = detector.generate_migrations();
		let operations = &migrations
			.iter()
			.find(|migration| migration.app_label == "users")
			.expect("users migration should be generated")
			.operations;

		// Assert
		assert!(operations.iter().any(|operation| matches!(
			operation,
			super::super::Operation::RenameColumn { table, old_name, new_name }
				if table == "memberships"
					&& old_name == "users_id"
					&& new_name == "user_id"
		)));
		assert!(operations.iter().all(|operation| !matches!(
			operation,
			super::super::Operation::AddColumn { table, column, .. }
				if table == "memberships" && column.name == "user_id"
		)));
		assert!(operations.iter().all(|operation| !matches!(
			operation,
			super::super::Operation::DropColumn { table, column, .. }
				if table == "memberships" && column == "users_id"
		)));
	}

	#[rstest]
	fn detect_renamed_models_skips_struct_only_rename_with_same_table_name() {
		// Arrange: struct name changed (Clusters -> Cluster) but table name is the same
		let from_model =
			build_model_state_with_table_name("myapp", "Clusters", "clusters", sample_fields());
		let to_model =
			build_model_state_with_table_name("myapp", "Cluster", "clusters", sample_fields());

		let from_state = build_project_state(vec![(
			("myapp".to_string(), "Clusters".to_string()),
			from_model,
		)]);
		let to_state = build_project_state(vec![(
			("myapp".to_string(), "Cluster".to_string()),
			to_model,
		)]);

		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let changes = detector.detect_changes();

		// Assert: no rename should be detected
		assert!(
			changes.renamed_models.is_empty(),
			"struct-only rename with same table name should not produce renamed_models"
		);
	}

	#[rstest]
	fn detect_renamed_models_detects_actual_table_rename() {
		// Arrange: struct name changed AND table name changed
		let from_model =
			build_model_state_with_table_name("myapp", "OldModel", "old_table", sample_fields());
		let to_model =
			build_model_state_with_table_name("myapp", "NewModel", "new_table", sample_fields());

		let from_state = build_project_state(vec![(
			("myapp".to_string(), "OldModel".to_string()),
			from_model,
		)]);
		let to_state = build_project_state(vec![(
			("myapp".to_string(), "NewModel".to_string()),
			to_model,
		)]);

		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let changes = detector.detect_changes();
		let migrations = detector.generate_migrations();

		// Assert: rename should be detected and emitted as a single table
		// rename, matching Django's RenameModel-vs-create/drop contract.
		assert_eq!(
			changes.renamed_models.len(),
			1,
			"actual table rename should be detected"
		);
		assert_eq!(changes.renamed_models[0].1, "OldModel");
		assert_eq!(changes.renamed_models[0].2, "NewModel");
		assert!(
			changes.created_models.is_empty(),
			"confirmed model rename must not leave created_models noise: {:?}",
			changes.created_models
		);
		assert!(
			changes.deleted_models.is_empty(),
			"confirmed model rename must not leave deleted_models noise: {:?}",
			changes.deleted_models
		);
		assert_eq!(migrations.len(), 1, "unexpected migrations: {migrations:?}");
		assert_eq!(migrations[0].app_label, "myapp");
		let operations = &migrations[0].operations;
		assert_eq!(operations.len(), 1, "unexpected operations: {operations:?}");
		assert!(matches!(
			&operations[0],
			super::super::Operation::RenameTable { old_name, new_name }
				if old_name == "old_table" && new_name == "new_table"
		));
	}

	#[rstest]
	fn matching_from_model_prefers_renamed_model_before_target_table_owner() {
		let old_user =
			build_model_state_with_table_name("myapp", "OldUser", "users", sample_fields());
		let archive =
			build_model_state_with_table_name("myapp", "Archive", "user", sample_fields());
		let user = build_model_state_with_table_name("myapp", "User", "user", sample_fields());
		let from_state = build_project_state(vec![
			(("myapp".to_string(), "OldUser".to_string()), old_user),
			(("myapp".to_string(), "Archive".to_string()), archive),
		]);
		let to_state = build_project_state(vec![(("myapp".to_string(), "User".to_string()), user)]);
		let detector = MigrationAutodetector::new(from_state, to_state);
		let changes = DetectedChanges {
			renamed_models: vec![(
				"myapp".to_string(),
				"OldUser".to_string(),
				"User".to_string(),
			)],
			..DetectedChanges::default()
		};
		let target = detector.to_state.get_model("myapp", "User").unwrap();

		let matched = detector
			.matching_from_model_for_to_model("myapp", "User", target, &changes)
			.expect("renamed model source should be selected");

		assert_eq!(matched.name, "OldUser");
	}

	#[rstest]
	fn generate_migrations_renames_table_when_model_identity_is_unchanged() {
		// Arrange: only the explicit table name changes; the model identity and
		// fields remain unchanged.
		let from_model =
			build_model_state_with_table_name("users", "User", "users", sample_fields());
		let to_model = build_model_state_with_table_name("users", "User", "user", sample_fields());

		let from_state = build_project_state(vec![(
			("users".to_string(), "User".to_string()),
			from_model,
		)]);
		let to_state =
			build_project_state(vec![(("users".to_string(), "User".to_string()), to_model)]);

		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let migrations = detector.generate_migrations();

		// Assert: adopting the table-name convention must preserve data with a
		// rename instead of emitting a destructive drop/create pair.
		assert_eq!(migrations.len(), 1, "unexpected migrations: {migrations:?}");
		assert_eq!(migrations[0].app_label, "users");
		assert_eq!(
			migrations[0].operations,
			vec![super::super::Operation::RenameTable {
				old_name: "users".to_string(),
				new_name: "user".to_string(),
			}]
		);
	}

	#[rstest]
	fn generate_migrations_preserves_field_changes_with_table_rename() {
		let from_model =
			build_model_state_with_table_name("users", "User", "users", sample_fields());
		let mut to_fields = sample_fields();
		to_fields.push(FieldState::new(
			"email",
			super::super::FieldType::VarChar(255),
			false,
		));
		let to_model = build_model_state_with_table_name("users", "User", "user", to_fields);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("users".to_string(), "User".to_string()),
				from_model,
			)]),
			build_project_state(vec![(("users".to_string(), "User".to_string()), to_model)]),
		);

		let migrations = detector.generate_migrations();

		assert_eq!(migrations.len(), 1, "unexpected migrations: {migrations:?}");
		let operations = &migrations[0].operations;
		assert!(
			operations.iter().any(|operation| matches!(
				operation,
				super::super::Operation::RenameTable { old_name, new_name }
					if old_name == "users" && new_name == "user"
			)),
			"table rename is missing: {operations:?}"
		);
		assert!(
			operations.iter().any(|operation| matches!(
				operation,
				super::super::Operation::AddColumn { table, column, .. }
					if table == "user" && column.name == "email"
			)),
			"field addition is missing: {operations:?}"
		);
	}

	#[rstest]
	fn generate_operations_emits_table_renames() {
		let from_model =
			build_model_state_with_table_name("users", "User", "users", sample_fields());
		let to_model = build_model_state_with_table_name("users", "User", "user", sample_fields());
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("users".to_string(), "User".to_string()),
				from_model,
			)]),
			build_project_state(vec![(("users".to_string(), "User".to_string()), to_model)]),
		);

		assert_eq!(
			detector.generate_operations(),
			vec![super::super::Operation::RenameTable {
				old_name: "users".to_string(),
				new_name: "user".to_string(),
			}]
		);
	}

	#[rstest]
	fn generate_migrations_drops_deleted_rename_target_owner_before_renaming() {
		let from_user =
			build_model_state_with_table_name("users", "User", "users", sample_fields());
		let from_archive =
			build_model_state_with_table_name("users", "Archive", "user", sample_fields());
		let to_user = build_model_state_with_table_name("users", "User", "user", sample_fields());
		let detector = MigrationAutodetector::new(
			build_project_state(vec![
				(("users".to_string(), "User".to_string()), from_user),
				(("users".to_string(), "Archive".to_string()), from_archive),
			]),
			build_project_state(vec![(("users".to_string(), "User".to_string()), to_user)]),
		);

		let operations = &detector.generate_migrations()[0].operations;
		let drop_index = operations
			.iter()
			.position(
				|operation| matches!(operation, super::super::Operation::DropTable { name } if name == "user"),
			)
			.expect("deleted rename-target owner must be dropped: {operations:?}");
		let rename_index = operations
			.iter()
			.position(|operation| matches!(operation, super::super::Operation::RenameTable { old_name, new_name } if old_name == "users" && new_name == "user"))
			.expect("table rename is missing: {operations:?}");

		assert!(
			drop_index < rename_index,
			"target owner must be dropped before rename: {operations:?}"
		);
	}

	#[rstest]
	fn generate_operations_orders_table_rename_before_new_table_edits() {
		let from_model =
			build_model_state_with_table_name("users", "User", "users", sample_fields());
		let mut to_fields = sample_fields();
		to_fields.push(FieldState::new(
			"email",
			super::super::FieldType::VarChar(255),
			false,
		));
		let to_model = build_model_state_with_table_name("users", "User", "user", to_fields);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("users".to_string(), "User".to_string()),
				from_model,
			)]),
			build_project_state(vec![(("users".to_string(), "User".to_string()), to_model)]),
		);

		let operations = detector.generate_operations();
		assert!(
			matches!(
				&operations[..2],
				[
					super::super::Operation::RenameTable { old_name, new_name },
					super::super::Operation::AddColumn { table, column, .. },
				]
					if old_name == "users" && new_name == "user" && table == "user" && column.name == "email"
			),
			"rename must precede edits to its new table: {operations:?}"
		);
	}

	#[rstest]
	fn generate_operations_orders_rename_before_fk_dependent_create_table() {
		let from_user =
			build_model_state_with_table_name("users", "User", "users", sample_fields());
		let to_user = build_model_state_with_table_name("users", "User", "user", sample_fields());
		let mut profile_fields = sample_fields();
		profile_fields.push(FieldState::new(
			"user_id",
			super::super::FieldType::Integer,
			false,
		));
		let mut to_profile =
			build_model_state_with_table_name("users", "Profile", "profiles", profile_fields);
		to_profile.constraints.push(ConstraintDefinition {
			name: "profiles_user_id_fk".to_string(),
			constraint_type: "foreign_key".to_string(),
			fields: vec!["user_id".to_string()],
			expression: None,
			foreign_key_info: Some(ForeignKeyConstraintInfo {
				referenced_table: "user".to_string(),
				referenced_columns: vec!["id".to_string()],
				on_delete: ForeignKeyAction::Cascade,
				on_update: ForeignKeyAction::Cascade,
			}),
		});
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(("users".to_string(), "User".to_string()), from_user)]),
			build_project_state(vec![
				(("users".to_string(), "User".to_string()), to_user),
				(("users".to_string(), "Profile".to_string()), to_profile),
			]),
		);

		let operations = detector.generate_operations();
		let rename_index = operations
			.iter()
			.position(|operation| {
				matches!(
					operation,
					super::super::Operation::RenameTable { old_name, new_name }
						if old_name == "users" && new_name == "user"
				)
			})
			.expect("table rename is missing: {operations:?}");
		let profile_index = operations
			.iter()
			.position(|operation| {
				matches!(
					operation,
					super::super::Operation::CreateTable { name, constraints, .. }
						if name == "profiles" && constraints.iter().any(|constraint| matches!(
							constraint,
							super::super::operations::Constraint::ForeignKey { referenced_table, .. }
								if referenced_table == "user"
						))
				)
			})
			.expect("foreign-key-dependent table creation is missing: {operations:?}");

		assert!(
			rename_index < profile_index,
			"the renamed target must exist before its foreign-key-dependent table is created: {operations:?}"
		);
	}

	#[rstest]
	fn generate_operations_preserves_implicit_many_to_many_artifact_renames() {
		let from_user =
			build_model_state_with_table_name("users", "User", "users", sample_fields());
		let from_group =
			build_model_state_with_table_name("users", "Group", "groups", sample_fields());
		let from_through = build_model_state_with_table_name(
			"users",
			"UsersGroups",
			"users_groups",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				many_to_many_foreign_key_field("users_id", "users"),
				many_to_many_foreign_key_field("groups_id", "groups"),
			],
		);
		let mut to_user =
			build_model_state_with_table_name("users", "User", "user", sample_fields());
		to_user
			.many_to_many_fields
			.push(ManyToManyMetadata::new("groups", "Group"));
		let to_group =
			build_model_state_with_table_name("users", "Group", "groups", sample_fields());
		let to_through = build_model_state_with_table_name(
			"users",
			"UserGroups",
			"user_groups",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				many_to_many_foreign_key_field("user_id", "user"),
				many_to_many_foreign_key_field("groups_id", "groups"),
			],
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![
				(("users".to_string(), "User".to_string()), from_user),
				(("users".to_string(), "Group".to_string()), from_group),
				(
					("users".to_string(), "UsersGroups".to_string()),
					from_through,
				),
			]),
			build_project_state(vec![
				(("users".to_string(), "User".to_string()), to_user),
				(("users".to_string(), "Group".to_string()), to_group),
				(("users".to_string(), "UserGroups".to_string()), to_through),
			]),
		);

		let operations = detector.generate_operations();
		assert!(
			operations.iter().any(|operation| matches!(
				operation,
				super::super::Operation::RenameTable { old_name, new_name }
					if old_name == "users_groups" && new_name == "user_groups"
			)),
			"through-table rename is missing: {operations:?}"
		);
		assert!(
			operations.iter().any(|operation| matches!(
				operation,
				super::super::Operation::RenameColumn { table, old_name, new_name }
					if table == "user_groups" && old_name == "users_id" && new_name == "user_id"
			)),
			"through-table source-column rename is missing: {operations:?}"
		);
	}

	#[rstest]
	fn generate_migrations_stages_table_name_swaps() {
		let from_foo = build_model_state_with_table_name("app", "Foo", "foo", sample_fields());
		let from_bar = build_model_state_with_table_name("app", "Bar", "bar", sample_fields());
		let to_foo = build_model_state_with_table_name("app", "Foo", "bar", sample_fields());
		let to_bar = build_model_state_with_table_name("app", "Bar", "foo", sample_fields());
		let detector = MigrationAutodetector::new(
			build_project_state(vec![
				(("app".to_string(), "Foo".to_string()), from_foo),
				(("app".to_string(), "Bar".to_string()), from_bar),
			]),
			build_project_state(vec![
				(("app".to_string(), "Foo".to_string()), to_foo),
				(("app".to_string(), "Bar".to_string()), to_bar),
			]),
		);

		let operations = &detector.generate_migrations()[0].operations;
		assert_eq!(
			operations.len(),
			3,
			"table swap must be staged through a temporary name: {operations:?}"
		);
		assert!(matches!(
			&operations[0],
			super::super::Operation::RenameTable { old_name, new_name }
				if old_name == "bar" && new_name.starts_with("__reinhardt_rename_tmp_")
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::RenameTable { old_name, new_name }
				if old_name == "foo" && new_name == "bar"
		));
		assert!(matches!(
			&operations[2],
			super::super::Operation::RenameTable { old_name, new_name }
				if old_name.starts_with("__reinhardt_rename_tmp_") && new_name == "foo"
		));
	}

	#[rstest]
	fn generate_migrations_creates_model_that_reuses_renamed_table() {
		let from_user =
			build_model_state_with_table_name("users", "User", "users", sample_fields());
		let to_user = build_model_state_with_table_name("users", "User", "user", sample_fields());
		let archive =
			build_model_state_with_table_name("users", "Archive", "users", sample_fields());
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(("users".to_string(), "User".to_string()), from_user)]),
			build_project_state(vec![
				(("users".to_string(), "User".to_string()), to_user),
				(("users".to_string(), "Archive".to_string()), archive),
			]),
		);

		let operations = &detector.generate_migrations()[0].operations;
		assert!(operations.iter().any(|operation| matches!(
			operation,
			super::super::Operation::RenameTable { old_name, new_name }
				if old_name == "users" && new_name == "user"
		)));
		assert!(operations.iter().any(|operation| matches!(
			operation,
			super::super::Operation::CreateTable { name, .. } if name == "users"
		)));
	}

	#[rstest]
	fn detect_changes_does_not_create_a_model_that_is_already_being_table_renamed() {
		// Arrange
		let from_foo = build_model_state_with_table_name("app", "Foo", "foo", sample_fields());
		let from_bar = build_model_state_with_table_name("app", "Bar", "bar", sample_fields());
		let to_foo = build_model_state_with_table_name("app", "Foo", "baz", sample_fields());
		let to_bar = build_model_state_with_table_name("app", "Bar", "foo", sample_fields());
		let detector = MigrationAutodetector::new(
			build_project_state(vec![
				(("app".to_string(), "Foo".to_string()), from_foo),
				(("app".to_string(), "Bar".to_string()), from_bar),
			]),
			build_project_state(vec![
				(("app".to_string(), "Foo".to_string()), to_foo),
				(("app".to_string(), "Bar".to_string()), to_bar),
			]),
		);

		// Act
		let changes = detector.detect_changes();

		// Assert
		assert!(
			!changes
				.created_models
				.contains(&("app".to_string(), "Bar".to_string())),
			"a model that is already being table-renamed must not also be created: {changes:?}"
		);
	}

	#[rstest]
	fn detect_changes_recovers_owner_displaced_by_cross_app_move() {
		// Regression for #5673: a moved model can claim a table that was owned by
		// a model removed from the target app.
		let from_profile = build_model_state_with_table_name(
			"legacy",
			"Profile",
			"legacy_profile",
			sample_fields(),
		);
		let from_owner = build_model_state_with_table_name(
			"accounts",
			"User",
			"shared",
			vec![FieldState::new(
				"id",
				super::super::FieldType::BigInteger,
				false,
			)],
		);
		let to_profile =
			build_model_state_with_table_name("accounts", "Profile", "shared", sample_fields());
		let detector = MigrationAutodetector::new(
			build_project_state(vec![
				(("legacy".to_string(), "Profile".to_string()), from_profile),
				(("accounts".to_string(), "User".to_string()), from_owner),
			]),
			build_project_state(vec![(
				("accounts".to_string(), "Profile".to_string()),
				to_profile,
			)]),
		);

		let changes = detector.detect_changes();

		assert!(
			changes.moved_models.iter().any(
				|(from_app, from_model, to_app, to_model, rename_table, old_table, new_table)| {
					from_app == "legacy"
						&& from_model == "Profile"
						&& to_app == "accounts"
						&& to_model == "Profile"
						&& *rename_table && old_table.as_deref() == Some("legacy_profile")
						&& new_table.as_deref() == Some("shared")
				}
			),
			"cross-app move was not detected: {:?}",
			changes.moved_models
		);
		assert!(
			changes
				.deleted_models
				.contains(&("accounts".to_string(), "User".to_string())),
			"the displaced table owner must be recovered as deleted: {changes:?}"
		);
	}

	#[rstest]
	fn detect_changes_does_not_create_a_cross_app_move_target_reusing_a_renamed_table() {
		// Regression for #5673: a moved target that reuses the old table name of
		// another model must not emit a conflicting CreateTable operation.
		let from_existing =
			build_model_state_with_table_name("accounts", "Existing", "shared", sample_fields());
		let from_profile = build_model_state_with_table_name(
			"legacy",
			"Profile",
			"legacy_profile",
			sample_fields(),
		);
		let to_existing = build_model_state_with_table_name(
			"accounts",
			"Existing",
			"accounts_existing",
			sample_fields(),
		);
		let to_profile =
			build_model_state_with_table_name("accounts", "Profile", "shared", sample_fields());
		let detector = MigrationAutodetector::new(
			build_project_state(vec![
				(
					("accounts".to_string(), "Existing".to_string()),
					from_existing,
				),
				(("legacy".to_string(), "Profile".to_string()), from_profile),
			]),
			build_project_state(vec![
				(
					("accounts".to_string(), "Existing".to_string()),
					to_existing,
				),
				(("accounts".to_string(), "Profile".to_string()), to_profile),
			]),
		);

		let changes = detector.detect_changes();

		assert!(changes.moved_models.iter().any(
			|(_from_app, _from_model, to_app, to_model, _, _, _)| {
				to_app == "accounts" && to_model == "Profile"
			}
		));
		assert!(
			!changes
				.created_models
				.contains(&("accounts".to_string(), "Profile".to_string())),
			"moved target must not remain in created_models: {changes:?}"
		);

		let operations: Vec<_> = detector
			.generate_migrations()
			.into_iter()
			.flat_map(|migration| migration.operations)
			.collect();
		assert!(
			operations.iter().all(|operation| !matches!(
				operation,
				super::super::Operation::CreateTable { name, .. } if name == "shared"
			)),
			"cross-app move must not recreate the reused table: {operations:?}"
		);
	}

	#[test]
	fn order_renamed_table_operations_advances_past_processed_renames() {
		// Arrange
		let mut operations = vec![
			super::super::Operation::RenameTable {
				old_name: "bar".to_string(),
				new_name: "__reinhardt_rename_tmp_bar".to_string(),
			},
			super::super::Operation::RenameTable {
				old_name: "foo".to_string(),
				new_name: "bar".to_string(),
			},
			super::super::Operation::RenameTable {
				old_name: "__reinhardt_rename_tmp_bar".to_string(),
				new_name: "foo".to_string(),
			},
		];
		let expected = operations.clone();

		// Act
		MigrationAutodetector::order_renamed_table_operations(&mut operations);

		// Assert
		assert_eq!(operations, expected);
	}

	#[rstest]
	fn generate_migrations_preserves_composite_pk_with_table_rename() {
		// Arrange
		let composite_pk = ConstraintDefinition {
			name: "users_user_pkey".to_string(),
			constraint_type: "primary_key".to_string(),
			fields: vec!["id".to_string(), "tenant_id".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let fields = vec![
			FieldState::new("id", super::super::FieldType::Integer, false),
			FieldState::new("tenant_id", super::super::FieldType::Integer, false),
		];
		let mut from_model =
			build_model_state_with_table_name("users", "User", "users", fields.clone());
		from_model.constraints.push(composite_pk.clone());
		let mut to_model = build_model_state_with_table_name("users", "User", "user", fields);
		to_model.constraints.push(composite_pk);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("users".to_string(), "User".to_string()),
				from_model,
			)]),
			build_project_state(vec![(("users".to_string(), "User".to_string()), to_model)]),
		);

		// Act
		let changes = detector.detect_changes();
		let migrations = detector.generate_migrations();

		// Assert
		assert!(
			changes.added_composite_primary_keys.is_empty(),
			"unchanged composite PK must not be recreated: {:?}",
			changes.added_composite_primary_keys
		);
		assert_eq!(migrations.len(), 1, "unexpected migrations: {migrations:?}");
		assert_eq!(
			migrations[0].operations,
			vec![super::super::Operation::RenameTable {
				old_name: "users".to_string(),
				new_name: "user".to_string(),
			}]
		);
	}

	#[rstest]
	fn table_rename_recreates_single_field_unique_constraint_with_new_name() {
		// Arrange
		let email = FieldState::new("email", super::super::FieldType::VarChar(255), false);
		let old_constraint = ConstraintDefinition {
			name: "old_table_email_uniq".to_string(),
			constraint_type: "unique".to_string(),
			fields: vec!["email".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let new_constraint = ConstraintDefinition {
			name: "new_table_email_uniq".to_string(),
			..old_constraint.clone()
		};
		let mut from_model = build_model_state_with_table_name(
			"myapp",
			"OldModel",
			"old_table",
			vec![email.clone()],
		);
		from_model.constraints.push(old_constraint.clone());
		let mut to_model =
			build_model_state_with_table_name("myapp", "NewModel", "new_table", vec![email]);
		to_model.constraints.push(new_constraint.clone());
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("myapp".to_string(), "OldModel".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("myapp".to_string(), "NewModel".to_string()),
				to_model,
			)]),
		);

		// Act
		let migrations = detector.generate_migrations();

		// Assert
		assert_eq!(migrations.len(), 1);
		assert_eq!(
			migrations[0].operations,
			vec![
				super::super::Operation::DropConstraintDefinition {
					table: "old_table".to_string(),
					constraint: old_constraint.to_constraint(),
				},
				super::super::Operation::RenameTable {
					old_name: "old_table".to_string(),
					new_name: "new_table".to_string(),
				},
				super::super::Operation::AddConstraint {
					table: "new_table".to_string(),
					constraint_sql: new_constraint.to_constraint().to_string(),
				},
			]
		);
		assert_eq!(
			migrations[0].operations[0]
				.to_reverse_operation(&ProjectState::new())
				.expect("typed unique constraint drop should be reversible"),
			Some(super::super::Operation::AddConstraintDefinition {
				table: "old_table".to_string(),
				constraint: old_constraint.to_constraint(),
			})
		);
	}

	#[rstest]
	fn has_field_changed_ignores_non_schema_params() {
		// Arrange: same schema, but to_field has extra non-schema params
		let from_field = FieldState {
			name: "email".to_string(),
			field_type: super::super::FieldType::VarChar(255),
			nullable: false,
			params: std::collections::HashMap::new(),
			generated: None,
			domain: None,
			foreign_key: None,
		};
		let mut to_params = std::collections::HashMap::new();
		to_params.insert("max_length".to_string(), "255".to_string());
		to_params.insert("null".to_string(), "false".to_string());
		to_params.insert("blank".to_string(), "false".to_string());
		let to_field = FieldState {
			name: "email".to_string(),
			field_type: super::super::FieldType::VarChar(255),
			nullable: false,
			params: to_params,
			generated: None,
			domain: None,
			foreign_key: None,
		};

		let detector = MigrationAutodetector::new(ProjectState::new(), ProjectState::new());

		// Act
		let changed =
			detector.has_field_changed_with_unique("email", &from_field, &to_field, None, None);

		// Assert: should NOT be detected as changed
		assert!(
			!changed,
			"fields with identical schema but different non-schema params should not be detected as changed"
		);
	}

	fn file_field_state(upload_to: &str, file_storage: &str, storage: &str) -> FieldState {
		let mut field = FieldState::new("avatar", super::super::FieldType::VarChar(255), false);
		field
			.params
			.insert("model_field_type".to_string(), "file".to_string());
		field
			.params
			.insert("upload_to".to_string(), upload_to.to_string());
		field
			.params
			.insert("file_storage".to_string(), file_storage.to_string());
		field
			.params
			.insert("max_length".to_string(), "255".to_string());
		field
			.params
			.insert("storage".to_string(), storage.to_string());
		field
	}

	fn image_field_state(cleanup: &str, max_width: &str, max_height: &str) -> FieldState {
		let mut field = FieldState::new("image", super::super::FieldType::VarChar(255), false);
		for (key, value) in [
			("model_field_type", "image"),
			("upload_to", "images/%Y/%m/%d"),
			("file_storage", "media"),
			("max_length", "255"),
			("cleanup", cleanup),
			("max_width", max_width),
			("max_height", max_height),
		] {
			field.params.insert(key.to_owned(), value.to_owned());
		}
		field
	}

	fn altered_file_fields(from_field: FieldState, to_field: FieldState) -> DetectedChanges {
		let key = ("media".to_string(), "Asset".to_string());
		let from_model =
			build_model_state("media", "Asset", vec![from_field], Vec::new(), Vec::new());
		let to_model = build_model_state("media", "Asset", vec![to_field], Vec::new(), Vec::new());
		MigrationAutodetector::new(
			build_project_state(vec![(key.clone(), from_model)]),
			build_project_state(vec![(key, to_model)]),
		)
		.detect_changes()
	}

	#[test]
	fn file_field_semantic_params_are_detected_without_changing_physical_column_type() {
		let upload_to_change = altered_file_fields(
			file_field_state("avatars/%Y/%m/%d", "private_uploads", "external"),
			file_field_state("profiles/%Y/%m/%d", "private_uploads", "external"),
		);
		assert_eq!(
			upload_to_change.altered_fields,
			vec![(
				"media".to_string(),
				"Asset".to_string(),
				"avatar".to_string()
			)]
		);

		let file_storage_change = altered_file_fields(
			file_field_state("avatars/%Y/%m/%d", "private_uploads", "external"),
			file_field_state("avatars/%Y/%m/%d", "archive_uploads", "external"),
		);
		assert_eq!(
			file_storage_change.altered_fields,
			vec![(
				"media".to_string(),
				"Asset".to_string(),
				"avatar".to_string()
			)]
		);

		let from = file_field_state("avatars/%Y/%m/%d", "private_uploads", "external");
		let to = file_field_state("avatars/%Y/%m/%d", "private_uploads", "main");
		let physical_storage_change = altered_file_fields(from.clone(), to.clone());
		assert_eq!(
			physical_storage_change.altered_fields,
			vec![(
				"media".to_string(),
				"Asset".to_string(),
				"avatar".to_string()
			)]
		);
		assert_eq!(from.field_type, to.field_type);
		assert_eq!(from.params["file_storage"], to.params["file_storage"]);
		assert_ne!(from.params["storage"], to.params["storage"]);
	}

	#[rstest]
	fn legacy_file_field_without_cleanup_matches_explicit_default() {
		let from_field = file_field_state("avatars", "private_uploads", "external");
		let mut to_field = from_field.clone();
		to_field
			.params
			.insert("cleanup".to_owned(), "false".to_owned());
		let key = ("media".to_owned(), "Asset".to_owned());
		let from_state = build_project_state(vec![(
			key.clone(),
			build_model_state("media", "Asset", vec![from_field], Vec::new(), Vec::new()),
		)]);
		let to_state = build_project_state(vec![(
			key,
			build_model_state("media", "Asset", vec![to_field], Vec::new(), Vec::new()),
		)]);

		let detector = MigrationAutodetector::new(from_state, to_state);

		assert_eq!(detector.detect_changes().altered_fields, Vec::new());
	}

	#[rstest]
	fn legacy_file_field_without_cleanup_preserves_rename_detection() {
		let mut from_field = file_field_state("avatars", "private_uploads", "external");
		from_field.name = "avatar_path".to_owned();
		let mut to_field = from_field.clone();
		to_field.name = "avatar".to_owned();
		to_field
			.params
			.insert("cleanup".to_owned(), "false".to_owned());
		let key = ("media".to_owned(), "Asset".to_owned());
		let from_state = build_project_state(vec![(
			key.clone(),
			build_model_state(
				"media",
				"Asset",
				vec![from_field.clone()],
				Vec::new(),
				Vec::new(),
			),
		)]);
		let to_state = build_project_state(vec![(
			key,
			build_model_state(
				"media",
				"Asset",
				vec![to_field.clone()],
				Vec::new(),
				Vec::new(),
			),
		)]);

		let operations = MigrationAutodetector::new(from_state, to_state)
			.try_generate_operations()
			.expect("compatible storage field rename should generate operations");

		assert_eq!(from_field.field_type, super::super::FieldType::VarChar(255));
		assert_eq!(to_field.field_type, super::super::FieldType::VarChar(255));
		assert_eq!(
			operations,
			vec![super::super::Operation::RenameColumn {
				table: "media_asset".to_owned(),
				old_name: "avatar_path".to_owned(),
				new_name: "avatar".to_owned(),
			}]
		);
	}

	#[test]
	fn file_field_physical_storage_change_emits_postgres_set_storage() {
		let key = ("media".to_string(), "Asset".to_string());
		let from_state = build_project_state(vec![(
			key.clone(),
			build_model_state(
				"media",
				"Asset",
				vec![file_field_state(
					"avatars/%Y/%m/%d",
					"private_uploads",
					"external",
				)],
				Vec::new(),
				Vec::new(),
			),
		)]);
		let to_state = build_project_state(vec![(
			key,
			build_model_state(
				"media",
				"Asset",
				vec![file_field_state(
					"avatars/%Y/%m/%d",
					"private_uploads",
					"main",
				)],
				Vec::new(),
				Vec::new(),
			),
		)]);
		let operations = MigrationAutodetector::new(from_state, to_state).generate_operations();

		assert_eq!(operations.len(), 1);
		let sql = operations[0].to_sql(&super::super::operations::SqlDialect::Postgres);
		assert!(
			sql.contains("ALTER COLUMN avatar SET STORAGE MAIN"),
			"physical storage migration must be rendered: {sql}"
		);
	}

	#[test]
	fn generic_postgres_storage_metadata_does_not_create_repeating_changes() {
		let mut from_field = FieldState::new("title", super::super::FieldType::VarChar(255), false);
		from_field
			.params
			.insert("storage".to_string(), "external".to_string());
		let mut to_field = from_field.clone();
		to_field
			.params
			.insert("storage".to_string(), "main".to_string());
		let detector = MigrationAutodetector::new(ProjectState::new(), ProjectState::new());

		assert!(!detector.has_field_changed_with_unique(
			"title",
			&from_field,
			&to_field,
			None,
			None,
		));
	}

	#[test]
	fn file_field_storage_removal_resets_postgres_storage_to_extended() {
		let from_field = file_field_state("avatars", "private_uploads", "external");
		let mut to_field = file_field_state("avatars", "private_uploads", "external");
		to_field.params.remove("storage");
		let key = ("media".to_string(), "Asset".to_string());
		let from_state = build_project_state(vec![(
			key.clone(),
			build_model_state("media", "Asset", vec![from_field], Vec::new(), Vec::new()),
		)]);
		let to_state = build_project_state(vec![(
			key,
			build_model_state("media", "Asset", vec![to_field], Vec::new(), Vec::new()),
		)]);

		let operations = MigrationAutodetector::new(from_state, to_state).generate_operations();
		let sql = operations[0].to_sql(&super::super::operations::SqlDialect::Postgres);

		assert!(
			sql.contains("ALTER COLUMN avatar SET STORAGE EXTENDED"),
			"storage removal must restore PostgreSQL's default: {sql}"
		);
	}

	#[test]
	fn file_field_alter_replay_preserves_semantic_params_without_churn() {
		let from_field = file_field_state("avatars/%Y/%m/%d", "private_uploads", "external");
		let to_field = file_field_state("profiles/%Y/%m/%d", "private_uploads", "external");
		let key = ("media".to_string(), "Asset".to_string());
		let from_state = build_project_state(vec![(
			key.clone(),
			build_model_state("media", "Asset", vec![from_field], Vec::new(), Vec::new()),
		)]);
		let to_state = build_project_state(vec![(
			key,
			build_model_state("media", "Asset", vec![to_field], Vec::new(), Vec::new()),
		)]);

		let detector = MigrationAutodetector::new(from_state.clone(), to_state.clone());
		let operations = detector.generate_operations();
		assert_eq!(
			operations.len(),
			1,
			"policy change should emit one AlterColumn"
		);
		let sql = operations[0].to_sql(&super::super::operations::SqlDialect::Postgres);
		assert!(
			!sql.contains("__reinhardt_file_field_metadata_v1__"),
			"file-field policy envelope must never leak into SQL: {sql}"
		);

		let mut replayed_state = from_state;
		replayed_state.apply_migration_operations(&operations, "media");
		let replayed_field = replayed_state
			.get_model("media", "Asset")
			.and_then(|model| model.get_field("avatar"))
			.expect("replayed file field");
		assert_eq!(
			replayed_field.params.get("upload_to").map(String::as_str),
			Some("profiles/%Y/%m/%d"),
			"replay must retain the changed upload policy"
		);
		assert_eq!(
			replayed_field
				.params
				.get("file_storage")
				.map(String::as_str),
			Some("private_uploads"),
			"replay must retain the storage alias"
		);

		let serialized = serde_json::to_string(&operations[0]).expect("serialize AlterColumn");
		let reparsed: super::super::Operation =
			serde_json::from_str(&serialized).expect("deserialize AlterColumn");
		let mut serde_replayed_state = detector.from_state.clone();
		serde_replayed_state.apply_migration_operations(&[reparsed], "media");
		assert_eq!(
			serde_replayed_state, replayed_state,
			"serialized migration operations must preserve the same file-field state"
		);

		let rerun = MigrationAutodetector::new(replayed_state, to_state).detect_changes();
		assert!(
			rerun.altered_fields.is_empty(),
			"re-running detection after applying the migration must not emit a policy-only AlterColumn"
		);
	}

	#[test]
	fn image_field_policy_changes_emit_one_physical_path_alter_and_replay_exactly() {
		for (key, from_value, to_value) in [
			("cleanup", "true", "false"),
			("max_width", "800", "1024"),
			("max_height", "600", "768"),
		] {
			let from = image_field_state("true", "800", "600");
			let mut to = from.clone();
			to.params.insert(key.to_owned(), to_value.to_owned());
			assert_eq!(from.params[key], from_value);
			let changes = altered_file_fields(from, to);
			assert_eq!(
				changes.altered_fields,
				vec![("media".to_owned(), "Asset".to_owned(), "image".to_owned())],
				"changing only {key} must emit one AlterField"
			);
		}

		let from_field = image_field_state("true", "800", "600");
		let to_field = image_field_state("false", "800", "600");
		let key = ("media".to_owned(), "Asset".to_owned());
		let from_state = build_project_state(vec![(
			key.clone(),
			build_model_state("media", "Asset", vec![from_field], Vec::new(), Vec::new()),
		)]);
		let to_state = build_project_state(vec![(
			key,
			build_model_state("media", "Asset", vec![to_field], Vec::new(), Vec::new()),
		)]);
		let detector = MigrationAutodetector::new(from_state.clone(), to_state.clone());
		let operations = detector.generate_operations();

		assert_eq!(operations.len(), 1);
		let super::super::Operation::AlterColumn {
			old_definition: Some(old_definition),
			new_definition,
			..
		} = &operations[0]
		else {
			panic!(
				"image policy change must emit AlterColumn: {:?}",
				operations[0]
			);
		};
		assert_eq!(
			old_definition.type_definition,
			super::super::FieldType::VarChar(255)
		);
		assert_eq!(
			new_definition.type_definition,
			super::super::FieldType::VarChar(255)
		);
		let sql = operations[0].to_sql(&super::super::operations::SqlDialect::Postgres);
		assert!(!sql.contains("__reinhardt_file_field_metadata_v1__"));

		let serialized = serde_json::to_string(&operations[0]).unwrap();
		let replayed: super::super::Operation = serde_json::from_str(&serialized).unwrap();
		let mut replayed_state = from_state;
		replayed_state.apply_migration_operations(&[replayed], "media");
		let replayed_field = replayed_state
			.get_model("media", "Asset")
			.and_then(|model| model.get_field("image"))
			.unwrap();
		for (key, value) in [
			("model_field_type", "image"),
			("upload_to", "images/%Y/%m/%d"),
			("file_storage", "media"),
			("max_length", "255"),
			("cleanup", "false"),
			("max_width", "800"),
			("max_height", "600"),
		] {
			assert_eq!(
				replayed_field.params.get(key).map(String::as_str),
				Some(value)
			);
		}
		assert!(
			MigrationAutodetector::new(replayed_state, to_state)
				.detect_changes()
				.altered_fields
				.is_empty()
		);
	}

	#[rstest]
	fn has_field_changed_detects_database_default_changes() {
		// Arrange
		let from_field = FieldState::new("is_active", super::super::FieldType::Boolean, false);
		let mut to_field = FieldState::new("is_active", super::super::FieldType::Boolean, false);
		to_field
			.params
			.insert("default".to_string(), "true".to_string());
		let detector = MigrationAutodetector::new(ProjectState::new(), ProjectState::new());

		// Act
		let changed =
			detector.has_field_changed_with_unique("is_active", &from_field, &to_field, None, None);

		// Assert
		assert!(
			changed,
			"database default changes must be detected as schema-affecting field changes"
		);
	}

	#[rstest]
	fn has_field_changed_detects_generated_column_changes() {
		// Arrange
		let from_field = FieldState::new("full_name", super::super::FieldType::VarChar(201), false);
		let mut to_field =
			FieldState::new("full_name", super::super::FieldType::VarChar(201), false);
		to_field.generated = Some(super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::concat([
				super::super::SchemaExpr::col("first_name"),
				super::super::SchemaExpr::val(" "),
				super::super::SchemaExpr::col("last_name"),
			]),
			"SchemaExpr::concat([SchemaExpr::col(\"first_name\"), SchemaExpr::val(\" \"), SchemaExpr::col(\"last_name\")])",
			super::super::GeneratedStorage::Stored,
		));
		let detector = MigrationAutodetector::new(ProjectState::new(), ProjectState::new());

		// Act
		let changed =
			detector.has_field_changed_with_unique("full_name", &from_field, &to_field, None, None);

		// Assert
		assert!(
			changed,
			"generated column metadata changes must be detected as schema-affecting field changes"
		);
	}

	#[rstest]
	fn generated_column_changes_emit_drop_and_add_not_alter() {
		// Arrange
		let mut from_field =
			FieldState::new("full_name", super::super::FieldType::VarChar(201), false);
		let from_generated = super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::concat([
				super::super::SchemaExpr::col("first_name"),
				super::super::SchemaExpr::val(" "),
				super::super::SchemaExpr::col("last_name"),
			]),
			"SchemaExpr::concat([SchemaExpr::col(\"first_name\"), SchemaExpr::val(\" \"), SchemaExpr::col(\"last_name\")])",
			super::super::GeneratedStorage::Stored,
		);
		from_field.generated = Some(from_generated.clone());
		let mut to_field =
			FieldState::new("full_name", super::super::FieldType::VarChar(201), false);
		let to_generated = super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("display_name"),
			"SchemaExpr::col(\"display_name\")",
			super::super::GeneratedStorage::Stored,
		);
		to_field.generated = Some(to_generated.clone());
		let from_model =
			build_model_state("accounts", "User", vec![from_field], Vec::new(), Vec::new());
		let to_model =
			build_model_state("accounts", "User", vec![to_field], Vec::new(), Vec::new());
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				to_model,
			)]),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert_eq!(operations.len(), 2, "unexpected operations: {operations:?}");
		assert!(matches!(
			&operations[0],
			super::super::Operation::DropColumn {
				table,
				column,
				old_definition: Some(old_definition)
			} if table == "accounts_user"
				&& column == "full_name"
				&& old_definition.generated == Some(from_generated.clone())
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::AddColumn { table, column, .. }
				if table == "accounts_user"
					&& column.name == "full_name"
					&& column.generated == Some(to_generated.clone())
		));
		assert!(
			!operations
				.iter()
				.any(|operation| matches!(operation, super::super::Operation::AlterColumn { .. })),
			"generated column changes must not emit no-op AlterColumn: {operations:?}"
		);
	}

	#[rstest]
	fn mixed_generated_and_type_changes_emit_drop_and_add_not_alter() {
		// Arrange
		let mut from_field =
			FieldState::new("full_name", super::super::FieldType::VarChar(201), false);
		from_field.generated = Some(super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::concat([
				super::super::SchemaExpr::col("first_name"),
				super::super::SchemaExpr::val(" "),
				super::super::SchemaExpr::col("last_name"),
			]),
			"SchemaExpr::concat([SchemaExpr::col(\"first_name\"), SchemaExpr::val(\" \"), SchemaExpr::col(\"last_name\")])",
			super::super::GeneratedStorage::Stored,
		));
		let mut to_field = FieldState::new("full_name", super::super::FieldType::Text, false);
		to_field.generated = Some(super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("display_name"),
			"SchemaExpr::col(\"display_name\")",
			super::super::GeneratedStorage::Stored,
		));
		let from_model =
			build_model_state("accounts", "User", vec![from_field], Vec::new(), Vec::new());
		let to_model =
			build_model_state("accounts", "User", vec![to_field], Vec::new(), Vec::new());
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				to_model,
			)]),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert!(matches!(
			&operations[0],
			super::super::Operation::DropColumn { table, column, .. }
				if table == "accounts_user" && column == "full_name"
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::AddColumn { table, column, .. }
				if table == "accounts_user"
					&& column.name == "full_name"
					&& matches!(column.type_definition, super::super::FieldType::Text)
		));
		assert!(
			!operations
				.iter()
				.any(|operation| matches!(operation, super::super::Operation::AlterColumn { .. })),
			"mixed generated column changes must not emit no-op AlterColumn: {operations:?}"
		);
	}

	#[rstest]
	fn generated_column_replacement_recreates_dependent_generated_columns() {
		// Arrange
		let mut from_full_name =
			FieldState::new("full_name", super::super::FieldType::VarChar(201), false);
		from_full_name.generated = Some(super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("name"),
			"SchemaExpr::col(\"name\")",
			super::super::GeneratedStorage::Stored,
		));
		let mut to_full_name =
			FieldState::new("full_name", super::super::FieldType::VarChar(201), false);
		let to_full_name_generated = super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("display_name"),
			"SchemaExpr::col(\"display_name\")",
			super::super::GeneratedStorage::Stored,
		);
		to_full_name.generated = Some(to_full_name_generated.clone());

		let mut from_search_name =
			FieldState::new("search_name", super::super::FieldType::VarChar(201), false);
		let search_name_generated = super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("full_name"),
			"SchemaExpr::col(\"full_name\")",
			super::super::GeneratedStorage::Stored,
		);
		from_search_name.generated = Some(search_name_generated.clone());
		let mut to_search_name =
			FieldState::new("search_name", super::super::FieldType::VarChar(201), false);
		to_search_name.generated = Some(search_name_generated.clone());
		let mut from_search_key =
			FieldState::new("search_key", super::super::FieldType::VarChar(201), false);
		let search_key_generated = super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("search_name"),
			"SchemaExpr::col(\"search_name\")",
			super::super::GeneratedStorage::Stored,
		);
		from_search_key.generated = Some(search_key_generated.clone());
		let mut to_search_key =
			FieldState::new("search_key", super::super::FieldType::VarChar(201), false);
		to_search_key.generated = Some(search_key_generated.clone());

		let from_model = build_model_state(
			"accounts",
			"User",
			vec![from_full_name, from_search_name, from_search_key],
			Vec::new(),
			Vec::new(),
		);
		let to_model = build_model_state(
			"accounts",
			"User",
			vec![to_full_name, to_search_name, to_search_key],
			Vec::new(),
			Vec::new(),
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				to_model,
			)]),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert_eq!(operations.len(), 6, "unexpected operations: {operations:?}");
		assert!(matches!(
			&operations[0],
			super::super::Operation::DropColumn {
				table,
				column,
				old_definition: Some(old_definition)
			} if table == "accounts_user"
				&& column == "search_key"
				&& old_definition.generated == Some(search_key_generated.clone())
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::DropColumn {
				table,
				column,
				old_definition: Some(old_definition)
			} if table == "accounts_user"
				&& column == "search_name"
				&& old_definition.generated == Some(search_name_generated.clone())
		));
		assert!(matches!(
			&operations[2],
			super::super::Operation::DropColumn { table, column, .. }
				if table == "accounts_user" && column == "full_name"
		));
		assert!(matches!(
			&operations[3],
			super::super::Operation::AddColumn { table, column, .. }
				if table == "accounts_user"
					&& column.name == "full_name"
					&& column.generated == Some(to_full_name_generated.clone())
		));
		assert!(matches!(
			&operations[4],
			super::super::Operation::AddColumn { table, column, .. }
				if table == "accounts_user"
					&& column.name == "search_name"
					&& column.generated == Some(search_name_generated.clone())
		));
		assert!(matches!(
			&operations[5],
			super::super::Operation::AddColumn { table, column, .. }
				if table == "accounts_user"
					&& column.name == "search_key"
			&& column.generated == Some(search_key_generated.clone())
		));
	}

	#[rstest]
	fn added_generated_columns_follow_new_source_columns() {
		// Arrange
		let from_model = build_model_state("accounts", "User", Vec::new(), Vec::new(), Vec::new());
		let last_name = FieldState::new("last_name", super::super::FieldType::VarChar(100), false);
		let mut full_name =
			FieldState::new("full_name", super::super::FieldType::VarChar(201), false);
		let full_name_generated = super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("last_name"),
			"SchemaExpr::col(\"last_name\")",
			super::super::GeneratedStorage::Stored,
		);
		full_name.generated = Some(full_name_generated.clone());
		let to_model = build_model_state(
			"accounts",
			"User",
			vec![full_name, last_name],
			Vec::new(),
			Vec::new(),
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				to_model,
			)]),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert_eq!(operations.len(), 2, "unexpected operations: {operations:?}");
		assert!(matches!(
			&operations[0],
			super::super::Operation::AddColumn { table, column, .. }
				if table == "accounts_user" && column.name == "last_name"
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::AddColumn { table, column, .. }
				if table == "accounts_user"
					&& column.name == "full_name"
					&& column.generated == Some(full_name_generated)
		));
	}

	#[rstest]
	fn added_generated_columns_wait_for_source_alters() {
		// Arrange
		let from_name = FieldState::new("name", super::super::FieldType::Integer, false);
		let to_name = FieldState::new("name", super::super::FieldType::BigInteger, false);
		let mut full_name =
			FieldState::new("full_name", super::super::FieldType::VarChar(201), false);
		let full_name_generated = super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("name"),
			"SchemaExpr::col(\"name\")",
			super::super::GeneratedStorage::Stored,
		);
		full_name.generated = Some(full_name_generated.clone());
		let from_model =
			build_model_state("accounts", "User", vec![from_name], Vec::new(), Vec::new());
		let to_model = build_model_state(
			"accounts",
			"User",
			vec![full_name, to_name],
			Vec::new(),
			Vec::new(),
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				to_model,
			)]),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert_eq!(operations.len(), 2, "unexpected operations: {operations:?}");
		assert!(matches!(
			&operations[0],
			super::super::Operation::AlterColumn {
				table,
				column,
				new_definition,
				..
			} if table == "accounts_user"
				&& column == "name"
				&& matches!(new_definition.type_definition, super::super::FieldType::BigInteger)
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::AddColumn { table, column, .. }
				if table == "accounts_user"
					&& column.name == "full_name"
					&& column.generated == Some(full_name_generated)
		));
	}

	#[rstest]
	fn removed_generated_dependents_drop_before_sources() {
		// Arrange
		let source = FieldState::new("a", super::super::FieldType::Integer, false);
		let mut dependent = FieldState::new("z", super::super::FieldType::Integer, false);
		let dependent_generated = super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("a"),
			"SchemaExpr::col(\"a\")",
			super::super::GeneratedStorage::Stored,
		);
		dependent.generated = Some(dependent_generated.clone());
		let from_model = build_model_state(
			"accounts",
			"Metric",
			vec![source, dependent],
			Vec::new(),
			Vec::new(),
		);
		let to_model = build_model_state("accounts", "Metric", Vec::new(), Vec::new(), Vec::new());
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("accounts".to_string(), "Metric".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("accounts".to_string(), "Metric".to_string()),
				to_model,
			)]),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert_eq!(operations.len(), 2, "unexpected operations: {operations:?}");
		assert!(matches!(
			&operations[0],
			super::super::Operation::DropColumn {
				table,
				column,
				old_definition: Some(old_definition),
			} if table == "accounts_metric"
				&& column == "z"
				&& old_definition.generated == Some(dependent_generated)
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::DropColumn { table, column, .. }
				if table == "accounts_metric" && column == "a"
		));
	}

	#[rstest]
	fn removed_generated_dependent_drops_before_source_alter() {
		// Arrange
		let from_source = FieldState::new("a", super::super::FieldType::Integer, false);
		let to_source = FieldState::new("a", super::super::FieldType::BigInteger, false);
		let mut dependent = FieldState::new("z", super::super::FieldType::Integer, false);
		let dependent_generated = super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("a"),
			"SchemaExpr::col(\"a\")",
			super::super::GeneratedStorage::Stored,
		);
		dependent.generated = Some(dependent_generated.clone());
		let from_model = build_model_state(
			"accounts",
			"Metric",
			vec![from_source, dependent],
			Vec::new(),
			Vec::new(),
		);
		let to_model = build_model_state(
			"accounts",
			"Metric",
			vec![to_source],
			Vec::new(),
			Vec::new(),
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("accounts".to_string(), "Metric".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("accounts".to_string(), "Metric".to_string()),
				to_model,
			)]),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert_eq!(operations.len(), 2, "unexpected operations: {operations:?}");
		assert!(matches!(
			&operations[0],
			super::super::Operation::DropColumn {
				table,
				column,
				old_definition: Some(old_definition),
			} if table == "accounts_metric"
				&& column == "z"
				&& old_definition.generated == Some(dependent_generated)
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::AlterColumn {
				table,
				column,
				new_definition,
				..
			} if table == "accounts_metric"
				&& column == "a"
				&& matches!(new_definition.type_definition, super::super::FieldType::BigInteger)
		));
	}

	#[rstest]
	fn source_column_alter_recreates_generated_dependents() {
		// Arrange
		let from_amount = FieldState::new("amount", super::super::FieldType::Integer, false);
		let mut from_amount_text =
			FieldState::new("amount_text", super::super::FieldType::VarChar(20), false);
		let amount_text_generated = super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("amount"),
			"SchemaExpr::col(\"amount\")",
			super::super::GeneratedStorage::Stored,
		);
		from_amount_text.generated = Some(amount_text_generated.clone());

		let to_amount = FieldState::new("amount", super::super::FieldType::BigInteger, false);
		let mut to_amount_text =
			FieldState::new("amount_text", super::super::FieldType::VarChar(20), false);
		to_amount_text.generated = Some(amount_text_generated.clone());

		let from_model = build_model_state(
			"billing",
			"Invoice",
			vec![from_amount, from_amount_text],
			Vec::new(),
			Vec::new(),
		);
		let to_model = build_model_state(
			"billing",
			"Invoice",
			vec![to_amount, to_amount_text],
			Vec::new(),
			Vec::new(),
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("billing".to_string(), "Invoice".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("billing".to_string(), "Invoice".to_string()),
				to_model,
			)]),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert_eq!(operations.len(), 3, "unexpected operations: {operations:?}");
		assert!(matches!(
			&operations[0],
			super::super::Operation::DropColumn {
				table,
				column,
				old_definition: Some(old_definition),
			} if table == "billing_invoice"
				&& column == "amount_text"
				&& old_definition.generated == Some(amount_text_generated.clone())
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::AlterColumn {
				table,
				column,
				new_definition,
				..
			} if table == "billing_invoice"
				&& column == "amount"
				&& matches!(new_definition.type_definition, super::super::FieldType::BigInteger)
		));
		assert!(matches!(
			&operations[2],
			super::super::Operation::AddColumn { table, column, .. }
				if table == "billing_invoice"
					&& column.name == "amount_text"
					&& column.generated == Some(amount_text_generated)
		));
	}

	#[rstest]
	fn changed_generated_dependent_waits_for_source_alter() {
		// Arrange
		let from_name = FieldState::new("name", super::super::FieldType::Integer, false);
		let to_name = FieldState::new("name", super::super::FieldType::BigInteger, false);
		let mut from_full_name =
			FieldState::new("full_name", super::super::FieldType::VarChar(201), false);
		let from_full_name_generated = super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("name"),
			"SchemaExpr::col(\"name\")",
			super::super::GeneratedStorage::Stored,
		);
		from_full_name.generated = Some(from_full_name_generated.clone());
		let mut to_full_name =
			FieldState::new("full_name", super::super::FieldType::VarChar(201), false);
		let to_full_name_generated = super::super::GeneratedColumnDefinition::raw_sql(
			"lower(name)",
			super::super::GeneratedStorage::Stored,
		);
		to_full_name.generated = Some(to_full_name_generated.clone());
		let from_model = build_model_state(
			"accounts",
			"User",
			vec![from_full_name, from_name],
			Vec::new(),
			Vec::new(),
		);
		let to_model = build_model_state(
			"accounts",
			"User",
			vec![to_full_name, to_name],
			Vec::new(),
			Vec::new(),
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				to_model,
			)]),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert_eq!(operations.len(), 3, "unexpected operations: {operations:?}");
		assert!(matches!(
			&operations[0],
			super::super::Operation::DropColumn {
				table,
				column,
				old_definition: Some(old_definition),
			} if table == "accounts_user"
				&& column == "full_name"
				&& old_definition.generated == Some(from_full_name_generated)
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::AlterColumn {
				table,
				column,
				new_definition,
				..
			} if table == "accounts_user"
				&& column == "name"
				&& matches!(new_definition.type_definition, super::super::FieldType::BigInteger)
		));
		assert!(matches!(
			&operations[2],
			super::super::Operation::AddColumn { table, column, .. }
				if table == "accounts_user"
					&& column.name == "full_name"
					&& column.generated == Some(to_full_name_generated)
		));
	}

	#[rstest]
	fn created_model_orders_generated_columns_by_dependencies() {
		// Arrange
		let source = FieldState::new("source", super::super::FieldType::Integer, false);
		let mut z = FieldState::new("z", super::super::FieldType::Integer, false);
		z.generated = Some(super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("source"),
			"SchemaExpr::col(\"source\")",
			super::super::GeneratedStorage::Stored,
		));
		let mut a = FieldState::new("a", super::super::FieldType::Integer, false);
		a.generated = Some(super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("z"),
			"SchemaExpr::col(\"z\")",
			super::super::GeneratedStorage::Stored,
		));
		let to_model = build_model_state(
			"metrics",
			"Metric",
			vec![a, source, z],
			Vec::new(),
			Vec::new(),
		);
		let detector = MigrationAutodetector::new(
			ProjectState::new(),
			build_project_state(vec![(
				("metrics".to_string(), "Metric".to_string()),
				to_model,
			)]),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		let super::super::Operation::CreateTable { columns, .. } = &operations[0] else {
			panic!("expected CreateTable operation, got: {operations:?}");
		};
		let column_names: Vec<_> = columns.iter().map(|column| column.name.as_str()).collect();
		assert_eq!(column_names, vec!["source", "z", "a"]);
	}

	#[rstest]
	fn raw_generated_dependency_matching_is_case_insensitive() {
		// Arrange
		let mut from_name = FieldState::new("name", super::super::FieldType::VarChar(100), false);
		let from_name_generated = super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("raw_name"),
			"SchemaExpr::col(\"raw_name\")",
			super::super::GeneratedStorage::Stored,
		);
		from_name.generated = Some(from_name_generated.clone());
		let mut to_name = FieldState::new("name", super::super::FieldType::VarChar(100), false);
		let to_name_generated = super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("display_name"),
			"SchemaExpr::col(\"display_name\")",
			super::super::GeneratedStorage::Stored,
		);
		to_name.generated = Some(to_name_generated);
		let mut from_search_name =
			FieldState::new("search_name", super::super::FieldType::VarChar(100), false);
		let search_name_generated = super::super::GeneratedColumnDefinition::raw_sql(
			"LOWER(NAME)",
			super::super::GeneratedStorage::Stored,
		);
		from_search_name.generated = Some(search_name_generated.clone());
		let mut to_search_name =
			FieldState::new("search_name", super::super::FieldType::VarChar(100), false);
		to_search_name.generated = Some(search_name_generated.clone());

		let from_model = build_model_state(
			"accounts",
			"User",
			vec![from_name, from_search_name],
			Vec::new(),
			Vec::new(),
		);
		let to_model = build_model_state(
			"accounts",
			"User",
			vec![to_name, to_search_name],
			Vec::new(),
			Vec::new(),
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				to_model,
			)]),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert!(matches!(
			&operations[0],
			super::super::Operation::DropColumn {
				table,
				column,
				old_definition: Some(old_definition),
			} if table == "accounts_user"
				&& column == "search_name"
				&& old_definition.generated == Some(search_name_generated)
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::DropColumn {
				table,
				column,
				old_definition: Some(old_definition),
			} if table == "accounts_user"
				&& column == "name"
				&& old_definition.generated == Some(from_name_generated)
		));
	}

	#[rstest]
	fn generated_column_replacement_keeps_changed_dependent_in_replacement_order() {
		// Arrange
		let mut from_full_name =
			FieldState::new("full_name", super::super::FieldType::VarChar(201), false);
		let from_full_name_generated = super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("name"),
			"SchemaExpr::col(\"name\")",
			super::super::GeneratedStorage::Stored,
		);
		from_full_name.generated = Some(from_full_name_generated.clone());
		let mut to_full_name =
			FieldState::new("full_name", super::super::FieldType::VarChar(201), false);
		let to_full_name_generated = super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("display_name"),
			"SchemaExpr::col(\"display_name\")",
			super::super::GeneratedStorage::Stored,
		);
		to_full_name.generated = Some(to_full_name_generated.clone());

		let mut from_search_name =
			FieldState::new("search_name", super::super::FieldType::VarChar(201), false);
		let from_search_name_generated = super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("full_name"),
			"SchemaExpr::col(\"full_name\")",
			super::super::GeneratedStorage::Stored,
		);
		from_search_name.generated = Some(from_search_name_generated.clone());
		let mut to_search_name =
			FieldState::new("search_name", super::super::FieldType::VarChar(201), false);
		let to_search_name_generated = super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::concat([
				super::super::SchemaExpr::col("full_name"),
				super::super::SchemaExpr::val("!"),
			]),
			"SchemaExpr::concat([SchemaExpr::col(\"full_name\"), SchemaExpr::val(\"!\")])",
			super::super::GeneratedStorage::Stored,
		);
		to_search_name.generated = Some(to_search_name_generated.clone());

		let from_model = build_model_state(
			"accounts",
			"User",
			vec![from_full_name, from_search_name],
			Vec::new(),
			Vec::new(),
		);
		let to_model = build_model_state(
			"accounts",
			"User",
			vec![to_full_name, to_search_name],
			Vec::new(),
			Vec::new(),
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				to_model,
			)]),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert_eq!(operations.len(), 4, "unexpected operations: {operations:?}");
		assert!(matches!(
			&operations[0],
			super::super::Operation::DropColumn {
				table,
				column,
				old_definition: Some(old_definition)
			} if table == "accounts_user"
				&& column == "search_name"
				&& old_definition.generated == Some(from_search_name_generated.clone())
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::DropColumn {
				table,
				column,
				old_definition: Some(old_definition)
			} if table == "accounts_user"
				&& column == "full_name"
				&& old_definition.generated == Some(from_full_name_generated.clone())
		));
		assert!(matches!(
			&operations[2],
			super::super::Operation::AddColumn { table, column, .. }
				if table == "accounts_user"
					&& column.name == "full_name"
					&& column.generated == Some(to_full_name_generated.clone())
		));
		assert!(matches!(
			&operations[3],
			super::super::Operation::AddColumn { table, column, .. }
				if table == "accounts_user"
					&& column.name == "search_name"
					&& column.generated == Some(to_search_name_generated.clone())
		));
	}

	#[rstest]
	fn generated_column_replacement_stays_after_source_column_rename() {
		// Arrange
		let from_first_name =
			FieldState::new("first_name", super::super::FieldType::VarChar(100), false);
		let mut from_full_name =
			FieldState::new("full_name", super::super::FieldType::VarChar(201), false);
		let from_full_name_generated = super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("first_name"),
			"SchemaExpr::col(\"first_name\")",
			super::super::GeneratedStorage::Stored,
		);
		from_full_name.generated = Some(from_full_name_generated.clone());

		let to_given_name =
			FieldState::new("given_name", super::super::FieldType::VarChar(100), false);
		let mut to_full_name =
			FieldState::new("full_name", super::super::FieldType::VarChar(201), false);
		let to_full_name_generated = super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("given_name"),
			"SchemaExpr::col(\"given_name\")",
			super::super::GeneratedStorage::Stored,
		);
		to_full_name.generated = Some(to_full_name_generated.clone());

		let from_model = build_model_state(
			"accounts",
			"User",
			vec![from_first_name, from_full_name],
			Vec::new(),
			Vec::new(),
		);
		let to_model = build_model_state(
			"accounts",
			"User",
			vec![to_given_name, to_full_name],
			Vec::new(),
			Vec::new(),
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				to_model,
			)]),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert_eq!(operations.len(), 3, "unexpected operations: {operations:?}");
		assert!(matches!(
			&operations[0],
			super::super::Operation::RenameColumn {
				table,
				old_name,
				new_name,
			} if table == "accounts_user"
				&& old_name == "first_name"
				&& new_name == "given_name"
		));
		assert!(matches!(
		&operations[1],
		super::super::Operation::DropColumn {
			table,
			column,
			old_definition: Some(old_definition)
		} if table == "accounts_user"
			&& column == "full_name"
			&& old_definition.generated == Some(from_full_name_generated.clone())
		));
		assert!(matches!(
			&operations[2],
			super::super::Operation::AddColumn { table, column, .. }
				if table == "accounts_user"
					&& column.name == "full_name"
					&& column.generated == Some(to_full_name_generated.clone())
		));
	}

	#[rstest]
	fn generated_column_addition_stays_after_source_column_rename() {
		// Arrange
		let from_first_name =
			FieldState::new("first_name", super::super::FieldType::VarChar(100), false);
		let to_given_name =
			FieldState::new("given_name", super::super::FieldType::VarChar(100), false);
		let mut to_full_name =
			FieldState::new("full_name", super::super::FieldType::VarChar(201), false);
		let to_full_name_generated = super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("given_name"),
			"SchemaExpr::col(\"given_name\")",
			super::super::GeneratedStorage::Stored,
		);
		to_full_name.generated = Some(to_full_name_generated.clone());

		let from_model = build_model_state(
			"accounts",
			"User",
			vec![from_first_name],
			Vec::new(),
			Vec::new(),
		);
		let to_model = build_model_state(
			"accounts",
			"User",
			vec![to_given_name, to_full_name],
			Vec::new(),
			Vec::new(),
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				to_model,
			)]),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert_eq!(operations.len(), 2, "unexpected operations: {operations:?}");
		assert!(matches!(
			&operations[0],
			super::super::Operation::RenameColumn {
				table,
				old_name,
				new_name,
			} if table == "accounts_user"
				&& old_name == "first_name"
				&& new_name == "given_name"
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::AddColumn { table, column, .. }
				if table == "accounts_user"
					&& column.name == "full_name"
					&& column.generated == Some(to_full_name_generated.clone())
		));
	}

	#[cfg(feature = "pgvector")]
	#[rstest]
	fn generated_column_replacement_recreates_dependent_indexes_and_constraints() {
		// Arrange
		let mut from_field =
			FieldState::new("full_name", super::super::FieldType::VarChar(201), false);
		from_field.generated = Some(super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("name"),
			"SchemaExpr::col(\"name\")",
			super::super::GeneratedStorage::Stored,
		));
		let mut to_field =
			FieldState::new("full_name", super::super::FieldType::VarChar(201), false);
		to_field.generated = Some(super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("display_name"),
			"SchemaExpr::col(\"display_name\")",
			super::super::GeneratedStorage::Stored,
		));
		let index = IndexDefinition {
			name: "idx_accounts_user_full_name".to_string(),
			fields: vec!["full_name".to_string()],
			unique: false,
			where_clause: None,
			index_type: Some(crate::migrations::operations::IndexType::Hnsw {
				m: Some(24),
				ef_construction: Some(96),
			}),
			operator_class: Some("vector_ip_ops".to_string()),
			expressions: None,
		};
		let constraint = ConstraintDefinition {
			name: "uq_accounts_user_full_name".to_string(),
			constraint_type: "unique".to_string(),
			fields: vec!["full_name".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let from_model = build_model_state(
			"accounts",
			"User",
			vec![from_field],
			vec![index.clone()],
			vec![constraint.clone()],
		);
		let to_model = build_model_state(
			"accounts",
			"User",
			vec![to_field],
			vec![index],
			vec![constraint],
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				to_model,
			)]),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert!(matches!(
			&operations[0],
			super::super::Operation::RestoreIndexOnRollback {
				table,
				name: Some(name),
				columns,
				unique,
				index_type: Some(crate::migrations::operations::IndexType::Hnsw {
					m: Some(24),
					ef_construction: Some(96),
				}),
				operator_class: Some(operator_class),
				..
			} if table == "accounts_user"
				&& name == "idx_accounts_user_full_name"
				&& columns == &vec!["full_name".to_string()]
				&& !unique
				&& operator_class == "vector_ip_ops"
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::RestoreConstraintOnRollback { table, constraint_sql }
				if table == "accounts_user"
					&& constraint_sql.contains("uq_accounts_user_full_name")
					&& constraint_sql.contains("full_name")
		));
		assert!(matches!(
			&operations[2],
			super::super::Operation::DropColumn { table, column, .. }
				if table == "accounts_user" && column == "full_name"
		));
		assert!(matches!(
			&operations[3],
			super::super::Operation::AddColumn { table, column, .. }
				if table == "accounts_user" && column.name == "full_name"
		));
		assert!(operations.iter().any(|operation| matches!(
			operation,
			super::super::Operation::RestoreIndexOnRollback {
				table,
				name: Some(name),
				columns,
				unique,
				..
			}
				if table == "accounts_user"
					&& name == "idx_accounts_user_full_name"
					&& columns == &vec!["full_name".to_string()]
					&& !unique
		)));
		assert!(operations.iter().any(|operation| matches!(
			operation,
			super::super::Operation::RestoreConstraintOnRollback { table, constraint_sql }
				if table == "accounts_user"
					&& constraint_sql.contains("uq_accounts_user_full_name")
					&& constraint_sql.contains("full_name")
		)));
		assert!(operations.iter().any(|operation| matches!(
			operation,
			super::super::Operation::CreateIndexRepair {
				table,
				name: Some(name),
				columns,
				unique,
				index_type: Some(crate::migrations::operations::IndexType::Hnsw {
					m: Some(24),
					ef_construction: Some(96),
				}),
				operator_class: Some(operator_class),
				..
			}
				if table == "accounts_user"
					&& name == "idx_accounts_user_full_name"
					&& columns == &vec!["full_name".to_string()]
					&& !unique
					&& operator_class == "vector_ip_ops"
		)));
		assert!(operations.iter().any(|operation| matches!(
				operation,
				super::super::Operation::AddConstraintRepair { table, constraint_sql }
					if table == "accounts_user"
						&& constraint_sql.contains("uq_accounts_user_full_name")
						&& constraint_sql.contains("full_name")
		)));
	}

	#[rstest]
	fn generated_column_replacement_suppresses_stale_removed_constraints() {
		// Arrange
		let first_name =
			FieldState::new("first_name", super::super::FieldType::VarChar(100), false);
		let display_name =
			FieldState::new("display_name", super::super::FieldType::VarChar(100), false);
		let mut from_field =
			FieldState::new("full_name", super::super::FieldType::VarChar(201), false);
		from_field.generated = Some(super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("first_name"),
			"SchemaExpr::col(\"first_name\")",
			super::super::GeneratedStorage::Stored,
		));
		let mut to_field =
			FieldState::new("full_name", super::super::FieldType::VarChar(201), false);
		to_field.generated = Some(super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("display_name"),
			"SchemaExpr::col(\"display_name\")",
			super::super::GeneratedStorage::Stored,
		));
		let constraint = ConstraintDefinition {
			name: "ck_accounts_user_full_name".to_string(),
			constraint_type: "check".to_string(),
			fields: Vec::new(),
			expression: Some("length(full_name) > 0".to_string()),
			foreign_key_info: None,
		};
		let from_model = build_model_state(
			"accounts",
			"User",
			vec![first_name.clone(), from_field],
			Vec::new(),
			vec![constraint],
		);
		let to_model = build_model_state(
			"accounts",
			"User",
			vec![first_name, display_name, to_field],
			Vec::new(),
			Vec::new(),
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				to_model,
			)]),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert!(operations.iter().any(|operation| matches!(
			operation,
			super::super::Operation::DropColumn { table, column, .. }
				if table == "accounts_user" && column == "full_name"
		)));
		assert!(operations.iter().any(|operation| matches!(
			operation,
			super::super::Operation::AddColumn { table, column, .. }
				if table == "accounts_user" && column.name == "full_name"
		)));
		assert!(
			operations.iter().all(|operation| !matches!(
				operation,
				super::super::Operation::DropConstraint { .. }
			)),
			"generated replacement should not emit stale DropConstraint: {operations:?}"
		);
	}

	#[rstest]
	fn generated_column_replacement_recreates_expression_constraints() {
		// Arrange
		let mut from_field =
			FieldState::new("full_name", super::super::FieldType::VarChar(201), false);
		from_field.generated = Some(super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("name"),
			"SchemaExpr::col(\"name\")",
			super::super::GeneratedStorage::Stored,
		));
		let mut to_field =
			FieldState::new("full_name", super::super::FieldType::VarChar(201), false);
		to_field.generated = Some(super::super::GeneratedColumnDefinition::typed(
			super::super::SchemaExpr::col("display_name"),
			"SchemaExpr::col(\"display_name\")",
			super::super::GeneratedStorage::Stored,
		));
		let constraint = ConstraintDefinition {
			name: "ck_accounts_user_full_name_not_empty".to_string(),
			constraint_type: "check".to_string(),
			fields: Vec::new(),
			expression: Some("full_name <> ''".to_string()),
			foreign_key_info: None,
		};
		let from_model = build_model_state(
			"accounts",
			"User",
			vec![from_field],
			Vec::new(),
			vec![constraint.clone()],
		);
		let to_model = build_model_state(
			"accounts",
			"User",
			vec![to_field],
			Vec::new(),
			vec![constraint],
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				to_model,
			)]),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert!(operations.iter().any(|operation| matches!(
			operation,
			super::super::Operation::RestoreConstraintOnRollback { table, constraint_sql }
				if table == "accounts_user"
					&& constraint_sql
						== "CONSTRAINT ck_accounts_user_full_name_not_empty CHECK (full_name <> '')"
		)));
		assert!(operations.iter().any(|operation| matches!(
			operation,
			super::super::Operation::AddConstraintRepair { table, constraint_sql }
				if table == "accounts_user"
					&& constraint_sql
						== "CONSTRAINT ck_accounts_user_full_name_not_empty CHECK (full_name <> '')"
		)));
	}

	#[rstest]
	fn generate_operations_carries_old_definition_for_database_default_changes() {
		// Arrange
		let mut from_field = FieldState::new("is_active", super::super::FieldType::Boolean, false);
		from_field
			.params
			.insert("default".to_string(), "true".to_string());
		let to_field = FieldState::new("is_active", super::super::FieldType::Boolean, false);
		let from_model =
			build_model_state("accounts", "User", vec![from_field], Vec::new(), Vec::new());
		let to_model =
			build_model_state("accounts", "User", vec![to_field], Vec::new(), Vec::new());
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				from_model,
			)]),
			build_project_state(vec![(
				("accounts".to_string(), "User".to_string()),
				to_model,
			)]),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		let operation = operations
			.iter()
			.find(|operation| {
				matches!(
					operation,
					super::super::Operation::AlterColumn { column, .. } if column == "is_active"
				)
			})
			.expect("default removal should emit AlterColumn");
		let super::super::Operation::AlterColumn {
			old_definition,
			new_definition,
			..
		} = operation
		else {
			unreachable!("matched AlterColumn above");
		};
		assert_eq!(
			old_definition
				.as_ref()
				.and_then(|definition| definition.default.as_deref()),
			Some("true")
		);
		assert_eq!(new_definition.default, None);
	}

	#[rstest]
	fn generate_operations_empty_for_struct_only_rename() {
		// Arrange: struct name changed but table name and fields are the same
		let from_model =
			build_model_state_with_table_name("myapp", "Clusters", "clusters", sample_fields());
		let to_model =
			build_model_state_with_table_name("myapp", "Cluster", "clusters", sample_fields());

		let from_state = build_project_state(vec![(
			("myapp".to_string(), "Clusters".to_string()),
			from_model,
		)]);
		let to_state = build_project_state(vec![(
			("myapp".to_string(), "Cluster".to_string()),
			to_model,
		)]);

		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let operations = detector.generate_operations();

		// Assert: no operations should be generated
		assert!(
			operations.is_empty(),
			"struct-only rename with same table name and identical fields should produce no operations, got: {:?}",
			operations
		);
	}

	#[rstest]
	fn detect_composite_pk_added_emits_create_composite_primary_key() {
		// Arrange
		let id_field = FieldState::new("id", super::super::FieldType::Integer, false);
		let tenant_id_field = FieldState::new("tenant_id", super::super::FieldType::Integer, false);

		let from_model = build_model_state(
			"billing",
			"Invoice",
			vec![id_field.clone(), tenant_id_field.clone()],
			Vec::new(),
			Vec::new(),
		);
		let composite_pk = ConstraintDefinition {
			name: "billing_invoice_pkey".to_string(),
			constraint_type: "primary_key".to_string(),
			fields: vec!["id".to_string(), "tenant_id".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let to_model = build_model_state(
			"billing",
			"Invoice",
			vec![id_field, tenant_id_field],
			Vec::new(),
			vec![composite_pk],
		);

		let from_state = build_project_state(vec![(
			("billing".to_string(), "Invoice".to_string()),
			from_model,
		)]);
		let to_state = build_project_state(vec![(
			("billing".to_string(), "Invoice".to_string()),
			to_model,
		)]);
		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert_eq!(operations.len(), 1);
		assert!(
			matches!(
				&operations[0],
				super::super::Operation::CreateCompositePrimaryKey {
					table,
					columns,
					..
				} if table == "billing_invoice"
					&& columns == &["id".to_string(), "tenant_id".to_string()]
			),
			"expected CreateCompositePrimaryKey, got: {:?}",
			operations
		);
	}

	#[rstest]
	fn detect_composite_pk_unchanged_emits_no_operations() {
		// Arrange — same composite PK in both states should produce no operations
		let composite_pk = ConstraintDefinition {
			name: "billing_invoice_pkey".to_string(),
			constraint_type: "primary_key".to_string(),
			fields: vec!["id".to_string(), "tenant_id".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let from_model = build_model_state(
			"billing",
			"Invoice",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				FieldState::new("tenant_id", super::super::FieldType::Integer, false),
			],
			Vec::new(),
			vec![composite_pk.clone()],
		);
		let to_model = build_model_state(
			"billing",
			"Invoice",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				FieldState::new("tenant_id", super::super::FieldType::Integer, false),
			],
			Vec::new(),
			vec![composite_pk],
		);

		let from_state = build_project_state(vec![(
			("billing".to_string(), "Invoice".to_string()),
			from_model,
		)]);
		let to_state = build_project_state(vec![(
			("billing".to_string(), "Invoice".to_string()),
			to_model,
		)]);
		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert!(
			operations.is_empty(),
			"unchanged composite PK should produce no operations, got: {:?}",
			operations
		);
	}

	#[rstest]
	fn detect_composite_pk_changed_fields_emits_drop_and_create() {
		// Arrange — same constraint name but different field set
		let composite_pk_from = ConstraintDefinition {
			name: "billing_invoice_pkey".to_string(),
			constraint_type: "primary_key".to_string(),
			fields: vec!["id".to_string(), "tenant_id".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let composite_pk_to = ConstraintDefinition {
			name: "billing_invoice_pkey".to_string(),
			constraint_type: "primary_key".to_string(),
			fields: vec!["id".to_string(), "org_id".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let from_model = build_model_state(
			"billing",
			"Invoice",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				FieldState::new("tenant_id", super::super::FieldType::Integer, false),
			],
			Vec::new(),
			vec![composite_pk_from],
		);
		let to_model = build_model_state(
			"billing",
			"Invoice",
			vec![
				FieldState::new("id", super::super::FieldType::Integer, false),
				FieldState::new("org_id", super::super::FieldType::Integer, false),
			],
			Vec::new(),
			vec![composite_pk_to],
		);
		let from_state = build_project_state(vec![(
			("billing".to_string(), "Invoice".to_string()),
			from_model,
		)]);
		let to_state = build_project_state(vec![(
			("billing".to_string(), "Invoice".to_string()),
			to_model,
		)]);
		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let operations = detector.generate_operations();

		// Assert — expect DropConstraint followed by CreateCompositePrimaryKey
		let drop_op = operations.iter().find(|op| {
			matches!(op, super::super::Operation::DropConstraintDefinition {
				constraint: super::super::Constraint::PrimaryKey { name, columns }, ..
			} if name == "billing_invoice_pkey"
				&& columns == &["id".to_string(), "tenant_id".to_string()])
		});
		let create_op = operations.iter().find(|op| {
			matches!(op, super::super::Operation::CreateCompositePrimaryKey { columns, .. }
				if columns == &["id".to_string(), "org_id".to_string()])
		});
		assert!(
			drop_op.is_some(),
			"expected typed DropConstraintDefinition for modified composite PK, got: {:?}",
			operations
		);
		assert!(
			create_op.is_some(),
			"expected CreateCompositePrimaryKey with new fields, got: {:?}",
			operations
		);
	}

	#[rstest]
	fn detect_sequence_reset_emits_set_auto_increment_value() {
		// Arrange
		let mut id_field = FieldState::new("id", super::super::FieldType::BigInteger, false);
		id_field
			.params
			.insert("auto_increment".to_string(), "true".to_string());

		let from_model = build_model_state(
			"shop",
			"Order",
			vec![id_field.clone()],
			Vec::new(),
			Vec::new(),
		);
		let mut to_model =
			build_model_state("shop", "Order", vec![id_field], Vec::new(), Vec::new());
		to_model
			.options
			.insert("sequence_reset".to_string(), "1000".to_string());

		let from_state = build_project_state(vec![(
			("shop".to_string(), "Order".to_string()),
			from_model,
		)]);
		let to_state =
			build_project_state(vec![(("shop".to_string(), "Order".to_string()), to_model)]);
		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert_eq!(operations.len(), 1);
		assert!(
			matches!(
				&operations[0],
				super::super::Operation::SetAutoIncrementValue {
					table,
					column,
					value,
				} if table == "shop_order" && column == "id" && *value == 1000
			),
			"expected SetAutoIncrementValue, got: {:?}",
			operations
		);
	}

	#[rstest]
	fn detect_added_unique_together_emits_add_constraint() {
		// Arrange — same model in both states, but to_state adds a UNIQUE
		// constraint over (organization_id, name). This mirrors the
		// `unique_together = ("organization_id", "name")` macro form.
		let id_field = FieldState::new("id", super::super::FieldType::Integer, false);
		let org_field = FieldState::new("organization_id", super::super::FieldType::Integer, false);
		let name_field = FieldState::new("name", super::super::FieldType::VarChar(255), false);

		let from_model = build_model_state(
			"clusters",
			"Cluster",
			vec![id_field.clone(), org_field.clone(), name_field.clone()],
			Vec::new(),
			Vec::new(),
		);
		let unique_constraint = ConstraintDefinition {
			name: "clusters_cluster_organization_id_name_uniq".to_string(),
			constraint_type: "unique".to_string(),
			fields: vec!["organization_id".to_string(), "name".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let to_model = build_model_state(
			"clusters",
			"Cluster",
			vec![id_field, org_field, name_field],
			Vec::new(),
			vec![unique_constraint],
		);

		let from_state = build_project_state(vec![(
			("clusters".to_string(), "Cluster".to_string()),
			from_model,
		)]);
		let to_state = build_project_state(vec![(
			("clusters".to_string(), "Cluster".to_string()),
			to_model,
		)]);
		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let operations = detector.generate_operations();

		// Assert — exactly one AddConstraint targeting the cluster table
		// with SQL referencing both columns of the composite UNIQUE.
		assert_eq!(
			operations.len(),
			1,
			"expected exactly one AddConstraint operation, got: {:?}",
			operations
		);
		let super::super::Operation::AddConstraint {
			table,
			constraint_sql,
		} = &operations[0]
		else {
			panic!(
				"expected Operation::AddConstraint, got: {:?}",
				operations[0]
			);
		};
		assert_eq!(table, "clusters_cluster");
		assert!(
			constraint_sql.contains("UNIQUE"),
			"constraint SQL should declare UNIQUE, got: {}",
			constraint_sql
		);
		assert!(
			constraint_sql.contains("organization_id"),
			"constraint SQL should reference organization_id, got: {}",
			constraint_sql
		);
		assert!(
			constraint_sql.contains("name"),
			"constraint SQL should reference name, got: {}",
			constraint_sql
		);
		assert!(
			constraint_sql.contains("clusters_cluster_organization_id_name_uniq"),
			"constraint SQL should carry the constraint name, got: {}",
			constraint_sql
		);
	}

	#[rstest]
	fn detect_removed_unique_together_emits_drop_constraint() {
		// Arrange — symmetric reverse: from_state has the UNIQUE, to_state
		// drops it. The autodetector must emit a DropConstraint so the DB
		// is brought back in sync.
		let id_field = FieldState::new("id", super::super::FieldType::Integer, false);
		let org_field = FieldState::new("organization_id", super::super::FieldType::Integer, false);
		let name_field = FieldState::new("name", super::super::FieldType::VarChar(255), false);

		let unique_constraint = ConstraintDefinition {
			name: "clusters_cluster_organization_id_name_uniq".to_string(),
			constraint_type: "unique".to_string(),
			fields: vec!["organization_id".to_string(), "name".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let from_model = build_model_state(
			"clusters",
			"Cluster",
			vec![id_field.clone(), org_field.clone(), name_field.clone()],
			Vec::new(),
			vec![unique_constraint],
		);
		let to_model = build_model_state(
			"clusters",
			"Cluster",
			vec![id_field, org_field, name_field],
			Vec::new(),
			Vec::new(),
		);

		let from_state = build_project_state(vec![(
			("clusters".to_string(), "Cluster".to_string()),
			from_model,
		)]);
		let to_state = build_project_state(vec![(
			("clusters".to_string(), "Cluster".to_string()),
			to_model,
		)]);
		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert_eq!(
			operations.len(),
			1,
			"expected exactly one DropConstraint operation, got: {:?}",
			operations
		);
		let super::super::Operation::DropConstraintDefinition {
			table,
			constraint: super::super::Constraint::Unique { name, .. },
		} = &operations[0]
		else {
			panic!(
				"expected Operation::DropConstraintDefinition, got: {:?}",
				operations[0]
			);
		};
		assert_eq!(table, "clusters_cluster");
		assert_eq!(name, "clusters_cluster_organization_id_name_uniq");
	}

	#[rstest]
	fn detect_added_unique_together_via_offline_reconstructed_from_state() {
		// Arrange — regression for issue #4032.
		//
		// When `makemigrations` falls back to file-based state reconstruction
		// (no DB available), `from_state` is rebuilt from migration
		// `Operation::CreateTable` entries and keyed by the PascalCase form
		// of the table name (e.g. table `"clusters"` -> key `"Clusters"`),
		// while `to_state` is keyed by the registered struct name
		// (e.g. `"Cluster"`). Both share the same `table_name`.
		//
		// Constraint diffing must locate the corresponding model by
		// `table_name` rather than by struct-name key, otherwise added
		// `unique_together` constraints are silently dropped.
		let id_field = FieldState::new("id", super::super::FieldType::Integer, false);
		let org_field = FieldState::new("organization_id", super::super::FieldType::Integer, false);
		let name_field = FieldState::new("name", super::super::FieldType::VarChar(255), false);

		// from_state: keyed by table-derived name "Clusters", no constraints.
		let mut from_model = build_model_state(
			"clusters",
			"Clusters",
			vec![id_field.clone(), org_field.clone(), name_field.clone()],
			Vec::new(),
			Vec::new(),
		);
		from_model.table_name = "clusters_cluster".to_string();

		// to_state: keyed by struct name "Cluster", carries the unique constraint.
		let unique_constraint = ConstraintDefinition {
			name: "clusters_cluster_organization_id_name_uniq".to_string(),
			constraint_type: "unique".to_string(),
			fields: vec!["organization_id".to_string(), "name".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let to_model = build_model_state(
			"clusters",
			"Cluster",
			vec![id_field, org_field, name_field],
			Vec::new(),
			vec![unique_constraint],
		);

		let from_state = build_project_state(vec![(
			("clusters".to_string(), "Clusters".to_string()),
			from_model,
		)]);
		let to_state = build_project_state(vec![(
			("clusters".to_string(), "Cluster".to_string()),
			to_model,
		)]);
		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let operations = detector.generate_operations();

		// Assert — exactly one AddConstraint, targeted at the shared table.
		// No spurious operations (in particular no AlterColumn/RenameModel)
		// must leak through from the model-name mismatch.
		assert_eq!(
			operations.len(),
			1,
			"expected exactly one AddConstraint operation, got: {:?}",
			operations
		);
		let super::super::Operation::AddConstraint {
			table,
			constraint_sql,
		} = &operations[0]
		else {
			panic!(
				"expected Operation::AddConstraint, got: {:?}",
				operations[0]
			);
		};
		assert_eq!(table, "clusters_cluster");
		assert!(
			constraint_sql.contains("clusters_cluster_organization_id_name_uniq"),
			"constraint SQL should carry the constraint name, got: {}",
			constraint_sql
		);
	}

	#[rstest]
	fn detect_removed_unique_together_via_offline_reconstructed_from_state() {
		// Arrange — symmetric regression for issue #4032 covering the
		// removal direction: offline-reconstructed `from_state` retains a
		// `unique_together` constraint, and the registered model in
		// `to_state` no longer declares it. The diff must emit a
		// `DropConstraint` despite the model-name key mismatch.
		let id_field = FieldState::new("id", super::super::FieldType::Integer, false);
		let org_field = FieldState::new("organization_id", super::super::FieldType::Integer, false);
		let name_field = FieldState::new("name", super::super::FieldType::VarChar(255), false);

		let unique_constraint = ConstraintDefinition {
			name: "clusters_cluster_organization_id_name_uniq".to_string(),
			constraint_type: "unique".to_string(),
			fields: vec!["organization_id".to_string(), "name".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let mut from_model = build_model_state(
			"clusters",
			"Clusters",
			vec![id_field.clone(), org_field.clone(), name_field.clone()],
			Vec::new(),
			vec![unique_constraint],
		);
		from_model.table_name = "clusters_cluster".to_string();

		let to_model = build_model_state(
			"clusters",
			"Cluster",
			vec![id_field, org_field, name_field],
			Vec::new(),
			Vec::new(),
		);

		let from_state = build_project_state(vec![(
			("clusters".to_string(), "Clusters".to_string()),
			from_model,
		)]);
		let to_state = build_project_state(vec![(
			("clusters".to_string(), "Cluster".to_string()),
			to_model,
		)]);
		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert_eq!(
			operations.len(),
			1,
			"expected exactly one DropConstraint operation, got: {:?}",
			operations
		);
		let super::super::Operation::DropConstraintDefinition {
			table,
			constraint: super::super::Constraint::Unique { name, .. },
		} = &operations[0]
		else {
			panic!(
				"expected Operation::DropConstraintDefinition, got: {:?}",
				operations[0]
			);
		};
		assert_eq!(table, "clusters_cluster");
		assert_eq!(name, "clusters_cluster_organization_id_name_uniq");
	}

	#[rstest]
	fn has_field_changed_ignores_param_population_skew() {
		// Arrange — regression for issue #4049.
		//
		// `from_state` is rebuilt from migration files via
		// `column_def_to_field_state`, which only inserts schema-affecting
		// params (`primary_key`, `auto_increment`, `unique`, `default`) when
		// their value is true/Some. `to_state` is rebuilt from the macro
		// registry, which inserts explicit "true"/"false" strings for
		// `not_null`, `null`, etc. on every field.
		//
		// The two `FieldState` HashMaps are therefore asymmetric even when
		// the underlying schema is identical, and `has_field_changed` must
		// not treat that asymmetry as a real change.
		let mut from_params = std::collections::HashMap::new();
		from_params.insert("primary_key".to_string(), "true".to_string());
		from_params.insert("auto_increment".to_string(), "true".to_string());
		let from_field = FieldState {
			name: "id".to_string(),
			field_type: super::super::FieldType::BigInteger,
			nullable: false,
			params: from_params,
			generated: None,
			domain: None,
			foreign_key: None,
		};

		// to_field carries the macro-registry-style asymmetric params. In
		// particular, schema-affecting keys like `unique` and `default` may be
		// populated explicitly with their false/empty value on the to side
		// while the migration-replay from side simply omits the key. The raw
		// HashMap comparison previously surfaced this as a difference; the
		// canonical ColumnDefinition collapses None and "false" to the same
		// `false`, so the field is correctly seen as unchanged.
		let mut to_params = std::collections::HashMap::new();
		to_params.insert("primary_key".to_string(), "true".to_string());
		to_params.insert("auto_increment".to_string(), "true".to_string());
		to_params.insert("not_null".to_string(), "true".to_string());
		to_params.insert("null".to_string(), "false".to_string());
		to_params.insert("unique".to_string(), "false".to_string());
		let to_field = FieldState {
			name: "id".to_string(),
			field_type: super::super::FieldType::BigInteger,
			nullable: false,
			params: to_params,
			generated: None,
			domain: None,
			foreign_key: None,
		};

		let detector = MigrationAutodetector::new(ProjectState::new(), ProjectState::new());

		// Act
		let changed =
			detector.has_field_changed_with_unique("id", &from_field, &to_field, None, None);

		// Assert — schema is identical (BigInteger PK NOT NULL auto_increment,
		// not unique). Asymmetric param maps must not surface as a change.
		assert!(
			!changed,
			"identical schema with asymmetric param populations between migration replay and macro registry must not be detected as changed"
		);
	}

	#[rstest]
	fn generate_operations_no_spurious_altercolumn_for_pk_via_offline_reconstructed_state() {
		// Arrange — regression for issue #4049.
		//
		// When `makemigrations` runs offline (no live DB), `from_state` is
		// reconstructed from migration files and keyed by the PascalCase form
		// of the table name (e.g. table `"clusters"` -> key `"Clusters"`),
		// while `to_state` is keyed by the registered struct name (`"Cluster"`).
		// On top of that, the two states populate `FieldState.params`
		// asymmetrically: migration replay only inserts schema-affecting params
		// when their value is true/Some, whereas the macro registry inserts
		// explicit "true"/"false" strings for `not_null`, `null`, etc.
		//
		// The diff must NOT emit a no-op `Operation::AlterColumn` for the
		// unchanged `id` primary key, even though the params HashMap differs.
		let mut from_id_params = std::collections::HashMap::new();
		from_id_params.insert("primary_key".to_string(), "true".to_string());
		from_id_params.insert("auto_increment".to_string(), "true".to_string());
		let from_id_field = FieldState {
			name: "id".to_string(),
			field_type: super::super::FieldType::BigInteger,
			nullable: false,
			params: from_id_params,
			generated: None,
			domain: None,
			foreign_key: None,
		};
		let org_field = FieldState::new("organization_id", super::super::FieldType::Integer, false);
		let name_field = FieldState::new("name", super::super::FieldType::VarChar(255), false);

		// from_state: keyed by table-derived name "Clusters", no constraints,
		// migration-replay-style sparse params on the PK column.
		let mut from_model = build_model_state(
			"clusters",
			"Clusters",
			vec![from_id_field, org_field.clone(), name_field.clone()],
			Vec::new(),
			Vec::new(),
		);
		from_model.table_name = "clusters_cluster".to_string();

		// to_state: keyed by struct name "Cluster", carries an added
		// unique_together constraint, macro-registry-style dense params on the
		// PK column (`not_null`, `null`, `unique` explicitly populated even
		// when their value is the default false). The from side omits these
		// keys entirely, which previously surfaced as a fictitious
		// difference in `has_field_changed`'s raw HashMap comparison.
		let mut to_id_params = std::collections::HashMap::new();
		to_id_params.insert("primary_key".to_string(), "true".to_string());
		to_id_params.insert("auto_increment".to_string(), "true".to_string());
		to_id_params.insert("not_null".to_string(), "true".to_string());
		to_id_params.insert("null".to_string(), "false".to_string());
		to_id_params.insert("unique".to_string(), "false".to_string());
		let to_id_field = FieldState {
			name: "id".to_string(),
			field_type: super::super::FieldType::BigInteger,
			nullable: false,
			params: to_id_params,
			generated: None,
			domain: None,
			foreign_key: None,
		};
		let unique_constraint = ConstraintDefinition {
			name: "clusters_cluster_organization_id_name_uniq".to_string(),
			constraint_type: "unique".to_string(),
			fields: vec!["organization_id".to_string(), "name".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let to_model = build_model_state(
			"clusters",
			"Cluster",
			vec![to_id_field, org_field, name_field],
			Vec::new(),
			vec![unique_constraint],
		);

		let from_state = build_project_state(vec![(
			("clusters".to_string(), "Clusters".to_string()),
			from_model,
		)]);
		let to_state = build_project_state(vec![(
			("clusters".to_string(), "Cluster".to_string()),
			to_model,
		)]);
		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let operations = detector.generate_operations();

		// Assert — exactly one AddConstraint and no spurious AlterColumn for
		// the unchanged PK. The asymmetric param populations must collapse to
		// the same canonical `ColumnDefinition` and never surface as a diff.
		assert!(
			!operations
				.iter()
				.any(|op| matches!(op, super::super::Operation::AlterColumn { .. })),
			"no AlterColumn must be emitted for unchanged PK under offline state reconstruction, got: {:?}",
			operations
		);
		assert_eq!(
			operations.len(),
			1,
			"expected exactly one AddConstraint operation, got: {:?}",
			operations
		);
		assert!(
			matches!(
				&operations[0],
				super::super::Operation::AddConstraint { .. }
			),
			"expected the single operation to be AddConstraint, got: {:?}",
			operations[0]
		);
	}

	#[rstest]
	fn generate_operations_no_spurious_altercolumn_for_option_pk_via_apply_migration_operations() {
		// Arrange — regression for issue #4052 (residual after #4050).
		//
		// Reproduces the production CLI path that #4050's regression test
		// missed: from_state is built by feeding a synthetic
		// `Operation::CreateTable` (modeled on `0001_initial.rs`) through
		// `ProjectState::apply_migration_operations`, so its `id` FieldState
		// flows through `column_def_to_field_state`. to_state mirrors the
		// `#[model]` macro's output for `id: Option<i64>` with
		// `#[field(primary_key = true)]` AFTER the macro fix that suppresses
		// `null = "true"` for primary keys (the Option<T> wrapper for PKs
		// reflects "id is None until DB assigns it on insert", not DB-level
		// nullability).
		//
		// Pre-fix, the macro emitted `null = "true"` for any Option<T>
		// field, including PKs. `to_model_state` then set
		// `FieldState.nullable = true` while `column_def_to_field_state`
		// produced `nullable = false`. `has_field_changed`'s direct
		// `nullable != nullable` short-circuit (added by #4050 to keep the
		// authoritative NOT NULL bit on the canonical comparison path)
		// returned true before the canonical `ColumnDefinition::from_field_state`
		// folding could absorb the asymmetry, surfacing as a no-op
		// `Operation::AlterColumn { old_definition: None, .. }` for the
		// unchanged PK.

		// Build to_state via the model registry layer that the macro feeds.
		// Mirror the FIXED macro params for `id: Option<i64>` with
		// `#[field(primary_key = true)]`: `null = "false"` (forced by the
		// fix), `not_null = "true"`, `primary_key = "true"`,
		// `auto_increment = "true"`. The dense param population is exactly
		// what `ModelMetadata::to_model_state` consumes.
		let mut id_meta =
			super::super::model_registry::FieldMetadata::new(super::super::FieldType::BigInteger);
		id_meta = id_meta
			.with_param("primary_key", "true")
			.with_param("auto_increment", "true")
			.with_param("not_null", "true")
			.with_nullable(false);
		let mut name_meta =
			super::super::model_registry::FieldMetadata::new(super::super::FieldType::VarChar(255));
		name_meta = name_meta
			.with_param("max_length", "255")
			.with_param("not_null", "true")
			.with_nullable(false);

		let mut metadata =
			super::super::model_registry::ModelMetadata::new("clusters", "Cluster", "clusters");
		metadata.add_field("id".to_string(), id_meta);
		metadata.add_field("name".to_string(), name_meta);

		let to_model = metadata.to_model_state();
		// Sanity: the FIXED macro contract must fold `null = "false"` into
		// `FieldState.nullable = false` for the PK, matching the migration
		// replay side.
		let to_id = to_model.fields.get("id").expect("id field present");
		assert!(
			!to_id.nullable,
			"to_state PK FieldState.nullable must be false; got nullable=true \
			 with params={:?}. Did the #[model] macro regress to emitting \
			 null=\"true\" for Option<T> PKs?",
			to_id.params
		);

		let to_state = build_project_state(vec![(
			("clusters".to_string(), "Cluster".to_string()),
			to_model,
		)]);

		// Build from_state via the production CLI path: feed a CreateTable
		// (modeled on 0001_initial.rs) through apply_migration_operations.
		// This populates from_state via column_def_to_field_state, which
		// derives nullability from `not_null` (sparse params).
		let create_clusters = super::super::Operation::CreateTable {
			name: "clusters".to_string(),
			columns: vec![
				super::super::ColumnDefinition {
					name: "id".to_string(),
					type_definition: super::super::FieldType::BigInteger,
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: true,
					default: None,
					generated: None,
					domain: None,
				},
				super::super::ColumnDefinition {
					name: "name".to_string(),
					type_definition: super::super::FieldType::VarChar(255),
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
					generated: None,
					domain: None,
				},
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		};
		let mut from_state = ProjectState::new();
		from_state.apply_migration_operations(&[create_clusters], "clusters");

		// Sanity: the migration-replay path must produce nullable=false for
		// the PK.
		let from_clusters = from_state
			.find_model_by_table("clusters")
			.expect("clusters model present in from_state");
		assert!(
			!from_clusters
				.fields
				.get("id")
				.expect("id field in from_state")
				.nullable,
			"from_state PK FieldState.nullable must be false (column_def_to_field_state derives \
			 from not_null); got nullable=true"
		);

		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act — call BOTH lower-level generate_operations() and the CLI
		// entry generate_migrations(), since #4052's reproducer asserts
		// against both.
		let direct_ops = detector.generate_operations();
		let migrations = detector.generate_migrations();
		let migration_ops: Vec<&super::super::Operation> = migrations
			.iter()
			.flat_map(|m| m.operations.iter())
			.collect();

		// Assert — neither path may emit AlterColumn for the unchanged `id`
		// PK. Pre-fix, both emitted exactly such an AlterColumn.
		assert!(
			!direct_ops.iter().any(|op| matches!(
				op,
				super::super::Operation::AlterColumn { column, .. } if column == "id"
			)),
			"generate_operations() emitted spurious AlterColumn for unchanged `id` PK \
			 under apply_migration_operations from_state. ops={:?}",
			direct_ops
		);
		assert!(
			!migration_ops.iter().any(|op| matches!(
				op,
				super::super::Operation::AlterColumn { column, .. } if column == "id"
			)),
			"generate_migrations() emitted spurious AlterColumn for unchanged `id` PK \
			 under apply_migration_operations from_state. ops={:?}",
			migration_ops
		);
	}

	#[rstest]
	fn generate_operations_no_spurious_altercolumn_for_replayed_foreign_key_column() {
		// Arrange — regression for the basis tutorial migration check.
		//
		// The replayed migration state only knows the physical `_id` column
		// type (`BigInteger`). The macro registry state keeps the
		// `ForeignKeyField<T>` placeholder type (`Uuid`) plus `fk_target`
		// params, and `ColumnDefinition::from_field_state` resolves that
		// placeholder to the referenced model's PK type. The autodetector must
		// compare the resolved column definitions, not the raw logical
		// `FieldState.field_type`, or it emits a no-op AlterColumn.
		let mut target_metadata = super::super::model_registry::ModelMetadata::new(
			"fk_drift_target_app",
			"FkDriftTarget",
			"fk_drift_targets",
		);
		target_metadata.add_field(
			"id".to_string(),
			super::super::model_registry::FieldMetadata::new(super::super::FieldType::BigInteger)
				.with_param("primary_key", "true")
				.with_param("auto_increment", "true")
				.with_param("not_null", "true")
				.with_nullable(false),
		);
		super::super::model_registry::global_registry().register_model(target_metadata);

		let create_sources = super::super::Operation::CreateTable {
			name: "fk_drift_sources".to_string(),
			columns: vec![
				super::super::ColumnDefinition {
					name: "id".to_string(),
					type_definition: super::super::FieldType::BigInteger,
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: true,
					default: None,
					generated: None,
					domain: None,
				},
				super::super::ColumnDefinition {
					name: "target_id".to_string(),
					type_definition: super::super::FieldType::BigInteger,
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
					generated: None,
					domain: None,
				},
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		};
		let mut from_state = ProjectState::new();
		from_state.apply_migration_operations(&[create_sources], "fk_drift_source_app");

		let mut source_metadata = super::super::model_registry::ModelMetadata::new(
			"fk_drift_source_app",
			"FkDriftSource",
			"fk_drift_sources",
		);
		source_metadata.add_field(
			"id".to_string(),
			super::super::model_registry::FieldMetadata::new(super::super::FieldType::BigInteger)
				.with_param("primary_key", "true")
				.with_param("auto_increment", "true")
				.with_param("not_null", "true")
				.with_nullable(false),
		);
		source_metadata.add_field(
			"target_id".to_string(),
			super::super::model_registry::FieldMetadata::new(super::super::FieldType::Uuid)
				.with_param("fk_target", "FkDriftTarget")
				.with_param("fk_target_app", "fk_drift_target_app")
				.with_param("not_null", "true")
				.with_nullable(false),
		);
		let to_state = build_project_state(vec![(
			(
				"fk_drift_source_app".to_string(),
				"FkDriftSource".to_string(),
			),
			source_metadata.to_model_state(),
		)]);
		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert!(
			!operations.iter().any(|op| matches!(
				op,
				super::super::Operation::AlterColumn { column, .. } if column == "target_id"
			)),
			"unchanged FK _id column must not emit no-op AlterColumn, got: {:?}",
			operations
		);
		assert!(
			operations.is_empty(),
			"replayed FK column should be in sync with registry state, got: {:?}",
			operations
		);
	}

	#[rstest]
	fn generate_operations_no_spurious_drift_for_replayed_auth_schema() {
		// Arrange - regression for issue #5367.
		//
		// The file-based replay path reconstructs old migrations from
		// `ColumnDefinition` values. That state can differ from today's
		// registry state even when the applied schema is equivalent:
		// - old UUID PK migrations may carry `auto_increment = true`;
		// - field-level UNIQUE may have been added through `AddConstraint`;
		// - DB defaults must round-trip through both replayed migrations and
		//   registry metadata.
		let create_auth_users = super::super::Operation::CreateTable {
			name: "auth_users".to_string(),
			columns: vec![
				super::super::ColumnDefinition {
					name: "id".to_string(),
					type_definition: super::super::FieldType::Uuid,
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: false,
					default: None,
					generated: None,
					domain: None,
				},
				super::super::ColumnDefinition {
					name: "username".to_string(),
					type_definition: super::super::FieldType::VarChar(150),
					not_null: true,
					unique: true,
					primary_key: false,
					auto_increment: false,
					default: None,
					generated: None,
					domain: None,
				},
				super::super::ColumnDefinition {
					name: "email".to_string(),
					type_definition: super::super::FieldType::VarChar(254),
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
					generated: None,
					domain: None,
				},
				super::super::ColumnDefinition {
					name: "first_name".to_string(),
					type_definition: super::super::FieldType::VarChar(150),
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: Some("''".to_string()),
					generated: None,
					domain: None,
				},
				super::super::ColumnDefinition {
					name: "last_name".to_string(),
					type_definition: super::super::FieldType::VarChar(150),
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: Some("''".to_string()),
					generated: None,
					domain: None,
				},
				super::super::ColumnDefinition {
					name: "is_active".to_string(),
					type_definition: super::super::FieldType::Boolean,
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: Some("true".to_string()),
					generated: None,
					domain: None,
				},
				super::super::ColumnDefinition {
					name: "is_staff".to_string(),
					type_definition: super::super::FieldType::Boolean,
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: Some("false".to_string()),
					generated: None,
					domain: None,
				},
				super::super::ColumnDefinition {
					name: "is_superuser".to_string(),
					type_definition: super::super::FieldType::Boolean,
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: Some("false".to_string()),
					generated: None,
					domain: None,
				},
			],
			constraints: vec![super::super::operations::Constraint::Unique {
				name: "auth_user_username_uniq".to_string(),
				columns: vec!["username".to_string()],
			}],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		};
		let add_email_unique = super::super::Operation::AddConstraint {
			table: "auth_users".to_string(),
			constraint_sql: "CONSTRAINT auth_user_email_uniq UNIQUE (email)".to_string(),
		};
		let create_auth_permission = super::super::Operation::CreateTable {
			name: "auth_permission".to_string(),
			columns: vec![
				super::super::ColumnDefinition {
					name: "id".to_string(),
					type_definition: super::super::FieldType::Uuid,
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: true,
					default: None,
					generated: None,
					domain: None,
				},
				super::super::ColumnDefinition {
					name: "name".to_string(),
					type_definition: super::super::FieldType::VarChar(255),
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
					generated: None,
					domain: None,
				},
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		};

		let mut from_state = ProjectState::new();
		from_state.apply_migration_operations(
			&[create_auth_users, add_email_unique, create_auth_permission],
			"auth",
		);

		let mut user_metadata =
			super::super::model_registry::ModelMetadata::new("auth", "User", "auth_users");
		user_metadata.add_field(
			"id".to_string(),
			super::super::model_registry::FieldMetadata::new(super::super::FieldType::Uuid)
				.with_param("primary_key", "true")
				.with_param("not_null", "true")
				.with_nullable(false),
		);
		user_metadata.add_field(
			"username".to_string(),
			super::super::model_registry::FieldMetadata::new(super::super::FieldType::VarChar(150))
				.with_param("max_length", "150")
				.with_param("unique", "true")
				.with_param("not_null", "true")
				.with_nullable(false),
		);
		user_metadata.add_field(
			"email".to_string(),
			super::super::model_registry::FieldMetadata::new(super::super::FieldType::VarChar(254))
				.with_param("max_length", "254")
				.with_param("unique", "true")
				.with_param("not_null", "true")
				.with_nullable(false),
		);
		user_metadata.add_field(
			"first_name".to_string(),
			super::super::model_registry::FieldMetadata::new(super::super::FieldType::VarChar(150))
				.with_param("max_length", "150")
				.with_param("default", "''")
				.with_param("not_null", "true")
				.with_nullable(false),
		);
		user_metadata.add_field(
			"last_name".to_string(),
			super::super::model_registry::FieldMetadata::new(super::super::FieldType::VarChar(150))
				.with_param("max_length", "150")
				.with_param("default", "''")
				.with_param("not_null", "true")
				.with_nullable(false),
		);
		user_metadata.add_field(
			"is_active".to_string(),
			super::super::model_registry::FieldMetadata::new(super::super::FieldType::Boolean)
				.with_param("default", "true")
				.with_param("not_null", "true")
				.with_nullable(false),
		);
		user_metadata.add_field(
			"is_staff".to_string(),
			super::super::model_registry::FieldMetadata::new(super::super::FieldType::Boolean)
				.with_param("default", "false")
				.with_param("not_null", "true")
				.with_nullable(false),
		);
		user_metadata.add_field(
			"is_superuser".to_string(),
			super::super::model_registry::FieldMetadata::new(super::super::FieldType::Boolean)
				.with_param("default", "false")
				.with_param("not_null", "true")
				.with_nullable(false),
		);

		let mut permission_metadata = super::super::model_registry::ModelMetadata::new(
			"auth",
			"AuthPermission",
			"auth_permission",
		);
		permission_metadata.add_field(
			"id".to_string(),
			super::super::model_registry::FieldMetadata::new(super::super::FieldType::Uuid)
				.with_param("primary_key", "true")
				.with_param("not_null", "true")
				.with_nullable(false),
		);
		permission_metadata.add_field(
			"name".to_string(),
			super::super::model_registry::FieldMetadata::new(super::super::FieldType::VarChar(255))
				.with_param("max_length", "255")
				.with_param("not_null", "true")
				.with_nullable(false),
		);

		let to_state = build_project_state(vec![
			(
				("auth".to_string(), "User".to_string()),
				user_metadata.to_model_state(),
			),
			(
				("auth".to_string(), "AuthPermission".to_string()),
				permission_metadata.to_model_state(),
			),
		]);
		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert!(
			operations.is_empty(),
			"replayed auth schema should be in sync with registry state, got: {:?}",
			operations
		);
	}

	#[rstest]
	fn generate_migrations_emits_add_constraint_for_added_unique_together() {
		// Arrange — regression for issue #4040.
		//
		// `generate_migrations()` is the entry point used by the
		// `makemigrations` CLI, in contrast to `generate_operations()`
		// which is used by tests / direct callers. PR #3998 added
		// `added_constraints` handling to `generate_operations()` but the
		// symmetric loop was missing from `generate_migrations()`, so the
		// CLI silently dropped `Operation::AddConstraint` for non-PK
		// constraints (e.g. `unique_together`) added to existing models.
		//
		// This test exercises the CLI path directly and asserts the
		// migration carries an `Operation::AddConstraint`.
		let id_field = FieldState::new("id", super::super::FieldType::Integer, false);
		let org_field = FieldState::new("organization_id", super::super::FieldType::Integer, false);
		let name_field = FieldState::new("name", super::super::FieldType::VarChar(255), false);

		// from_state mirrors the CLI's offline-reconstructed state with no
		// constraints declared on the existing model (matching what the
		// `0001_initial` migration produces before the constraint is added).
		let mut from_model = build_model_state(
			"clusters",
			"Cluster",
			vec![id_field.clone(), org_field.clone(), name_field.clone()],
			Vec::new(),
			Vec::new(),
		);
		from_model.table_name = "clusters_cluster".to_string();

		// to_state carries the unique_together constraint declared via the
		// macro on the registered struct.
		let unique_constraint = ConstraintDefinition {
			name: "clusters_cluster_organization_id_name_uniq".to_string(),
			constraint_type: "unique".to_string(),
			fields: vec!["organization_id".to_string(), "name".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let mut to_model = build_model_state(
			"clusters",
			"Cluster",
			vec![id_field, org_field, name_field],
			Vec::new(),
			vec![unique_constraint],
		);
		to_model.table_name = "clusters_cluster".to_string();

		let from_state = build_project_state(vec![(
			("clusters".to_string(), "Cluster".to_string()),
			from_model,
		)]);
		let to_state = build_project_state(vec![(
			("clusters".to_string(), "Cluster".to_string()),
			to_model,
		)]);
		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let migrations = detector.generate_migrations();

		// Assert — exactly one Migration for app "clusters" with exactly
		// one AddConstraint operation, targeted at the shared table.
		assert_eq!(
			migrations.len(),
			1,
			"expected exactly one Migration, got: {:?}",
			migrations
		);
		assert_eq!(migrations[0].app_label, "clusters");
		assert_eq!(
			migrations[0].operations.len(),
			1,
			"expected exactly one operation in the migration, got: {:?}",
			migrations[0].operations
		);
		let super::super::Operation::AddConstraint {
			table,
			constraint_sql,
		} = &migrations[0].operations[0]
		else {
			panic!(
				"expected Operation::AddConstraint, got: {:?}",
				migrations[0].operations[0]
			);
		};
		assert_eq!(table, "clusters_cluster");
		assert!(
			constraint_sql.contains("clusters_cluster_organization_id_name_uniq"),
			"constraint SQL should carry the constraint name, got: {}",
			constraint_sql
		);
	}

	#[rstest]
	fn generate_migrations_emits_drop_constraint_for_removed_unique_together() {
		// Arrange — symmetric regression for issue #4040 covering the
		// removal direction: the existing model declares a
		// `unique_together` constraint, the new struct has dropped it, and
		// the CLI path must emit `Operation::DropConstraint`.
		let id_field = FieldState::new("id", super::super::FieldType::Integer, false);
		let org_field = FieldState::new("organization_id", super::super::FieldType::Integer, false);
		let name_field = FieldState::new("name", super::super::FieldType::VarChar(255), false);

		let unique_constraint = ConstraintDefinition {
			name: "clusters_cluster_organization_id_name_uniq".to_string(),
			constraint_type: "unique".to_string(),
			fields: vec!["organization_id".to_string(), "name".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let mut from_model = build_model_state(
			"clusters",
			"Cluster",
			vec![id_field.clone(), org_field.clone(), name_field.clone()],
			Vec::new(),
			vec![unique_constraint],
		);
		from_model.table_name = "clusters_cluster".to_string();

		let mut to_model = build_model_state(
			"clusters",
			"Cluster",
			vec![id_field, org_field, name_field],
			Vec::new(),
			Vec::new(),
		);
		to_model.table_name = "clusters_cluster".to_string();

		let from_state = build_project_state(vec![(
			("clusters".to_string(), "Cluster".to_string()),
			from_model,
		)]);
		let to_state = build_project_state(vec![(
			("clusters".to_string(), "Cluster".to_string()),
			to_model,
		)]);
		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let migrations = detector.generate_migrations();

		// Assert
		assert_eq!(
			migrations.len(),
			1,
			"expected exactly one Migration, got: {:?}",
			migrations
		);
		assert_eq!(migrations[0].app_label, "clusters");
		assert_eq!(
			migrations[0].operations.len(),
			1,
			"expected exactly one operation in the migration, got: {:?}",
			migrations[0].operations
		);
		let super::super::Operation::DropConstraintDefinition {
			table,
			constraint: super::super::Constraint::Unique { name, .. },
		} = &migrations[0].operations[0]
		else {
			panic!(
				"expected Operation::DropConstraintDefinition, got: {:?}",
				migrations[0].operations[0]
			);
		};
		assert_eq!(table, "clusters_cluster");
		assert_eq!(name, "clusters_cluster_organization_id_name_uniq");
	}

	#[rstest]
	fn shared_per_app_emissions_are_consistent_between_generate_paths() {
		// Arrange — regression for issue #4040 (structural).
		//
		// Issue #4040 was caused by `generate_operations()` and
		// `generate_migrations()` carrying parallel-but-divergent
		// per-change-set emission loops; PR #3998 updated only the former.
		// After the structural fix both methods route through
		// `emit_shared_per_app_operations()`, so the shared subset of ops
		// (CreateTable / column ops / constraint ops / auto-increment
		// resets) MUST always agree.
		//
		// This test exercises a representative scenario: an existing model
		// gains a unique constraint AND a new column. Both emissions must
		// appear identically in both methods. M2M / rename / move
		// divergences are intentionally not exercised here because they
		// remain method-specific by design.
		let id_field = FieldState::new("id", super::super::FieldType::Integer, false);
		let org_field = FieldState::new("organization_id", super::super::FieldType::Integer, false);
		let name_field = FieldState::new("name", super::super::FieldType::VarChar(255), false);
		let new_col = FieldState::new("region", super::super::FieldType::VarChar(64), false);

		let mut from_model = build_model_state(
			"clusters",
			"Cluster",
			vec![id_field.clone(), org_field.clone(), name_field.clone()],
			Vec::new(),
			Vec::new(),
		);
		from_model.table_name = "clusters_cluster".to_string();

		let unique_constraint = ConstraintDefinition {
			name: "clusters_cluster_organization_id_name_uniq".to_string(),
			constraint_type: "unique".to_string(),
			fields: vec!["organization_id".to_string(), "name".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let mut to_model = build_model_state(
			"clusters",
			"Cluster",
			vec![id_field, org_field, name_field, new_col],
			Vec::new(),
			vec![unique_constraint],
		);
		to_model.table_name = "clusters_cluster".to_string();

		let from_state = build_project_state(vec![(
			("clusters".to_string(), "Cluster".to_string()),
			from_model,
		)]);
		let to_state = build_project_state(vec![(
			("clusters".to_string(), "Cluster".to_string()),
			to_model,
		)]);
		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let ops = detector.generate_operations();
		let migrations = detector.generate_migrations();

		// Assert — flatten migrations into the same shape as `ops` and
		// compare as multisets (order is determined by per-app dependency
		// sort, which is the same algorithm but called with a different
		// scope, so we compare unordered).
		let mig_ops: Vec<&super::super::Operation> = migrations
			.iter()
			.flat_map(|m| m.operations.iter())
			.collect();

		assert_eq!(
			ops.len(),
			mig_ops.len(),
			"shared per-app emissions diverged between generate_operations() ({:?}) and generate_migrations() ({:?})",
			ops,
			mig_ops
		);
		// Every op produced by generate_operations() must also appear in
		// generate_migrations() output.
		for op in &ops {
			assert!(
				mig_ops.iter().any(|m| *m == op),
				"generate_operations() produced {:?} but generate_migrations() did not",
				op
			);
		}
		// And vice versa.
		for op in &mig_ops {
			assert!(
				ops.iter().any(|o| o == *op),
				"generate_migrations() produced {:?} but generate_operations() did not",
				op
			);
		}
	}

	#[rstest]
	fn detect_added_composite_pk_does_not_double_emit_add_constraint() {
		// Arrange — adding a composite PK should be emitted by the
		// `CreateCompositePrimaryKey` path only. The new
		// `added_constraints` emitter must skip composite PKs to avoid
		// emitting a redundant AddConstraint alongside it.
		let id_field = FieldState::new("id", super::super::FieldType::Integer, false);
		let tenant_field = FieldState::new("tenant_id", super::super::FieldType::Integer, false);

		let from_model = build_model_state(
			"billing",
			"Invoice",
			vec![id_field.clone(), tenant_field.clone()],
			Vec::new(),
			Vec::new(),
		);
		let composite_pk = ConstraintDefinition {
			name: "billing_invoice_pkey".to_string(),
			constraint_type: "primary_key".to_string(),
			fields: vec!["id".to_string(), "tenant_id".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let to_model = build_model_state(
			"billing",
			"Invoice",
			vec![id_field, tenant_field],
			Vec::new(),
			vec![composite_pk],
		);
		let from_state = build_project_state(vec![(
			("billing".to_string(), "Invoice".to_string()),
			from_model,
		)]);
		let to_state = build_project_state(vec![(
			("billing".to_string(), "Invoice".to_string()),
			to_model,
		)]);
		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let operations = detector.generate_operations();

		// Assert — exactly one operation, and it must be the composite PK
		// path, not a duplicate AddConstraint.
		assert_eq!(operations.len(), 1, "got: {:?}", operations);
		assert!(
			matches!(
				&operations[0],
				super::super::Operation::CreateCompositePrimaryKey { columns, .. }
					if columns == &["id".to_string(), "tenant_id".to_string()]
			),
			"expected only CreateCompositePrimaryKey, got: {:?}",
			operations
		);
	}

	/// Reproduces reinhardt-web#4448: when the file-based `from_state`
	/// reconstruction stores a column's uniqueness as `params["unique"] =
	/// "true"` (the path through `ProjectState::apply_migration_operations`
	/// → `column_def_to_field_state`), and the live model registry's
	/// `to_state` materialises the same column as a synthesised
	/// single-field `ConstraintDefinition`, the autodetector must NOT emit
	/// an `Operation::AddConstraint` for the redundant UNIQUE.
	#[rstest]
	fn inline_unique_param_on_from_side_does_not_emit_redundant_add_constraint() {
		// Arrange — `from_state` mimics what `apply_migration_operations`
		// produces for `0001_initial.rs`: the `username` column carries
		// `params["unique"] = "true"` and the model has NO peer constraint.
		let mut username_field =
			FieldState::new("username", super::super::FieldType::VarChar(150), false);
		username_field
			.params
			.insert("unique".to_string(), "true".to_string());
		let id_field = FieldState::new("id", super::super::FieldType::Integer, false);
		let from_model = build_model_state(
			"users",
			"User",
			vec![id_field.clone(), username_field.clone()],
			Vec::new(),
			Vec::new(),
		);

		// `to_state` mimics what `ModelMetadata::to_model_state()` produces
		// after the fix: a single-field UNIQUE `ConstraintDefinition` named
		// per the `{table}_{field}_uniq` convention, without a duplicate
		// inline field flag.
		let synthesised = ConstraintDefinition {
			name: "users_username_uniq".to_string(),
			constraint_type: "unique".to_string(),
			fields: vec!["username".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let to_model = build_model_state(
			"users",
			"User",
			vec![
				id_field,
				FieldState::new("username", super::super::FieldType::VarChar(150), false),
			],
			Vec::new(),
			vec![synthesised],
		);

		let from_state = build_project_state(vec![(
			("users".to_string(), "User".to_string()),
			from_model,
		)]);
		let to_state =
			build_project_state(vec![(("users".to_string(), "User".to_string()), to_model)]);
		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let operations = detector.generate_operations();

		// Assert — no AddConstraint is emitted, because the column is
		// already covered by inline `params["unique"]` in `from_state`.
		assert!(
			operations
				.iter()
				.all(|op| !matches!(op, super::super::Operation::AddConstraint { .. })),
			"expected NO Operation::AddConstraint, got: {:?}",
			operations
		);
	}

	#[test]
	fn legacy_inline_and_named_unique_removal_changes_the_field_definition() {
		// Arrange
		let id_field = FieldState::new("id", super::super::FieldType::Integer, false);
		let mut legacy_username =
			FieldState::new("username", super::super::FieldType::VarChar(150), false);
		legacy_username
			.params
			.insert("unique".to_string(), "true".to_string());
		let named_constraint = ConstraintDefinition {
			name: "users_username_uniq".to_string(),
			constraint_type: "unique".to_string(),
			fields: vec!["username".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let from_model = build_model_state(
			"users",
			"User",
			vec![id_field.clone(), legacy_username],
			Vec::new(),
			vec![named_constraint],
		);
		let to_model = build_model_state(
			"users",
			"User",
			vec![
				id_field,
				FieldState::new("username", super::super::FieldType::VarChar(150), false),
			],
			Vec::new(),
			Vec::new(),
		);
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("users".to_string(), "User".to_string()),
				from_model,
			)]),
			build_project_state(vec![(("users".to_string(), "User".to_string()), to_model)]),
		);

		// Act
		let operations = detector.generate_operations();

		// Assert
		assert!(operations.iter().any(|operation| {
			matches!(
				operation,
				super::super::Operation::AlterColumn {
					new_definition,
					column,
					..
				} if column == "username" && !new_definition.unique
			)
		}));
		assert!(operations.iter().any(|operation| {
			matches!(
				operation,
				super::super::Operation::DropConstraintDefinition {
					constraint: super::super::Constraint::Unique { name, .. },
					..
				} if name == "users_username_uniq"
			)
		}));
	}

	#[test]
	fn constraint_definition_parser_handles_quoted_unique_identifiers() {
		// Arrange
		let sql = "CONSTRAINT uq_profile UNIQUE (\"profile,id\", \"name)\", \"display\"\"name\")";

		// Act
		let constraint = ProjectState::constraint_definition_from_sql(sql)
			.expect("quoted UNIQUE columns should parse");

		// Assert
		assert_eq!(constraint.name, "uq_profile");
		assert_eq!(
			constraint.fields,
			vec!["profile,id", "name)", "display\"name"]
		);
	}

	#[test]
	fn removed_unique_constraint_precedes_removed_column_without_alter_column() {
		let id_field = FieldState::new("id", super::super::FieldType::Integer, false);
		let username_field =
			FieldState::new("username", super::super::FieldType::VarChar(150), false);
		let unique_constraint = ConstraintDefinition {
			name: "users_username_uniq".to_string(),
			constraint_type: "unique".to_string(),
			fields: vec!["username".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let from_model = build_model_state(
			"users",
			"User",
			vec![id_field.clone(), username_field],
			Vec::new(),
			vec![unique_constraint],
		);
		let to_model = build_model_state("users", "User", vec![id_field], Vec::new(), Vec::new());
		let detector = MigrationAutodetector::new(
			build_project_state(vec![(
				("users".to_string(), "User".to_string()),
				from_model,
			)]),
			build_project_state(vec![(("users".to_string(), "User".to_string()), to_model)]),
		);

		let operations = detector.generate_operations();
		assert_eq!(operations.len(), 2, "unexpected operations: {operations:?}");
		assert!(matches!(
			&operations[0],
			super::super::Operation::DropConstraintDefinition {
				constraint: super::super::Constraint::Unique { name, .. },
				..
			} if name == "users_username_uniq"
		));
		assert!(matches!(
			&operations[1],
			super::super::Operation::DropColumn { column, .. } if column == "username"
		));
		assert!(
			operations
				.iter()
				.all(|operation| !matches!(operation, super::super::Operation::AlterColumn { .. }))
		);
	}

	/// Reproduces the DB-introspection variant of reinhardt-web#4448:
	/// `from_state` carries a single-field UNIQUE constraint with a
	/// dialect-specific auto-name (e.g. SQLite's
	/// `sqlite_autoindex_users_1`), and `to_state` declares the same
	/// column's UNIQUE with the table-derived name
	/// (`users_username_uniq`). The names differ but the semantics
	/// are identical — no `AddConstraint` must be emitted.
	#[rstest]
	fn single_field_unique_constraint_renames_do_not_emit_redundant_add_constraint() {
		// Arrange
		let id_field = FieldState::new("id", super::super::FieldType::Integer, false);
		let username_field =
			FieldState::new("username", super::super::FieldType::VarChar(150), false);
		let auto_named = ConstraintDefinition {
			name: "sqlite_autoindex_users_1".to_string(),
			constraint_type: "unique".to_string(),
			fields: vec!["username".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let model_named = ConstraintDefinition {
			name: "users_username_uniq".to_string(),
			constraint_type: "unique".to_string(),
			fields: vec!["username".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		let from_model = build_model_state(
			"users",
			"User",
			vec![id_field.clone(), username_field.clone()],
			Vec::new(),
			vec![auto_named],
		);
		let to_model = build_model_state(
			"users",
			"User",
			vec![id_field, username_field],
			Vec::new(),
			vec![model_named],
		);
		let from_state = build_project_state(vec![(
			("users".to_string(), "User".to_string()),
			from_model,
		)]);
		let to_state =
			build_project_state(vec![(("users".to_string(), "User".to_string()), to_model)]);
		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let operations = detector.generate_operations();

		// Assert — neither AddConstraint nor DropConstraint is emitted.
		// A pure rename of an internal constraint name on a single-column
		// UNIQUE is treated as a no-op; emitting either would be invalid on
		// SQLite (no ALTER TABLE ADD CONSTRAINT) and would leave duplicate
		// constraints on dialects that recreate the table.
		let constraint_ops: Vec<_> = operations
			.iter()
			.filter(|op| {
				matches!(
					op,
					super::super::Operation::AddConstraint { .. }
						| super::super::Operation::DropConstraint { .. }
				)
			})
			.collect();
		assert!(
			constraint_ops.is_empty(),
			"expected no Add/DropConstraint ops, got: {:?}",
			constraint_ops
		);
	}

	/// Symmetric guard for reinhardt-web#4448: when `from_state` has a
	/// single-field UNIQUE constraint and `to_state` represents the same
	/// uniqueness inline via `params["unique"] = "true"` only, no
	/// `DropConstraint` must be emitted. Without the symmetric check in
	/// `detect_removed_constraints` the fix would shift the redundancy
	/// from `AddConstraint` into `DropConstraint`.
	#[rstest]
	fn from_side_unique_constraint_matched_by_inline_unique_on_to_side_emits_no_drop() {
		// Arrange
		let id_field = FieldState::new("id", super::super::FieldType::Integer, false);
		let mut username_field =
			FieldState::new("username", super::super::FieldType::VarChar(150), false);
		username_field
			.params
			.insert("unique".to_string(), "true".to_string());
		let unique_constraint = ConstraintDefinition {
			name: "users_username_uniq".to_string(),
			constraint_type: "unique".to_string(),
			fields: vec!["username".to_string()],
			expression: None,
			foreign_key_info: None,
		};
		// from_state: constraint present, field has NO inline unique param.
		let bare_username =
			FieldState::new("username", super::super::FieldType::VarChar(150), false);
		let from_model = build_model_state(
			"users",
			"User",
			vec![id_field.clone(), bare_username],
			Vec::new(),
			vec![unique_constraint],
		);
		// to_state: no peer constraint, inline param only.
		let to_model = build_model_state(
			"users",
			"User",
			vec![id_field, username_field],
			Vec::new(),
			Vec::new(),
		);
		let from_state = build_project_state(vec![(
			("users".to_string(), "User".to_string()),
			from_model,
		)]);
		let to_state =
			build_project_state(vec![(("users".to_string(), "User".to_string()), to_model)]);
		let detector = MigrationAutodetector::new(from_state, to_state);

		// Act
		let operations = detector.generate_operations();

		// Assert — no DropConstraint emitted; the inline `params["unique"]`
		// on the to-side already covers the column.
		assert!(
			operations
				.iter()
				.all(|op| !matches!(op, super::super::Operation::DropConstraint { .. })),
			"expected NO Operation::DropConstraint, got: {:?}",
			operations
		);
	}

	/// Direct unit test for the dedup pass
	/// `MigrationAutodetector::dedup_redundant_unique_add_constraints`.
	/// Manually constructs a per-app operation list where an
	/// `Operation::AddColumn { column.unique = true }` is followed by a
	/// peer `Operation::AddConstraint` that ascribes a UNIQUE to the same
	/// column. The dedup pass must drop the redundant `AddConstraint`,
	/// regardless of how it got there. Second safety net for
	/// reinhardt-web#4448.
	#[rstest]
	fn dedup_pass_drops_add_constraint_redundant_with_unique_add_column() {
		// Arrange
		let ops = vec![
			super::super::Operation::AddColumn {
				table: "users".to_string(),
				column: super::super::ColumnDefinition {
					name: "username".to_string(),
					type_definition: super::super::FieldType::VarChar(150),
					not_null: true,
					unique: true,
					primary_key: false,
					auto_increment: false,
					default: None,
					generated: None,
					domain: None,
				},
				mysql_options: None,
			},
			super::super::Operation::AddConstraint {
				table: "users".to_string(),
				constraint_sql: "CONSTRAINT users_username_uniq UNIQUE (username)".to_string(),
			},
		];
		let mut by_app: std::collections::BTreeMap<String, Vec<super::super::Operation>> =
			std::collections::BTreeMap::new();
		by_app.insert("users".to_string(), ops);

		// Act
		MigrationAutodetector::dedup_redundant_unique_add_constraints(&mut by_app);

		// Assert — only AddColumn survives; the AddConstraint is dropped.
		let remaining = &by_app["users"];
		assert_eq!(
			remaining.len(),
			1,
			"expected one operation after dedup, got: {:?}",
			remaining
		);
		assert!(
			matches!(remaining[0], super::super::Operation::AddColumn { .. }),
			"expected the surviving op to be AddColumn, got: {:?}",
			remaining[0]
		);
	}

	#[test]
	fn model_state_new_uses_the_app_prefixed_table_name_convention() {
		assert_eq!(
			ModelState::new("accounts", "User").table_name,
			"accounts_user"
		);
	}

	#[test]
	fn rename_table_in_app_updates_replayed_foreign_key_constraints_without_field_metadata() {
		let mut state = ProjectState::new();
		let mut user = ModelState::new("accounts", "User");
		user.table_name = "users".to_string();
		state.add_model(user);
		let mut post = ModelState::new("blog", "Post");
		post.table_name = "posts".to_string();
		post.constraints.push(ConstraintDefinition {
			name: "posts_user_id_fk".to_string(),
			constraint_type: "foreign_key".to_string(),
			fields: vec!["user_id".to_string()],
			expression: None,
			foreign_key_info: Some(ForeignKeyConstraintInfo {
				referenced_table: "users".to_string(),
				referenced_columns: vec!["id".to_string()],
				on_delete: ForeignKeyAction::Cascade,
				on_update: ForeignKeyAction::Cascade,
			}),
		});
		state.add_model(post);

		state.rename_table_in_app("accounts", "users", "accounts_user");

		assert_eq!(
			state.get_model("blog", "Post").unwrap().constraints[0]
				.foreign_key_info
				.as_ref()
				.unwrap()
				.referenced_table,
			"accounts_user"
		);
	}

	#[test]
	fn many_to_many_artifact_fallback_keeps_the_target_app_prefix() {
		let mut from_state = ProjectState::new();
		let mut old_group = ModelState::new("groups", "Group");
		old_group.table_name = "groups_group".to_string();
		from_state.add_model(old_group);

		let mut old_through = ModelState::new("groups", "GroupPermissions");
		old_through.table_name = "groups_group_permissions".to_string();
		old_through.add_field(FieldState::new(
			"groups_group_id".to_string(),
			super::super::FieldType::Integer,
			false,
		));
		old_through.add_field(FieldState::new(
			"auth_user_id".to_string(),
			super::super::FieldType::Integer,
			false,
		));
		from_state.add_model(old_through);

		let mut to_state = ProjectState::new();
		let mut new_group = ModelState::new("groups", "Group");
		new_group.table_name = "groups_group_v2".to_string();
		new_group
			.many_to_many_fields
			.push(super::super::model_registry::ManyToManyMetadata::new(
				"permissions",
				"auth.User",
			));
		to_state.add_model(new_group);

		let detector = MigrationAutodetector::new(from_state, to_state);
		let rename = detector
			.find_many_to_many_artifact_rename(
				&DetectedChanges::default(),
				"groups",
				"Group",
				&super::super::model_registry::ManyToManyMetadata::new("permissions", "auth.User"),
			)
			.expect("the absent target model should use its app-prefixed fallback table");

		assert_eq!(rename.old_target_column, "auth_user_id");
		assert_eq!(rename.new_target_column, "auth_user_id");
	}

	#[test]
	fn generate_migrations_uses_snake_case_for_an_absent_many_to_many_target() {
		// Arrange
		let mut source = ModelState::new("groups", "Group");
		source.table_name = "groups_group".to_string();
		source
			.many_to_many_fields
			.push(super::super::model_registry::ManyToManyMetadata::new(
				"api_keys",
				"auth.APIKey",
			));
		let mut to_state = ProjectState::new();
		to_state.add_model(source);
		let detector = MigrationAutodetector::new(ProjectState::new(), to_state);

		// Act
		let migrations = detector.generate_migrations();
		let operations = &migrations
			.iter()
			.find(|migration| migration.app_label == "groups")
			.expect("groups migration should be generated")
			.operations;

		// Assert
		let constraints = operations
			.iter()
			.find_map(|operation| match operation {
				super::super::Operation::CreateTable {
					name, constraints, ..
				} if name == "groups_group_api_keys" => Some(constraints),
				_ => None,
			})
			.expect("many-to-many table should be created");
		assert!(constraints.iter().any(|constraint| matches!(
			constraint,
			super::super::operations::Constraint::ForeignKey { referenced_table, .. }
				if referenced_table == "auth_api_key"
		)));
	}

	#[test]
	fn validate_rename_destinations_rejects_a_moved_model_collision() {
		// Arrange
		let mut to_state = ProjectState::new();
		let mut moved_profile = ModelState::new("profiles", "Profile");
		moved_profile.table_name = "shared".to_string();
		to_state.add_model(moved_profile);
		let mut retained_user = ModelState::new("accounts", "User");
		retained_user.table_name = "shared".to_string();
		to_state.add_model(retained_user);
		let detector = MigrationAutodetector::new(ProjectState::new(), to_state);
		let changes = DetectedChanges {
			moved_models: vec![(
				"legacy".to_string(),
				"Profile".to_string(),
				"profiles".to_string(),
				"Profile".to_string(),
				true,
				Some("legacy_profile".to_string()),
				Some("shared".to_string()),
			)],
			..DetectedChanges::default()
		};

		// Act
		let error = detector
			.validate_rename_destinations(&changes)
			.expect_err("moved models must not rename into an occupied table");

		// Assert
		assert!(error.to_string().contains("multiple target models claim"));
	}

	#[test]
	fn try_detect_changes_rejects_cross_app_rename_destination_collisions() {
		let mut from_state = ProjectState::new();
		let mut old_profile = ModelState::new("accounts", "Profile");
		old_profile.table_name = "accounts_profile".to_string();
		from_state.add_model(old_profile);
		let mut audit_user = ModelState::new("audit", "User");
		audit_user.table_name = "users".to_string();
		from_state.add_model(audit_user);

		let mut to_state = ProjectState::new();
		let mut renamed_profile = ModelState::new("accounts", "Profile");
		renamed_profile.table_name = "users".to_string();
		to_state.add_model(renamed_profile);
		let mut retained_audit_user = ModelState::new("audit", "User");
		retained_audit_user.table_name = "users".to_string();
		to_state.add_model(retained_audit_user);

		let error = MigrationAutodetector::new(from_state, to_state)
			.try_detect_changes()
			.expect_err("renames must not claim a table that another app still owns");

		assert!(error.to_string().contains("multiple target models claim"));
	}

	fn integer_id_field() -> FieldState {
		FieldState::new("id", super::super::FieldType::Integer, false)
	}

	fn integer_fk_field(name: &str, referenced_table: &str) -> FieldState {
		FieldState::with_foreign_key(
			name,
			super::super::FieldType::Integer,
			false,
			ForeignKeyInfo {
				referenced_table: referenced_table.to_string(),
				referenced_column: "id".to_string(),
				on_delete: ForeignKeyAction::Cascade,
				on_update: ForeignKeyAction::Cascade,
			},
		)
	}

	fn model_with_table(
		app_label: &str,
		name: &str,
		table_name: &str,
		fields: Vec<FieldState>,
	) -> ModelState {
		let mut model = ModelState::new(app_label, name);
		model.table_name = table_name.to_string();
		for field in fields {
			let field_name = field.name.clone();
			model.add_field(field);
			if model
				.fields
				.get(&field_name)
				.and_then(|field| field.foreign_key.as_ref())
				.is_some()
			{
				model.add_foreign_key_constraint_from_field(&field_name);
			}
		}
		model
	}

	fn create_table_names(migration: &super::super::Migration) -> Vec<String> {
		migration
			.operations
			.iter()
			.filter_map(|operation| match operation {
				super::super::Operation::CreateTable { name, .. } => Some(name.clone()),
				_ => None,
			})
			.collect()
	}

	#[rstest]
	fn detect_model_dependencies_reads_scalar_foreign_key_metadata() {
		// Arrange: FK ID column keeps a scalar field type; the relationship
		// lives on FieldState.foreign_key, which alphabetical detection missed.
		let mut to_state = ProjectState::new();
		to_state.add_model(model_with_table(
			"auth",
			"User",
			"auth_users",
			vec![integer_id_field()],
		));
		to_state.add_model(model_with_table(
			"auth",
			"ApiKey",
			"auth_api_keys",
			vec![
				integer_id_field(),
				integer_fk_field("user_id", "auth_users"),
			],
		));
		let detector = MigrationAutodetector::new(ProjectState::new(), to_state);

		// Act
		let changes = detector.detect_changes();

		// Assert
		let deps = changes
			.model_dependencies
			.get(&("auth".to_string(), "ApiKey".to_string()))
			.expect("ApiKey must record a dependency on User");
		assert_eq!(
			deps,
			&vec![("auth".to_string(), "User".to_string())],
			"scalar Integer FK columns must contribute model_dependencies"
		);
	}

	#[rstest]
	fn generate_migrations_orders_same_app_create_tables_by_foreign_key() {
		// Arrange: auth_api_keys sorts before auth_users lexicographically.
		let mut to_state = ProjectState::new();
		to_state.add_model(model_with_table(
			"auth",
			"ApiKey",
			"auth_api_keys",
			vec![
				integer_id_field(),
				integer_fk_field("user_id", "auth_users"),
			],
		));
		to_state.add_model(model_with_table(
			"auth",
			"User",
			"auth_users",
			vec![integer_id_field()],
		));
		let detector = MigrationAutodetector::new(ProjectState::new(), to_state);

		// Act
		let migrations = detector.generate_migrations();
		let operations = detector.generate_operations();

		// Assert
		let auth = migrations
			.iter()
			.find(|migration| migration.app_label == "auth")
			.expect("auth migration");
		let table_names = create_table_names(auth);
		let users = table_names
			.iter()
			.position(|name| name == "auth_users")
			.expect("auth_users CreateTable");
		let api_keys = table_names
			.iter()
			.position(|name| name == "auth_api_keys")
			.expect("auth_api_keys CreateTable");
		assert!(
			users < api_keys,
			"auth_users must be created before auth_api_keys, got {table_names:?}"
		);

		let operation_tables: Vec<String> = operations
			.iter()
			.filter_map(|operation| match operation {
				super::super::Operation::CreateTable { name, .. } => Some(name.clone()),
				_ => None,
			})
			.collect();
		let users = operation_tables
			.iter()
			.position(|name| name == "auth_users")
			.expect("auth_users in generate_operations");
		let api_keys = operation_tables
			.iter()
			.position(|name| name == "auth_api_keys")
			.expect("auth_api_keys in generate_operations");
		assert!(
			users < api_keys,
			"generate_operations must also emit auth_users before auth_api_keys, got {operation_tables:?}"
		);
	}

	#[rstest]
	fn generate_migrations_records_cross_app_foreign_key_graph() {
		// Arrange: auth <- organizations <- clusters <- deployments <- github
		let mut to_state = ProjectState::new();
		to_state.add_model(model_with_table(
			"auth",
			"User",
			"auth_users",
			vec![integer_id_field()],
		));
		to_state.add_model(model_with_table(
			"organizations",
			"Organization",
			"organizations",
			vec![
				integer_id_field(),
				integer_fk_field("owner_id", "auth_users"),
			],
		));
		to_state.add_model(model_with_table(
			"clusters",
			"Cluster",
			"clusters",
			vec![
				integer_id_field(),
				integer_fk_field("organization_id", "organizations"),
			],
		));
		to_state.add_model(model_with_table(
			"deployments",
			"Deployment",
			"deployments",
			vec![
				integer_id_field(),
				integer_fk_field("organization_id", "organizations"),
				integer_fk_field("cluster_id", "clusters"),
			],
		));
		to_state.add_model(model_with_table(
			"github",
			"Project",
			"github_projects",
			vec![
				integer_id_field(),
				integer_fk_field("organization_id", "organizations"),
				integer_fk_field("deployment_id", "deployments"),
			],
		));
		let detector = MigrationAutodetector::new(ProjectState::new(), to_state.clone());

		// Act
		let changes = detector.detect_changes();
		let migrations = detector.generate_migrations();
		let github_ops = migrations
			.iter()
			.find(|migration| migration.app_label == "github")
			.expect("github migration")
			.operations
			.as_slice();
		let providers =
			MigrationAutodetector::foreign_key_provider_apps(&to_state, github_ops, "github");
		let second_pass =
			MigrationAutodetector::new(to_state.clone(), to_state).generate_migrations();

		// Assert
		assert_eq!(
			changes
				.model_dependencies
				.get(&("organizations".to_string(), "Organization".to_string()))
				.expect("organizations depends on auth"),
			&vec![("auth".to_string(), "User".to_string())]
		);
		let cluster_deps = changes
			.model_dependencies
			.get(&("clusters".to_string(), "Cluster".to_string()))
			.expect("clusters depends on organizations");
		assert_eq!(
			cluster_deps,
			&vec![("organizations".to_string(), "Organization".to_string())]
		);
		let deployment_deps = changes
			.model_dependencies
			.get(&("deployments".to_string(), "Deployment".to_string()))
			.expect("deployments depends on organizations and clusters");
		assert!(
			deployment_deps.contains(&("organizations".to_string(), "Organization".to_string()))
		);
		assert!(deployment_deps.contains(&("clusters".to_string(), "Cluster".to_string())));
		assert_eq!(
			providers,
			vec!["deployments".to_string(), "organizations".to_string()]
		);
		assert!(
			second_pass.is_empty(),
			"second autodetect against the same state must not emit extra operations, got {second_pass:?}"
		);
	}

	#[rstest]
	fn generate_migrations_survives_circular_foreign_keys() {
		// Arrange: A <-> B. Cycles must warn rather than panic, and both
		// tables must still be created.
		let mut to_state = ProjectState::new();
		to_state.add_model(model_with_table(
			"cycles",
			"Alpha",
			"cycles_alpha",
			vec![
				integer_id_field(),
				integer_fk_field("beta_id", "cycles_beta"),
			],
		));
		to_state.add_model(model_with_table(
			"cycles",
			"Beta",
			"cycles_beta",
			vec![
				integer_id_field(),
				integer_fk_field("alpha_id", "cycles_alpha"),
			],
		));
		let detector = MigrationAutodetector::new(ProjectState::new(), to_state);

		// Act
		let migrations = detector.generate_migrations();

		// Assert
		let cycle_migration = migrations
			.iter()
			.find(|migration| migration.app_label == "cycles")
			.expect("cycles migration");
		let tables = create_table_names(cycle_migration);
		assert!(tables.contains(&"cycles_alpha".to_string()));
		assert!(tables.contains(&"cycles_beta".to_string()));
	}
}
