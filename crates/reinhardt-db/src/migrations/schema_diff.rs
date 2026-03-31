//! Schema diff detection
//!
//! Detects differences between current database schema and model definitions:
//! - Table additions/removals
//! - Column modifications
//! - Index changes
//! - Constraint changes

use super::ColumnDefinition;
use super::introspection;
use super::operations::Operation;
use std::collections::BTreeMap;

/// Schema difference detector
pub struct SchemaDiff {
	/// Current database schema
	current_schema: DatabaseSchema,
	/// Target schema from models
	target_schema: DatabaseSchema,
}

/// Database schema representation
#[derive(Debug, Clone, Default)]
pub struct DatabaseSchema {
	/// Table definitions (BTreeMap for deterministic iteration order)
	pub tables: BTreeMap<String, TableSchema>,
}

impl From<introspection::DatabaseSchema> for DatabaseSchema {
	fn from(intro_schema: introspection::DatabaseSchema) -> Self {
		let mut tables = BTreeMap::new();

		for (table_name, intro_table) in intro_schema.tables {
			let mut columns = BTreeMap::new();
			for (col_name, intro_col) in intro_table.columns {
				// Simplified conversion
				columns.insert(
					col_name.clone(),
					ColumnSchema {
						name: intro_col.name,
						data_type: intro_col.column_type,
						nullable: intro_col.nullable,
						default: intro_col.default,
						primary_key: intro_table.primary_key.contains(&col_name), // Check if column is in primary_key list
						auto_increment: intro_col.auto_increment,
					},
				);
			}

			let indexes: Vec<IndexSchema> = intro_table
				.indexes
				.values()
				.map(|idx| IndexSchema {
					name: idx.name.clone(),
					columns: idx.columns.clone(),
					unique: idx.unique,
				})
				.collect();

			let mut constraints: Vec<ConstraintSchema> = intro_table
				.unique_constraints
				.iter()
				.map(|uc| ConstraintSchema {
					name: uc.name.clone(),
					constraint_type: "UNIQUE".to_string(),
					definition: format!(
						"CONSTRAINT {} UNIQUE ({})",
						uc.name,
						uc.columns.join(", ")
					),
					foreign_key_info: None,
				})
				.collect();

			// Process foreign keys with structured information
			for fk in &intro_table.foreign_keys {
				let on_delete = fk
					.on_delete
					.clone()
					.unwrap_or_else(|| "NO ACTION".to_string());
				let on_update = fk
					.on_update
					.clone()
					.unwrap_or_else(|| "NO ACTION".to_string());

				let mut definition = format!(
					"CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}({})",
					fk.name,
					fk.columns.join(", "),
					fk.referenced_table,
					fk.referenced_columns.join(", ")
				);

				if on_delete != "NO ACTION" {
					definition.push_str(&format!(" ON DELETE {}", on_delete));
				}

				if on_update != "NO ACTION" {
					definition.push_str(&format!(" ON UPDATE {}", on_update));
				}

				constraints.push(ConstraintSchema {
					name: fk.name.clone(),
					constraint_type: "FOREIGN KEY".to_string(),
					definition,
					foreign_key_info: Some(ForeignKeySchemaInfo {
						columns: fk.columns.clone(),
						referenced_table: fk.referenced_table.clone(),
						referenced_columns: fk.referenced_columns.clone(),
						on_delete,
						on_update,
					}),
				});
			}

			tables.insert(
				table_name,
				TableSchema {
					name: intro_table.name,
					columns,
					indexes,
					constraints,
				},
			);
		}

		DatabaseSchema { tables }
	}
}

impl DatabaseSchema {
	/// Filter tables by app_label prefix
	///
	/// This method filters tables based on the Django-style naming convention
	/// where table names are prefixed with the app label (e.g., "users_user", "todos_todo").
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::schema_diff::DatabaseSchema;
	///
	/// let schema = DatabaseSchema::default();
	/// let filtered = schema.filter_by_app("users");
	/// // filtered contains only tables starting with "users_"
	/// ```
	pub fn filter_by_app(&self, app_label: &str) -> DatabaseSchema {
		let mut filtered_tables = BTreeMap::new();
		let prefix = format!("{}_", app_label);

		for (table_name, table_schema) in &self.tables {
			if table_name.starts_with(&prefix) {
				filtered_tables.insert(table_name.clone(), table_schema.clone());
			}
		}

		DatabaseSchema {
			tables: filtered_tables,
		}
	}
}

/// Table schema
#[derive(Debug, Clone)]
pub struct TableSchema {
	/// Table name
	pub name: String,
	/// Column definitions (BTreeMap for deterministic iteration order)
	pub columns: BTreeMap<String, ColumnSchema>,
	/// Indexes
	pub indexes: Vec<IndexSchema>,
	/// Constraints
	pub constraints: Vec<ConstraintSchema>,
}

/// Column schema
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnSchema {
	/// Column name
	pub name: String,
	/// Data type
	pub data_type: super::FieldType,
	/// Nullable
	pub nullable: bool,
	/// Default value
	pub default: Option<String>,
	/// Primary key
	pub primary_key: bool,
	/// Auto increment
	pub auto_increment: bool,
}

/// Index schema
#[derive(Debug, Clone, PartialEq)]
pub struct IndexSchema {
	/// Index name
	pub name: String,
	/// Columns
	pub columns: Vec<String>,
	/// Unique index
	pub unique: bool,
}

/// Constraint schema
#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintSchema {
	/// Constraint name
	pub name: String,
	/// Constraint type (UNIQUE, FOREIGN KEY, CHECK, etc.)
	pub constraint_type: String,
	/// Definition (columns for UNIQUE, expression for CHECK, etc.)
	pub definition: String,
	/// Foreign key specific information (only for FOREIGN KEY / ONE_TO_ONE types)
	pub foreign_key_info: Option<ForeignKeySchemaInfo>,
}

