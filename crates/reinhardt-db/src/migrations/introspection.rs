//! Database schema introspection
//!
//! This module provides functionality to read the current database schema
//! and extract table definitions, column metadata, indexes, and constraints.

use async_trait::async_trait;
use std::collections::{HashMap, HashSet};

use super::{MigrationError, Result};
use crate::backends::{DatabaseConnection, DatabaseType};

/// Schema information extracted from a database
#[derive(Debug, Clone, PartialEq)]
pub struct DatabaseSchema {
	/// All tables in the schema
	pub tables: HashMap<String, TableInfo>,
}

/// Options controlling the schema objects selected for inspectdb output.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InspectDbOptions {
	/// Exact table names to include. An empty list includes every selected object.
	pub tables: Vec<String>,
	/// Whether database views should be included with tables.
	pub include_views: bool,
	/// Whether PostgreSQL partitions should be included with tables.
	pub include_partitions: bool,
}

/// Table metadata
#[derive(Debug, Clone, PartialEq)]
pub struct TableInfo {
	/// Table name
	pub name: String,
	/// Columns in the table
	pub columns: HashMap<String, ColumnInfo>,
	/// Indexes on the table
	pub indexes: HashMap<String, IndexInfo>,
	/// Primary key columns
	pub primary_key: Vec<String>,
	/// Foreign key constraints
	pub foreign_keys: Vec<ForeignKeyInfo>,
	/// Unique constraints (excluding unique indexes)
	pub unique_constraints: Vec<UniqueConstraintInfo>,
	/// CHECK constraints
	pub check_constraints: Vec<CheckConstraintInfo>,
}

/// Column metadata
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnInfo {
	/// Column name
	pub name: String,
	/// Column type
	pub column_type: super::FieldType,
	/// Whether the column is nullable
	pub nullable: bool,
	/// Default value expression
	pub default: Option<String>,
	/// Whether this is an auto-increment column
	pub auto_increment: bool,
	/// Generated-column metadata, when the backend exposes it.
	pub generated: Option<super::GeneratedColumnDefinition>,
}

/// Index metadata
#[derive(Debug, Clone, PartialEq)]
pub struct IndexInfo {
	/// Index name
	pub name: String,
	/// Columns in the index (in order)
	pub columns: Vec<String>,
	/// Whether the index is unique
	pub unique: bool,
	/// Raw database access method (e.g., btree, hash, hnsw)
	#[cfg(feature = "pgvector")]
	pub access_method: Option<String>,
	/// Typed migration index method and options when representable
	#[cfg(feature = "pgvector")]
	pub index_type: Option<super::IndexType>,
	/// Raw database index method (e.g., BTREE, HASH).
	#[cfg(not(feature = "pgvector"))]
	pub index_type: Option<String>,
	/// Index expressions, when the index does not target plain columns
	#[cfg(feature = "pgvector")]
	pub expressions: Option<Vec<String>>,
	/// PostgreSQL operator class for the indexed target
	#[cfg(feature = "pgvector")]
	pub operator_class: Option<String>,
	/// Whether PostgreSQL reports the operator class as the access method default
	#[cfg(feature = "pgvector")]
	pub operator_class_is_default: bool,
}

#[cfg(feature = "pgvector")]
fn parse_postgres_index_type(
	access_method: &str,
	reloptions: &[String],
) -> Result<Option<super::IndexType>> {
	use super::IndexType;

	match access_method {
		"btree" => Ok(Some(IndexType::BTree)),
		"hash" => Ok(Some(IndexType::Hash)),
		"gin" => Ok(Some(IndexType::Gin)),
		"gist" => Ok(Some(IndexType::Gist)),
		"brin" => Ok(Some(IndexType::Brin)),
		"hnsw" => {
			let mut m = None;
			let mut ef_construction = None;
			for option in reloptions {
				let Some((name, value)) = option.split_once('=') else {
					return Err(MigrationError::IntrospectionError(format!(
						"invalid PostgreSQL hnsw reloption {option}"
					)));
				};
				match name {
					"m" => {
						m = Some(value.parse::<u16>().map_err(|_| {
							MigrationError::IntrospectionError(format!(
								"invalid PostgreSQL hnsw reloption {option}"
							))
						})?);
					}
					"ef_construction" => {
						ef_construction = Some(value.parse::<u16>().map_err(|_| {
							MigrationError::IntrospectionError(format!(
								"invalid PostgreSQL hnsw reloption {option}"
							))
						})?);
					}
					_ => {
						return Err(MigrationError::IntrospectionError(format!(
							"unsupported PostgreSQL hnsw reloption {option}"
						)));
					}
				}
			}
			Ok(Some(IndexType::Hnsw { m, ef_construction }))
		}
		"ivfflat" => {
			let mut lists = None;
			for option in reloptions {
				let Some((name, value)) = option.split_once('=') else {
					return Err(MigrationError::IntrospectionError(format!(
						"invalid PostgreSQL ivfflat reloption {option}"
					)));
				};
				if name != "lists" {
					return Err(MigrationError::IntrospectionError(format!(
						"unsupported PostgreSQL ivfflat reloption {option}"
					)));
				}
				lists = Some(value.parse::<u32>().map_err(|_| {
					MigrationError::IntrospectionError(format!(
						"invalid PostgreSQL ivfflat reloption {option}"
					))
				})?);
			}
			Ok(Some(IndexType::Ivfflat { lists }))
		}
		_ => Ok(None),
	}
}

#[cfg(feature = "pgvector")]
fn resolve_postgres_operator_class(
	index_name: &str,
	index_type: Option<super::IndexType>,
	operator_classes: &[String],
	operator_class_defaults: &[bool],
) -> Result<(Option<String>, bool)> {
	if operator_classes.is_empty() || operator_classes.len() != operator_class_defaults.len() {
		return Err(MigrationError::IntrospectionError(format!(
			"PostgreSQL index {index_name} returned inconsistent operator class metadata"
		)));
	}

	if matches!(
		index_type,
		Some(super::IndexType::Hnsw { .. } | super::IndexType::Ivfflat { .. })
	) && operator_classes.len() != 1
	{
		return Err(MigrationError::IntrospectionError(format!(
			"PostgreSQL approximate index {index_name} must have exactly one key"
		)));
	}

	let canonical_operator_classes = operator_classes
		.iter()
		.map(|operator_class| {
			operator_class
				.rsplit('.')
				.next()
				.unwrap_or(operator_class.as_str())
		})
		.collect::<Vec<_>>();
	let first_operator_class = canonical_operator_classes[0];
	let first_is_default = operator_class_defaults[0];
	let all_default = operator_class_defaults.iter().all(|is_default| *is_default);
	let homogeneous = canonical_operator_classes
		.iter()
		.all(|operator_class| *operator_class == first_operator_class)
		&& operator_class_defaults
			.iter()
			.all(|is_default| *is_default == first_is_default);
	if operator_classes.len() > 1 && !all_default && !homogeneous {
		// The migration schema represents only one operator class per index. Preserve the
		// otherwise valid index metadata without claiming a lossy representative class.
		return Ok((None, false));
	}

	Ok((Some(first_operator_class.to_string()), all_default))
}

#[cfg(feature = "pgvector")]
fn validate_postgres_index_key_kinds(
	index_name: &str,
	column_key_count: i64,
	expression_key_count: i64,
) -> Result<()> {
	if column_key_count > 0 && expression_key_count > 0 {
		return Err(MigrationError::IntrospectionError(format!(
			"PostgreSQL index {index_name} mixes column and expression keys, which cannot be represented"
		)));
	}
	Ok(())
}

/// Foreign key constraint
#[derive(Debug, Clone, PartialEq)]
pub struct ForeignKeyInfo {
	/// Constraint name
	pub name: String,
	/// Columns in this table
	pub columns: Vec<String>,
	/// Referenced table
	pub referenced_table: String,
	/// Referenced columns
	pub referenced_columns: Vec<String>,
	/// ON DELETE action
	pub on_delete: Option<String>,
	/// ON UPDATE action
	pub on_update: Option<String>,
}

/// Unique constraint
#[derive(Debug, Clone, PartialEq)]
pub struct UniqueConstraintInfo {
	/// Constraint name
	pub name: String,
	/// Columns in the constraint
	pub columns: Vec<String>,
}

/// CHECK constraint
#[derive(Debug, Clone, PartialEq)]
pub struct CheckConstraintInfo {
	/// Constraint name (None for anonymous CHECK constraints)
	pub name: Option<String>,
	/// CHECK expression (without the CHECK keyword and outer parentheses)
	pub expression: String,
}

/// Trait for database-specific schema introspection
#[async_trait]
pub trait DatabaseIntrospector: Send + Sync {
	/// Read the complete database schema
	async fn read_schema(&self) -> Result<DatabaseSchema>;

	/// Read a specific table schema
	async fn read_table(&self, table_name: &str) -> Result<Option<TableInfo>>;
}

/// Read a schema through a connection and select the requested schema objects.
///
/// Table names in [`InspectDbOptions::tables`] are exact identifiers. Unknown names are
/// reported as errors so a misspelled command-line argument cannot silently produce a
/// partial model module.
pub async fn inspect_database(
	connection: &DatabaseConnection,
	options: &InspectDbOptions,
) -> Result<DatabaseSchema> {
	validate_partition_option(connection.database_type(), options.include_partitions)?;

	let mut schema = match connection.database_type() {
		DatabaseType::Postgres => {
			#[cfg(feature = "postgres")]
			{
				let pool = connection.into_postgres().ok_or_else(|| {
					MigrationError::UnsupportedDatabase("PostgreSQL connection".to_string())
				})?;
				read_postgres_schema(pool, options).await?
			}
			#[cfg(not(feature = "postgres"))]
			{
				return Err(MigrationError::UnsupportedDatabase(
					"PostgreSQL".to_string(),
				));
			}
		}
		DatabaseType::Mysql => {
			#[cfg(feature = "mysql")]
			{
				let pool = connection.into_mysql().ok_or_else(|| {
					MigrationError::UnsupportedDatabase("MySQL connection".to_string())
				})?;
				read_mysql_schema(pool, options).await?
			}
			#[cfg(not(feature = "mysql"))]
			{
				return Err(MigrationError::UnsupportedDatabase("MySQL".to_string()));
			}
		}
		DatabaseType::Sqlite => {
			#[cfg(feature = "sqlite")]
			{
				let pool = connection.into_sqlite().ok_or_else(|| {
					MigrationError::UnsupportedDatabase("SQLite connection".to_string())
				})?;
				read_sqlite_schema(pool, options).await?
			}
			#[cfg(not(feature = "sqlite"))]
			{
				return Err(MigrationError::UnsupportedDatabase("SQLite".to_string()));
			}
		}
	};

	if options.tables.is_empty() {
		return Ok(schema);
	}

	let mut selected = HashMap::with_capacity(options.tables.len());
	let mut requested = HashSet::with_capacity(options.tables.len());
	for table_name in &options.tables {
		if !requested.insert(table_name) {
			continue;
		}
		let table = schema.tables.remove(table_name).ok_or_else(|| {
			MigrationError::IntrospectionError(format!("Requested table not found: {table_name}"))
		})?;
		selected.insert(table_name.clone(), table);
	}

	Ok(DatabaseSchema { tables: selected })
}

fn validate_partition_option(database_type: DatabaseType, include_partitions: bool) -> Result<()> {
	if include_partitions && database_type != DatabaseType::Postgres {
		return Err(MigrationError::IntrospectionError(
			"include_partitions is only supported for PostgreSQL".to_string(),
		));
	}
	Ok(())
}

fn filter_postgres_partitions(
	schema: &mut DatabaseSchema,
	partition_names: &HashSet<String>,
	include_partitions: bool,
) {
	if !include_partitions {
		schema
			.tables
			.retain(|name, _| !partition_names.contains(name));
	}
}

#[cfg(feature = "postgres")]
#[derive(Debug, Clone, PartialEq, Eq)]
struct PostgresRelation {
	name: String,
	is_partition: bool,
}

#[cfg(feature = "postgres")]
fn postgres_partition_names(
	relations: impl IntoIterator<Item = PostgresRelation>,
) -> HashSet<String> {
	relations
		.into_iter()
		.filter_map(|relation| relation.is_partition.then_some(relation.name))
		.collect()
}

