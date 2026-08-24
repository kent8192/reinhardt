//! Schema diff detection
//!
//! Detects differences between current database schema and model definitions:
//! - Table additions/removals
//! - Column modifications
//! - Index changes
//! - Constraint changes

use super::ColumnDefinition;
use super::introspection;
use super::operations::{IndexType, Operation, SqlDialect};
use std::collections::{BTreeMap, BTreeSet};

/// Schema difference detector
pub struct SchemaDiff {
	/// Current database schema
	current_schema: DatabaseSchema,
	/// Target schema from models
	target_schema: DatabaseSchema,
	/// SQL dialect used when comparing introspected raw generated SQL.
	dialect: Option<SqlDialect>,
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
						generated: intro_col.generated,
					},
				);
			}

			let indexes: Vec<IndexSchema> = intro_table
				.indexes
				.values()
				.map(|idx| {
					#[cfg(feature = "pgvector")]
					{
						IndexSchema {
							name: idx.name.clone(),
							columns: idx.columns.clone(),
							unique: idx.unique,
							access_method: idx.access_method.clone(),
							index_type: idx.index_type,
							expressions: idx.expressions.clone(),
							operator_class: if idx.operator_class_is_default
								&& !matches!(
									idx.index_type,
									Some(IndexType::Hnsw { .. } | IndexType::Ivfflat { .. })
								) {
								None
							} else {
								idx.operator_class.clone()
							},
						}
					}
					#[cfg(not(feature = "pgvector"))]
					{
						IndexSchema {
							name: idx.name.clone(),
							columns: idx.columns.clone(),
							unique: idx.unique,
							access_method: idx.index_type.clone(),
							index_type: None,
							expressions: None,
							operator_class: None,
						}
					}
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
	/// Generated-column metadata
	pub generated: Option<super::GeneratedColumnDefinition>,
}

#[derive(Debug, Clone)]
struct GeneratedColumnDependent {
	name: String,
	old_column: ColumnSchema,
	new_column: Option<ColumnSchema>,
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
	/// Raw database access method
	pub access_method: Option<String>,
	/// Typed migration index method and options
	pub index_type: Option<IndexType>,
	/// Index expressions
	pub expressions: Option<Vec<String>>,
	/// PostgreSQL operator class
	pub operator_class: Option<String>,
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
			dialect: None,
		}
	}

	/// Create a schema diff detector that compares generated SQL for one dialect.
	pub fn with_dialect(
		current_schema: DatabaseSchema,
		target_schema: DatabaseSchema,
		dialect: SqlDialect,
	) -> Self {
		Self {
			current_schema,
			target_schema,
			dialect: Some(dialect),
		}
	}

	fn columns_equal(&self, current_col: &ColumnSchema, target_col: &ColumnSchema) -> bool {
		current_col.name == target_col.name
			&& current_col.data_type == target_col.data_type
			&& current_col.nullable == target_col.nullable
			&& current_col.default == target_col.default
			&& current_col.primary_key == target_col.primary_key
			&& current_col.auto_increment == target_col.auto_increment
			&& self.generated_columns_equal(&current_col.generated, &target_col.generated)
	}

	fn generated_columns_equal(
		&self,
		current: &Option<super::GeneratedColumnDefinition>,
		target: &Option<super::GeneratedColumnDefinition>,
	) -> bool {
		match (current.as_ref(), target.as_ref(), self.dialect.as_ref()) {
			(Some(current), Some(target), Some(dialect)) => current.eq_for_dialect(target, dialect),
			(Some(current), Some(target), None)
				if Self::is_typed_to_raw_generated_comparison(current, target) =>
			{
				false
			}
			_ => current == target,
		}
	}

	fn indexes_equal(&self, current: &IndexSchema, target: &IndexSchema) -> bool {
		current.name == target.name
			&& current.columns == target.columns
			&& current.unique == target.unique
			&& current.expressions == target.expressions
			&& Self::canonical_operator_class(current.operator_class.as_deref())
				== Self::canonical_operator_class(target.operator_class.as_deref())
			&& Self::effective_index_method(current)
				.eq_ignore_ascii_case(Self::effective_index_method(target))
			&& Self::approximate_index_options_equal(current, target)
	}

	fn canonical_operator_class(operator_class: Option<&str>) -> Option<&str> {
		operator_class.map(|name| name.rsplit('.').next().unwrap_or(name))
	}

	#[cfg(feature = "pgvector")]
	fn index_has_explicit_name(table: &str, index: &IndexSchema) -> bool {
		index.name != super::operations::default_index_name(table, &index.columns.join("_"))
	}

	fn create_index_operation(table: &str, index: &IndexSchema) -> Operation {
		#[cfg(feature = "pgvector")]
		if Self::index_has_explicit_name(table, index) {
			return Operation::CreateNamedIndex {
				table: table.to_string(),
				name: index.name.clone(),
				columns: index.columns.clone(),
				unique: index.unique,
				index_type: index.index_type,
				where_clause: None,
				concurrently: false,
				expressions: index.expressions.clone(),
				mysql_options: None,
				operator_class: index.operator_class.clone(),
			};
		}

		Operation::CreateIndex {
			table: table.to_string(),
			columns: index.columns.clone(),
			unique: index.unique,
			index_type: index.index_type,
			where_clause: None,
			concurrently: false,
			expressions: index.expressions.clone(),
			mysql_options: None,
			operator_class: index.operator_class.clone(),
		}
	}

	fn drop_index_operation(table: &str, index: &IndexSchema) -> Operation {
		#[cfg(feature = "pgvector")]
		if Self::index_has_explicit_name(table, index) {
			return Operation::DropNamedIndex {
				table: table.to_string(),
				name: index.name.clone(),
				columns: index.columns.clone(),
				unique: index.unique,
				index_type: index.index_type,
				where_clause: None,
				concurrently: false,
				expressions: index.expressions.clone(),
				mysql_options: None,
				operator_class: index.operator_class.clone(),
			};
		}

		Operation::DropIndex {
			table: table.to_string(),
			columns: index.columns.clone(),
		}
	}

	fn effective_index_method(index: &IndexSchema) -> &str {
		match index.index_type {
			Some(IndexType::Hash) => "hash",
			Some(IndexType::Gin) => "gin",
			Some(IndexType::Gist) => "gist",
			Some(IndexType::Brin) => "brin",
			Some(IndexType::Fulltext) => "fulltext",
			Some(IndexType::Spatial) => "spatial",
			#[cfg(feature = "pgvector")]
			Some(IndexType::Hnsw { .. }) => "hnsw",
			#[cfg(feature = "pgvector")]
			Some(IndexType::Ivfflat { .. }) => "ivfflat",
			Some(IndexType::BTree) | None => index.access_method.as_deref().unwrap_or("btree"),
		}
	}

	fn approximate_index_options_equal(current: &IndexSchema, target: &IndexSchema) -> bool {
		#[cfg(feature = "pgvector")]
		let current_is_approximate = matches!(
			current.index_type,
			Some(IndexType::Hnsw { .. } | IndexType::Ivfflat { .. })
		);
		#[cfg(not(feature = "pgvector"))]
		let current_is_approximate = false;
		#[cfg(feature = "pgvector")]
		let target_is_approximate = matches!(
			target.index_type,
			Some(IndexType::Hnsw { .. } | IndexType::Ivfflat { .. })
		);
		#[cfg(not(feature = "pgvector"))]
		let target_is_approximate = false;
		if current_is_approximate || target_is_approximate {
			current.index_type == target.index_type
		} else {
			true
		}
	}

	fn is_typed_to_raw_generated_comparison(
		current: &super::GeneratedColumnDefinition,
		target: &super::GeneratedColumnDefinition,
	) -> bool {
		let current_is_typed = current.typed_expr().is_some() && current.raw_sql.is_none();
		let target_is_typed = target.typed_expr().is_some() && target.raw_sql.is_none();
		let current_is_raw = current.raw_sql.is_some() && current.typed_expr().is_none();
		let target_is_raw = target.raw_sql.is_some() && target.typed_expr().is_none();

		(current_is_typed && target_is_raw) || (current_is_raw && target_is_typed)
	}

	fn column_definition_from_schema(
		schema: &DatabaseSchema,
		table_name: &str,
		column_name: &str,
		col: &ColumnSchema,
	) -> ColumnDefinition {
		ColumnDefinition {
			name: column_name.to_string(),
			type_definition: col.data_type.clone(),
			not_null: !col.nullable,
			default: col.default.as_ref().cloned(),
			unique: Self::column_has_unique(schema, table_name, column_name),
			primary_key: col.primary_key,
			auto_increment: Self::is_auto_increment(col),
			generated: col.generated.clone(),
			domain: None,
		}
	}

	fn generated_replacement_column_definition_from_schema(
		schema: &DatabaseSchema,
		table_name: &str,
		column_name: &str,
		col: &ColumnSchema,
	) -> ColumnDefinition {
		let mut definition =
			Self::column_definition_from_schema(schema, table_name, column_name, col);
		definition.unique = false;
		definition
	}

	fn generated_column_references_column(
		generated: &super::GeneratedColumnDefinition,
		column: &str,
	) -> bool {
		if let Some(expr) = generated.typed_expr() {
			return Self::schema_expr_references_column(&expr, column);
		}
		if let Some(raw_sql) = generated.raw_sql.as_deref() {
			return Self::expression_text_references_column(raw_sql, column);
		}
		generated
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

	fn constraint_schema_references_any_column(
		constraint: &ConstraintSchema,
		columns: &BTreeSet<String>,
	) -> bool {
		constraint
			.foreign_key_info
			.as_ref()
			.is_some_and(|fk| fk.columns.iter().any(|column| columns.contains(column)))
			|| columns.iter().any(|column| {
				Self::expression_text_references_column(&constraint.definition, column)
			})
	}

	fn index_schema_references_any_column(index: &IndexSchema, columns: &BTreeSet<String>) -> bool {
		index.columns.iter().any(|column| columns.contains(column))
			|| index.expressions.as_ref().is_some_and(|expressions| {
				expressions.iter().any(|expression| {
					columns
						.iter()
						.any(|column| Self::expression_text_references_column(expression, column))
				})
			})
	}

	fn recreated_columns_for_table(
		recreated_generated_columns: &BTreeSet<(String, String)>,
		table_name: &str,
	) -> BTreeSet<String> {
		recreated_generated_columns
			.iter()
			.filter(|(table, _)| table == table_name)
			.map(|(_, column)| column.clone())
			.collect()
	}

	fn emit_generated_column_replacement_dependencies(
		&self,
		operations: &mut Vec<Operation>,
		table_name: &str,
		affected_columns: &BTreeSet<String>,
	) {
		let Some(current_table) = self.current_schema.tables.get(table_name) else {
			return;
		};
		let Some(target_table) = self.target_schema.tables.get(table_name) else {
			return;
		};

		for index in target_table.indexes.iter().filter(|index| {
			Self::index_schema_references_any_column(index, affected_columns)
				&& current_table
					.indexes
					.iter()
					.any(|current_index| self.indexes_equal(current_index, index))
		}) {
			operations.push(Operation::CreateIndexRepair {
				table: table_name.to_string(),
				name: Some(index.name.clone()),
				columns: index.columns.clone(),
				unique: index.unique,
				index_type: index.index_type,
				where_clause: None,
				concurrently: false,
				expressions: index.expressions.clone(),
				mysql_options: None,
				operator_class: index.operator_class.clone(),
			});
		}

		for constraint in target_table.constraints.iter().filter(|constraint| {
			Self::constraint_schema_references_any_column(constraint, affected_columns)
				&& current_table
					.constraints
					.iter()
					.any(|current_constraint| current_constraint == *constraint)
		}) {
			operations.push(Operation::AddConstraintRepair {
				table: table_name.to_string(),
				constraint_sql: Self::constraint_schema_to_sql(constraint),
			});
		}
	}

	fn emit_generated_column_replacement_dependency_rollbacks(
		&self,
		operations: &mut Vec<Operation>,
		table_name: &str,
		affected_columns: &BTreeSet<String>,
	) {
		let Some(current_table) = self.current_schema.tables.get(table_name) else {
			return;
		};
		let Some(target_table) = self.target_schema.tables.get(table_name) else {
			return;
		};

		for index in current_table.indexes.iter().filter(|index| {
			Self::index_schema_references_any_column(index, affected_columns)
				&& target_table
					.indexes
					.iter()
					.any(|target_index| self.indexes_equal(index, target_index))
		}) {
			operations.push(Operation::RestoreIndexOnRollback {
				table: table_name.to_string(),
				name: Some(index.name.clone()),
				columns: index.columns.clone(),
				unique: index.unique,
				index_type: index.index_type,
				where_clause: None,
				concurrently: false,
				expressions: index.expressions.clone(),
				mysql_options: None,
				operator_class: index.operator_class.clone(),
			});
		}

		for constraint in current_table.constraints.iter().filter(|constraint| {
			Self::constraint_schema_references_any_column(constraint, affected_columns)
				&& target_table
					.constraints
					.iter()
					.any(|target_constraint| target_constraint == *constraint)
		}) {
			operations.push(Operation::RestoreConstraintOnRollback {
				table: table_name.to_string(),
				constraint_sql: Self::constraint_schema_to_sql(constraint),
			});
		}
	}

	fn order_added_columns_by_generated_dependencies(
		&self,
		columns_to_add: &[(String, String)],
	) -> Vec<(String, String)> {
		let mut remaining = columns_to_add.to_vec();
		let mut ordered = Vec::with_capacity(remaining.len());

		while !remaining.is_empty() {
			let next_index = remaining
				.iter()
				.enumerate()
				.find(|(_, (table_name, column_name))| {
					let Some(table) = self.target_schema.tables.get(table_name) else {
						return true;
					};
					let Some(column) = table.columns.get(column_name) else {
						return true;
					};
					let Some(generated) = column.generated.as_ref() else {
						return true;
					};

					!remaining.iter().any(|(other_table, other_column)| {
						other_table == table_name
							&& other_column != column_name
							&& Self::generated_column_references_column(generated, other_column)
					})
				})
				.map(|(index, _)| index);

			if let Some(index) = next_index {
				ordered.push(remaining.remove(index));
			} else {
				let cycle = remaining
					.iter()
					.map(|(_, column)| column.as_str())
					.collect::<Vec<_>>()
					.join(", ");
				panic!("generated-column dependency cycle detected among columns: {cycle}");
			}
		}

		ordered
	}

	fn order_table_columns_by_generated_dependencies(
		table_schema: &TableSchema,
	) -> Vec<(&String, &ColumnSchema)> {
		let mut remaining: Vec<_> = table_schema.columns.iter().collect();
		let mut ordered = Vec::with_capacity(remaining.len());

		while !remaining.is_empty() {
			let next_index = remaining
				.iter()
				.enumerate()
				.find(|(_, (column_name, column))| {
					let Some(generated) = column.generated.as_ref() else {
						return true;
					};

					!remaining.iter().any(|(other_column_name, _)| {
						other_column_name.as_str() != column_name.as_str()
							&& Self::generated_column_references_column(
								generated,
								other_column_name.as_str(),
							)
					})
				})
				.map(|(index, _)| index);

			if let Some(index) = next_index {
				ordered.push(remaining.remove(index));
			} else {
				let cycle = remaining
					.iter()
					.map(|(name, _)| name.as_str())
					.collect::<Vec<_>>()
					.join(", ");
				panic!("generated-column dependency cycle detected among columns: {cycle}");
			}
		}

		ordered
	}

	fn added_generated_column_references_modified_source(
		&self,
		columns_to_modify: &[(String, String, ColumnSchema, ColumnSchema)],
		table_name: &str,
		column_name: &str,
	) -> bool {
		let Some(table) = self.target_schema.tables.get(table_name) else {
			return false;
		};
		let Some(column) = table.columns.get(column_name) else {
			return false;
		};
		let Some(generated) = column.generated.as_ref() else {
			return false;
		};

		columns_to_modify
			.iter()
			.any(|(other_table_name, other_column_name, _, _)| {
				other_table_name == table_name
					&& other_column_name != column_name
					&& Self::generated_column_references_column(generated, other_column_name)
			})
	}

	fn modified_generated_column_references_other_modified_column(
		columns_to_modify: &[(String, String, ColumnSchema, ColumnSchema)],
		table_name: &str,
		column_name: &str,
		old_column: &ColumnSchema,
		new_column: &ColumnSchema,
	) -> bool {
		columns_to_modify
			.iter()
			.any(|(other_table_name, other_column_name, _, _)| {
				other_table_name == table_name
					&& other_column_name != column_name
					&& (old_column.generated.as_ref().is_some_and(|generated| {
						Self::generated_column_references_column(generated, other_column_name)
					}) || new_column.generated.as_ref().is_some_and(|generated| {
						Self::generated_column_references_column(generated, other_column_name)
					}))
			})
	}

	fn modified_generated_dependents_for_removed_source(
		columns_to_modify: &[(String, String, ColumnSchema, ColumnSchema)],
		table_name: &str,
		removed_column_name: &str,
	) -> Vec<(String, ColumnSchema, ColumnSchema)> {
		columns_to_modify
			.iter()
			.filter_map(
				|(other_table_name, other_column_name, old_column, new_column)| {
					if other_table_name == table_name
						&& other_column_name != removed_column_name
						&& old_column.generated != new_column.generated
						&& old_column.generated.as_ref().is_some_and(|generated| {
							Self::generated_column_references_column(generated, removed_column_name)
						}) {
						Some((
							other_column_name.clone(),
							old_column.clone(),
							new_column.clone(),
						))
					} else {
						None
					}
				},
			)
			.collect()
	}

	fn push_add_column_operation(
		&self,
		operations: &mut Vec<Operation>,
		table_name: &str,
		column_name: &str,
	) {
		if let Some(table_schema) = self.target_schema.tables.get(table_name)
			&& let Some(col_schema) = table_schema.columns.get(column_name)
		{
			operations.push(Operation::AddColumn {
				table: table_name.to_string(),
				column: Self::column_definition_from_schema(
					&self.target_schema,
					table_name,
					column_name,
					col_schema,
				),
				mysql_options: None,
			});
		}
	}

	fn validate_postgres_generated_add_column_dependencies(&self, operations: &[Operation]) {
		let Some(dialect) = self.dialect.as_ref() else {
			return;
		};
		if !matches!(dialect, SqlDialect::Postgres | SqlDialect::Cockroachdb) {
			return;
		}

		for operation in operations {
			let Operation::AddColumn { table, column, .. } = operation else {
				continue;
			};
			let Some(generated) = column.generated.as_ref() else {
				continue;
			};
			let Some(table_schema) = self.target_schema.tables.get(table) else {
				continue;
			};

			let generated_column_names = table_schema
				.columns
				.iter()
				.filter(|(name, schema)| name.as_str() != column.name && schema.generated.is_some())
				.map(|(name, _)| name.as_str());
			if let Some(referenced_column) =
				Operation::postgres_generated_column_dependency_violation(
					column.name.as_str(),
					generated,
					generated_column_names,
				) {
				panic!(
					"PostgreSQL-compatible generated column `{}` cannot reference generated column `{}`",
					column.name, referenced_column
				);
			}
		}
	}

	fn order_removed_columns_by_generated_dependencies(
		&self,
		columns_to_remove: &[(String, String)],
	) -> Vec<(String, String)> {
		let mut remaining = columns_to_remove.to_vec();
		let mut ordered = Vec::with_capacity(remaining.len());

		while !remaining.is_empty() {
			let next_index = remaining
				.iter()
				.enumerate()
				.find(|(_, (table_name, column_name))| {
					!remaining.iter().any(|(other_table, other_column)| {
						if other_table != table_name || other_column == column_name {
							return false;
						}
						self.current_schema
							.tables
							.get(other_table)
							.and_then(|table| table.columns.get(other_column))
							.and_then(|column| column.generated.as_ref())
							.is_some_and(|generated| {
								Self::generated_column_references_column(generated, column_name)
							})
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

	fn generated_column_dependents(
		&self,
		table_name: &str,
		column_name: &str,
	) -> Vec<GeneratedColumnDependent> {
		let Some(current_table) = self.current_schema.tables.get(table_name) else {
			return Vec::new();
		};
		let Some(target_table) = self.target_schema.tables.get(table_name) else {
			return Vec::new();
		};

		let mut affected_columns = BTreeSet::from([column_name.to_string()]);
		let mut dependents = Vec::new();

		loop {
			let mut added = false;
			for (dependent_name, current_column) in &current_table.columns {
				if affected_columns.contains(dependent_name) {
					continue;
				}
				let target_column = target_table.columns.get(dependent_name);
				let references_affected_column =
					current_column.generated.as_ref().is_some_and(|generated| {
						affected_columns.iter().any(|column| {
							Self::generated_column_references_column(generated, column)
						})
					}) || target_column.is_some_and(|target_column| {
						target_column.generated.as_ref().is_some_and(|generated| {
							affected_columns.iter().any(|column| {
								Self::generated_column_references_column(generated, column)
							})
						})
					});

				if references_affected_column {
					affected_columns.insert(dependent_name.clone());
					dependents.push(GeneratedColumnDependent {
						name: dependent_name.clone(),
						old_column: current_column.clone(),
						new_column: target_column.cloned(),
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
		let system_tables = ["reinhardt_migrations", "reinhardt_admin_history"];

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
						&& !self.columns_equal(current_col, target_col)
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
					if !current_table
						.indexes
						.iter()
						.any(|current_index| self.indexes_equal(current_index, target_index))
					{
						result
							.indexes_to_add
							.push((table_name_owned.clone(), target_index.clone()));
					}
				}

				for current_index in &current_table.indexes {
					if !target_table
						.indexes
						.iter()
						.any(|target_index| self.indexes_equal(current_index, target_index))
					{
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
				let columns: Vec<_> =
					Self::order_table_columns_by_generated_dependencies(table_schema)
						.into_iter()
						.map(|(name, col)| {
							Self::column_definition_from_schema(
								&self.target_schema,
								table_name,
								name,
								col,
							)
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
					operations.push(Self::create_index_operation(table_name, index));
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
		let ordered_columns_to_add =
			self.order_added_columns_by_generated_dependencies(&diff.columns_to_add);
		let mut deferred_columns_to_add = Vec::new();
		for (table_name, col_name) in ordered_columns_to_add {
			if self.added_generated_column_references_modified_source(
				&diff.columns_to_modify,
				&table_name,
				&col_name,
			) {
				deferred_columns_to_add.push((table_name, col_name));
			} else {
				self.push_add_column_operation(&mut operations, &table_name, &col_name);
			}
		}

		let removed_columns: BTreeSet<(String, String)> =
			diff.columns_to_remove.iter().cloned().collect();
		let mut recreated_generated_columns: BTreeSet<(String, String)> = BTreeSet::new();

		// Remove columns
		let ordered_columns_to_remove =
			self.order_removed_columns_by_generated_dependencies(&diff.columns_to_remove);
		for (table_name, col_name) in &ordered_columns_to_remove {
			for (dependent_name, old_column, new_column) in
				Self::modified_generated_dependents_for_removed_source(
					&diff.columns_to_modify,
					table_name,
					col_name,
				) {
				let dependent_key = (table_name.clone(), dependent_name.clone());
				if recreated_generated_columns.insert(dependent_key) {
					let affected_columns = BTreeSet::from([dependent_name.clone()]);
					self.emit_generated_column_replacement_dependency_rollbacks(
						&mut operations,
						table_name,
						&affected_columns,
					);
					operations.push(Operation::DropColumn {
						table: table_name.clone(),
						column: dependent_name.clone(),
						old_definition: Some(Self::column_definition_from_schema(
							&self.current_schema,
							table_name,
							&dependent_name,
							&old_column,
						)),
					});
					operations.push(Operation::AddColumn {
						table: table_name.clone(),
						column: Self::generated_replacement_column_definition_from_schema(
							&self.target_schema,
							table_name,
							&dependent_name,
							&new_column,
						),
						mysql_options: None,
					});
					self.emit_generated_column_replacement_dependencies(
						&mut operations,
						table_name,
						&affected_columns,
					);
				}
			}

			operations.push(Operation::DropColumn {
				table: table_name.clone(),
				column: col_name.clone(),
				old_definition: None,
			});
		}

		// Alter columns (type changes, nullability changes, etc.)
		for (table_name, col_name, old_col, new_col) in &diff.columns_to_modify {
			let recreation_key = (table_name.clone(), col_name.clone());
			if recreated_generated_columns.contains(&recreation_key) {
				continue;
			}

			let old_definition = Self::column_definition_from_schema(
				&self.current_schema,
				table_name,
				col_name,
				old_col,
			);
			let new_definition = Self::column_definition_from_schema(
				&self.target_schema,
				table_name,
				col_name,
				new_col,
			);
			let generated_change =
				!self.generated_columns_equal(&old_col.generated, &new_col.generated);
			if generated_change
				&& Self::modified_generated_column_references_other_modified_column(
					&diff.columns_to_modify,
					table_name,
					col_name,
					old_col,
					new_col,
				) {
				continue;
			}
			let dependent_generated_columns = self
				.generated_column_dependents(table_name, col_name)
				.into_iter()
				.filter(|dependent| {
					!removed_columns.contains(&(table_name.clone(), dependent.name.clone()))
				})
				.collect::<Vec<_>>();
			let source_change_requires_dependent_recreation =
				!generated_change && !dependent_generated_columns.is_empty();

			if generated_change || source_change_requires_dependent_recreation {
				let dependent_column_names: BTreeSet<String> = dependent_generated_columns
					.iter()
					.map(|dependent| dependent.name.clone())
					.collect();
				let affected_columns: BTreeSet<String> = if generated_change {
					std::iter::once(col_name.clone())
						.chain(dependent_column_names.iter().cloned())
						.collect()
				} else {
					dependent_column_names.clone()
				};
				recreated_generated_columns.insert(recreation_key);
				for dependent_name in &dependent_column_names {
					recreated_generated_columns
						.insert((table_name.clone(), dependent_name.clone()));
				}

				self.emit_generated_column_replacement_dependency_rollbacks(
					&mut operations,
					table_name,
					&affected_columns,
				);

				for dependent in dependent_generated_columns.iter().rev() {
					operations.push(Operation::DropColumn {
						table: table_name.clone(),
						column: dependent.name.clone(),
						old_definition: Some(Self::column_definition_from_schema(
							&self.current_schema,
							table_name,
							&dependent.name,
							&dependent.old_column,
						)),
					});
				}

				if generated_change {
					operations.push(Operation::DropColumn {
						table: table_name.clone(),
						column: col_name.clone(),
						old_definition: Some(old_definition),
					});
					operations.push(Operation::AddColumn {
						table: table_name.clone(),
						column: Self::generated_replacement_column_definition_from_schema(
							&self.target_schema,
							table_name,
							col_name,
							new_col,
						),
						mysql_options: None,
					});
				} else {
					operations.push(Operation::AlterColumn {
						table: table_name.clone(),
						column: col_name.clone(),
						old_definition: Some(old_definition),
						new_definition,
						mysql_options: None,
					});
				}

				for dependent in dependent_generated_columns {
					if let Some(new_column) = dependent.new_column {
						operations.push(Operation::AddColumn {
							table: table_name.clone(),
							column: Self::generated_replacement_column_definition_from_schema(
								&self.target_schema,
								table_name,
								&dependent.name,
								&new_column,
							),
							mysql_options: None,
						});
					}
				}

				self.emit_generated_column_replacement_dependencies(
					&mut operations,
					table_name,
					&affected_columns,
				);
			} else {
				operations.push(Operation::AlterColumn {
					table: table_name.clone(),
					column: col_name.clone(),
					old_definition: Some(old_definition),
					new_definition,
					mysql_options: None,
				});
			}
		}

		for (table_name, col_name) in &deferred_columns_to_add {
			self.push_add_column_operation(&mut operations, table_name, col_name);
		}

		// Remove indexes before adding replacements that retain the same physical name.
		for (table_name, index) in &diff.indexes_to_remove {
			let recreated_columns =
				Self::recreated_columns_for_table(&recreated_generated_columns, table_name);
			if Self::index_schema_references_any_column(index, &recreated_columns) {
				continue;
			}
			operations.push(Self::drop_index_operation(table_name, index));
		}

		// Add indexes
		for (table_name, index) in &diff.indexes_to_add {
			operations.push(Self::create_index_operation(table_name, index));
		}

		// Remove constraints before adding replacements with the same name.
		for (table_name, constraint) in &diff.constraints_to_remove {
			let recreated_columns =
				Self::recreated_columns_for_table(&recreated_generated_columns, table_name);
			if !recreated_columns.is_empty()
				&& Self::constraint_schema_references_any_column(constraint, &recreated_columns)
			{
				continue;
			}
			operations.push(Operation::DropConstraint {
				table: table_name.clone(),
				constraint_name: constraint.name.clone(),
			});
		}

		// Add constraints after obsolete definitions have been removed.
		for (table_name, constraint) in &diff.constraints_to_add {
			let constraint_sql = Self::constraint_schema_to_sql(constraint);
			operations.push(Operation::AddConstraint {
				table: table_name.clone(),
				constraint_sql,
			});
		}

		self.validate_postgres_generated_add_column_dependencies(&operations);

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
	use crate::migrations::operations::SqlDialect;
	use crate::migrations::{
		FieldType, GeneratedColumnDefinition, GeneratedStorage, IndexType, SchemaExpr,
	};

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

	#[rstest::rstest]
	fn admin_history_system_table_is_not_removed() {
		// Arrange
		let mut current = DatabaseSchema::default();
		for table_name in ["reinhardt_migrations", "reinhardt_admin_history", "users"] {
			current.tables.insert(
				table_name.to_string(),
				TableSchema {
					name: table_name.to_string(),
					columns: BTreeMap::new(),
					indexes: Vec::new(),
					constraints: Vec::new(),
				},
			);
		}

		// Act
		let result = SchemaDiff::new(current, DatabaseSchema::default()).detect();

		// Assert
		assert_eq!(result.tables_to_remove, ["users"]);
	}

	#[test]
	fn test_introspection_conversion_preserves_generated_metadata() {
		// Arrange
		let generated = GeneratedColumnDefinition::raw_sql("lower(name)", GeneratedStorage::Stored);
		let mut intro_columns = std::collections::HashMap::new();
		intro_columns.insert(
			"normalized_name".to_string(),
			introspection::ColumnInfo {
				name: "normalized_name".to_string(),
				column_type: FieldType::VarChar(255),
				nullable: false,
				default: None,
				auto_increment: false,
				identity_generation: None,
				generated: Some(generated.clone()),
			},
		);
		let mut intro_tables = std::collections::HashMap::new();
		intro_tables.insert(
			"users".to_string(),
			introspection::TableInfo {
				name: "users".to_string(),
				columns: intro_columns,
				indexes: std::collections::HashMap::new(),
				primary_key: Vec::new(),
				foreign_keys: Vec::new(),
				unique_constraints: Vec::new(),
				check_constraints: Vec::new(),
			},
		);

		// Act
		let schema = DatabaseSchema::from(introspection::DatabaseSchema {
			tables: intro_tables,
		});

		// Assert
		assert_eq!(
			schema
				.tables
				.get("users")
				.and_then(|table| table.columns.get("normalized_name"))
				.and_then(|column| column.generated.as_ref()),
			Some(&generated)
		);
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn vector_index_introspection_conversion_retains_default_operator_class() {
		// Arrange
		let mut intro_indexes = std::collections::HashMap::new();
		intro_indexes.insert(
			"source_embedding_cosine_hnsw".to_string(),
			introspection::IndexInfo {
				name: "source_embedding_cosine_hnsw".to_string(),
				columns: vec!["embedding".to_string()],
				unique: false,
				access_method: Some("hnsw".to_string()),
				index_type: Some(IndexType::Hnsw {
					m: Some(16),
					ef_construction: Some(64),
				}),
				expressions: None,
				operator_class: Some("vector_cosine_ops".to_string()),
				operator_class_is_default: true,
			},
		);
		let mut intro_tables = std::collections::HashMap::new();
		intro_tables.insert(
			"source".to_string(),
			introspection::TableInfo {
				name: "source".to_string(),
				columns: std::collections::HashMap::new(),
				indexes: intro_indexes,
				primary_key: Vec::new(),
				foreign_keys: Vec::new(),
				unique_constraints: Vec::new(),
				check_constraints: Vec::new(),
			},
		);

		// Act
		let schema = DatabaseSchema::from(introspection::DatabaseSchema {
			tables: intro_tables,
		});

		// Assert
		assert_eq!(
			schema.tables["source"].indexes,
			vec![IndexSchema {
				name: "source_embedding_cosine_hnsw".to_string(),
				columns: vec!["embedding".to_string()],
				unique: false,
				access_method: Some("hnsw".to_string()),
				index_type: Some(IndexType::Hnsw {
					m: Some(16),
					ef_construction: Some(64),
				}),
				expressions: None,
				operator_class: Some("vector_cosine_ops".to_string()),
			}]
		);
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn postgres_default_btree_operator_class_is_normalized_during_conversion() {
		// Arrange
		let mut intro_indexes = std::collections::HashMap::new();
		intro_indexes.insert(
			"idx_events_sequence".to_string(),
			introspection::IndexInfo {
				name: "idx_events_sequence".to_string(),
				columns: vec!["sequence".to_string()],
				unique: false,
				access_method: Some("btree".to_string()),
				index_type: Some(IndexType::BTree),
				expressions: None,
				operator_class: Some("int4_ops".to_string()),
				operator_class_is_default: true,
			},
		);
		let mut intro_tables = std::collections::HashMap::new();
		intro_tables.insert(
			"events".to_string(),
			introspection::TableInfo {
				name: "events".to_string(),
				columns: std::collections::HashMap::new(),
				indexes: intro_indexes,
				primary_key: Vec::new(),
				foreign_keys: Vec::new(),
				unique_constraints: Vec::new(),
				check_constraints: Vec::new(),
			},
		);

		// Act
		let schema = DatabaseSchema::from(introspection::DatabaseSchema {
			tables: intro_tables,
		});

		// Assert
		assert_eq!(schema.tables["events"].indexes[0].operator_class, None);
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
				generated: None,
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
			generated: None,
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
			generated: None,
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
	fn test_generate_operations_create_table_orders_generated_columns_by_dependencies() {
		// Arrange
		let current = DatabaseSchema::default();
		let source = col("source", FieldType::Integer, false);
		let mut z = col("z", FieldType::Integer, false);
		z.generated = Some(GeneratedColumnDefinition::raw_sql(
			"source + 1",
			GeneratedStorage::Stored,
		));
		let mut a = col("a", FieldType::Integer, false);
		a.generated = Some(GeneratedColumnDefinition::raw_sql(
			"z + 1",
			GeneratedStorage::Stored,
		));
		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"metrics".to_string(),
			table_with_cols("metrics", vec![("a", a), ("source", source), ("z", z)]),
		);

		// Act
		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		// Assert
		let Operation::CreateTable { columns, .. } = &ops[0] else {
			panic!("expected CreateTable operation, got: {ops:?}");
		};
		let column_names: Vec<_> = columns.iter().map(|column| column.name.as_str()).collect();
		assert_eq!(column_names, vec!["source", "z", "a"]);
	}

	#[test]
	#[should_panic(expected = "generated-column dependency cycle detected among columns")]
	fn test_generate_operations_create_table_rejects_generated_dependency_cycles() {
		let current = DatabaseSchema::default();
		let mut a = col("a", FieldType::Integer, false);
		a.generated = Some(GeneratedColumnDefinition::raw_sql(
			"b + 1",
			GeneratedStorage::Stored,
		));
		let mut b = col("b", FieldType::Integer, false);
		b.generated = Some(GeneratedColumnDefinition::raw_sql(
			"a + 1",
			GeneratedStorage::Stored,
		));
		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"metrics".to_string(),
			table_with_cols("metrics", vec![("a", a), ("b", b)]),
		);

		let _ = SchemaDiff::new(current, target).generate_operations();
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
	fn test_generate_operations_add_column_preserves_generated_metadata() {
		// Arrange
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"users".to_string(),
			table_with_cols("users", vec![("id", pk_col("id"))]),
		);
		let mut full_name = col("full_name", FieldType::VarChar(255), false);
		let generated = GeneratedColumnDefinition::raw_sql(
			"first_name || ' ' || last_name",
			GeneratedStorage::Stored,
		);
		full_name.generated = Some(generated.clone());

		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"users".to_string(),
			table_with_cols(
				"users",
				vec![("id", pk_col("id")), ("full_name", full_name)],
			),
		);

		// Act
		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		// Assert
		assert_eq!(ops.len(), 1);
		assert!(matches!(
			&ops[0],
			Operation::AddColumn { table, column, .. }
				if table == "users"
					&& column.name == "full_name"
			&& column.generated == Some(generated)
		));
	}

	#[test]
	#[should_panic(
		expected = "PostgreSQL-compatible generated column `search_name` cannot reference generated column `full_name`"
	)]
	fn test_generate_operations_postgres_rejects_added_generated_column_chain() {
		// Arrange
		let mut current = DatabaseSchema::default();
		let mut full_name = col("full_name", FieldType::VarChar(255), false);
		full_name.generated = Some(GeneratedColumnDefinition::typed(
			SchemaExpr::col("first_name"),
			"SchemaExpr::col(\"first_name\")",
			GeneratedStorage::Stored,
		));
		current.tables.insert(
			"users".to_string(),
			table_with_cols(
				"users",
				vec![("id", pk_col("id")), ("full_name", full_name.clone())],
			),
		);

		let mut search_name = col("search_name", FieldType::VarChar(255), false);
		search_name.generated = Some(GeneratedColumnDefinition::typed(
			SchemaExpr::col("full_name"),
			"SchemaExpr::col(\"full_name\")",
			GeneratedStorage::Stored,
		));
		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"users".to_string(),
			table_with_cols(
				"users",
				vec![
					("id", pk_col("id")),
					("full_name", full_name),
					("search_name", search_name),
				],
			),
		);

		// Act
		let diff = SchemaDiff::with_dialect(current, target, SqlDialect::Postgres);
		let _ = diff.generate_operations();
	}

	#[test]
	fn test_generate_operations_adds_generated_columns_after_new_sources() {
		// Arrange
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"users".to_string(),
			table_with_cols("users", vec![("id", pk_col("id"))]),
		);
		let last_name = col("last_name", FieldType::VarChar(100), false);
		let mut full_name = col("full_name", FieldType::VarChar(255), false);
		let generated =
			GeneratedColumnDefinition::raw_sql("last_name || ''", GeneratedStorage::Stored);
		full_name.generated = Some(generated.clone());

		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"users".to_string(),
			table_with_cols(
				"users",
				vec![
					("id", pk_col("id")),
					("full_name", full_name),
					("last_name", last_name),
				],
			),
		);

		// Act
		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		// Assert
		assert_eq!(ops.len(), 2, "unexpected operations: {ops:?}");
		assert!(matches!(
			&ops[0],
			Operation::AddColumn { table, column, .. }
				if table == "users" && column.name == "last_name"
		));
		assert!(matches!(
			&ops[1],
			Operation::AddColumn { table, column, .. }
				if table == "users"
					&& column.name == "full_name"
					&& column.generated == Some(generated)
		));
	}

	#[test]
	fn test_generate_operations_adds_generated_columns_after_source_alters() {
		// Arrange
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"users".to_string(),
			table_with_cols(
				"users",
				vec![("name", col("name", FieldType::Integer, false))],
			),
		);
		let mut full_name = col("full_name", FieldType::VarChar(255), false);
		let generated = GeneratedColumnDefinition::raw_sql("name", GeneratedStorage::Stored);
		full_name.generated = Some(generated.clone());
		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"users".to_string(),
			table_with_cols(
				"users",
				vec![
					("full_name", full_name),
					("name", col("name", FieldType::BigInteger, false)),
				],
			),
		);

		// Act
		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		// Assert
		assert_eq!(ops.len(), 2, "unexpected operations: {ops:?}");
		assert!(matches!(
			&ops[0],
			Operation::AlterColumn {
				table,
				column,
				new_definition,
				..
			} if table == "users"
				&& column == "name"
				&& matches!(new_definition.type_definition, FieldType::BigInteger)
		));
		assert!(matches!(
			&ops[1],
			Operation::AddColumn { table, column, .. }
				if table == "users"
					&& column.name == "full_name"
					&& column.generated == Some(generated)
		));
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
			matches!(&ops[0], Operation::DropColumn { table, column, .. }
				if table == "users" && column == "bio"),
			"Should generate DropColumn for 'bio' on 'users'"
		);
	}

	#[test]
	fn test_generate_operations_drops_generated_dependents_before_sources() {
		// Arrange
		let mut dependent = col("z", FieldType::Integer, false);
		let generated = GeneratedColumnDefinition::raw_sql("a + 1", GeneratedStorage::Stored);
		dependent.generated = Some(generated.clone());
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"metrics".to_string(),
			table_with_cols(
				"metrics",
				vec![
					("id", pk_col("id")),
					("a", col("a", FieldType::Integer, false)),
					("z", dependent),
				],
			),
		);
		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"metrics".to_string(),
			table_with_cols("metrics", vec![("id", pk_col("id"))]),
		);

		// Act
		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		// Assert
		assert_eq!(ops.len(), 2, "unexpected operations: {ops:?}");
		assert!(matches!(
			&ops[0],
			Operation::DropColumn { table, column, .. }
				if table == "metrics" && column == "z"
		));
		assert!(matches!(
			&ops[1],
			Operation::DropColumn { table, column, .. }
				if table == "metrics" && column == "a"
		));
	}

	#[test]
	fn test_generate_operations_drops_removed_generated_dependents_once_before_source_alter() {
		let mut dependent = col("z", FieldType::Integer, false);
		dependent.generated = Some(GeneratedColumnDefinition::raw_sql(
			"a + 1",
			GeneratedStorage::Stored,
		));
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"metrics".to_string(),
			table_with_cols(
				"metrics",
				vec![("a", col("a", FieldType::Integer, false)), ("z", dependent)],
			),
		);

		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"metrics".to_string(),
			table_with_cols(
				"metrics",
				vec![("a", col("a", FieldType::BigInteger, false))],
			),
		);

		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		assert_eq!(ops.len(), 2, "unexpected operations: {ops:?}");
		assert!(matches!(
			&ops[0],
			Operation::DropColumn { table, column, .. }
				if table == "metrics" && column == "z"
		));
		assert!(matches!(
			&ops[1],
			Operation::AlterColumn { table, column, .. }
				if table == "metrics" && column == "a"
		));
	}

	#[test]
	fn test_generate_operations_replaces_modified_generated_dependent_before_source_removal() {
		let mut current_dependent = col("b", FieldType::Integer, false);
		let old_generated = GeneratedColumnDefinition::raw_sql("a + 1", GeneratedStorage::Stored);
		current_dependent.generated = Some(old_generated.clone());
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"metrics".to_string(),
			table_with_cols(
				"metrics",
				vec![
					("a", col("a", FieldType::Integer, false)),
					("b", current_dependent),
				],
			),
		);

		let mut target_dependent = col("b", FieldType::Integer, false);
		let new_generated = GeneratedColumnDefinition::raw_sql("1", GeneratedStorage::Stored);
		target_dependent.generated = Some(new_generated.clone());
		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"metrics".to_string(),
			table_with_cols("metrics", vec![("b", target_dependent)]),
		);

		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		assert_eq!(ops.len(), 3, "unexpected operations: {ops:?}");
		assert!(matches!(
			&ops[0],
			Operation::DropColumn {
				table,
				column,
				old_definition: Some(old_definition),
			} if table == "metrics"
				&& column == "b"
				&& old_definition.generated == Some(old_generated)
		));
		assert!(matches!(
			&ops[1],
			Operation::AddColumn { table, column, .. }
				if table == "metrics"
					&& column.name == "b"
					&& column.generated == Some(new_generated)
		));
		assert!(matches!(
			&ops[2],
			Operation::DropColumn { table, column, .. }
				if table == "metrics" && column == "a"
		));
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
	fn test_generate_operations_replaces_generated_metadata_changes() {
		// Arrange
		let mut current_generated = col("slug", FieldType::VarChar(255), false);
		let old_generated =
			GeneratedColumnDefinition::raw_sql("lower(title)", GeneratedStorage::Stored);
		current_generated.generated = Some(old_generated.clone());
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"posts".to_string(),
			table_with_cols(
				"posts",
				vec![("id", pk_col("id")), ("slug", current_generated)],
			),
		);

		let mut target_generated = col("slug", FieldType::VarChar(255), false);
		let new_generated =
			GeneratedColumnDefinition::raw_sql("lower(display_title)", GeneratedStorage::Stored);
		target_generated.generated = Some(new_generated.clone());
		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"posts".to_string(),
			table_with_cols(
				"posts",
				vec![("id", pk_col("id")), ("slug", target_generated)],
			),
		);

		// Act
		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		// Assert
		assert_eq!(ops.len(), 2, "unexpected operations: {ops:?}");
		assert!(matches!(
			&ops[0],
			Operation::DropColumn {
				table,
				column,
				old_definition: Some(old_definition),
			} if table == "posts"
				&& column == "slug"
				&& old_definition.generated == Some(old_generated)
		));
		assert!(matches!(
			&ops[1],
			Operation::AddColumn { table, column, .. }
				if table == "posts"
					&& column.name == "slug"
					&& column.generated == Some(new_generated)
		));
	}

	#[test]
	fn test_generate_operations_recreates_generated_dependents_for_source_alters() {
		// Arrange
		let mut current_dependent = col("amount_text", FieldType::VarChar(20), false);
		let generated = GeneratedColumnDefinition::raw_sql("amount", GeneratedStorage::Stored);
		current_dependent.generated = Some(generated.clone());
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"invoices".to_string(),
			table_with_cols(
				"invoices",
				vec![
					("id", pk_col("id")),
					("amount", col("amount", FieldType::Integer, false)),
					("amount_text", current_dependent),
				],
			),
		);

		let mut target_dependent = col("amount_text", FieldType::VarChar(20), false);
		target_dependent.generated = Some(generated.clone());
		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"invoices".to_string(),
			table_with_cols(
				"invoices",
				vec![
					("id", pk_col("id")),
					("amount", col("amount", FieldType::BigInteger, false)),
					("amount_text", target_dependent),
				],
			),
		);

		// Act
		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		// Assert
		assert_eq!(ops.len(), 3, "unexpected operations: {ops:?}");
		assert!(matches!(
			&ops[0],
			Operation::DropColumn { table, column, .. }
				if table == "invoices" && column == "amount_text"
		));
		assert!(matches!(
			&ops[1],
			Operation::AlterColumn {
				table,
				column,
				new_definition,
				..
			} if table == "invoices"
				&& column == "amount"
				&& matches!(new_definition.type_definition, FieldType::BigInteger)
		));
		assert!(matches!(
			&ops[2],
			Operation::AddColumn { table, column, .. }
				if table == "invoices"
					&& column.name == "amount_text"
					&& column.generated == Some(generated)
		));
	}

	#[test]
	fn test_generate_operations_changed_generated_dependent_waits_for_source_alter() {
		// Arrange
		let mut current_dependent = col("full_name", FieldType::VarChar(201), false);
		let old_generated = GeneratedColumnDefinition::raw_sql("name", GeneratedStorage::Stored);
		current_dependent.generated = Some(old_generated.clone());
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"users".to_string(),
			table_with_cols(
				"users",
				vec![
					("full_name", current_dependent),
					("name", col("name", FieldType::Integer, false)),
				],
			),
		);

		let mut target_dependent = col("full_name", FieldType::VarChar(201), false);
		let new_generated =
			GeneratedColumnDefinition::raw_sql("lower(name)", GeneratedStorage::Stored);
		target_dependent.generated = Some(new_generated.clone());
		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"users".to_string(),
			table_with_cols(
				"users",
				vec![
					("full_name", target_dependent),
					("name", col("name", FieldType::BigInteger, false)),
				],
			),
		);

		// Act
		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		// Assert
		assert_eq!(ops.len(), 3, "unexpected operations: {ops:?}");
		assert!(matches!(
			&ops[0],
			Operation::DropColumn {
				table,
				column,
				old_definition: Some(old_definition),
			} if table == "users"
				&& column == "full_name"
				&& old_definition.generated == Some(old_generated)
		));
		assert!(matches!(
			&ops[1],
			Operation::AlterColumn {
				table,
				column,
				new_definition,
				..
			} if table == "users"
				&& column == "name"
				&& matches!(new_definition.type_definition, FieldType::BigInteger)
		));
		assert!(matches!(
			&ops[2],
			Operation::AddColumn { table, column, .. }
				if table == "users"
					&& column.name == "full_name"
					&& column.generated == Some(new_generated)
		));
	}

	#[test]
	fn test_generate_operations_repairs_generated_column_dependencies() {
		// Arrange
		let mut current_generated = col("full_name", FieldType::VarChar(201), false);
		let old_generated = GeneratedColumnDefinition::typed(
			SchemaExpr::col("name"),
			"SchemaExpr::col(\"name\")",
			GeneratedStorage::Stored,
		);
		current_generated.generated = Some(old_generated);
		let mut current_table = table_with_cols(
			"accounts_user",
			vec![("id", pk_col("id")), ("full_name", current_generated)],
		);
		current_table.indexes.push(IndexSchema {
			name: "idx_accounts_user_full_name".to_string(),
			columns: vec!["full_name".to_string()],
			unique: false,
			access_method: None,
			index_type: None,
			expressions: None,
			operator_class: None,
		});
		current_table.constraints.push(ConstraintSchema {
			name: "uq_accounts_user_full_name".to_string(),
			constraint_type: "UNIQUE".to_string(),
			definition: "full_name".to_string(),
			foreign_key_info: None,
		});
		let mut current = DatabaseSchema::default();
		current
			.tables
			.insert("accounts_user".to_string(), current_table);

		let mut target_generated = col("full_name", FieldType::VarChar(201), false);
		let new_generated = GeneratedColumnDefinition::typed(
			SchemaExpr::col("display_name"),
			"SchemaExpr::col(\"display_name\")",
			GeneratedStorage::Stored,
		);
		target_generated.generated = Some(new_generated);
		let mut target_table = table_with_cols(
			"accounts_user",
			vec![("id", pk_col("id")), ("full_name", target_generated)],
		);
		target_table.indexes.push(IndexSchema {
			name: "idx_accounts_user_full_name".to_string(),
			columns: vec!["full_name".to_string()],
			unique: false,
			access_method: None,
			index_type: None,
			expressions: None,
			operator_class: None,
		});
		target_table.constraints.push(ConstraintSchema {
			name: "uq_accounts_user_full_name".to_string(),
			constraint_type: "UNIQUE".to_string(),
			definition: "full_name".to_string(),
			foreign_key_info: None,
		});
		let mut target = DatabaseSchema::default();
		target
			.tables
			.insert("accounts_user".to_string(), target_table);

		// Act
		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		// Assert
		assert_eq!(ops.len(), 6, "unexpected operations: {ops:?}");
		assert!(matches!(
			&ops[0],
			Operation::RestoreIndexOnRollback {
				table,
				name: Some(name),
				columns,
				unique,
				..
			} if table == "accounts_user"
				&& name == "idx_accounts_user_full_name"
				&& columns == &vec!["full_name".to_string()]
				&& !unique
		));
		assert!(matches!(
			&ops[1],
			Operation::RestoreConstraintOnRollback {
				table,
				constraint_sql,
			} if table == "accounts_user"
				&& constraint_sql == "CONSTRAINT uq_accounts_user_full_name UNIQUE (full_name)"
		));
		assert!(matches!(
			&ops[2],
			Operation::DropColumn { table, column, .. }
				if table == "accounts_user" && column == "full_name"
		));
		assert!(matches!(
			&ops[3],
			Operation::AddColumn { table, column, .. }
				if table == "accounts_user" && column.name == "full_name"
		));
		assert!(matches!(
			&ops[4],
			Operation::CreateIndexRepair {
				table,
				name: Some(name),
				columns,
				unique,
				..
			} if table == "accounts_user"
				&& name == "idx_accounts_user_full_name"
				&& columns == &vec!["full_name".to_string()]
				&& !unique
		));
		assert!(matches!(
			&ops[5],
			Operation::AddConstraintRepair {
				table,
				constraint_sql,
			} if table == "accounts_user"
				&& constraint_sql == "CONSTRAINT uq_accounts_user_full_name UNIQUE (full_name)"
		));
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn vector_index_generated_column_repair_and_restore_preserve_expression_metadata() {
		// Arrange
		let mut current_embedding = col("embedding", FieldType::Vector { dimensions: 3 }, false);
		current_embedding.generated = Some(GeneratedColumnDefinition::raw_sql(
			"source_embedding",
			GeneratedStorage::Stored,
		));
		let mut current_table = table_with_cols("source", vec![("embedding", current_embedding)]);
		let index = IndexSchema {
			name: "source_normalized_embedding_hnsw".to_string(),
			columns: vec![],
			unique: false,
			access_method: Some("hnsw".to_string()),
			index_type: Some(IndexType::Hnsw {
				m: Some(16),
				ef_construction: Some(64),
			}),
			expressions: Some(vec!["normalize(embedding)".to_string()]),
			operator_class: Some("vector_cosine_ops".to_string()),
		};
		current_table.indexes.push(index.clone());
		let mut current = DatabaseSchema::default();
		current.tables.insert("source".to_string(), current_table);

		let mut target_embedding = col("embedding", FieldType::Vector { dimensions: 3 }, false);
		target_embedding.generated = Some(GeneratedColumnDefinition::raw_sql(
			"normalize(source_embedding)",
			GeneratedStorage::Stored,
		));
		let mut target_table = table_with_cols("source", vec![("embedding", target_embedding)]);
		target_table.indexes.push(index);
		let mut target = DatabaseSchema::default();
		target.tables.insert("source".to_string(), target_table);

		// Act
		let operations = SchemaDiff::new(current, target).generate_operations();

		// Assert
		assert!(operations.iter().any(|operation| matches!(
			operation,
			Operation::RestoreIndexOnRollback {
				name: Some(name),
				columns,
				index_type: Some(IndexType::Hnsw {
					m: Some(16),
					ef_construction: Some(64),
				}),
				expressions: Some(expressions),
				operator_class: Some(operator_class),
				..
			} if name == "source_normalized_embedding_hnsw"
				&& columns.is_empty()
				&& expressions == &vec!["normalize(embedding)".to_string()]
				&& operator_class == "vector_cosine_ops"
		)));
		assert!(operations.iter().any(|operation| matches!(
			operation,
			Operation::CreateIndexRepair {
				name: Some(name),
				columns,
				index_type: Some(IndexType::Hnsw {
					m: Some(16),
					ef_construction: Some(64),
				}),
				expressions: Some(expressions),
				operator_class: Some(operator_class),
				..
			} if name == "source_normalized_embedding_hnsw"
				&& columns.is_empty()
				&& expressions == &vec!["normalize(embedding)".to_string()]
				&& operator_class == "vector_cosine_ops"
		)));
	}

	#[test]
	fn test_generate_operations_replacement_avoids_inline_unique_for_composite_unique() {
		// Arrange
		let mut current_generated = col("slug", FieldType::VarChar(201), false);
		current_generated.generated = Some(GeneratedColumnDefinition::typed(
			SchemaExpr::col("old_name"),
			"SchemaExpr::col(\"old_name\")",
			GeneratedStorage::Stored,
		));
		let mut current_table = table_with_cols(
			"accounts_user",
			vec![
				("id", pk_col("id")),
				("scope", col("scope", FieldType::VarChar(64), false)),
				("slug", current_generated),
			],
		);
		current_table.constraints.push(ConstraintSchema {
			name: "uq_accounts_user_scope_slug".to_string(),
			constraint_type: "UNIQUE".to_string(),
			definition: "scope, slug".to_string(),
			foreign_key_info: None,
		});
		let mut current = DatabaseSchema::default();
		current
			.tables
			.insert("accounts_user".to_string(), current_table);

		let mut target_generated = col("slug", FieldType::VarChar(201), false);
		target_generated.generated = Some(GeneratedColumnDefinition::typed(
			SchemaExpr::col("display_name"),
			"SchemaExpr::col(\"display_name\")",
			GeneratedStorage::Stored,
		));
		let mut target_table = table_with_cols(
			"accounts_user",
			vec![
				("id", pk_col("id")),
				("scope", col("scope", FieldType::VarChar(64), false)),
				("slug", target_generated),
			],
		);
		target_table.constraints.push(ConstraintSchema {
			name: "uq_accounts_user_scope_slug".to_string(),
			constraint_type: "UNIQUE".to_string(),
			definition: "scope, slug".to_string(),
			foreign_key_info: None,
		});
		let mut target = DatabaseSchema::default();
		target
			.tables
			.insert("accounts_user".to_string(), target_table);

		// Act
		let diff = SchemaDiff::new(current, target);
		let ops = diff.generate_operations();

		// Assert
		let Operation::AddColumn { column, .. } = &ops[2] else {
			panic!("expected replacement AddColumn at index 2, got {ops:?}");
		};
		assert_eq!(column.name, "slug");
		assert!(
			!column.unique,
			"replacement AddColumn must not create a transient single-column UNIQUE"
		);
		assert!(matches!(
			ops.last(),
			Some(Operation::AddConstraintRepair { constraint_sql, .. })
				if constraint_sql == "CONSTRAINT uq_accounts_user_scope_slug UNIQUE (scope, slug)"
		));
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
			access_method: None,
			index_type: None,
			expressions: None,
			operator_class: None,
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

	#[cfg(feature = "pgvector")]
	#[test]
	fn vector_index_schema_diff_preserves_metadata_in_create_operation() {
		// Arrange
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"source".to_string(),
			table_with_cols(
				"source",
				vec![(
					"embedding",
					col("embedding", FieldType::Vector { dimensions: 3 }, false),
				)],
			),
		);
		let mut target = current.clone();
		target
			.tables
			.get_mut("source")
			.unwrap()
			.indexes
			.push(IndexSchema {
				name: "source_embedding_cosine_hnsw".to_string(),
				columns: vec!["embedding".to_string()],
				unique: false,
				access_method: Some("hnsw".to_string()),
				index_type: Some(IndexType::Hnsw {
					m: Some(16),
					ef_construction: Some(64),
				}),
				expressions: None,
				operator_class: Some("vector_cosine_ops".to_string()),
			});

		// Act
		let operations = SchemaDiff::new(current, target).generate_operations();

		// Assert
		assert_eq!(
			operations,
			vec![Operation::CreateNamedIndex {
				table: "source".to_string(),
				name: "source_embedding_cosine_hnsw".to_string(),
				columns: vec!["embedding".to_string()],
				unique: false,
				index_type: Some(IndexType::Hnsw {
					m: Some(16),
					ef_construction: Some(64),
				}),
				where_clause: None,
				concurrently: false,
				expressions: None,
				mysql_options: None,
				operator_class: Some("vector_cosine_ops".to_string()),
			}]
		);
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn vector_index_schema_diff_observes_typed_option_changes() {
		// Arrange
		let mut current_table = table_with_cols(
			"source",
			vec![(
				"embedding",
				col("embedding", FieldType::Vector { dimensions: 3 }, false),
			)],
		);
		let mut target_table = current_table.clone();
		let current_index = IndexSchema {
			name: "source_embedding_hnsw".to_string(),
			columns: vec!["embedding".to_string()],
			unique: false,
			access_method: Some("hnsw".to_string()),
			index_type: Some(IndexType::Hnsw {
				m: Some(8),
				ef_construction: Some(64),
			}),
			expressions: None,
			operator_class: Some("vector_l2_ops".to_string()),
		};
		let mut target_index = current_index.clone();
		target_index.index_type = Some(IndexType::Hnsw {
			m: Some(16),
			ef_construction: Some(64),
		});
		current_table.indexes.push(current_index);
		target_table.indexes.push(target_index);
		let mut current = DatabaseSchema::default();
		current.tables.insert("source".to_string(), current_table);
		let mut target = DatabaseSchema::default();
		target.tables.insert("source".to_string(), target_table);

		// Act
		let diff = SchemaDiff::new(current, target);
		let detected = diff.detect();
		let operations = diff.generate_operations();

		// Assert
		assert_eq!(detected.indexes_to_remove.len(), 1);
		assert_eq!(detected.indexes_to_add.len(), 1);
		match operations.as_slice() {
			[
				Operation::DropNamedIndex {
					name: dropped_name, ..
				},
				Operation::CreateNamedIndex {
					name,
					index_type: Some(IndexType::Hnsw { m: Some(16), .. }),
					..
				},
			] => {
				assert_eq!(name, "source_embedding_hnsw");
				assert_eq!(dropped_name, "source_embedding_hnsw");
			}
			other => panic!("Expected named index replacement operations, got {other:?}"),
		}
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn schema_qualified_vector_operator_class_matches_model_metadata() {
		// Arrange
		let mut current_table = table_with_cols(
			"source",
			vec![(
				"embedding",
				col("embedding", FieldType::Vector { dimensions: 3 }, false),
			)],
		);
		let index = IndexSchema {
			name: "source_embedding_hnsw".to_string(),
			columns: vec!["embedding".to_string()],
			unique: false,
			access_method: Some("hnsw".to_string()),
			index_type: Some(IndexType::Hnsw {
				m: Some(16),
				ef_construction: Some(64),
			}),
			expressions: None,
			operator_class: Some("public.vector_l2_ops".to_string()),
		};
		current_table.indexes.push(index.clone());
		let mut target_table = current_table.clone();
		target_table.indexes[0].operator_class = Some("vector_l2_ops".to_string());
		let current = DatabaseSchema {
			tables: BTreeMap::from([("source".to_string(), current_table)]),
		};
		let target = DatabaseSchema {
			tables: BTreeMap::from([("source".to_string(), target_table)]),
		};

		// Act
		let detected = SchemaDiff::new(current, target).detect();

		// Assert
		assert!(detected.indexes_to_add.is_empty());
		assert!(detected.indexes_to_remove.is_empty());
	}

	#[test]
	fn legacy_postgres_default_btree_index_matches_introspected_metadata() {
		// Arrange
		let mut current_table = table_with_cols(
			"events",
			vec![("sequence", col("sequence", FieldType::Integer, false))],
		);
		current_table.indexes.push(IndexSchema {
			name: "idx_events_sequence".to_string(),
			columns: vec!["sequence".to_string()],
			unique: false,
			access_method: Some("btree".to_string()),
			index_type: Some(IndexType::BTree),
			expressions: None,
			operator_class: None,
		});
		let mut target_table = current_table.clone();
		target_table.indexes[0].access_method = None;
		target_table.indexes[0].index_type = None;
		let mut current = DatabaseSchema::default();
		current.tables.insert("events".to_string(), current_table);
		let mut target = DatabaseSchema::default();
		target.tables.insert("events".to_string(), target_table);

		// Act
		let result = SchemaDiff::with_dialect(current, target, SqlDialect::Postgres).detect();

		// Assert
		assert!(result.indexes_to_add.is_empty());
		assert!(result.indexes_to_remove.is_empty());
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn legacy_postgres_multi_column_default_index_matches_introspected_metadata() {
		// Arrange
		let mut intro_indexes = std::collections::HashMap::new();
		intro_indexes.insert(
			"idx_events_sequence_name".to_string(),
			introspection::IndexInfo {
				name: "idx_events_sequence_name".to_string(),
				columns: vec!["sequence".to_string(), "name".to_string()],
				unique: false,
				access_method: Some("btree".to_string()),
				index_type: Some(IndexType::BTree),
				expressions: None,
				operator_class: Some("int4_ops".to_string()),
				operator_class_is_default: true,
			},
		);
		let mut intro_tables = std::collections::HashMap::new();
		intro_tables.insert(
			"events".to_string(),
			introspection::TableInfo {
				name: "events".to_string(),
				columns: std::collections::HashMap::new(),
				indexes: intro_indexes,
				primary_key: Vec::new(),
				foreign_keys: Vec::new(),
				unique_constraints: Vec::new(),
				check_constraints: Vec::new(),
			},
		);
		let current = DatabaseSchema::from(introspection::DatabaseSchema {
			tables: intro_tables,
		});
		let mut target = current.clone();
		let target_index = &mut target.tables.get_mut("events").unwrap().indexes[0];
		target_index.access_method = None;
		target_index.index_type = None;

		// Act
		let result = SchemaDiff::with_dialect(current, target, SqlDialect::Postgres).detect();

		// Assert
		assert!(result.indexes_to_add.is_empty());
		assert!(result.indexes_to_remove.is_empty());
	}

	#[test]
	fn legacy_mysql_default_index_matches_introspected_metadata() {
		// Arrange
		let mut current_table = table_with_cols(
			"events",
			vec![("sequence", col("sequence", FieldType::Integer, false))],
		);
		current_table.indexes.push(IndexSchema {
			name: "idx_events_sequence".to_string(),
			columns: vec!["sequence".to_string()],
			unique: false,
			access_method: Some("BTREE".to_string()),
			index_type: None,
			expressions: None,
			operator_class: None,
		});
		let mut target_table = current_table.clone();
		target_table.indexes[0].access_method = None;
		let mut current = DatabaseSchema::default();
		current.tables.insert("events".to_string(), current_table);
		let mut target = DatabaseSchema::default();
		target.tables.insert("events".to_string(), target_table);

		// Act
		let result = SchemaDiff::with_dialect(current, target, SqlDialect::Mysql).detect();

		// Assert
		assert!(result.indexes_to_add.is_empty());
		assert!(result.indexes_to_remove.is_empty());
	}

	#[test]
	fn custom_postgres_access_method_remains_significant() {
		// Arrange
		let mut current_table = table_with_cols(
			"events",
			vec![("payload", col("payload", FieldType::Json, false))],
		);
		current_table.indexes.push(IndexSchema {
			name: "idx_events_payload".to_string(),
			columns: vec!["payload".to_string()],
			unique: false,
			access_method: Some("gin".to_string()),
			index_type: Some(IndexType::Gin),
			expressions: None,
			operator_class: None,
		});
		let mut target_table = current_table.clone();
		target_table.indexes[0].access_method = None;
		target_table.indexes[0].index_type = None;
		let mut current = DatabaseSchema::default();
		current.tables.insert("events".to_string(), current_table);
		let mut target = DatabaseSchema::default();
		target.tables.insert("events".to_string(), target_table);

		// Act
		let result = SchemaDiff::with_dialect(current, target, SqlDialect::Postgres).detect();

		// Assert
		assert_eq!(result.indexes_to_add.len(), 1);
		assert_eq!(result.indexes_to_remove.len(), 1);
	}

	#[test]
	fn custom_postgres_operator_class_remains_significant() {
		// Arrange
		let mut current_table = table_with_cols(
			"events",
			vec![("sequence", col("sequence", FieldType::Integer, false))],
		);
		current_table.indexes.push(IndexSchema {
			name: "idx_events_sequence".to_string(),
			columns: vec!["sequence".to_string()],
			unique: false,
			access_method: Some("btree".to_string()),
			index_type: Some(IndexType::BTree),
			expressions: None,
			operator_class: Some("custom_int4_ops".to_string()),
		});
		let mut target_table = current_table.clone();
		target_table.indexes[0].access_method = None;
		target_table.indexes[0].index_type = None;
		target_table.indexes[0].operator_class = None;
		let mut current = DatabaseSchema::default();
		current.tables.insert("events".to_string(), current_table);
		let mut target = DatabaseSchema::default();
		target.tables.insert("events".to_string(), target_table);

		// Act
		let result = SchemaDiff::with_dialect(current, target, SqlDialect::Postgres).detect();

		// Assert
		assert_eq!(result.indexes_to_add.len(), 1);
		assert_eq!(result.indexes_to_remove.len(), 1);
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn hnsw_and_ivfflat_access_methods_remain_distinct() {
		// Arrange
		let mut current_table = table_with_cols(
			"source",
			vec![(
				"embedding",
				col("embedding", FieldType::Vector { dimensions: 3 }, false),
			)],
		);
		current_table.indexes.push(IndexSchema {
			name: "source_embedding_approximate".to_string(),
			columns: vec!["embedding".to_string()],
			unique: false,
			access_method: Some("hnsw".to_string()),
			index_type: Some(IndexType::Hnsw {
				m: Some(16),
				ef_construction: Some(64),
			}),
			expressions: None,
			operator_class: Some("vector_l2_ops".to_string()),
		});
		let mut target_table = current_table.clone();
		target_table.indexes[0].access_method = Some("ivfflat".to_string());
		target_table.indexes[0].index_type = Some(IndexType::Ivfflat { lists: Some(100) });
		let mut current = DatabaseSchema::default();
		current.tables.insert("source".to_string(), current_table);
		let mut target = DatabaseSchema::default();
		target.tables.insert("source".to_string(), target_table);

		// Act
		let result = SchemaDiff::with_dialect(current, target, SqlDialect::Postgres).detect();

		// Assert
		assert_eq!(result.indexes_to_add.len(), 1);
		assert_eq!(result.indexes_to_remove.len(), 1);
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
			access_method: None,
			index_type: None,
			expressions: None,
			operator_class: None,
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
	fn test_detect_generated_sql_uses_configured_dialect() {
		let mut current_full_name = col("full_name", FieldType::VarChar(255), false);
		current_full_name.generated = Some(GeneratedColumnDefinition::raw_sql(
			r#"CONCAT(`first_name`, `last_name`)"#,
			GeneratedStorage::Stored,
		));
		let mut current = DatabaseSchema::default();
		current.tables.insert(
			"users".to_string(),
			table_with_cols(
				"users",
				vec![
					(
						"first_name",
						col("first_name", FieldType::VarChar(255), false),
					),
					("full_name", current_full_name),
					(
						"last_name",
						col("last_name", FieldType::VarChar(255), false),
					),
				],
			),
		);

		let mut target_full_name = col("full_name", FieldType::VarChar(255), false);
		target_full_name.generated = Some(GeneratedColumnDefinition::typed(
			SchemaExpr::concat([SchemaExpr::col("first_name"), SchemaExpr::col("last_name")]),
			"SchemaExpr::concat([SchemaExpr::col(\"first_name\"), SchemaExpr::col(\"last_name\")])",
			GeneratedStorage::Stored,
		));
		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"users".to_string(),
			table_with_cols(
				"users",
				vec![
					(
						"first_name",
						col("first_name", FieldType::VarChar(255), false),
					),
					("full_name", target_full_name),
					(
						"last_name",
						col("last_name", FieldType::VarChar(255), false),
					),
				],
			),
		);

		let broad_diff = SchemaDiff::new(current.clone(), target.clone());
		assert_eq!(broad_diff.detect().columns_to_modify.len(), 1);

		let mysql_diff =
			SchemaDiff::with_dialect(current.clone(), target.clone(), SqlDialect::Mysql);
		assert!(mysql_diff.detect().columns_to_modify.is_empty());

		let postgres_diff =
			SchemaDiff::with_dialect(current.clone(), target.clone(), SqlDialect::Postgres);
		let result = postgres_diff.detect();

		assert_eq!(result.columns_to_modify.len(), 1);
		assert_eq!(
			(
				result.columns_to_modify[0].0.as_str(),
				result.columns_to_modify[0].1.as_str()
			),
			("users", "full_name")
		);

		let postgres_ops =
			SchemaDiff::with_dialect(current, target, SqlDialect::Postgres).generate_operations();
		assert!(postgres_ops.iter().any(|operation| matches!(
			operation,
			Operation::DropColumn { table, column, .. }
				if table == "users" && column == "full_name"
		)));
		assert!(postgres_ops.iter().any(|operation| matches!(
			operation,
			Operation::AddColumn { table, column, .. }
				if table == "users" && column.name == "full_name"
		)));
		assert!(!postgres_ops.iter().any(|operation| matches!(
			operation,
			Operation::AlterColumn { table, column, .. }
				if table == "users" && column == "full_name"
		)));
	}

	#[test]
	fn test_generate_operations_suppresses_stale_dependency_drops_after_generated_replacement() {
		let mut current_full_name = col("full_name", FieldType::VarChar(201), false);
		current_full_name.generated = Some(GeneratedColumnDefinition::typed(
			SchemaExpr::col("first_name"),
			"SchemaExpr::col(\"first_name\")",
			GeneratedStorage::Stored,
		));
		let mut current_table = table_with_cols(
			"users",
			vec![
				("id", pk_col("id")),
				(
					"first_name",
					col("first_name", FieldType::VarChar(100), false),
				),
				("full_name", current_full_name),
			],
		);
		current_table.indexes.push(IndexSchema {
			name: "idx_users_full_name".to_string(),
			columns: vec!["full_name".to_string()],
			unique: false,
			access_method: None,
			index_type: None,
			expressions: None,
			operator_class: None,
		});
		current_table.constraints.push(check_constraint(
			"ck_users_full_name",
			"length(full_name) > 0",
		));
		let mut current = DatabaseSchema::default();
		current.tables.insert("users".to_string(), current_table);

		let mut target_full_name = col("full_name", FieldType::VarChar(201), false);
		target_full_name.generated = Some(GeneratedColumnDefinition::typed(
			SchemaExpr::col("display_name"),
			"SchemaExpr::col(\"display_name\")",
			GeneratedStorage::Stored,
		));
		let mut target = DatabaseSchema::default();
		target.tables.insert(
			"users".to_string(),
			table_with_cols(
				"users",
				vec![
					("id", pk_col("id")),
					(
						"display_name",
						col("display_name", FieldType::VarChar(100), false),
					),
					(
						"first_name",
						col("first_name", FieldType::VarChar(100), false),
					),
					("full_name", target_full_name),
				],
			),
		);

		let operations = SchemaDiff::new(current, target).generate_operations();

		assert!(operations.iter().any(|operation| matches!(
			operation,
			Operation::DropColumn { table, column, .. }
				if table == "users" && column == "full_name"
		)));
		assert!(operations.iter().any(|operation| matches!(
			operation,
			Operation::AddColumn { table, column, .. }
				if table == "users" && column.name == "full_name"
		)));
		assert!(
			operations
				.iter()
				.all(|operation| !matches!(operation, Operation::DropIndex { .. })),
			"generated replacement should not emit stale DropIndex: {operations:?}"
		);
		assert!(
			operations
				.iter()
				.all(|operation| !matches!(operation, Operation::DropConstraint { .. })),
			"generated replacement should not emit stale DropConstraint: {operations:?}"
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
			access_method: None,
			index_type: None,
			expressions: None,
			operator_class: None,
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
			access_method: None,
			index_type: None,
			expressions: None,
			operator_class: None,
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
		let operations = diff.generate_operations();
		assert!(matches!(
			operations.as_slice(),
			[
				Operation::DropConstraint { constraint_name, .. },
				Operation::AddConstraint { constraint_sql, .. },
			] if constraint_name == "ck_amount"
				&& constraint_sql.contains("amount >= 0")
		));
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
				..
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
			access_method: None,
			index_type: None,
			expressions: None,
			operator_class: None,
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