/// Foreign key constraint information for schema diff
///
/// This struct holds structured information about foreign key constraints,
/// enabling proper constraint extraction and comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct ForeignKeySchemaInfo {
	/// Source columns in the referencing table
	pub columns: Vec<String>,
	/// Referenced table name
	pub referenced_table: String,
	/// Referenced columns in the target table
	pub referenced_columns: Vec<String>,
	/// ON DELETE action (CASCADE, SET NULL, SET DEFAULT, RESTRICT, NO ACTION)
	pub on_delete: String,
	/// ON UPDATE action (CASCADE, SET NULL, SET DEFAULT, RESTRICT, NO ACTION)
	pub on_update: String,
}

/// Schema diff result
#[derive(Debug, Clone)]
pub struct SchemaDiffResult {
	/// Tables to add
	pub tables_to_add: Vec<String>,
	/// Tables to remove
	pub tables_to_remove: Vec<String>,
	/// Columns to add (table_name, column_name)
	pub columns_to_add: Vec<(String, String)>,
	/// Columns to remove (table_name, column_name)
	pub columns_to_remove: Vec<(String, String)>,
	/// Columns to modify (table_name, column_name, old, new)
	pub columns_to_modify: Vec<(String, String, ColumnSchema, ColumnSchema)>,
	/// Indexes to add
	pub indexes_to_add: Vec<(String, IndexSchema)>,
	/// Indexes to remove
	pub indexes_to_remove: Vec<(String, IndexSchema)>,
	/// Constraints to add (table_name, constraint)
	pub constraints_to_add: Vec<(String, ConstraintSchema)>,
	/// Constraints to remove (table_name, constraint)
	pub constraints_to_remove: Vec<(String, ConstraintSchema)>,
}

impl SchemaDiff {
	/// Create a new schema diff detector
	pub fn new(current_schema: DatabaseSchema, target_schema: DatabaseSchema) -> Self {
		Self {
			current_schema,
			target_schema,
		}
	}

	/// Detect differences between schemas
	pub fn detect(&self) -> SchemaDiffResult {
		let mut result = SchemaDiffResult {
			tables_to_add: Vec::new(),
			tables_to_remove: Vec::new(),
			columns_to_add: Vec::new(),
			columns_to_remove: Vec::new(),
			columns_to_modify: Vec::new(),
			indexes_to_add: Vec::new(),
			indexes_to_remove: Vec::new(),
			constraints_to_add: Vec::new(),
			constraints_to_remove: Vec::new(),
		};

		// System tables to exclude from migration generation
		let system_tables = ["reinhardt_migrations"];

		// Detect table additions
		for table_name in self.target_schema.tables.keys() {
			if !self.current_schema.tables.contains_key(table_name) {
				result.tables_to_add.push(table_name.clone());
			}
		}

		// Detect table removals (skip system tables)
		for table_name in self.current_schema.tables.keys() {
			if system_tables.contains(&table_name.as_str()) {
				continue; // Skip system tables
			}
			if !self.target_schema.tables.contains_key(table_name) {
				result.tables_to_remove.push(table_name.clone());
			}
		}

		// Detect column changes for existing tables
		for (table_name, target_table) in &self.target_schema.tables {
			if let Some(current_table) = self.current_schema.tables.get(table_name) {
				// Clone table_name once for reuse across all change types
				let table_name_owned = table_name.clone();

				// Column additions
				for col_name in target_table.columns.keys() {
					if !current_table.columns.contains_key(col_name) {
						result
							.columns_to_add
							.push((table_name_owned.clone(), col_name.clone()));
					}
				}

				// Column removals
				for col_name in current_table.columns.keys() {
					if !target_table.columns.contains_key(col_name) {
						result
							.columns_to_remove
							.push((table_name_owned.clone(), col_name.clone()));
					}
				}

				// Column modifications
				for (col_name, target_col) in &target_table.columns {
					if let Some(current_col) = current_table.columns.get(col_name)
						&& current_col != target_col
					{
						result.columns_to_modify.push((
							table_name_owned.clone(),
							col_name.clone(),
							current_col.clone(),
							target_col.clone(),
						));
					}
				}

				// Index changes
				for target_index in &target_table.indexes {
					if !current_table.indexes.contains(target_index) {
						result
							.indexes_to_add
							.push((table_name_owned.clone(), target_index.clone()));
					}
				}

				for current_index in &current_table.indexes {
					if !target_table.indexes.contains(current_index) {
						result
							.indexes_to_remove
							.push((table_name_owned.clone(), current_index.clone()));
					}
				}

				// Constraint additions
				for target_constraint in &target_table.constraints {
					if !current_table.constraints.contains(target_constraint) {
						result
							.constraints_to_add
							.push((table_name_owned.clone(), target_constraint.clone()));
					}
				}

				// Constraint removals
				for current_constraint in &current_table.constraints {
					if !target_table.constraints.contains(current_constraint) {
						result
							.constraints_to_remove
							.push((table_name_owned.clone(), current_constraint.clone()));
					}
				}
			}
		}

		result
	}