/// PostgreSQL schema introspector
#[cfg(feature = "postgres")]
pub struct PostgresIntrospector {
	pool: sqlx::PgPool,
}

#[cfg(feature = "postgres")]
impl PostgresIntrospector {
	/// Creates a new instance.
	pub fn new(pool: sqlx::PgPool) -> Self {
		Self { pool }
	}

	/// Maps PostgreSQL udt_name/data_type to FieldType
	fn parse_pg_type(
		udt_name: &str,
		data_type: &str,
		type_definition: Option<&str>,
		char_max_length: Option<i32>,
		numeric_precision: Option<i32>,
		numeric_scale: Option<i32>,
		enum_values: Option<Vec<String>>,
	) -> super::FieldType {
		use super::FieldType;
		#[cfg(not(feature = "pgvector"))]
		let _ = type_definition;

		match udt_name {
			// Integer types
			"int4" | "serial" => FieldType::Integer,
			"int8" | "bigserial" => FieldType::BigInteger,
			"int2" | "smallserial" => FieldType::SmallInteger,

			// String types
			"varchar" => FieldType::VarChar(char_max_length.unwrap_or(255) as u32),
			"bpchar" => FieldType::Char(char_max_length.unwrap_or(1) as u32),
			"text" => FieldType::Text,

			// Boolean
			"bool" => FieldType::Boolean,

			// Floating point
			"float4" => FieldType::Real,
			"float8" => FieldType::Double,

			// Numeric/Decimal
			"numeric" => FieldType::Decimal {
				precision: numeric_precision.unwrap_or(10) as u32,
				scale: numeric_scale.unwrap_or(2) as u32,
			},

			// Date/Time types
			"timestamp" => FieldType::DateTime,
			"timestamptz" => FieldType::TimestampTz,
			"date" => FieldType::Date,
			"time" | "timetz" => FieldType::Time,

			// Binary
			"bytea" => FieldType::Bytea,

			// JSON
			"json" => FieldType::Json,
			"jsonb" => FieldType::JsonBinary,

			// UUID
			"uuid" => FieldType::Uuid,

			// Text search
			"tsvector" => FieldType::TsVector,
			"tsquery" => FieldType::TsQuery,

			// Range types
			"int4range" => FieldType::Int4Range,
			"int8range" => FieldType::Int8Range,
			"numrange" => FieldType::NumRange,
			"tsrange" => FieldType::TsRange,
			"tstzrange" => FieldType::TsTzRange,
			"daterange" => FieldType::DateRange,

			#[cfg(feature = "pgvector")]
			"vector" => type_definition
				.and_then(Self::parse_pgvector_dimensions)
				.map_or_else(
					|| FieldType::Custom(udt_name.to_string()),
					|dimensions| FieldType::Vector { dimensions },
				),

			// Array types (udt_name starts with _)
			name if name.starts_with('_') => {
				#[cfg(feature = "pgvector")]
				let inner_type_definition = if &name[1..] == "vector" {
					type_definition.and_then(|definition| definition.strip_suffix("[]"))
				} else {
					None
				};
				#[cfg(not(feature = "pgvector"))]
				let inner_type_definition = None;
				let inner = Self::parse_pg_type(
					&name[1..],
					data_type,
					inner_type_definition,
					char_max_length,
					numeric_precision,
					numeric_scale,
					None,
				);
				FieldType::Array(Box::new(inner))
			}

			// Geometric types
			"point" => FieldType::Custom("POINT".to_string()),
			"line" => FieldType::Custom("LINE".to_string()),
			"lseg" => FieldType::Custom("LSEG".to_string()),
			"box" => FieldType::Custom("BOX".to_string()),
			"path" => FieldType::Custom("PATH".to_string()),
			"polygon" => FieldType::Custom("POLYGON".to_string()),
			"circle" => FieldType::Custom("CIRCLE".to_string()),

			// Network address types
			"cidr" => FieldType::Custom("CIDR".to_string()),
			"inet" => FieldType::Custom("INET".to_string()),
			"macaddr" => FieldType::Custom("MACADDR".to_string()),
			"macaddr8" => FieldType::Custom("MACADDR8".to_string()),

			// Bit string types
			"bit" => FieldType::Custom("BIT".to_string()),
			"varbit" => FieldType::Custom("VARBIT".to_string()),

			// XML
			"xml" => FieldType::Custom("XML".to_string()),

			// Money
			"money" => FieldType::Custom("MONEY".to_string()),

			// Interval
			"interval" => FieldType::Custom("INTERVAL".to_string()),

			// PG_LSN
			"pg_lsn" => FieldType::Custom("PG_LSN".to_string()),

			// User-defined types (enums)
			_ if data_type == "USER-DEFINED" => {
				if let Some(values) = enum_values {
					FieldType::Enum { values }
				} else {
					FieldType::Custom(udt_name.to_string())
				}
			}

			// Fallback
			_ => FieldType::Custom(udt_name.to_string()),
		}
	}

	#[cfg(feature = "pgvector")]
	fn parse_pgvector_dimensions(type_definition: &str) -> Option<usize> {
		let unqualified_definition = type_definition
			.rsplit_once('.')
			.map_or(type_definition, |(_, definition)| definition);
		let dimensions = unqualified_definition
			.strip_prefix("vector(")?
			.strip_suffix(')')?
			.parse()
			.ok()?;
		(1..=2000).contains(&dimensions).then_some(dimensions)
	}

	/// Fetches enum label values for a given PostgreSQL enum type name
	async fn fetch_enum_values(&self, type_name: &str) -> Result<Vec<String>> {
		use sqlx::Row;
		let query = r#"
			SELECT e.enumlabel
			FROM pg_enum e
			JOIN pg_type t ON e.enumtypid = t.oid
			JOIN pg_namespace n ON t.typnamespace = n.oid
			WHERE t.typname = $1 AND n.nspname = current_schema()
			ORDER BY e.enumsortorder
		"#;
		let rows = sqlx::query(query)
			.bind(type_name)
			.fetch_all(&self.pool)
			.await
			.map_err(|e| {
				MigrationError::IntrospectionError(format!(
					"Failed to fetch enum values for {}: {}",
					type_name, e
				))
			})?;
		Ok(rows
			.iter()
			.map(|r| r.try_get::<String, _>("enumlabel").unwrap_or_default())
			.collect())
	}