	/// Generate migration operations from diff
	pub fn generate_operations(&self) -> Vec<Operation> {
		let diff = self.detect();
		let mut operations = Vec::new();

		// Add tables
		for table_name in &diff.tables_to_add {
			if let Some(table_schema) = self.target_schema.tables.get(table_name) {
				// Map columns to ColumnDefinition
				let columns: Vec<_> = table_schema
					.columns
					.iter()
					.map(|(name, col)| {
						let unique = self.extract_column_constraints(table_name, name);
						let auto_increment = Self::is_auto_increment(col);

						ColumnDefinition {
							name: name.clone(),
							type_definition: col.data_type.clone(),
							not_null: !col.nullable,
							default: col.default.as_ref().cloned(),
							unique,
							primary_key: col.primary_key,
							auto_increment,
						}
					})
					.collect();

				// Extract table-level constraints
				let constraints = self.extract_constraints(table_name);

				operations.push(Operation::CreateTable {
					name: table_name.clone(),
					columns,
					constraints,
					without_rowid: None,
					interleave_in_parent: None,
					partition: None,
				});

				// Generate CreateIndex for indexes on new tables
				// (detect() only compares indexes on existing tables)
				for index in &table_schema.indexes {
					operations.push(Operation::CreateIndex {
						table: table_name.clone(),
						columns: index.columns.clone(),
						unique: index.unique,
						index_type: None,
						where_clause: None,
						concurrently: false,
						expressions: None,
						mysql_options: None,
						operator_class: None,
					});
				}
			}
		}

		// Remove tables
		for table_name in &diff.tables_to_remove {
			operations.push(Operation::DropTable {
				name: table_name.clone(),
			});
		}

		// Add columns
		for (table_name, col_name) in &diff.columns_to_add {
			if let Some(table_schema) = self.target_schema.tables.get(table_name)
				&& let Some(col_schema) = table_schema.columns.get(col_name)
			{
				let unique = self.extract_column_constraints(table_name, col_name);
				let auto_increment = Self::is_auto_increment(col_schema);

				operations.push(Operation::AddColumn {
					table: table_name.clone(),
					column: ColumnDefinition {
						name: col_name.clone(),
						type_definition: col_schema.data_type.clone(),
						not_null: !col_schema.nullable,
						default: col_schema.default.as_ref().cloned(),
						unique,
						primary_key: col_schema.primary_key,
						auto_increment,
					},
					mysql_options: None,
				});
			}
		}

		// Remove columns
		for (table_name, col_name) in &diff.columns_to_remove {
			operations.push(Operation::DropColumn {
				table: table_name.clone(),
				column: col_name.clone(),
			});
		}

		// Alter columns (type changes, nullability changes, etc.)
		for (table_name, col_name, old_col, new_col) in &diff.columns_to_modify {
			let old_unique = Self::column_has_unique(&self.current_schema, table_name, col_name);
			let new_unique = Self::column_has_unique(&self.target_schema, table_name, col_name);

			operations.push(Operation::AlterColumn {
				table: table_name.clone(),
				column: col_name.clone(),
				old_definition: Some(ColumnDefinition {
					name: col_name.clone(),
					type_definition: old_col.data_type.clone(),
					not_null: !old_col.nullable,
					default: old_col.default.as_ref().cloned(),
					unique: old_unique,
					primary_key: old_col.primary_key,
					auto_increment: Self::is_auto_increment(old_col),
				}),
				new_definition: ColumnDefinition {
					name: col_name.clone(),
					type_definition: new_col.data_type.clone(),
					not_null: !new_col.nullable,
					default: new_col.default.as_ref().cloned(),
					unique: new_unique,
					primary_key: new_col.primary_key,
					auto_increment: Self::is_auto_increment(new_col),
				},
				mysql_options: None,
			});
		}

		// Add indexes
		for (table_name, index) in &diff.indexes_to_add {
			operations.push(Operation::CreateIndex {
				table: table_name.clone(),
				columns: index.columns.clone(),
				unique: index.unique,
				index_type: None,
				where_clause: None,
				concurrently: false,
				expressions: None,
				mysql_options: None,
				operator_class: None,
			});
		}

		// Remove indexes
		for (table_name, index) in &diff.indexes_to_remove {
			operations.push(Operation::DropIndex {
				table: table_name.clone(),
				columns: index.columns.clone(),
			});
		}

		// Add constraints
		for (table_name, constraint) in &diff.constraints_to_add {
			let constraint_sql = Self::constraint_schema_to_sql(constraint);
			operations.push(Operation::AddConstraint {
				table: table_name.clone(),
				constraint_sql,
			});
		}

		// Remove constraints
		for (table_name, constraint) in &diff.constraints_to_remove {
			operations.push(Operation::DropConstraint {
				table: table_name.clone(),
				constraint_name: constraint.name.clone(),
			});
		}

		operations
	}

	/// Check if diff has destructive changes
	pub fn has_destructive_changes(&self) -> bool {
		let diff = self.detect();
		!diff.tables_to_remove.is_empty()
			|| !diff.columns_to_remove.is_empty()
			|| !diff.columns_to_modify.is_empty()
			|| !diff.indexes_to_remove.is_empty()
			|| !diff.constraints_to_remove.is_empty()
	}

	/// Check if a column has a unique constraint or index in a given schema
	fn column_has_unique(schema: &DatabaseSchema, table_name: &str, column_name: &str) -> bool {
		let table_schema = match schema.tables.get(table_name) {
			Some(t) => t,
			None => return false,
		};

		let has_unique_constraint = table_schema.constraints.iter().any(|constraint| {
			constraint.constraint_type.to_uppercase() == "UNIQUE"
				&& constraint.definition.contains(column_name)
		});

		let has_unique_index = table_schema.indexes.iter().any(|index| {
			index.unique && index.columns.len() == 1 && index.columns[0] == column_name
		});

		has_unique_constraint || has_unique_index
	}

	/// Convert a ConstraintSchema to a SQL definition string for AddConstraint
	fn constraint_schema_to_sql(constraint: &ConstraintSchema) -> String {
		match constraint.constraint_type.to_uppercase().as_str() {
			"UNIQUE" => {
				format!(
					"CONSTRAINT {} UNIQUE ({})",
					constraint.name, constraint.definition
				)
			}
			"CHECK" => {
				format!(
					"CONSTRAINT {} CHECK ({})",
					constraint.name, constraint.definition
				)
			}
			"FOREIGN KEY" | "FOREIGN_KEY" => {
				if let Some(ref fk) = constraint.foreign_key_info {
					format!(
						"CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({}) ON DELETE {} ON UPDATE {}",
						constraint.name,
						fk.columns.join(", "),
						fk.referenced_table,
						fk.referenced_columns.join(", "),
						fk.on_delete,
						fk.on_update,
					)
				} else {
					format!(
						"CONSTRAINT {} FOREIGN KEY ({})",
						constraint.name, constraint.definition
					)
				}
			}
			"ONE_TO_ONE" => {
				if let Some(ref fk) = constraint.foreign_key_info {
					format!(
						"CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({}) ON DELETE {} ON UPDATE {}",
						constraint.name,
						fk.columns.join(", "),
						fk.referenced_table,
						fk.referenced_columns.join(", "),
						fk.on_delete,
						fk.on_update,
					)
				} else {
					format!(
						"CONSTRAINT {} UNIQUE ({})",
						constraint.name, constraint.definition
					)
				}
			}
			_ => {
				format!(
					"CONSTRAINT {} {} ({})",
					constraint.name, constraint.constraint_type, constraint.definition
				)
			}
		}
	}