	/// Introspects a single table using direct SQL queries against
	/// information_schema and pg_catalog
	async fn introspect_table(&self, table_name: &str) -> Result<TableInfo> {
		use sqlx::Row;

		// Fetch columns
		let col_query = r#"
			SELECT columns.column_name, columns.udt_name, columns.data_type,
			       columns.is_nullable, columns.column_default,
			       columns.character_maximum_length, columns.numeric_precision,
			       columns.numeric_scale, columns.is_identity,
			       columns.identity_generation, columns.is_generated,
			       columns.generation_expression,
			       pg_catalog.format_type(attributes.atttypid, attributes.atttypmod)
			           AS type_definition
			FROM information_schema.columns AS columns
			JOIN pg_catalog.pg_namespace AS schemas
			    ON schemas.nspname = columns.table_schema
			JOIN pg_catalog.pg_class AS tables
			    ON tables.relname = columns.table_name
			    AND tables.relnamespace = schemas.oid
			JOIN pg_catalog.pg_attribute AS attributes
			    ON attributes.attrelid = tables.oid
			    AND attributes.attname = columns.column_name
			    AND NOT attributes.attisdropped
			WHERE columns.table_schema = current_schema() AND columns.table_name = $1
			ORDER BY columns.ordinal_position
		"#;
		let col_rows = sqlx::query(col_query)
			.bind(table_name)
			.fetch_all(&self.pool)
			.await
			.map_err(|e| {
				MigrationError::IntrospectionError(format!(
					"Failed to fetch columns for table {}: {}",
					table_name, e
				))
			})?;

		let mut columns = HashMap::new();
		for row in &col_rows {
			let column_name: String = row.try_get("column_name").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get column_name: {}", e))
			})?;
			let udt_name: String = row.try_get("udt_name").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get udt_name: {}", e))
			})?;
			let data_type: String = row.try_get("data_type").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get data_type: {}", e))
			})?;
			let type_definition: String = row.try_get("type_definition").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get type_definition: {}", e))
			})?;
			let is_nullable: String = row.try_get("is_nullable").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get is_nullable: {}", e))
			})?;
			let column_default: Option<String> = row.try_get("column_default").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get column_default: {}", e))
			})?;
			let char_max_length: Option<i32> =
				row.try_get("character_maximum_length").map_err(|e| {
					MigrationError::IntrospectionError(format!(
						"Failed to get character_maximum_length: {}",
						e
					))
				})?;
			let numeric_precision: Option<i32> = row.try_get("numeric_precision").map_err(|e| {
				MigrationError::IntrospectionError(format!(
					"Failed to get numeric_precision: {}",
					e
				))
			})?;
			let numeric_scale: Option<i32> = row.try_get("numeric_scale").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get numeric_scale: {}", e))
			})?;
			let is_identity: String = row.try_get("is_identity").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get is_identity: {}", e))
			})?;
			let is_generated: String = row.try_get("is_generated").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get is_generated: {}", e))
			})?;
			let generation_expression: Option<String> =
				row.try_get("generation_expression").map_err(|e| {
					MigrationError::IntrospectionError(format!(
						"Failed to get generation_expression: {}",
						e
					))
				})?;

			// Detect auto-increment: nextval() in default or identity column
			let is_auto = column_default
				.as_ref()
				.is_some_and(|d| d.starts_with("nextval("))
				|| is_identity == "YES";

			// Detect serial types (auto-increment integer)
			let is_serial = matches!(udt_name.as_str(), "int4" | "int8" | "int2")
				&& column_default
					.as_ref()
					.is_some_and(|d| d.starts_with("nextval("));

			// Fetch enum values for USER-DEFINED types
			let enum_values = if data_type == "USER-DEFINED" {
				let values = self.fetch_enum_values(&udt_name).await?;
				if values.is_empty() {
					None
				} else {
					Some(values)
				}
			} else {
				None
			};

			let field_type = Self::parse_pg_type(
				&udt_name,
				&data_type,
				Some(&type_definition),
				char_max_length,
				numeric_precision,
				numeric_scale,
				enum_values,
			);

			columns.insert(
				column_name.clone(),
				ColumnInfo {
					name: column_name,
					column_type: field_type,
					nullable: is_nullable == "YES",
					default: column_default,
					auto_increment: is_auto || is_serial,
					generated: generation_expression
						.filter(|expression| is_generated == "ALWAYS" && !expression.is_empty())
						.map(|expression| {
							super::GeneratedColumnDefinition::raw_sql(
								expression,
								super::GeneratedStorage::Stored,
							)
						}),
				},
			);
		}

		// Fetch primary key
		let pk_query = r#"
			SELECT kcu.column_name
			FROM information_schema.table_constraints tc
			JOIN information_schema.key_column_usage kcu
			    ON tc.constraint_name = kcu.constraint_name
			    AND tc.table_schema = kcu.table_schema
			WHERE tc.table_schema = current_schema() AND tc.table_name = $1
			    AND tc.constraint_type = 'PRIMARY KEY'
			ORDER BY kcu.ordinal_position
		"#;
		let pk_rows = sqlx::query(pk_query)
			.bind(table_name)
			.fetch_all(&self.pool)
			.await
			.map_err(|e| {
				MigrationError::IntrospectionError(format!(
					"Failed to fetch primary key for table {}: {}",
					table_name, e
				))
			})?;
		let primary_key: Vec<String> = pk_rows
			.iter()
			.map(|r| r.try_get::<String, _>("column_name").unwrap_or_default())
			.collect();

		// Fetch foreign keys
		let fk_query = r#"
			SELECT tc.constraint_name, kcu.column_name,
			       ccu.table_name AS referenced_table, ccu.column_name AS referenced_column,
			       rc.update_rule, rc.delete_rule
			FROM information_schema.table_constraints tc
			JOIN information_schema.key_column_usage kcu
			    ON tc.constraint_name = kcu.constraint_name
			    AND tc.table_schema = kcu.table_schema
			JOIN information_schema.constraint_column_usage ccu
			    ON tc.constraint_name = ccu.constraint_name
			    AND tc.table_schema = ccu.table_schema
			JOIN information_schema.referential_constraints rc
			    ON tc.constraint_name = rc.constraint_name
			    AND tc.table_schema = rc.constraint_schema
			WHERE tc.table_schema = current_schema() AND tc.table_name = $1
			    AND tc.constraint_type = 'FOREIGN KEY'
			ORDER BY tc.constraint_name, kcu.ordinal_position
		"#;
		let fk_rows = sqlx::query(fk_query)
			.bind(table_name)
			.fetch_all(&self.pool)
			.await
			.map_err(|e| {
				MigrationError::IntrospectionError(format!(
					"Failed to fetch foreign keys for table {}: {}",
					table_name, e
				))
			})?;

		// Group FK rows by constraint name
		let mut fk_map: HashMap<String, ForeignKeyInfo> = HashMap::new();
		for row in &fk_rows {
			let constraint_name: String = row.try_get("constraint_name").unwrap_or_default();
			let column_name: String = row.try_get("column_name").unwrap_or_default();
			let ref_table: String = row.try_get("referenced_table").unwrap_or_default();
			let ref_column: String = row.try_get("referenced_column").unwrap_or_default();
			let update_rule: String = row.try_get("update_rule").unwrap_or_default();
			let delete_rule: String = row.try_get("delete_rule").unwrap_or_default();

			let entry = fk_map
				.entry(constraint_name.clone())
				.or_insert_with(|| ForeignKeyInfo {
					name: constraint_name,
					columns: Vec::new(),
					referenced_table: ref_table,
					referenced_columns: Vec::new(),
					on_delete: if delete_rule == "NO ACTION" {
						None
					} else {
						Some(delete_rule)
					},
					on_update: if update_rule == "NO ACTION" {
						None
					} else {
						Some(update_rule)
					},
				});
			if !entry.columns.contains(&column_name) {
				entry.columns.push(column_name);
			}
			if !entry.referenced_columns.contains(&ref_column) {
				entry.referenced_columns.push(ref_column);
			}
		}

		let foreign_keys: Vec<ForeignKeyInfo> = fk_map.into_values().collect();

		// Fetch unique constraints
		let uq_query = r#"
			SELECT tc.constraint_name, kcu.column_name
			FROM information_schema.table_constraints tc
			JOIN information_schema.key_column_usage kcu
			    ON tc.constraint_name = kcu.constraint_name
			    AND tc.table_schema = kcu.table_schema
			WHERE tc.table_schema = current_schema() AND tc.table_name = $1
			    AND tc.constraint_type = 'UNIQUE'
			ORDER BY tc.constraint_name, kcu.ordinal_position
		"#;
		let uq_rows = sqlx::query(uq_query)
			.bind(table_name)
			.fetch_all(&self.pool)
			.await
			.map_err(|e| {
				MigrationError::IntrospectionError(format!(
					"Failed to fetch unique constraints for table {}: {}",
					table_name, e
				))
			})?;

		let mut uq_map: HashMap<String, Vec<String>> = HashMap::new();
		for row in &uq_rows {
			let constraint_name: String = row.try_get("constraint_name").unwrap_or_default();
			let column_name: String = row.try_get("column_name").unwrap_or_default();
			uq_map.entry(constraint_name).or_default().push(column_name);
		}
		let unique_constraints: Vec<UniqueConstraintInfo> = uq_map
			.into_iter()
			.map(|(name, columns)| UniqueConstraintInfo { name, columns })
			.collect();

		// Fetch indexes (reuse existing method)
		let indexes = self.fetch_table_indexes(table_name).await?;

		// CHECK constraints not yet implemented
		let check_constraints: Vec<CheckConstraintInfo> = Vec::new();

		Ok(TableInfo {
			name: table_name.to_string(),
			columns,
			indexes,
			primary_key,
			foreign_keys,
			unique_constraints,
			check_constraints,
		})
	}

	/// Fetch index information for a specific table from PostgreSQL system catalogs
	async fn fetch_table_indexes(&self, table_name: &str) -> Result<HashMap<String, IndexInfo>> {
		use sqlx::Row;

		// Query PostgreSQL system catalogs to get index information
		// Excludes primary key indexes as they are handled separately
		let query = r#"
			SELECT
				i.relname AS index_name,
				COALESCE(
					array_agg(a.attname ORDER BY key.position)
						FILTER (WHERE a.attname IS NOT NULL),
					ARRAY[]::name[]
				) AS column_names,
				ix.indisunique AS is_unique,
				am.amname AS access_method,
				i.reloptions AS index_options,
				array_agg(
					pg_get_indexdef(ix.indexrelid, key.position + 1, true)
					ORDER BY key.position
				) FILTER (WHERE (ix.indkey::smallint[])[key.position] = 0)
					AS index_expressions,
				array_agg(
					CASE
						WHEN opn.nspname = 'pg_catalog' THEN opc.opcname
						ELSE format('%I.%I', opn.nspname, opc.opcname)
					END
					ORDER BY key.position
				) AS operator_classes,
				array_agg(opc.opcdefault ORDER BY key.position) AS operator_class_defaults,
				count(*) FILTER (WHERE (ix.indkey::smallint[])[key.position] <> 0)
					AS column_key_count,
				count(*) FILTER (WHERE (ix.indkey::smallint[])[key.position] = 0)
					AS expression_key_count
			FROM
				pg_class t
				JOIN pg_index ix ON t.oid = ix.indrelid
				JOIN pg_class i ON i.oid = ix.indexrelid
				JOIN pg_am am ON i.relam = am.oid
				JOIN pg_namespace n ON n.oid = t.relnamespace
				CROSS JOIN LATERAL generate_subscripts(ix.indclass::oid[], 1) AS key(position)
				LEFT JOIN pg_attribute a
					ON a.attrelid = t.oid
					AND a.attnum = (ix.indkey::smallint[])[key.position]
				LEFT JOIN pg_opclass opc
					ON opc.oid = (ix.indclass::oid[])[key.position]
				LEFT JOIN pg_namespace opn ON opn.oid = opc.opcnamespace
			WHERE
				t.relkind = 'r'
				AND t.relname = $1
				AND NOT ix.indisprimary
				AND n.nspname = current_schema()
			GROUP BY
				i.oid,
				i.relname,
				i.reloptions,
				ix.indisunique,
				am.amname
			ORDER BY i.relname
		"#;

		let rows = sqlx::query(query)
			.bind(table_name)
			.fetch_all(&self.pool)
			.await
			.map_err(|e| {
				MigrationError::IntrospectionError(format!(
					"Failed to fetch indexes for table {}: {}",
					table_name, e
				))
			})?;

		let mut indexes = HashMap::new();
		for row in rows {
			let index_name: String = row.try_get("index_name").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get index_name: {}", e))
			})?;
			let column_names: Vec<String> = row.try_get("column_names").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get column_names: {}", e))
			})?;
			let is_unique: bool = row.try_get("is_unique").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get is_unique: {}", e))
			})?;
			let access_method: String = row.try_get("access_method").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get access_method: {}", e))
			})?;
			#[cfg(feature = "pgvector")]
			let index_options: Option<Vec<String>> = row.try_get("index_options").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get index_options: {}", e))
			})?;
			#[cfg(feature = "pgvector")]
			let index_expressions: Option<Vec<String>> = row.try_get("index_expressions").map_err(|e| {
				MigrationError::IntrospectionError(format!(
					"Failed to get index_expressions: {}",
					e
				))
			})?;
			#[cfg(feature = "pgvector")]
			let operator_classes: Vec<String> = row.try_get("operator_classes").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get operator_classes: {}", e))
			})?;
			#[cfg(feature = "pgvector")]
			let operator_class_defaults: Vec<bool> =
				row.try_get("operator_class_defaults").map_err(|e| {
					MigrationError::IntrospectionError(format!(
						"Failed to get operator_class_defaults: {}",
						e
					))
				})?;
			#[cfg(feature = "pgvector")]
			let column_key_count: i64 = row.try_get("column_key_count").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get column_key_count: {e}"))
			})?;
			#[cfg(feature = "pgvector")]
			let expression_key_count: i64 = row.try_get("expression_key_count").map_err(|e| {
				MigrationError::IntrospectionError(format!(
					"Failed to get expression_key_count: {e}"
				))
			})?;
			#[cfg(feature = "pgvector")]
			validate_postgres_index_key_kinds(&index_name, column_key_count, expression_key_count)?;
			#[cfg(feature = "pgvector")]
			let index_type =
				parse_postgres_index_type(&access_method, index_options.as_deref().unwrap_or(&[]))?;
			#[cfg(feature = "pgvector")]
			let (operator_class, operator_class_is_default) = resolve_postgres_operator_class(
				&index_name,
				index_type,
				&operator_classes,
				&operator_class_defaults,
			)?;

			indexes.insert(
				index_name.clone(),
				IndexInfo {
					name: index_name,
					columns: column_names,
					unique: is_unique,
					#[cfg(feature = "pgvector")]
					access_method: Some(access_method),
					#[cfg(feature = "pgvector")]
					index_type,
					#[cfg(not(feature = "pgvector"))]
					index_type: Some(access_method),
					#[cfg(feature = "pgvector")]
					expressions: index_expressions,
					#[cfg(feature = "pgvector")]
					operator_class,
					#[cfg(feature = "pgvector")]
					operator_class_is_default,
				},
			);
		}

		Ok(indexes)
	}
}

#[cfg(feature = "postgres")]
#[async_trait]
impl DatabaseIntrospector for PostgresIntrospector {
	async fn read_schema(&self) -> Result<DatabaseSchema> {
		use sqlx::Row;

		let table_query = r#"
			SELECT table_name FROM information_schema.tables
			WHERE table_schema = current_schema() AND table_type = 'BASE TABLE'
		"#;
		let table_rows = sqlx::query(table_query)
			.fetch_all(&self.pool)
			.await
			.map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to fetch table list: {}", e))
			})?;

		let mut tables = HashMap::new();
		for row in &table_rows {
			let table_name: String = row.try_get("table_name").unwrap_or_default();
			let table_info = self.introspect_table(&table_name).await?;
			tables.insert(table_name, table_info);
		}

		Ok(DatabaseSchema { tables })
	}

	async fn read_table(&self, table_name: &str) -> Result<Option<TableInfo>> {
		// Check if table exists
		let exists_query = r#"
			SELECT table_name FROM information_schema.tables
			WHERE table_schema = current_schema() AND table_type = 'BASE TABLE' AND table_name = $1
		"#;
		let exists = sqlx::query(exists_query)
			.bind(table_name)
			.fetch_optional(&self.pool)
			.await
			.map_err(|e| {
				MigrationError::IntrospectionError(format!(
					"Failed to check table existence: {}",
					e
				))
			})?;

		if exists.is_some() {
			Ok(Some(self.introspect_table(table_name).await?))
		} else {
			Ok(None)
		}
	}
}

/// MySQL schema introspector
#[cfg(feature = "mysql")]
pub struct MySQLIntrospector {
	pool: sqlx::MySqlPool,
}

#[cfg(feature = "mysql")]
fn decode_mysql_text(row: &sqlx::mysql::MySqlRow, index: usize, kind: &str) -> Result<String> {
	use sqlx::Row;

	let bytes: Vec<u8> = row.try_get(index).map_err(|error| {
		MigrationError::IntrospectionError(format!("Failed to read MySQL {}: {}", kind, error))
	})?;
	String::from_utf8(bytes).map_err(|_| {
		MigrationError::IntrospectionError(format!("MySQL {} is not valid UTF-8", kind))
	})
}

#[cfg(feature = "mysql")]
fn decode_optional_mysql_text(
	row: &sqlx::mysql::MySqlRow,
	index: usize,
	kind: &str,
) -> Result<Option<String>> {
	use sqlx::Row;

	let bytes: Option<Vec<u8>> = row.try_get(index).map_err(|error| {
		MigrationError::IntrospectionError(format!("Failed to read MySQL {}: {}", kind, error))
	})?;
	bytes
		.map(|bytes| {
			String::from_utf8(bytes).map_err(|_| {
				MigrationError::IntrospectionError(format!("MySQL {} is not valid UTF-8", kind))
			})
		})
		.transpose()
}