	/// Extract column-level constraints from table constraints and indexes
	fn extract_column_constraints(&self, table_name: &str, column_name: &str) -> bool {
		Self::column_has_unique(&self.target_schema, table_name, column_name)
	}

	/// Detect if column is auto-increment based on column properties and data type
	fn is_auto_increment(column: &ColumnSchema) -> bool {
		// If already marked as auto_increment, trust it
		if column.auto_increment {
			return true;
		}

		let upper_type = column.data_type.to_sql_string().to_uppercase();

		// PostgreSQL SERIAL types (SMALLSERIAL, SERIAL, BIGSERIAL)
		if upper_type.contains("SERIAL") {
			return true;
		}

		// MySQL AUTO_INCREMENT (typically in extra metadata, but check data type comments)
		if upper_type.contains("AUTO_INCREMENT") {
			return true;
		}

		// SQLite: INTEGER PRIMARY KEY is auto-increment by default
		if column.primary_key && (upper_type == "INTEGER" || upper_type == "INT") {
			return true;
		}

		false
	}

	/// Extract table-level constraint definitions
	fn extract_constraints(&self, table_name: &str) -> Vec<super::Constraint> {
		let table_schema = match self.target_schema.tables.get(table_name) {
			Some(t) => t,
			None => return Vec::new(),
		};

		let mut constraints = Vec::new();

		// Extract PRIMARY KEY constraint as Unique constraint (composite keys only)
		let pk_columns: Vec<String> = table_schema
			.columns
			.iter()
			.filter_map(|(name, col)| {
				if col.primary_key {
					Some(name.clone())
				} else {
					None
				}
			})
			.collect();

		if pk_columns.len() > 1 {
			// Composite primary key represented as Unique constraint
			constraints.push(super::Constraint::Unique {
				name: format!("{}_pkey", table_name),
				columns: pk_columns,
			});
		}

		// Extract UNIQUE constraints from indexes (multi-column unique indexes)
		for index in &table_schema.indexes {
			if index.unique && index.columns.len() > 1 {
				constraints.push(super::Constraint::Unique {
					name: index.name.clone(),
					columns: index.columns.clone(),
				});
			}
		}

		// Extract constraints from table_schema.constraints (from model definitions)
		for constraint_schema in &table_schema.constraints {
			match constraint_schema.constraint_type.to_uppercase().as_str() {
				"UNIQUE" => {
					constraints.push(super::Constraint::Unique {
						name: constraint_schema.name.clone(),
						columns: constraint_schema
							.definition
							.split(", ")
							.map(String::from)
							.collect(),
					});
				}
				"FOREIGN KEY" | "FOREIGN_KEY" => {
					// Use structured FK info if available
					if let Some(ref fk_info) = constraint_schema.foreign_key_info {
						constraints.push(super::Constraint::ForeignKey {
							name: constraint_schema.name.clone(),
							columns: fk_info.columns.clone(),
							referenced_table: fk_info.referenced_table.clone(),
							referenced_columns: fk_info.referenced_columns.clone(),
							on_delete: Self::parse_fk_action(&fk_info.on_delete),
							on_update: Self::parse_fk_action(&fk_info.on_update),
							deferrable: None,
						});
					}
				}
				"ONE_TO_ONE" => {
					// OneToOne is similar to ForeignKey but typically single-column
					if let Some(ref fk_info) = constraint_schema.foreign_key_info {
						constraints.push(super::Constraint::OneToOne {
							name: constraint_schema.name.clone(),
							column: fk_info.columns.first().cloned().unwrap_or_default(),
							referenced_table: fk_info.referenced_table.clone(),
							referenced_column: fk_info
								.referenced_columns
								.first()
								.cloned()
								.unwrap_or_else(|| "id".to_string()),
							on_delete: Self::parse_fk_action(&fk_info.on_delete),
							on_update: Self::parse_fk_action(&fk_info.on_update),
							deferrable: None,
						});
					}
				}
				"CHECK" => {
					constraints.push(super::Constraint::Check {
						name: constraint_schema.name.clone(),
						expression: constraint_schema.definition.clone(),
					});
				}
				_ => {}
			}
		}

		constraints
	}