#[cfg(feature = "mysql")]
fn decode_optional_mysql_integer(
	row: &sqlx::mysql::MySqlRow,
	index: usize,
	kind: &str,
) -> Result<Option<i64>> {
	use sqlx::Row;

	row.try_get(index).map_err(|error| {
		MigrationError::IntrospectionError(format!("Failed to read MySQL {}: {}", kind, error))
	})
}

#[cfg(feature = "mysql")]
fn decode_optional_mysql_unsigned_integer(
	row: &sqlx::mysql::MySqlRow,
	index: usize,
	kind: &str,
) -> Result<Option<i64>> {
	use sqlx::Row;

	let value: Option<u64> = row.try_get(index).map_err(|error| {
		MigrationError::IntrospectionError(format!("Failed to read MySQL {}: {}", kind, error))
	})?;
	value
		.map(|value| {
			i64::try_from(value).map_err(|_| {
				MigrationError::IntrospectionError(format!(
					"MySQL {} exceeds the supported range",
					kind
				))
			})
		})
		.transpose()
}

#[cfg(feature = "mysql")]
fn mysql_catalog_u32(value: Option<i64>, default: u32, kind: &str) -> Result<u32> {
	u32::try_from(value.unwrap_or(i64::from(default))).map_err(|_| {
		MigrationError::IntrospectionError(format!(
			"MySQL {} is outside the supported u32 range",
			kind
		))
	})
}

#[cfg(feature = "mysql")]
impl MySQLIntrospector {
	/// Creates a new MySQL introspector with the given pool.
	pub fn new(pool: sqlx::MySqlPool) -> Self {
		Self { pool }
	}

	fn parse_mysql_type(
		data_type: &str,
		column_type: &str,
		char_max_length: Option<i64>,
		numeric_precision: Option<i64>,
		numeric_scale: Option<i64>,
	) -> Result<super::FieldType> {
		use super::FieldType;
		let data_type_lower = data_type.to_lowercase();
		let unsigned = column_type.to_ascii_lowercase().contains("unsigned");
		let field_type = match data_type_lower.as_str() {
			"tinyint" => {
				// MySQL uses tinyint(1) for boolean
				if column_type.to_lowercase().starts_with("tinyint(1)") {
					FieldType::Boolean
				} else {
					if unsigned {
						FieldType::Custom("u8".to_string())
					} else {
						FieldType::TinyInt
					}
				}
			}
			"smallint" => {
				if unsigned {
					FieldType::Custom("u16".to_string())
				} else {
					FieldType::SmallInteger
				}
			}
			"mediumint" | "int" | "integer" => {
				if unsigned {
					FieldType::Custom("u32".to_string())
				} else if data_type_lower == "mediumint" {
					FieldType::MediumInt
				} else {
					FieldType::Integer
				}
			}
			"bigint" => {
				if unsigned {
					FieldType::Custom("u64".to_string())
				} else {
					FieldType::BigInteger
				}
			}

			"varchar" => FieldType::VarChar(mysql_catalog_u32(
				char_max_length,
				255,
				"character maximum length",
			)?),
			"char" => FieldType::Char(mysql_catalog_u32(
				char_max_length,
				1,
				"character maximum length",
			)?),
			"text" => FieldType::Text,
			"tinytext" => FieldType::TinyText,
			"mediumtext" => FieldType::MediumText,
			"longtext" => FieldType::LongText,

			"decimal" | "numeric" => FieldType::Decimal {
				precision: mysql_catalog_u32(numeric_precision, 10, "numeric precision")?,
				scale: mysql_catalog_u32(numeric_scale, 2, "numeric scale")?,
			},
			"float" => FieldType::Float,
			"double" => FieldType::Double,

			"date" => FieldType::Date,
			"time" => FieldType::Time,
			"datetime" => FieldType::DateTime,
			"timestamp" => FieldType::DateTime,
			"year" => FieldType::Year,

			"binary" | "varbinary" => FieldType::Binary,
			"blob" => FieldType::Blob,
			"tinyblob" => FieldType::TinyBlob,
			"mediumblob" => FieldType::MediumBlob,
			"longblob" => FieldType::LongBlob,

			"json" => FieldType::Json,
			"bit" => FieldType::Boolean,

			"enum" => {
				// Parse enum values from column_type: enum('a','b','c')
				let values = Self::parse_enum_or_set_values(column_type);
				FieldType::Enum { values }
			}
			"set" => {
				// Parse set values from column_type: set('a','b','c')
				let values = Self::parse_enum_or_set_values(column_type);
				FieldType::Set { values }
			}

			_ => FieldType::Custom(data_type.to_string()),
		};
		Ok(field_type)
	}

	/// Parse enum or set values from MySQL column_type string.
	/// Input format: "enum('val1','val2','val3')" or "set('val1','val2')"
	fn parse_enum_or_set_values(column_type: &str) -> Vec<String> {
		// Find the opening parenthesis
		if let Some(start) = column_type.find('(')
			&& let Some(end) = column_type.rfind(')')
		{
			let inner = &column_type[start + 1..end];
			return inner
				.split(',')
				.map(|s| s.trim().trim_matches('\'').to_string())
				.collect();
		}
		Vec::new()
	}

	/// Introspects a single table using direct SQL queries against information_schema
	async fn introspect_table(&self, table_name: &str) -> Result<TableInfo> {
		use sqlx::Row;

		// Fetch columns from information_schema
		let col_query = r#"
			SELECT column_name, data_type, column_type, is_nullable, column_default,
			       column_key, extra, generation_expression, character_maximum_length,
			       numeric_precision, numeric_scale
			FROM information_schema.columns
			WHERE table_schema = DATABASE() AND table_name = ?
			ORDER BY ordinal_position
		"#;
		let col_rows = sqlx::query(col_query)
			.bind(table_name)
			.fetch_all(&self.pool)
			.await
			.map_err(|e| {
				MigrationError::IntrospectionError(format!(
					"Failed to fetch columns for table {}: {}",
					table_name, e
				))
			})?;

		let mut columns = HashMap::new();
		let mut primary_key = Vec::new();

		for row in &col_rows {
			let column_name = decode_mysql_text(row, 0, "column name")?;
			let data_type = decode_mysql_text(row, 1, "data type")?;
			let column_type_str = decode_mysql_text(row, 2, "column type")?;
			let is_nullable = decode_mysql_text(row, 3, "column nullability")?;
			let column_default = decode_optional_mysql_text(row, 4, "column default")?;
			let column_key = decode_mysql_text(row, 5, "column key")?;
			let extra = decode_mysql_text(row, 6, "column extra metadata")?;
			let generation_expression =
				decode_optional_mysql_text(row, 7, "generation expression")?;
			let char_max_length =
				decode_optional_mysql_unsigned_integer(row, 8, "character maximum length")?;
			let numeric_precision =
				decode_optional_mysql_unsigned_integer(row, 9, "numeric precision")?;
			let numeric_scale = decode_optional_mysql_unsigned_integer(row, 10, "numeric scale")?;

			// Primary key detection
			if column_key == "PRI" {
				primary_key.push(column_name.clone());
			}

			// Auto-increment detection
			let is_auto = extra.to_lowercase().contains("auto_increment");

			let field_type = Self::parse_mysql_type(
				&data_type,
				&column_type_str,
				char_max_length,
				numeric_precision,
				numeric_scale,
			)?;

			columns.insert(
				column_name.clone(),
				ColumnInfo {
					name: column_name,
					column_type: field_type,
					nullable: is_nullable == "YES",
					default: column_default,
					auto_increment: is_auto,
					generated: generation_expression
						.filter(|expression| !expression.is_empty())
						.map(|expression| {
							let storage = if extra.to_ascii_lowercase().contains("stored") {
								super::GeneratedStorage::Stored
							} else {
								super::GeneratedStorage::Virtual
							};
							super::GeneratedColumnDefinition::raw_sql(expression, storage)
						}),
				},
			);
		}

		// Fetch indexes from information_schema.statistics
		let idx_query = r#"
			SELECT index_name, column_name, non_unique, index_type
			FROM information_schema.statistics
			WHERE table_schema = DATABASE() AND table_name = ?
			ORDER BY index_name, seq_in_index
		"#;
		let idx_rows = sqlx::query(idx_query)
			.bind(table_name)
			.fetch_all(&self.pool)
			.await
			.map_err(|e| {
				MigrationError::IntrospectionError(format!(
					"Failed to fetch indexes for table {}: {}",
					table_name, e
				))
			})?;

		let mut idx_map: HashMap<String, (Vec<String>, bool, String)> = HashMap::new();
		for row in &idx_rows {
			let index_name = decode_mysql_text(row, 0, "index name")?;
			let column_name = decode_mysql_text(row, 1, "indexed column name")?;
			let non_unique: i64 = row.try_get(2).map_err(|error| {
				MigrationError::IntrospectionError(format!(
					"Failed to read MySQL index uniqueness: {error}"
				))
			})?;
			let index_type = decode_mysql_text(row, 3, "index type")?;

			let entry = idx_map
				.entry(index_name)
				.or_insert_with(|| (Vec::new(), non_unique == 0, index_type.clone()));
			entry.0.push(column_name);
		}

		let mut indexes = HashMap::new();
		let mut unique_constraints = Vec::new();
		for (name, (cols, is_unique, idx_type)) in &idx_map {
			// Skip PRIMARY key index - already handled
			if name == "PRIMARY" {
				continue;
			}

			indexes.insert(
				name.clone(),
				IndexInfo {
					name: name.clone(),
					columns: cols.clone(),
					unique: *is_unique,
					#[cfg(feature = "pgvector")]
					access_method: Some(idx_type.clone()),
					#[cfg(feature = "pgvector")]
					index_type: None,
					#[cfg(not(feature = "pgvector"))]
					index_type: Some(idx_type.clone()),
					#[cfg(feature = "pgvector")]
					expressions: None,
					#[cfg(feature = "pgvector")]
					operator_class: None,
					#[cfg(feature = "pgvector")]
					operator_class_is_default: false,
				},
			);

			if *is_unique {
				unique_constraints.push(UniqueConstraintInfo {
					name: name.clone(),
					columns: cols.clone(),
				});
			}
		}

		// Fetch foreign keys
		let fk_query = r#"
			SELECT rc.constraint_name, kcu.column_name,
			       kcu.referenced_table_name, kcu.referenced_column_name,
			       rc.update_rule, rc.delete_rule
			FROM information_schema.referential_constraints rc
			JOIN information_schema.key_column_usage kcu
			    ON rc.constraint_name = kcu.constraint_name
			    AND rc.constraint_schema = kcu.constraint_schema
			WHERE rc.constraint_schema = DATABASE() AND kcu.table_name = ?
			    AND kcu.referenced_table_name IS NOT NULL
			ORDER BY rc.constraint_name, kcu.ordinal_position
		"#;
		let fk_rows = sqlx::query(fk_query)
			.bind(table_name)
			.fetch_all(&self.pool)
			.await
			.map_err(|e| {
				MigrationError::IntrospectionError(format!(
					"Failed to fetch foreign keys for table {}: {}",
					table_name, e
				))
			})?;

		let mut fk_map: HashMap<String, ForeignKeyInfo> = HashMap::new();
		for row in &fk_rows {
			let constraint_name = decode_mysql_text(row, 0, "constraint name")?;
			let column_name = decode_mysql_text(row, 1, "foreign-key column name")?;
			let ref_table = decode_mysql_text(row, 2, "referenced table name")?;
			let ref_column = decode_mysql_text(row, 3, "referenced column name")?;
			let update_rule = decode_mysql_text(row, 4, "foreign-key update rule")?;
			let delete_rule = decode_mysql_text(row, 5, "foreign-key delete rule")?;

			let entry = fk_map
				.entry(constraint_name.clone())
				.or_insert_with(|| ForeignKeyInfo {
					name: constraint_name,
					columns: Vec::new(),
					referenced_table: ref_table,
					referenced_columns: Vec::new(),
					on_delete: Some(delete_rule),
					on_update: Some(update_rule),
				});
			if !entry.columns.contains(&column_name) {
				entry.columns.push(column_name);
			}
			if !entry.referenced_columns.contains(&ref_column) {
				entry.referenced_columns.push(ref_column);
			}
		}

		let foreign_keys: Vec<ForeignKeyInfo> = fk_map.into_values().collect();

		// MySQL CHECK constraints not yet implemented
		let check_constraints: Vec<CheckConstraintInfo> = Vec::new();

		Ok(TableInfo {
			name: table_name.to_string(),
			columns,
			indexes,
			primary_key,
			foreign_keys,
			unique_constraints,
			check_constraints,
		})
	}
}