	/// Parse FK action string to ForeignKeyAction enum
	fn parse_fk_action(action: &str) -> super::ForeignKeyAction {
		match action.to_uppercase().as_str() {
			"CASCADE" => super::ForeignKeyAction::Cascade,
			"SET NULL" => super::ForeignKeyAction::SetNull,
			"SET DEFAULT" => super::ForeignKeyAction::SetDefault,
			"RESTRICT" => super::ForeignKeyAction::Restrict,
			// "NO ACTION" is the default FK action in SQL, treat unknown actions as NoAction
			_ => super::ForeignKeyAction::NoAction,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::migrations::FieldType;

	#[test]
	fn test_detect_table_addition() {
		let current = DatabaseSchema::default();
		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"users".to_string(),
			TableSchema {
				name: "users".to_string(),
				columns: BTreeMap::new(),
				indexes: Vec::new(),
				constraints: Vec::new(),
			},
		);

		let diff = SchemaDiff::new(current, target);
		let result = diff.detect();

		assert_eq!(result.tables_to_add.len(), 1);
		assert_eq!(result.tables_to_add[0], "users");
	}

	#[test]
	fn test_detect_column_addition() {
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"users".to_string(),
			TableSchema {
				name: "users".to_string(),
				columns: BTreeMap::new(),
				indexes: Vec::new(),
				constraints: Vec::new(),
			},
		);

		let mut target = DatabaseSchema::default();
		let mut target_table = TableSchema {
			name: "users".to_string(),
			columns: BTreeMap::new(),
			indexes: Vec::new(),
			constraints: Vec::new(),
		};
		target_table.columns.insert(
			"email".to_string(),
			ColumnSchema {
				name: "email".to_string(),
				data_type: FieldType::VarChar(255),
				nullable: false,
				default: None,
				primary_key: false,
				auto_increment: false,
			},
		);
		target.tables.insert("users".to_string(), target_table);

		let diff = SchemaDiff::new(current, target);
		let result = diff.detect();

		assert_eq!(result.columns_to_add.len(), 1);
		assert_eq!(
			result.columns_to_add[0],
			("users".to_string(), "email".to_string())
		);
	}

	#[test]
	fn test_destructive_changes_detection() {
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"users".to_string(),
			TableSchema {
				name: "users".to_string(),
				columns: BTreeMap::new(),
				indexes: Vec::new(),
				constraints: Vec::new(),
			},
		);

		let target = DatabaseSchema::default();