#[cfg(feature = "mysql")]
#[async_trait]
impl DatabaseIntrospector for MySQLIntrospector {
	async fn read_schema(&self) -> Result<DatabaseSchema> {
		let table_query = r#"
			SELECT table_name FROM information_schema.tables
			WHERE table_schema = DATABASE() AND table_type = 'BASE TABLE'
		"#;
		let table_rows = sqlx::query(table_query)
			.fetch_all(&self.pool)
			.await
			.map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to fetch table list: {}", e))
			})?;

		let mut tables = HashMap::new();
		for row in &table_rows {
			let table_name = decode_mysql_text(row, 0, "table name")?;
			let table_info = self.introspect_table(&table_name).await?;
			tables.insert(table_name, table_info);
		}

		Ok(DatabaseSchema { tables })
	}

	async fn read_table(&self, table_name: &str) -> Result<Option<TableInfo>> {
		let exists_query = r#"
			SELECT table_name FROM information_schema.tables
			WHERE table_schema = DATABASE() AND table_type = 'BASE TABLE' AND table_name = ?
		"#;
		let exists = sqlx::query(exists_query)
			.bind(table_name)
			.fetch_optional(&self.pool)
			.await
			.map_err(|e| {
				MigrationError::IntrospectionError(format!(
					"Failed to check table existence: {}",
					e
				))
			})?;

		if exists.is_some() {
			Ok(Some(self.introspect_table(table_name).await?))
		} else {
			Ok(None)
		}
	}
}

/// SQLite schema introspector
#[cfg(feature = "sqlite")]
pub struct SQLiteIntrospector {
	pool: sqlx::SqlitePool,
}

#[cfg(feature = "sqlite")]
impl SQLiteIntrospector {
	/// Creates a new SQLite introspector with the given pool.
	pub fn new(pool: sqlx::SqlitePool) -> Self {
		Self { pool }
	}

	pub(crate) fn parse_sqlite_type(type_str: &str) -> super::FieldType {
		use super::FieldType;
		let upper = type_str.to_uppercase();
		let upper = upper.trim();
		match upper {
			"INTEGER" | "INT" => FieldType::Integer,
			"BIGINT" => FieldType::BigInteger,
			"SMALLINT" => FieldType::SmallInteger,
			"TINYINT" => FieldType::TinyInt,
			"TEXT" => FieldType::Text,
			"REAL" => FieldType::Real,
			"FLOAT" => FieldType::Float,
			"DOUBLE" | "DOUBLE PRECISION" => FieldType::Double,
			"BLOB" => FieldType::Blob,
			"BOOLEAN" => FieldType::Boolean,
			"DATE" => FieldType::Date,
			"TIME" => FieldType::Time,
			"DATETIME" => FieldType::DateTime,
			"TIMESTAMP" => FieldType::DateTime,
			"JSON" => FieldType::Json,
			"JSONB" => FieldType::JsonBinary,
			"UUID" => FieldType::Uuid,
			"NUMERIC" => FieldType::Decimal {
				precision: 10,
				scale: 2,
			},
			_ => {
				// Handle parameterized types: VARCHAR(n), CHAR(n), DECIMAL(p,s), etc.
				if let Some(rest) = upper.strip_prefix("VARCHAR(") {
					if let Some(len_str) = rest.strip_suffix(')')
						&& let Ok(len) = len_str.trim().parse::<u32>()
					{
						return FieldType::VarChar(len);
					}
					return FieldType::VarChar(255);
				}
				if let Some(rest) = upper.strip_prefix("CHAR(") {
					if let Some(len_str) = rest.strip_suffix(')')
						&& let Ok(len) = len_str.trim().parse::<u32>()
					{
						return FieldType::Char(len);
					}
					return FieldType::Char(1);
				}
				if let Some(rest) = upper.strip_prefix("DECIMAL(") {
					if let Some(params_str) = rest.strip_suffix(')') {
						let parts: Vec<&str> = params_str.split(',').collect();
						if parts.len() == 2
							&& let (Ok(p), Ok(s)) = (
								parts[0].trim().parse::<u32>(),
								parts[1].trim().parse::<u32>(),
							) {
							return FieldType::Decimal {
								precision: p,
								scale: s,
							};
						}
					}
					return FieldType::Decimal {
						precision: 10,
						scale: 2,
					};
				}
				if let Some(rest) = upper.strip_prefix("NUMERIC(") {
					if let Some(params_str) = rest.strip_suffix(')') {
						let parts: Vec<&str> = params_str.split(',').collect();
						if parts.len() == 2
							&& let (Ok(p), Ok(s)) = (
								parts[0].trim().parse::<u32>(),
								parts[1].trim().parse::<u32>(),
							) {
							return FieldType::Decimal {
								precision: p,
								scale: s,
							};
						}
					}
					return FieldType::Decimal {
						precision: 10,
						scale: 2,
					};
				}
				// Default fallback for unknown types
				// SQLite type affinity is determined by substrings in the declared
				// type, not by an exhaustive list of canonical spellings.
				if upper.contains("INT") {
					FieldType::Integer
				} else if upper.contains("CHAR") || upper.contains("CLOB") || upper.contains("TEXT")
				{
					FieldType::Text
				} else if upper.contains("BLOB") || upper.is_empty() {
					FieldType::Blob
				} else if upper.contains("REAL") || upper.contains("FLOA") || upper.contains("DOUB")
				{
					FieldType::Real
				} else {
					FieldType::Decimal {
						precision: 10,
						scale: 2,
					}
				}
			}
		}
	}

	/// Extracts foreign key information from SQLite using PRAGMA foreign_key_list.
	///
	/// SQLite's PRAGMA foreign_key_list returns:
	/// - id: FK constraint ID (for multi-column FKs, same ID for all columns)
	/// - seq: Column sequence in the FK
	/// - table: Referenced table name
	/// - from: Source column name
	/// - to: Referenced column name
	/// - on_update: ON UPDATE action
	/// - on_delete: ON DELETE action
	/// - match: MATCH clause (usually 'NONE')
	///
	/// This method also extracts actual constraint names from CREATE TABLE SQL
	/// when available (for named FK constraints like `CONSTRAINT fk_name FOREIGN KEY...`).
	async fn extract_foreign_keys(
		pool: &sqlx::SqlitePool,
		table_name: &str,
	) -> Result<Vec<ForeignKeyInfo>> {
		#[derive(sqlx::FromRow)]
		struct ForeignKeyRow {
			id: i64,
			seq: i64,
			table: String,
			from: String,
			to: String,
			on_update: String,
			on_delete: String,
			// Allow dead_code: field required for SQLite PRAGMA row deserialization
			#[allow(dead_code)]
			r#match: String,
		}

		// Get CREATE TABLE SQL to extract actual constraint names
		let create_sql = Self::get_create_table_sql(pool, table_name).await?;
		let named_fks = create_sql
			.as_ref()
			.map(|sql| Self::parse_fk_constraint_names(sql))
			.unwrap_or_default();

		// SQLite PRAGMA arguments cannot be passed via parameter binding, so
		// the identifier is interpolated through `quote_pragma_identifier`
		// (single-quoted, embedded quotes doubled). The `table_name` here
		// always originates from an internal `Operation` enum payload — never
		// from user-supplied input — so this is not a SQL-injection sink.
		// See issue #4454 for context.
		// nosemgrep: rust.actix.sql.sqlx-taint.sqlx-taint
		let query = format!(
			"PRAGMA foreign_key_list({})",
			super::sqlite_pragma::quote_pragma_identifier(table_name)
		);
		let rows: Vec<ForeignKeyRow> = sqlx::query_as(&query)
			.fetch_all(pool)
			.await
			.map_err(|e| MigrationError::IntrospectionError(e.to_string()))?;

		// Group by FK ID to handle multi-column foreign keys
		let mut fk_map: HashMap<i64, Vec<ForeignKeyRow>> = HashMap::new();
		for row in rows {
			fk_map.entry(row.id).or_default().push(row);
		}

		// Convert to ForeignKeyInfo
		let mut foreign_keys = Vec::new();
		for (fk_id, mut fk_rows) in fk_map {
			// Sort by sequence to maintain column order
			fk_rows.sort_by_key(|r| r.seq);

			let referenced_table = fk_rows[0].table.clone();
			let on_update = fk_rows[0].on_update.clone();
			let on_delete = fk_rows[0].on_delete.clone();

			let columns: Vec<String> = fk_rows.iter().map(|r| r.from.clone()).collect();
			let referenced_columns: Vec<String> = fk_rows.iter().map(|r| r.to.clone()).collect();

			// Try to find actual constraint name from CREATE TABLE SQL
			// If not found, fall back to generated name
			let signature = (columns.clone(), referenced_table.clone());
			let name = named_fks
				.get(&signature)
				.cloned()
				.unwrap_or_else(|| format!("fk_{}_{}", table_name, fk_id));

			foreign_keys.push(ForeignKeyInfo {
				name,
				columns,
				referenced_table,
				referenced_columns,
				on_delete: if on_delete == "NO ACTION" {
					None
				} else {
					Some(on_delete)
				},
				on_update: if on_update == "NO ACTION" {
					None
				} else {
					Some(on_update)
				},
			});
		}

		Ok(foreign_keys)
	}

	/// Extracts index information from SQLite using PRAGMA index_list and PRAGMA index_info.
	async fn extract_indexes(
		pool: &sqlx::SqlitePool,
		table_name: &str,
	) -> Result<HashMap<String, IndexInfo>> {
		#[derive(sqlx::FromRow)]
		struct IndexListRow {
			// Allow dead_code: field required for SQLite PRAGMA row deserialization
			#[allow(dead_code)]
			seq: i64,
			name: String,
			unique: i64,
			// Allow dead_code: field required for SQLite PRAGMA row deserialization
			#[allow(dead_code)]
			origin: String,
			// Allow dead_code: field required for SQLite PRAGMA row deserialization
			#[allow(dead_code)]
			partial: i64,
		}

		#[derive(sqlx::FromRow)]
		struct IndexInfoRow {
			// Allow dead_code: field required for SQLite PRAGMA row deserialization
			#[allow(dead_code)]
			seqno: i64,
			// Allow dead_code: field required for SQLite PRAGMA row deserialization
			#[allow(dead_code)]
			cid: i64,
			name: Option<String>,
		}

		let mut indexes = HashMap::new();

		// Get list of indexes for the table
		// SQLite PRAGMA: identifier interpolation via shared helper. Inputs
		// originate from internal migration operations; see issue #4454.
		// nosemgrep: rust.actix.sql.sqlx-taint.sqlx-taint
		let query = format!(
			"PRAGMA index_list({})",
			super::sqlite_pragma::quote_pragma_identifier(table_name)
		);
		let index_list: Vec<IndexListRow> = sqlx::query_as(&query)
			.fetch_all(pool)
			.await
			.map_err(|e| MigrationError::IntrospectionError(e.to_string()))?;

		for index_row in index_list {
			// Get columns for this index.
			// nosemgrep: rust.actix.sql.sqlx-taint.sqlx-taint
			let info_query = format!(
				"PRAGMA index_info({})",
				super::sqlite_pragma::quote_pragma_identifier(&index_row.name)
			);
			let index_info: Vec<IndexInfoRow> =
				sqlx::query_as(&info_query)
					.fetch_all(pool)
					.await
					.map_err(|e| MigrationError::IntrospectionError(e.to_string()))?;

			let columns: Vec<String> = index_info
				.into_iter()
				.filter_map(|info| info.name)
				.collect();

			indexes.insert(
				index_row.name.clone(),
				IndexInfo {
					name: index_row.name,
					columns,
					unique: index_row.unique != 0,
					#[cfg(feature = "pgvector")]
					access_method: None,
					index_type: None,
					#[cfg(feature = "pgvector")]
					expressions: None,
					#[cfg(feature = "pgvector")]
					operator_class: None,
					#[cfg(feature = "pgvector")]
					operator_class_is_default: false,
				},
			);
		}

		Ok(indexes)
	}

	/// Gets the CREATE TABLE SQL statement from sqlite_master.
	async fn get_create_table_sql(
		pool: &sqlx::SqlitePool,
		table_name: &str,
	) -> Result<Option<String>> {
		#[derive(sqlx::FromRow)]
		struct SqlRow {
			sql: Option<String>,
		}

		let query = "SELECT sql FROM sqlite_master WHERE type='table' AND name=?";
		let result: Option<SqlRow> = sqlx::query_as(query)
			.bind(table_name)
			.fetch_optional(pool)
			.await
			.map_err(|e| MigrationError::IntrospectionError(e.to_string()))?;

		Ok(result.and_then(|r| r.sql))
	}

	/// Extracts CHECK constraints from the CREATE TABLE SQL statement.
	///
	/// SQLite doesn't have a PRAGMA for CHECK constraints, so we parse them
	/// from the CREATE TABLE statement stored in sqlite_master.
	///
	/// Handles both:
	/// - Named CHECK: `CONSTRAINT check_name CHECK(expression)`
	/// - Anonymous CHECK: `CHECK(expression)`
	async fn extract_check_constraints(
		pool: &sqlx::SqlitePool,
		table_name: &str,
	) -> Result<Vec<CheckConstraintInfo>> {
		let create_sql = match Self::get_create_table_sql(pool, table_name).await? {
			Some(sql) => sql,
			None => return Ok(Vec::new()),
		};

		Self::parse_check_constraints(&create_sql)
	}