		let diff = SchemaDiff::new(current, target);
		assert!(diff.has_destructive_changes());
	}

	// ================================================================
	// generate_operations() tests (issue #3198 related)
	// ================================================================

	/// Helper to create a simple column schema
	fn col(name: &str, data_type: FieldType, nullable: bool) -> ColumnSchema {
		ColumnSchema {
			name: name.to_string(),
			data_type,
			nullable,
			default: None,
			primary_key: false,
			auto_increment: false,
		}
	}

	/// Helper to create a primary key column
	fn pk_col(name: &str) -> ColumnSchema {
		ColumnSchema {
			name: name.to_string(),
			data_type: FieldType::Integer,
			nullable: false,
			default: None,
			primary_key: true,
			auto_increment: true,
		}
	}

	/// Helper to create a table with given columns
	fn table_with_cols(name: &str, cols: Vec<(&str, ColumnSchema)>) -> TableSchema {
		let mut columns = BTreeMap::new();
		for (col_name, col_schema) in cols {
			columns.insert(col_name.to_string(), col_schema);
		}
		TableSchema {
			name: name.to_string(),
			columns,
			indexes: Vec::new(),
			constraints: Vec::new(),
		}
	}

	#[test]
	fn test_generate_operations_create_table() {
		// Arrange
		let current = DatabaseSchema::default();
		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"users".to_string(),
			table_with_cols(
				"users",
				vec![
					("id", pk_col("id")),
					("name", col("name", FieldType::VarChar(100), false)),
				],
			),
		);

		// Act
		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		// Assert
		assert_eq!(ops.len(), 1);
		assert!(
			matches!(&ops[0], Operation::CreateTable { name, .. } if name == "users"),
			"Should generate CreateTable for 'users'"
		);
	}

	#[test]
	fn test_generate_operations_drop_table() {
		// Arrange
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"old_table".to_string(),
			table_with_cols("old_table", vec![("id", pk_col("id"))]),
		);
		let target = DatabaseSchema::default();

		// Act
		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		// Assert
		assert_eq!(ops.len(), 1);
		assert!(
			matches!(&ops[0], Operation::DropTable { name } if name == "old_table"),
			"Should generate DropTable for 'old_table'"
		);
	}

	#[test]
	fn test_generate_operations_add_column() {
		// Arrange
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"users".to_string(),
			table_with_cols("users", vec![("id", pk_col("id"))]),
		);
		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"users".to_string(),
			table_with_cols(
				"users",
				vec![
					("id", pk_col("id")),
					("email", col("email", FieldType::VarChar(255), false)),
				],
			),
		);

		// Act
		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		// Assert
		assert_eq!(ops.len(), 1);
		assert!(
			matches!(&ops[0], Operation::AddColumn { table, column, .. }
				if table == "users" && column.name == "email"),
			"Should generate AddColumn for 'email' on 'users'"
		);
	}

	#[test]
	fn test_generate_operations_drop_column() {
		// Arrange
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"users".to_string(),
			table_with_cols(
				"users",
				vec![
					("id", pk_col("id")),
					("bio", col("bio", FieldType::Text, true)),
				],
			),
		);
		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"users".to_string(),
			table_with_cols("users", vec![("id", pk_col("id"))]),
		);

		// Act
		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		// Assert
		assert_eq!(ops.len(), 1);
		assert!(
			matches!(&ops[0], Operation::DropColumn { table, column }
				if table == "users" && column == "bio"),
			"Should generate DropColumn for 'bio' on 'users'"
		);
	}

	#[test]
	fn test_generate_operations_alter_column_type_change() {
		// Arrange: change 'price' from Integer to Float
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"items".to_string(),
			table_with_cols(
				"items",
				vec![
					("id", pk_col("id")),
					("price", col("price", FieldType::Integer, false)),
				],
			),
		);
		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"items".to_string(),
			table_with_cols(
				"items",
				vec![
					("id", pk_col("id")),
					("price", col("price", FieldType::Float, false)),
				],
			),
		);

		// Act
		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		// Assert
		assert_eq!(ops.len(), 1, "Should generate exactly one operation");
		match &ops[0] {
			Operation::AlterColumn {
				table,
				column,
				old_definition,
				new_definition,
				..
			} => {
				assert_eq!(table, "items");
				assert_eq!(column, "price");
				assert_eq!(
					old_definition.as_ref().unwrap().type_definition,
					FieldType::Integer,
					"Old definition should be Integer"
				);
				assert_eq!(
					new_definition.type_definition,
					FieldType::Float,
					"New definition should be Float"
				);
			}
			other => panic!("Expected AlterColumn, got {:?}", other),
		}
	}

	#[test]
	fn test_generate_operations_alter_column_nullability_change() {
		// Arrange: change 'email' from nullable to non-nullable
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"users".to_string(),
			table_with_cols(
				"users",
				vec![
					("id", pk_col("id")),
					("email", col("email", FieldType::VarChar(255), true)),
				],
			),
		);
		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"users".to_string(),
			table_with_cols(
				"users",
				vec![
					("id", pk_col("id")),
					("email", col("email", FieldType::VarChar(255), false)),
				],
			),
		);

		// Act
		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		// Assert
		assert_eq!(ops.len(), 1);
		match &ops[0] {
			Operation::AlterColumn {
				table,
				column,
				old_definition,
				new_definition,
				..
			} => {
				assert_eq!(table, "users");
				assert_eq!(column, "email");
				assert!(
					!old_definition.as_ref().unwrap().not_null,
					"Old should be nullable (not_null=false)"
				);
				assert!(
					new_definition.not_null,
					"New should be non-nullable (not_null=true)"
				);
			}
			other => panic!("Expected AlterColumn, got {:?}", other),
		}
	}

	#[test]
	fn test_generate_operations_add_index() {
		// Arrange
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"users".to_string(),
			table_with_cols(
				"users",
				vec![
					("id", pk_col("id")),
					("email", col("email", FieldType::VarChar(255), false)),
				],
			),
		);
		let mut target = DatabaseSchema::default();
		let mut target_table = table_with_cols(
			"users",
			vec![
				("id", pk_col("id")),
				("email", col("email", FieldType::VarChar(255), false)),
			],
		);
		target_table.indexes.push(IndexSchema {
			name: "idx_users_email".to_string(),
			columns: vec!["email".to_string()],
			unique: true,
		});
		target.tables.insert("users".to_string(), target_table);

		// Act
		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		// Assert
		assert_eq!(ops.len(), 1, "Should generate exactly one operation");
		match &ops[0] {
			Operation::CreateIndex {
				table,
				columns,
				unique,
				..
			} => {
				assert_eq!(table, "users");
				assert_eq!(columns, &vec!["email".to_string()]);
				assert!(unique, "Should be a unique index");
			}
			other => panic!("Expected CreateIndex, got {:?}", other),
		}
	}

	#[test]
	fn test_generate_operations_drop_index() {
		// Arrange
		let mut current = DatabaseSchema::default();
		let mut current_table = table_with_cols(
			"users",
			vec![
				("id", pk_col("id")),
				("email", col("email", FieldType::VarChar(255), false)),
			],
		);
		current_table.indexes.push(IndexSchema {
			name: "idx_users_email".to_string(),
			columns: vec!["email".to_string()],
			unique: false,
		});
		current.tables.insert("users".to_string(), current_table);

		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"users".to_string(),
			table_with_cols(
				"users",
				vec![
					("id", pk_col("id")),
					("email", col("email", FieldType::VarChar(255), false)),
				],
			),
		);

		// Act
		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		// Assert
		assert_eq!(ops.len(), 1);
		match &ops[0] {
			Operation::DropIndex { table, columns } => {
				assert_eq!(table, "users");
				assert_eq!(columns, &vec!["email".to_string()]);
			}
			other => panic!("Expected DropIndex, got {:?}", other),
		}
	}

	#[test]
	fn test_generate_operations_no_changes_returns_empty() {
		// Arrange: identical schemas
		let mut schema = DatabaseSchema::default();
		schema.tables.insert(
			"users".to_string(),
			table_with_cols("users", vec![("id", pk_col("id"))]),
		);

		// Act
		let diff = SchemaDiff::new(schema.clone(), schema);
		let ops = diff.generate_operations();

		// Assert
		assert!(
			ops.is_empty(),
			"Identical schemas should produce no operations"
		);
	}

	#[test]
	fn test_has_destructive_changes_index_drop() {
		// Arrange: dropping an index is destructive
		let mut current = DatabaseSchema::default();
		let mut current_table = table_with_cols("users", vec![("id", pk_col("id"))]);
		current_table.indexes.push(IndexSchema {
			name: "idx_email".to_string(),
			columns: vec!["email".to_string()],
			unique: false,
		});
		current.tables.insert("users".to_string(), current_table);

		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"users".to_string(),
			table_with_cols("users", vec![("id", pk_col("id"))]),
		);

		// Act & Assert
		let diff = SchemaDiff::new(current, target);
		assert!(
			diff.has_destructive_changes(),
			"Index removal should be flagged as destructive"
		);
	}

	#[test]
	fn test_has_destructive_changes_column_modify() {
		// Arrange: modifying a column type is destructive
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"items".to_string(),
			table_with_cols(
				"items",
				vec![
					("id", pk_col("id")),
					("price", col("price", FieldType::Integer, false)),
				],
			),
		);
		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"items".to_string(),
			table_with_cols(
				"items",
				vec![
					("id", pk_col("id")),
					("price", col("price", FieldType::Float, false)),
				],
			),
		);

		// Act & Assert
		let diff = SchemaDiff::new(current, target);
		assert!(
			diff.has_destructive_changes(),
			"Column type modification should be flagged as destructive"
		);
	}

	#[test]
	fn test_generate_operations_new_table_with_indexes() {
		// Arrange: new table with an index should generate both CreateTable and CreateIndex
		let current = DatabaseSchema::default();
		let mut target = DatabaseSchema::default();
		let mut table = table_with_cols(
			"users",
			vec![
				("id", pk_col("id")),
				("email", col("email", FieldType::VarChar(255), false)),
			],
		);
		table.indexes.push(IndexSchema {
			name: "idx_users_email".to_string(),
			columns: vec!["email".to_string()],
			unique: true,
		});
		target.tables.insert("users".to_string(), table);

		// Act
		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		// Assert: should have CreateTable + CreateIndex
		assert_eq!(ops.len(), 2, "Should generate 2 operations, got {:?}", ops);
		assert!(
			matches!(&ops[0], Operation::CreateTable { name, .. } if name == "users"),
			"First operation should be CreateTable"
		);
		assert!(
			matches!(&ops[1], Operation::CreateIndex { table, unique, .. }
				if table == "users" && *unique),
			"Second operation should be CreateIndex"
		);
	}

	// ================================================================
	// Constraint detection and generation tests (issue #3203)
	// ================================================================

	/// Helper to create a ConstraintSchema
	fn unique_constraint(name: &str, columns: &str) -> ConstraintSchema {
		ConstraintSchema {
			name: name.to_string(),
			constraint_type: "UNIQUE".to_string(),
			definition: columns.to_string(),
			foreign_key_info: None,
		}
	}

	fn check_constraint(name: &str, expression: &str) -> ConstraintSchema {
		ConstraintSchema {
			name: name.to_string(),
			constraint_type: "CHECK".to_string(),
			definition: expression.to_string(),
			foreign_key_info: None,
		}
	}

	fn fk_constraint(name: &str, col: &str, ref_table: &str, ref_col: &str) -> ConstraintSchema {
		ConstraintSchema {
			name: name.to_string(),
			constraint_type: "FOREIGN KEY".to_string(),
			definition: col.to_string(),
			foreign_key_info: Some(ForeignKeySchemaInfo {
				columns: vec![col.to_string()],
				referenced_table: ref_table.to_string(),
				referenced_columns: vec![ref_col.to_string()],
				on_delete: "CASCADE".to_string(),
				on_update: "NO ACTION".to_string(),
			}),
		}
	}

	#[test]
	fn test_detect_constraint_addition() {
		// Arrange
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"orders".to_string(),
			table_with_cols(
				"orders",
				vec![
					("id", pk_col("id")),
					("amount", col("amount", FieldType::Integer, false)),
				],
			),
		);

		let mut target = DatabaseSchema::default();
		let mut target_table = table_with_cols(
			"orders",
			vec![
				("id", pk_col("id")),
				("amount", col("amount", FieldType::Integer, false)),
			],
		);
		target_table
			.constraints
			.push(check_constraint("ck_amount_positive", "amount > 0"));
		target.tables.insert("orders".to_string(), target_table);

		// Act
		let diff = SchemaDiff::new(current, target);
		let result = diff.detect();

		// Assert
		assert_eq!(result.constraints_to_add.len(), 1);
		assert_eq!(result.constraints_to_add[0].0, "orders");
		assert_eq!(result.constraints_to_add[0].1.name, "ck_amount_positive");
		assert!(result.constraints_to_remove.is_empty());
	}

	#[test]
	fn test_detect_constraint_removal() {
		// Arrange
		let mut current = DatabaseSchema::default();
		let mut current_table = table_with_cols(
			"orders",
			vec![
				("id", pk_col("id")),
				("amount", col("amount", FieldType::Integer, false)),
			],
		);
		current_table
			.constraints
			.push(check_constraint("ck_amount_positive", "amount > 0"));
		current.tables.insert("orders".to_string(), current_table);

		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"orders".to_string(),
			table_with_cols(
				"orders",
				vec![
					("id", pk_col("id")),
					("amount", col("amount", FieldType::Integer, false)),
				],
			),
		);

		// Act
		let diff = SchemaDiff::new(current, target);
		let result = diff.detect();

		// Assert
		assert!(result.constraints_to_add.is_empty());
		assert_eq!(result.constraints_to_remove.len(), 1);
		assert_eq!(result.constraints_to_remove[0].1.name, "ck_amount_positive");
	}

	#[test]
	fn test_detect_constraint_modification() {
		// Arrange: changing CHECK expression is detected as remove old + add new
		let mut current = DatabaseSchema::default();
		let mut current_table = table_with_cols(
			"orders",
			vec![
				("id", pk_col("id")),
				("amount", col("amount", FieldType::Integer, false)),
			],
		);
		current_table
			.constraints
			.push(check_constraint("ck_amount", "amount > 0"));
		current.tables.insert("orders".to_string(), current_table);

		let mut target = DatabaseSchema::default();
		let mut target_table = table_with_cols(
			"orders",
			vec![
				("id", pk_col("id")),
				("amount", col("amount", FieldType::Integer, false)),
			],
		);
		target_table
			.constraints
			.push(check_constraint("ck_amount", "amount >= 0"));
		target.tables.insert("orders".to_string(), target_table);

		// Act
		let diff = SchemaDiff::new(current, target);
		let result = diff.detect();

		// Assert: old constraint removed, new added (same name, different definition)
		assert_eq!(result.constraints_to_remove.len(), 1);
		assert_eq!(result.constraints_to_add.len(), 1);
		assert_eq!(result.constraints_to_remove[0].1.definition, "amount > 0");
		assert_eq!(result.constraints_to_add[0].1.definition, "amount >= 0");
	}

	#[test]
	fn test_generate_operations_add_constraint() {
		// Arrange
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"orders".to_string(),
			table_with_cols(
				"orders",
				vec![
					("id", pk_col("id")),
					("amount", col("amount", FieldType::Integer, false)),
				],
			),
		);

		let mut target = DatabaseSchema::default();
		let mut target_table = table_with_cols(
			"orders",
			vec![
				("id", pk_col("id")),
				("amount", col("amount", FieldType::Integer, false)),
			],
		);
		target_table
			.constraints
			.push(check_constraint("ck_amount_positive", "amount > 0"));
		target.tables.insert("orders".to_string(), target_table);

		// Act
		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		// Assert
		assert_eq!(ops.len(), 1);
		match &ops[0] {
			Operation::AddConstraint {
				table,
				constraint_sql,
			} => {
				assert_eq!(table, "orders");
				assert!(
					constraint_sql.contains("ck_amount_positive"),
					"SQL should contain constraint name, got '{}'",
					constraint_sql
				);
				assert!(
					constraint_sql.contains("CHECK"),
					"SQL should contain CHECK keyword, got '{}'",
					constraint_sql
				);
				assert!(
					constraint_sql.contains("amount > 0"),
					"SQL should contain expression, got '{}'",
					constraint_sql
				);
			}
			other => panic!("Expected AddConstraint, got {:?}", other),
		}
	}

	#[test]
	fn test_generate_operations_drop_constraint() {
		// Arrange
		let mut current = DatabaseSchema::default();
		let mut current_table = table_with_cols(
			"orders",
			vec![
				("id", pk_col("id")),
				("amount", col("amount", FieldType::Integer, false)),
			],
		);
		current_table
			.constraints
			.push(unique_constraint("uq_orders_amount", "amount"));
		current.tables.insert("orders".to_string(), current_table);

		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"orders".to_string(),
			table_with_cols(
				"orders",
				vec![
					("id", pk_col("id")),
					("amount", col("amount", FieldType::Integer, false)),
				],
			),
		);

		// Act
		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		// Assert
		assert_eq!(ops.len(), 1);
		match &ops[0] {
			Operation::DropConstraint {
				table,
				constraint_name,
			} => {
				assert_eq!(table, "orders");
				assert_eq!(constraint_name, "uq_orders_amount");
			}
			other => panic!("Expected DropConstraint, got {:?}", other),
		}
	}

	#[test]
	fn test_generate_operations_add_unique_constraint() {
		// Arrange
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"users".to_string(),
			table_with_cols(
				"users",
				vec![
					("id", pk_col("id")),
					("email", col("email", FieldType::VarChar(255), false)),
				],
			),
		);

		let mut target = DatabaseSchema::default();
		let mut target_table = table_with_cols(
			"users",
			vec![
				("id", pk_col("id")),
				("email", col("email", FieldType::VarChar(255), false)),
			],
		);
		target_table
			.constraints
			.push(unique_constraint("uq_users_email", "email"));
		target.tables.insert("users".to_string(), target_table);

		// Act
		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		// Assert
		assert_eq!(ops.len(), 1);
		match &ops[0] {
			Operation::AddConstraint {
				table,
				constraint_sql,
			} => {
				assert_eq!(table, "users");
				assert!(constraint_sql.contains("UNIQUE"));
				assert!(constraint_sql.contains("email"));
			}
			other => panic!("Expected AddConstraint, got {:?}", other),
		}
	}

	#[test]
	fn test_generate_operations_add_foreign_key_constraint() {
		// Arrange
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"orders".to_string(),
			table_with_cols(
				"orders",
				vec![
					("id", pk_col("id")),
					("user_id", col("user_id", FieldType::Integer, false)),
				],
			),
		);

		let mut target = DatabaseSchema::default();
		let mut target_table = table_with_cols(
			"orders",
			vec![
				("id", pk_col("id")),
				("user_id", col("user_id", FieldType::Integer, false)),
			],
		);
		target_table
			.constraints
			.push(fk_constraint("fk_orders_user", "user_id", "users", "id"));
		target.tables.insert("orders".to_string(), target_table);

		// Act
		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		// Assert
		assert_eq!(ops.len(), 1);
		match &ops[0] {
			Operation::AddConstraint {
				table,
				constraint_sql,
			} => {
				assert_eq!(table, "orders");
				assert!(
					constraint_sql.contains("FOREIGN KEY"),
					"Should contain FOREIGN KEY, got '{}'",
					constraint_sql
				);
				assert!(constraint_sql.contains("REFERENCES users"));
				assert!(constraint_sql.contains("CASCADE"));
			}
			other => panic!("Expected AddConstraint, got {:?}", other),
		}
	}

	#[test]
	fn test_has_destructive_changes_constraint_drop() {
		// Arrange
		let mut current = DatabaseSchema::default();
		let mut current_table = table_with_cols("orders", vec![("id", pk_col("id"))]);
		current_table
			.constraints
			.push(check_constraint("ck_test", "id > 0"));
		current.tables.insert("orders".to_string(), current_table);

		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"orders".to_string(),
			table_with_cols("orders", vec![("id", pk_col("id"))]),
		);

		// Act & Assert
		let diff = SchemaDiff::new(current, target);
		assert!(
			diff.has_destructive_changes(),
			"Constraint removal should be flagged as destructive"
		);
	}

	#[test]
	fn test_unchanged_constraints_produce_no_operations() {
		// Arrange: same constraint on both sides
		let constraint = check_constraint("ck_amount", "amount > 0");

		let mut current = DatabaseSchema::default();
		let mut current_table = table_with_cols("orders", vec![("id", pk_col("id"))]);
		current_table.constraints.push(constraint.clone());
		current.tables.insert("orders".to_string(), current_table);

		let mut target = DatabaseSchema::default();
		let mut target_table = table_with_cols("orders", vec![("id", pk_col("id"))]);
		target_table.constraints.push(constraint);
		target.tables.insert("orders".to_string(), target_table);

		// Act
		let diff = SchemaDiff::new(current, target);
		let result = diff.detect();
		let ops = diff.generate_operations();

		// Assert
		assert!(result.constraints_to_add.is_empty());
		assert!(result.constraints_to_remove.is_empty());
		assert!(ops.is_empty());
	}

	#[test]
	fn test_no_destructive_changes_for_additions_only() {
		// Arrange: only adding tables/columns/indexes is NOT destructive
		let current = DatabaseSchema::default();
		let mut target = DatabaseSchema::default();
		let mut table = table_with_cols("users", vec![("id", pk_col("id"))]);
		table.indexes.push(IndexSchema {
			name: "idx_id".to_string(),
			columns: vec!["id".to_string()],
			unique: false,
		});
		target.tables.insert("users".to_string(), table);

		// Act & Assert
		let diff = SchemaDiff::new(current, target);
		assert!(
			!diff.has_destructive_changes(),
			"Additions only should NOT be flagged as destructive"
		);
	}
}