	/// Parses CHECK constraints from a CREATE TABLE SQL statement.
	pub(crate) fn parse_check_constraints(create_sql: &str) -> Result<Vec<CheckConstraintInfo>> {
		let mut constraints = Vec::new();

		// Named CHECK constraint pattern: CONSTRAINT name CHECK(...)
		// We need to handle nested parentheses in the expression
		let named_pattern = regex::Regex::new(r#"(?i)CONSTRAINT\s+["'`]?(\w+)["'`]?\s+CHECK\s*\("#)
			.map_err(|e| MigrationError::IntrospectionError(e.to_string()))?;

		// Anonymous CHECK pattern: CHECK(...) not preceded by CONSTRAINT
		let anon_pattern = regex::Regex::new(r#"(?i)CHECK\s*\("#)
			.map_err(|e| MigrationError::IntrospectionError(e.to_string()))?;

		// Pattern to check for CONSTRAINT name before CHECK
		let constraint_pattern =
			regex::Regex::new(r#"(?i)CONSTRAINT\s+["'`]?\w+["'`]?\s*$"#).unwrap();

		// Find named CHECK constraints
		for cap in named_pattern.captures_iter(create_sql) {
			let name = cap.get(1).map(|m| m.as_str().to_string());
			let match_end = cap.get(0).unwrap().end();

			// Extract expression by counting parentheses
			if let Some(expr) = Self::extract_parenthesized_expression(create_sql, match_end - 1) {
				constraints.push(CheckConstraintInfo {
					name,
					expression: expr,
				});
			}
		}

		// Find anonymous CHECK constraints
		// We need to exclude the named ones we already found
		for m in anon_pattern.find_iter(create_sql) {
			let start = m.start();

			// Check if this is preceded by CONSTRAINT (skip if so)
			let before = &create_sql[..start];
			if before.to_uppercase().trim_end().ends_with("CONSTRAINT") {
				continue;
			}

			// Also check for CONSTRAINT name pattern before CHECK
			if constraint_pattern.is_match(before.trim_end()) {
				continue;
			}

			let match_end = m.end();
			if let Some(expr) = Self::extract_parenthesized_expression(create_sql, match_end - 1) {
				constraints.push(CheckConstraintInfo {
					name: None,
					expression: expr,
				});
			}
		}

		Ok(constraints)
	}

	/// Extracts the content inside parentheses, handling nested parentheses.
	/// `start_pos` should be the position of the opening parenthesis.
	fn extract_parenthesized_expression(sql: &str, start_pos: usize) -> Option<String> {
		let chars: Vec<char> = sql.chars().collect();
		if start_pos >= chars.len() || chars[start_pos] != '(' {
			return None;
		}

		let mut depth = 0;
		let expr_start = start_pos + 1;
		let mut expr_end = start_pos + 1;

		for (i, &c) in chars.iter().enumerate().skip(start_pos) {
			match c {
				'(' => depth += 1,
				')' => {
					depth -= 1;
					if depth == 0 {
						expr_end = i;
						break;
					}
				}
				_ => {}
			}
		}

		if depth == 0 && expr_end > expr_start {
			let expr: String = chars[expr_start..expr_end].iter().collect();
			Some(expr.trim().to_string())
		} else {
			None
		}
	}

	/// Parses FK constraint names from a CREATE TABLE SQL statement.
	///
	/// Returns a HashMap where:
	/// - Key: (source_columns, referenced_table) as a signature
	/// - Value: constraint name
	///
	/// This is used to match PRAGMA foreign_key_list results with actual constraint names.
	pub(crate) fn parse_fk_constraint_names(
		create_sql: &str,
	) -> HashMap<(Vec<String>, String), String> {
		let mut result = HashMap::new();

		// Pattern: CONSTRAINT name FOREIGN KEY (cols) REFERENCES table(cols)
		let fk_pattern = regex::Regex::new(
			r#"(?i)CONSTRAINT\s+["'`]?(\w+)["'`]?\s+FOREIGN\s+KEY\s*\(([^)]+)\)\s*REFERENCES\s+["'`]?(\w+)["'`]?"#,
		);

		if let Ok(re) = fk_pattern {
			for cap in re.captures_iter(create_sql) {
				if let (Some(name), Some(cols), Some(ref_table)) =
					(cap.get(1), cap.get(2), cap.get(3))
				{
					let constraint_name = name.as_str().to_string();
					let columns: Vec<String> = cols
						.as_str()
						.split(',')
						.map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
						.collect();
					let referenced_table = ref_table.as_str().to_string();

					result.insert((columns, referenced_table), constraint_name);
				}
			}
		}

		result
	}

	/// Extracts unique constraints from PRAGMA index_list where origin = 'u'.
	async fn extract_unique_constraints(
		&self,
		table_name: &str,
	) -> Result<Vec<UniqueConstraintInfo>> {
		#[derive(sqlx::FromRow)]
		struct IndexListRow {
			// Allow dead_code: field required for SQLite PRAGMA row deserialization
			#[allow(dead_code)]
			// Column sequence number from PRAGMA index_list
			seq: i64,
			name: String,
			// Allow dead_code: field required for SQLite PRAGMA row deserialization
			#[allow(dead_code)]
			// Whether the index enforces uniqueness
			unique: i64,
			origin: String,
			// Allow dead_code: field required for SQLite PRAGMA row deserialization
			#[allow(dead_code)]
			// Whether this is a partial index
			partial: i64,
		}

		#[derive(sqlx::FromRow)]
		struct IndexInfoRow {
			// Allow dead_code: field required for SQLite PRAGMA row deserialization
			#[allow(dead_code)]
			// Column sequence number within the index
			seqno: i64,
			// Allow dead_code: field required for SQLite PRAGMA row deserialization
			#[allow(dead_code)]
			// Column ID in the table
			cid: i64,
			name: Option<String>,
		}

		// SQLite PRAGMA: identifier interpolation via shared helper. Inputs
		// originate from internal migration operations; see issue #4454.
		// nosemgrep: rust.actix.sql.sqlx-taint.sqlx-taint
		let query = format!(
			"PRAGMA index_list({})",
			super::sqlite_pragma::quote_pragma_identifier(table_name)
		);
		let index_list: Vec<IndexListRow> = sqlx::query_as(&query)
			.fetch_all(&self.pool)
			.await
			.map_err(|e| MigrationError::IntrospectionError(e.to_string()))?;

		let mut constraints = Vec::new();
		for index_row in index_list {
			if index_row.origin == "u" {
				// nosemgrep: rust.actix.sql.sqlx-taint.sqlx-taint
				let info_query = format!(
					"PRAGMA index_info({})",
					super::sqlite_pragma::quote_pragma_identifier(&index_row.name)
				);
				let index_info: Vec<IndexInfoRow> = sqlx::query_as(&info_query)
					.fetch_all(&self.pool)
					.await
					.map_err(|e| MigrationError::IntrospectionError(e.to_string()))?;

				let columns: Vec<String> = index_info
					.into_iter()
					.filter_map(|info| info.name)
					.collect();

				constraints.push(UniqueConstraintInfo {
					name: index_row.name,
					columns,
				});
			}
		}

		Ok(constraints)
	}

	/// Introspects a single table using PRAGMA queries.
	async fn introspect_table(&self, table_name: &str) -> Result<TableInfo> {
		#[derive(sqlx::FromRow)]
		struct TableInfoRow {
			// Allow dead_code: field required for SQLite PRAGMA row deserialization
			#[allow(dead_code)]
			// Column index from PRAGMA table_info
			cid: i64,
			name: String,
			r#type: String,
			notnull: i64,
			dflt_value: Option<String>,
			pk: i64,
			hidden: i64,
		}

		// SQLite PRAGMA: identifier interpolation via shared helper. Inputs
		// originate from internal migration operations; see issue #4454.
		// nosemgrep: rust.actix.sql.sqlx-taint.sqlx-taint
		let query = format!(
			"PRAGMA table_xinfo({})",
			super::sqlite_pragma::quote_pragma_identifier(table_name)
		);
		let rows: Vec<TableInfoRow> = sqlx::query_as(&query)
			.fetch_all(&self.pool)
			.await
			.map_err(|e| MigrationError::IntrospectionError(e.to_string()))?;

		// Check whether the table opts out of SQLite's rowid storage.
		let create_sql = Self::get_create_table_sql(&self.pool, table_name).await?;
		let without_rowid = create_sql
			.as_ref()
			.map(|sql| sql.to_uppercase().contains("WITHOUT ROWID"))
			.unwrap_or(false);
		let has_autoincrement = create_sql
			.as_ref()
			.map(|sql| sql.to_uppercase().contains("AUTOINCREMENT"))
			.unwrap_or(false);

		let mut columns = HashMap::new();

		// Build primary key ordered by pk field value
		let mut pk_entries: Vec<(i64, String)> = rows
			.iter()
			.filter(|r| r.pk > 0)
			.map(|r| (r.pk, r.name.clone()))
			.collect();
		pk_entries.sort_by_key(|(pk, _)| *pk);
		let primary_key: Vec<String> = pk_entries.into_iter().map(|(_, name)| name).collect();

		for row in &rows {
			let is_pk = row.pk > 0;

			// An INTEGER PRIMARY KEY aliases SQLite's rowid even without the
			// AUTOINCREMENT keyword. AUTOINCREMENT only changes rowid reuse.
			let is_rowid_alias = primary_key.len() == 1
				&& row.pk == 1
				&& row.r#type.trim().eq_ignore_ascii_case("INTEGER")
				&& !without_rowid;
			let is_auto = is_pk && (has_autoincrement || is_rowid_alias);

			// Primary key columns are implicitly NOT NULL in SQLite
			let nullable = if is_pk { false } else { row.notnull == 0 };

			// Preserve `dflt_value` verbatim as the raw SQL fragment (e.g.
			// `'pending'` including the surrounding quotes). Downstream DDL
			// emission paths in `operations.rs` round-trip this form
			// correctly:
			// - `format!("DEFAULT {}", default)` emits valid DDL
			//   (`DEFAULT 'pending'`, not the previously broken
			//   `DEFAULT pending`).
			// - `convert_default_value` already handles both quoted and
			//   plain forms.
			// See issue #4454 for context.
			let default = row
				.dflt_value
				.as_ref()
				.map(|v| super::sqlite_pragma::normalize_default_value(v));

			columns.insert(
				row.name.clone(),
				ColumnInfo {
					name: row.name.clone(),
					column_type: Self::parse_sqlite_type(&row.r#type),
					nullable,
					default,
					auto_increment: is_auto,
					generated: super::executor::parse_sqlite_generated_column(
						create_sql.as_deref(),
						&row.name,
						row.hidden,
					),
				},
			);
		}

		// Extract unique constraints from PRAGMA index_list where origin = 'u'
		let unique_constraints = self.extract_unique_constraints(table_name).await?;

		// Extract indexes using existing method
		let indexes = Self::extract_indexes(&self.pool, table_name).await?;

		// Extract foreign keys using existing method
		let foreign_keys = Self::extract_foreign_keys(&self.pool, table_name).await?;

		// Extract CHECK constraints using existing method
		let check_constraints = Self::extract_check_constraints(&self.pool, table_name).await?;

		Ok(TableInfo {
			name: table_name.to_string(),
			columns,
			indexes,
			primary_key,
			foreign_keys,
			unique_constraints,
			check_constraints,
		})
	}
}

#[cfg(feature = "sqlite")]
#[async_trait]
impl DatabaseIntrospector for SQLiteIntrospector {
	async fn read_schema(&self) -> Result<DatabaseSchema> {
		#[derive(sqlx::FromRow)]
		struct TableRow {
			name: String,
		}

		// Get all user tables (exclude SQLite internal tables)
		let query =
			"SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'";
		let table_rows: Vec<TableRow> = sqlx::query_as(query)
			.fetch_all(&self.pool)
			.await
			.map_err(|e| MigrationError::IntrospectionError(e.to_string()))?;

		let mut tables = HashMap::new();
		for table_row in table_rows {
			let table_info = self.introspect_table(&table_row.name).await?;
			tables.insert(table_info.name.clone(), table_info);
		}

		Ok(DatabaseSchema { tables })
	}

	async fn read_table(&self, table_name: &str) -> Result<Option<TableInfo>> {
		#[derive(sqlx::FromRow)]
		struct TableRow {
			// Allow dead_code: field required for SQLite PRAGMA row deserialization
			#[allow(dead_code)]
			// Table name from sqlite_master
			name: String,
		}

		// Check if the table exists
		let query = "SELECT name FROM sqlite_master WHERE type='table' AND name=?";
		let result: Option<TableRow> = sqlx::query_as(query)
			.bind(table_name)
			.fetch_optional(&self.pool)
			.await
			.map_err(|e| MigrationError::IntrospectionError(e.to_string()))?;

		match result {
			Some(_) => {
				let table_info = self.introspect_table(table_name).await?;
				Ok(Some(table_info))
			}
			None => Ok(None),
		}
	}
}

#[cfg(feature = "postgres")]
async fn read_postgres_schema(
	pool: sqlx::PgPool,
	options: &InspectDbOptions,
) -> Result<DatabaseSchema> {
	use sqlx::Row;

	let introspector = PostgresIntrospector::new(pool.clone());
	let mut schema = if options.tables.is_empty() {
		introspector.read_schema().await?
	} else {
		let mut tables = HashMap::new();
		for name in &options.tables {
			let table = match introspector.read_table(name).await? {
				Some(table) => table,
				None if options.include_views => {
					let view_exists = sqlx::query(
						"SELECT table_name FROM information_schema.views \
						 WHERE table_schema = current_schema() AND table_name = $1",
					)
					.bind(name)
					.fetch_optional(&pool)
					.await
					.map_err(|error| MigrationError::IntrospectionError(error.to_string()))?
					.is_some();
					if !view_exists {
						return Err(MigrationError::IntrospectionError(format!(
							"Table or view `{name}` was not found"
						)));
					}
					introspector.introspect_table(name).await?
				}
				None => {
					return Err(MigrationError::IntrospectionError(format!(
						"Table `{name}` was not found"
					)));
				}
			};
			tables.insert(name.clone(), table);
		}
		DatabaseSchema { tables }
	};
	let partition_names = read_postgres_partition_names(&pool).await?;
	filter_postgres_partitions(&mut schema, &partition_names, options.include_partitions);

	if options.include_views && options.tables.is_empty() {
		let rows = sqlx::query(
			"SELECT table_name FROM information_schema.views WHERE table_schema = current_schema() ORDER BY table_name",
		)
		.fetch_all(&pool)
		.await
		.map_err(|error| MigrationError::IntrospectionError(error.to_string()))?;
		for row in rows {
			let name: String = row.try_get("table_name").map_err(|error| {
				MigrationError::IntrospectionError(format!("Failed to read view name: {error}"))
			})?;
			let table = introspector.introspect_table(&name).await?;
			schema.tables.insert(name, table);
		}
	}

	Ok(schema)
}

#[cfg(feature = "postgres")]
async fn read_postgres_partition_names(pool: &sqlx::PgPool) -> Result<HashSet<String>> {
	use sqlx::Row;

	let rows = sqlx::query(
		"SELECT relation.relname AS table_name, relation.relispartition AS is_partition \
		 FROM pg_class relation \
		 JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace \
		 WHERE namespace.nspname = current_schema() AND relation.relkind IN ('r', 'p') \
		 ORDER BY relation.relname",
	)
	.fetch_all(pool)
	.await
	.map_err(|error| MigrationError::IntrospectionError(error.to_string()))?;

	let relations = rows
		.into_iter()
		.map(|row| {
			let name = row.try_get("table_name").map_err(|error| {
				MigrationError::IntrospectionError(format!(
					"Failed to read PostgreSQL relation name: {error}"
				))
			})?;
			let is_partition = row.try_get("is_partition").map_err(|error| {
				MigrationError::IntrospectionError(format!(
					"Failed to read PostgreSQL partition flag: {error}"
				))
			})?;
			Ok(PostgresRelation { name, is_partition })
		})
		.collect::<Result<Vec<_>>>()?;

	Ok(postgres_partition_names(relations))
}

#[cfg(feature = "mysql")]
async fn read_mysql_schema(
	pool: sqlx::MySqlPool,
	options: &InspectDbOptions,
) -> Result<DatabaseSchema> {
	let introspector = MySQLIntrospector::new(pool.clone());
	let mut schema = if options.tables.is_empty() {
		introspector.read_schema().await?
	} else {
		let mut tables = HashMap::new();
		for name in &options.tables {
			let table = match introspector.read_table(name).await? {
				Some(table) => table,
				None if options.include_views => {
					let view_exists = sqlx::query(
						"SELECT table_name FROM information_schema.views \
						 WHERE table_schema = DATABASE() AND table_name = ?",
					)
					.bind(name)
					.fetch_optional(&pool)
					.await
					.map_err(|error| MigrationError::IntrospectionError(error.to_string()))?
					.is_some();
					if !view_exists {
						return Err(MigrationError::IntrospectionError(format!(
							"Table or view `{name}` was not found"
						)));
					}
					introspector.introspect_table(name).await?
				}
				None => {
					return Err(MigrationError::IntrospectionError(format!(
						"Table `{name}` was not found"
					)));
				}
			};
			tables.insert(name.clone(), table);
		}
		DatabaseSchema { tables }
	};
	if !options.include_views || !options.tables.is_empty() {
		return Ok(schema);
	}

	let rows = sqlx::query(
		"SELECT table_name FROM information_schema.views WHERE table_schema = DATABASE() ORDER BY table_name",
	)
	.fetch_all(&pool)
	.await
	.map_err(|error| MigrationError::IntrospectionError(error.to_string()))?;
	for row in rows {
		let name = decode_mysql_text(&row, 0, "view name")?;
		let table = introspector.introspect_table(&name).await?;
		schema.tables.insert(name, table);
	}

	Ok(schema)
}

#[cfg(feature = "sqlite")]
async fn read_sqlite_schema(
	pool: sqlx::SqlitePool,
	options: &InspectDbOptions,
) -> Result<DatabaseSchema> {
	use sqlx::Row;

	let introspector = SQLiteIntrospector::new(pool.clone());
	let mut schema = if options.tables.is_empty() {
		introspector.read_schema().await?
	} else {
		let mut tables = HashMap::new();
		for name in &options.tables {
			let table = match introspector.read_table(name).await? {
				Some(table) => table,
				None if options.include_views => {
					let view_exists = sqlx::query(
						"SELECT name FROM sqlite_master WHERE type = 'view' AND name = ?",
					)
					.bind(name)
					.fetch_optional(&pool)
					.await
					.map_err(|error| MigrationError::IntrospectionError(error.to_string()))?
					.is_some();
					if !view_exists {
						return Err(MigrationError::IntrospectionError(format!(
							"Table or view `{name}` was not found"
						)));
					}
					introspector.introspect_table(name).await?
				}
				None => {
					return Err(MigrationError::IntrospectionError(format!(
						"Table `{name}` was not found"
					)));
				}
			};
			tables.insert(name.clone(), table);
		}
		DatabaseSchema { tables }
	};
	if !options.include_views || !options.tables.is_empty() {
		return Ok(schema);
	}

	let rows = sqlx::query(
		"SELECT name FROM sqlite_master WHERE type = 'view' AND name NOT LIKE 'sqlite_%' ORDER BY name",
	)
	.fetch_all(&pool)
	.await
	.map_err(|error| MigrationError::IntrospectionError(error.to_string()))?;
	for row in rows {
		let name: String = row.try_get("name").map_err(|error| {
			MigrationError::IntrospectionError(format!("Failed to read view name: {error}"))
		})?;
		let table = introspector.introspect_table(&name).await?;
		schema.tables.insert(name, table);
	}

	Ok(schema)
}

#[cfg(all(test, feature = "pgvector"))]
mod postgres_index_metadata_tests {
	use super::*;
	use crate::migrations::IndexType;

	#[test]
	fn vector_index_postgres_reloptions_parse_to_typed_methods() {
		// Act and assert
		assert_eq!(
			parse_postgres_index_type(
				"hnsw",
				&["ef_construction=64".to_string(), "m=16".to_string()]
			)
			.unwrap(),
			Some(IndexType::Hnsw {
				m: Some(16),
				ef_construction: Some(64),
			})
		);
		assert_eq!(
			parse_postgres_index_type("ivfflat", &["lists=100".to_string()]).unwrap(),
			Some(IndexType::Ivfflat { lists: Some(100) })
		);
	}

	#[test]
	fn vector_index_postgres_reloptions_reject_malformed_values() {
		// Act
		let result = parse_postgres_index_type("hnsw", &["ef_construction=invalid".to_string()]);

		// Assert
		assert!(matches!(
			result,
			Err(MigrationError::IntrospectionError(message))
				if message == "invalid PostgreSQL hnsw reloption ef_construction=invalid"
		));
	}

	#[test]
	fn mixed_multi_column_postgres_operator_classes_are_conservatively_represented() {
		// Arrange
		let operator_classes = vec!["int4_ops".to_string(), "text_pattern_ops".to_string()];
		let operator_class_defaults = vec![true, false];

		// Act
		let result = resolve_postgres_operator_class(
			"idx_events_sequence_name",
			Some(IndexType::BTree),
			&operator_classes,
			&operator_class_defaults,
		);

		// Assert
		assert_eq!(result.unwrap(), (None, false));
	}

	#[test]
	fn default_multi_column_postgres_operator_classes_are_representable() {
		// Arrange
		let operator_classes = vec!["int4_ops".to_string(), "text_ops".to_string()];
		let operator_class_defaults = vec![true, true];

		// Act
		let result = resolve_postgres_operator_class(
			"idx_events_sequence_name",
			Some(IndexType::BTree),
			&operator_classes,
			&operator_class_defaults,
		);

		// Assert
		assert_eq!(result.unwrap(), (Some("int4_ops".to_string()), true));
	}

	#[test]
	fn homogeneous_custom_multi_column_postgres_operator_classes_are_representable() {
		// Arrange
		let operator_classes = vec!["custom_ops".to_string(), "custom_ops".to_string()];
		let operator_class_defaults = vec![false, false];

		// Act
		let result = resolve_postgres_operator_class(
			"idx_events_sequence_partition",
			Some(IndexType::BTree),
			&operator_classes,
			&operator_class_defaults,
		);

		// Assert
		assert_eq!(result.unwrap(), (Some("custom_ops".to_string()), false));
	}

	#[test]
	fn multi_key_approximate_postgres_indexes_are_rejected() {
		// Arrange
		let operator_classes = vec!["vector_l2_ops".to_string(), "vector_l2_ops".to_string()];
		let operator_class_defaults = vec![true, true];

		// Act
		let result = resolve_postgres_operator_class(
			"idx_source_embeddings",
			Some(IndexType::Hnsw {
				m: Some(16),
				ef_construction: Some(64),
			}),
			&operator_classes,
			&operator_class_defaults,
		);

		// Assert
		assert!(matches!(
			result,
			Err(MigrationError::IntrospectionError(message))
				if message
					== "PostgreSQL approximate index idx_source_embeddings must have exactly one key"
		));
	}

	#[test]
	fn mixed_column_and_expression_indexes_are_rejected() {
		let error = validate_postgres_index_key_kinds("idx_mixed", 1, 1)
			.expect_err("mixed index keys must not produce lossy metadata");

		assert!(matches!(
			error,
			MigrationError::IntrospectionError(message)
				if message == "PostgreSQL index idx_mixed mixes column and expression keys, which cannot be represented"
		));
	}
}

#[cfg(all(test, feature = "postgres", feature = "pgvector"))]
mod postgres_tests {
	use super::PostgresIntrospector;
	use crate::migrations::FieldType;

	#[test]
	fn parses_dimensioned_pgvector_type() {
		let field_type = PostgresIntrospector::parse_pg_type(
			"vector",
			"USER-DEFINED",
			Some("vector(1536)"),
			None,
			None,
			None,
			None,
		);

		assert_eq!(field_type, FieldType::Vector { dimensions: 1536 });
	}

	#[test]
	fn parses_schema_qualified_pgvector_type() {
		let field_type = PostgresIntrospector::parse_pg_type(
			"vector",
			"USER-DEFINED",
			Some("extensions.vector(1536)"),
			None,
			None,
			None,
			None,
		);

		assert_eq!(field_type, FieldType::Vector { dimensions: 1536 });
	}

	#[test]
	fn parses_dimensioned_pgvector_array_type() {
		let field_type = PostgresIntrospector::parse_pg_type(
			"_vector",
			"ARRAY",
			Some("vector(3)[]"),
			None,
			None,
			None,
			None,
		);

		assert_eq!(
			field_type,
			FieldType::Array(Box::new(FieldType::Vector { dimensions: 3 }))
		);
	}

	#[test]
	fn rejects_undimensioned_or_out_of_range_pgvector_types() {
		for type_definition in ["vector", "vector(0)", "vector(2001)"] {
			let field_type = PostgresIntrospector::parse_pg_type(
				"vector",
				"USER-DEFINED",
				Some(type_definition),
				None,
				None,
				None,
				None,
			);

			assert_eq!(field_type, FieldType::Custom("vector".to_string()));
		}
	}
}

#[cfg(test)]
#[cfg(feature = "sqlite")]
mod tests {
	use super::*;
	use crate::migrations::FieldType;
	use std::collections::HashSet;

	#[test]
	fn partition_validation_rejects_mysql_without_connecting() {
		let error = validate_partition_option(DatabaseType::Mysql, true)
			.expect_err("MySQL must reject PostgreSQL-only partitions before database I/O");
		assert_eq!(
			error.to_string(),
			"Introspection error: include_partitions is only supported for PostgreSQL"
		);
	}

	#[test]
	#[cfg(feature = "mysql")]
	fn mysql_catalog_integer_conversion_rejects_values_outside_u32() {
		let negative = mysql_catalog_u32(Some(-1), 255, "character maximum length")
			.expect_err("negative catalog metadata must be rejected");
		assert_eq!(
			negative.to_string(),
			"Introspection error: MySQL character maximum length is outside the supported u32 range",
		);

		let overflow = mysql_catalog_u32(Some(i64::from(u32::MAX) + 1), 10, "numeric precision")
			.expect_err("oversized catalog metadata must be rejected");
		assert_eq!(
			overflow.to_string(),
			"Introspection error: MySQL numeric precision is outside the supported u32 range",
		);
	}

	#[test]
	#[cfg(feature = "mysql")]
	fn mysql_unsigned_integer_columns_map_to_unsigned_rust_types() {
		assert_eq!(
			MySQLIntrospector::parse_mysql_type("tinyint", "tinyint unsigned", None, None, None)
				.expect("type should parse"),
			FieldType::Custom("u8".to_string())
		);
		assert_eq!(
			MySQLIntrospector::parse_mysql_type("bigint", "bigint unsigned", None, None, None)
				.expect("type should parse"),
			FieldType::Custom("u64".to_string())
		);
	}

	#[test]
	fn sqlite_type_affinity_handles_legal_noncanonical_declarations() {
		assert_eq!(
			SQLiteIntrospector::parse_sqlite_type("UNSIGNED BIG INT"),
			FieldType::Integer
		);
		assert_eq!(
			SQLiteIntrospector::parse_sqlite_type("CHARACTER(20)"),
			FieldType::Text
		);
		assert_eq!(
			SQLiteIntrospector::parse_sqlite_type("DOUBLE UNSIGNED"),
			FieldType::Real
		);
	}

	#[test]
	#[cfg(feature = "postgres")]
	fn postgres_partition_filter_excludes_only_partition_children() {
		let mut tables = HashMap::new();
		for name in ["orders", "ordinary_inheritance_child", "orders_2026"] {
			tables.insert(
				name.to_string(),
				TableInfo {
					name: name.to_string(),
					columns: HashMap::new(),
					indexes: HashMap::new(),
					primary_key: Vec::new(),
					foreign_keys: Vec::new(),
					unique_constraints: Vec::new(),
					check_constraints: Vec::new(),
				},
			);
		}
		let partition_names = postgres_partition_names([
			PostgresRelation {
				name: "orders".to_string(),
				is_partition: false,
			},
			PostgresRelation {
				name: "ordinary_inheritance_child".to_string(),
				is_partition: false,
			},
			PostgresRelation {
				name: "orders_2026".to_string(),
				is_partition: true,
			},
		]);

		let mut without_partitions = DatabaseSchema {
			tables: tables.clone(),
		};
		filter_postgres_partitions(&mut without_partitions, &partition_names, false);
		assert_eq!(
			without_partitions
				.tables
				.keys()
				.cloned()
				.collect::<HashSet<_>>(),
			HashSet::from([
				"orders".to_string(),
				"ordinary_inheritance_child".to_string(),
			])
		);

		let mut with_partitions = DatabaseSchema { tables };
		filter_postgres_partitions(&mut with_partitions, &partition_names, true);
		assert_eq!(
			with_partitions
				.tables
				.keys()
				.cloned()
				.collect::<HashSet<_>>(),
			HashSet::from([
				"orders".to_string(),
				"ordinary_inheritance_child".to_string(),
				"orders_2026".to_string(),
			])
		);
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn test_sqlite_introspector_read_schema() {
		use sqlx::SqlitePool;

		let pool = SqlitePool::connect("sqlite::memory:")
			.await
			.expect("Failed to create pool");

		// Create a test table
		sqlx::query(
			r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                email TEXT UNIQUE NOT NULL,
                age INTEGER
            )
            "#,
		)
		.execute(&pool)
		.await
		.expect("Failed to create table");

		let introspector = SQLiteIntrospector::new(pool);
		let schema = introspector
			.read_schema()
			.await
			.expect("Failed to read schema");

		assert!(schema.tables.contains_key("users"));
		let users_table = &schema.tables["users"];
		assert_eq!(users_table.name, "users");
		assert_eq!(users_table.columns.len(), 4);

		// Check id column
		let id_col = &users_table.columns["id"];
		assert_eq!(id_col.name, "id");
		assert_eq!(id_col.column_type, FieldType::Integer);
		assert!(id_col.auto_increment);
		assert!(!id_col.nullable);

		// Check name column
		let name_col = &users_table.columns["name"];
		assert_eq!(name_col.name, "name");
		assert_eq!(name_col.column_type, FieldType::Text);
		assert!(!name_col.nullable);
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn sqlite_integer_primary_key_without_autoincrement_is_generated() {
		use sqlx::SqlitePool;

		let pool = SqlitePool::connect("sqlite::memory:")
			.await
			.expect("in-memory SQLite pool should connect");
		sqlx::query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
			.execute(&pool)
			.await
			.expect("test table should be created");

		let schema = SQLiteIntrospector::new(pool)
			.read_schema()
			.await
			.expect("schema should be introspected");
		assert!(schema.tables["users"].columns["id"].auto_increment);
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn test_sqlite_introspector_read_table() {
		use sqlx::SqlitePool;

		let pool = SqlitePool::connect("sqlite::memory:")
			.await
			.expect("Failed to create pool");

		// Create a test table
		sqlx::query(
			r#"
            CREATE TABLE posts (
                id INTEGER PRIMARY KEY,
                title TEXT NOT NULL,
                content TEXT
            )
            "#,
		)
		.execute(&pool)
		.await
		.expect("Failed to create table");

		let introspector = SQLiteIntrospector::new(pool);
		let table = introspector
			.read_table("posts")
			.await
			.expect("Failed to read table");

		let posts_table = table.unwrap();
		assert_eq!(posts_table.name, "posts");
		assert_eq!(posts_table.columns.len(), 3);

		// Check non-existent table
		let missing = introspector
			.read_table("non_existent")
			.await
			.expect("Failed to read table");
		assert!(missing.is_none());
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn test_sqlite_introspector_with_indexes() {
		use sqlx::SqlitePool;

		let pool = SqlitePool::connect("sqlite::memory:")
			.await
			.expect("Failed to create pool");

		// Create table with indexes
		sqlx::query(
			r#"
            CREATE TABLE products (
                id INTEGER PRIMARY KEY,
                sku TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL
            )
            "#,
		)
		.execute(&pool)
		.await
		.expect("Failed to create table");

		sqlx::query("CREATE INDEX idx_products_name ON products(name)")
			.execute(&pool)
			.await
			.expect("Failed to create index");

		let introspector = SQLiteIntrospector::new(pool);
		let schema = introspector
			.read_schema()
			.await
			.expect("Failed to read schema");

		let products_table = &schema.tables["products"];
		assert!(!products_table.indexes.is_empty());
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn test_sqlite_introspector_foreign_keys_and_unique_constraints() {
		use sqlx::SqlitePool;

		let pool = SqlitePool::connect("sqlite::memory:")
			.await
			.expect("Failed to create pool");

		// Create tables with foreign keys and unique constraints
		sqlx::query(
			r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                email TEXT UNIQUE NOT NULL,
                username TEXT UNIQUE NOT NULL,
                name TEXT NOT NULL
            )
            "#,
		)
		.execute(&pool)
		.await
		.expect("Failed to create users table");

		sqlx::query(
			r#"
            CREATE TABLE posts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                title TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE ON UPDATE CASCADE
            )
            "#,
		)
		.execute(&pool)
		.await
		.expect("Failed to create posts table");

		let introspector = SQLiteIntrospector::new(pool);
		let schema = introspector
			.read_schema()
			.await
			.expect("Failed to read schema");

		// Check users table
		assert!(schema.tables.contains_key("users"));
		let users_table = &schema.tables["users"];

		// Check unique constraints (email and username)
		assert!(
			!users_table.unique_constraints.is_empty(),
			"Users table should have unique constraints"
		);

		// Check posts table
		assert!(schema.tables.contains_key("posts"));
		let posts_table = &schema.tables["posts"];

		// Verify foreign key on posts table
		assert!(
			!posts_table.foreign_keys.is_empty(),
			"Posts table should have foreign key constraint"
		);

		let fk = &posts_table.foreign_keys[0];
		assert_eq!(
			fk.referenced_table, "users",
			"Foreign key should reference users table"
		);
		assert_eq!(
			fk.columns,
			vec!["user_id"],
			"Foreign key should be on user_id column"
		);
		assert_eq!(
			fk.referenced_columns,
			vec!["id"],
			"Foreign key should reference id column"
		);
		assert_eq!(
			fk.on_delete,
			Some("CASCADE".to_string()),
			"Foreign key should have CASCADE on delete"
		);
		assert_eq!(
			fk.on_update,
			Some("CASCADE".to_string()),
			"Foreign key should have CASCADE on update"
		);
	}
}
