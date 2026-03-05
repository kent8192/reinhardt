//! Database schema introspection
//!
//! This module provides functionality to read the current database schema
//! and extract table definitions, column metadata, indexes, and constraints.

use async_trait::async_trait;
use std::collections::HashMap;

use super::{MigrationError, Result};

/// Schema information extracted from a database
#[derive(Debug, Clone, PartialEq)]
pub struct DatabaseSchema {
	/// All tables in the schema
	pub tables: HashMap<String, TableInfo>,
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
	/// Index type (e.g., BTREE, HASH)
	pub index_type: Option<String>,
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

/// PostgreSQL schema introspector
#[cfg(feature = "postgres")]
pub struct PostgresIntrospector {
	pool: sqlx::PgPool,
}

#[cfg(feature = "postgres")]
impl PostgresIntrospector {
	pub fn new(pool: sqlx::PgPool) -> Self {
		Self { pool }
	}

	/// Maps PostgreSQL udt_name/data_type to FieldType
	fn parse_pg_type(
		udt_name: &str,
		data_type: &str,
		char_max_length: Option<i32>,
		numeric_precision: Option<i32>,
		numeric_scale: Option<i32>,
		enum_values: Option<Vec<String>>,
	) -> super::FieldType {
		use super::FieldType;
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

			// Array types (udt_name starts with _)
			name if name.starts_with('_') => {
				let inner = Self::parse_pg_type(
					&name[1..],
					data_type,
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

	/// Fetches enum label values for a given PostgreSQL enum type name
	async fn fetch_enum_values(&self, type_name: &str) -> Result<Vec<String>> {
		use sqlx::Row;
		let query = r#"
			SELECT e.enumlabel
			FROM pg_enum e
			JOIN pg_type t ON e.enumtypid = t.oid
			JOIN pg_namespace n ON t.typnamespace = n.oid
			WHERE t.typname = $1 AND n.nspname = 'public'
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
			SELECT column_name, udt_name, data_type, is_nullable, column_default,
			       character_maximum_length, numeric_precision, numeric_scale,
			       is_identity, identity_generation
			FROM information_schema.columns
			WHERE table_schema = 'public' AND table_name = $1
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
			WHERE tc.table_schema = 'public' AND tc.table_name = $1
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
			WHERE tc.table_schema = 'public' AND tc.table_name = $1
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
			WHERE tc.table_schema = 'public' AND tc.table_name = $1
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
				array_agg(a.attname ORDER BY array_position(ix.indkey, a.attnum)) AS column_names,
				ix.indisunique AS is_unique,
				am.amname AS index_type
			FROM
				pg_class t,
				pg_class i,
				pg_index ix,
				pg_attribute a,
				pg_am am,
				pg_namespace n
			WHERE
				t.oid = ix.indrelid
				AND i.oid = ix.indexrelid
				AND a.attrelid = t.oid
				AND a.attnum = ANY(ix.indkey)
				AND t.relkind = 'r'
				AND t.relname = $1
				AND i.relam = am.oid
				AND NOT ix.indisprimary
				AND n.oid = t.relnamespace
				AND n.nspname = 'public'
			GROUP BY i.relname, ix.indisunique, am.amname
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
			let index_type: String = row.try_get("index_type").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get index_type: {}", e))
			})?;

			indexes.insert(
				index_name.clone(),
				IndexInfo {
					name: index_name,
					columns: column_names,
					unique: is_unique,
					index_type: Some(index_type),
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
			WHERE table_schema = 'public' AND table_type = 'BASE TABLE'
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
			WHERE table_schema = 'public' AND table_type = 'BASE TABLE' AND table_name = $1
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
impl MySQLIntrospector {
	pub fn new(pool: sqlx::MySqlPool) -> Self {
		Self { pool }
	}

	fn parse_mysql_type(
		data_type: &str,
		column_type: &str,
		char_max_length: Option<i64>,
		numeric_precision: Option<i64>,
		numeric_scale: Option<i64>,
	) -> super::FieldType {
		use super::FieldType;
		let data_type_lower = data_type.to_lowercase();
		match data_type_lower.as_str() {
			"tinyint" => {
				// MySQL uses tinyint(1) for boolean
				if column_type.to_lowercase().starts_with("tinyint(1)") {
					FieldType::Boolean
				} else {
					FieldType::TinyInt
				}
			}
			"smallint" => FieldType::SmallInteger,
			"mediumint" => FieldType::MediumInt,
			"int" | "integer" => FieldType::Integer,
			"bigint" => FieldType::BigInteger,

			"varchar" => FieldType::VarChar(char_max_length.unwrap_or(255) as u32),
			"char" => FieldType::Char(char_max_length.unwrap_or(1) as u32),
			"text" => FieldType::Text,
			"tinytext" => FieldType::TinyText,
			"mediumtext" => FieldType::MediumText,
			"longtext" => FieldType::LongText,

			"decimal" | "numeric" => FieldType::Decimal {
				precision: numeric_precision.unwrap_or(10) as u32,
				scale: numeric_scale.unwrap_or(2) as u32,
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
		}
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
			       column_key, extra, character_maximum_length, numeric_precision, numeric_scale
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
			let column_name: String = row.try_get("column_name").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get column_name: {}", e))
			})?;
			let data_type: String = row.try_get("data_type").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get data_type: {}", e))
			})?;
			let column_type_str: String = row.try_get("column_type").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get column_type: {}", e))
			})?;
			let is_nullable: String = row.try_get("is_nullable").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get is_nullable: {}", e))
			})?;
			let column_default: Option<String> = row.try_get("column_default").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get column_default: {}", e))
			})?;
			let column_key: String = row.try_get("column_key").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get column_key: {}", e))
			})?;
			let extra: String = row.try_get("extra").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get extra: {}", e))
			})?;
			let char_max_length: Option<i64> =
				row.try_get("character_maximum_length").map_err(|e| {
					MigrationError::IntrospectionError(format!(
						"Failed to get character_maximum_length: {}",
						e
					))
				})?;
			let numeric_precision: Option<i64> = row.try_get("numeric_precision").map_err(|e| {
				MigrationError::IntrospectionError(format!(
					"Failed to get numeric_precision: {}",
					e
				))
			})?;
			let numeric_scale: Option<i64> = row.try_get("numeric_scale").map_err(|e| {
				MigrationError::IntrospectionError(format!("Failed to get numeric_scale: {}", e))
			})?;

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
			);

			columns.insert(
				column_name.clone(),
				ColumnInfo {
					name: column_name,
					column_type: field_type,
					nullable: is_nullable == "YES",
					default: column_default,
					auto_increment: is_auto,
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
			let index_name: String = row.try_get("index_name").unwrap_or_default();
			let column_name: String = row.try_get("column_name").unwrap_or_default();
			let non_unique: i64 = row.try_get("non_unique").unwrap_or(1);
			let index_type: String = row.try_get("index_type").unwrap_or_default();

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
					index_type: Some(idx_type.clone()),
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
			let constraint_name: String = row.try_get("constraint_name").unwrap_or_default();
			let column_name: String = row.try_get("column_name").unwrap_or_default();
			let ref_table: String = row.try_get("referenced_table_name").unwrap_or_default();
			let ref_column: String = row.try_get("referenced_column_name").unwrap_or_default();
			let update_rule: String = row.try_get("update_rule").unwrap_or_default();
			let delete_rule: String = row.try_get("delete_rule").unwrap_or_default();

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
		use sqlx::Row;

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
			let table_name: String = row.try_get("table_name").unwrap_or_default();
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
	pub fn new(pool: sqlx::SqlitePool) -> Self {
		Self { pool }
	}

	fn parse_sqlite_type(type_str: &str) -> super::FieldType {
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
				FieldType::Custom(type_str.to_string())
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

		let query = format!("PRAGMA foreign_key_list({})", table_name);
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
		let query = format!("PRAGMA index_list({})", table_name);
		let index_list: Vec<IndexListRow> = sqlx::query_as(&query)
			.fetch_all(pool)
			.await
			.map_err(|e| MigrationError::IntrospectionError(e.to_string()))?;

		for index_row in index_list {
			// Get columns for this index
			let info_query = format!("PRAGMA index_info({})", index_row.name);
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
					index_type: None,
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
	fn parse_check_constraints(create_sql: &str) -> Result<Vec<CheckConstraintInfo>> {
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
	fn parse_fk_constraint_names(create_sql: &str) -> HashMap<(Vec<String>, String), String> {
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

		let query = format!("PRAGMA index_list({})", table_name);
		let index_list: Vec<IndexListRow> = sqlx::query_as(&query)
			.fetch_all(&self.pool)
			.await
			.map_err(|e| MigrationError::IntrospectionError(e.to_string()))?;

		let mut constraints = Vec::new();
		for index_row in index_list {
			if index_row.origin == "u" {
				let info_query = format!("PRAGMA index_info({})", index_row.name);
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
		}

		let query = format!("PRAGMA table_info({})", table_name);
		let rows: Vec<TableInfoRow> = sqlx::query_as(&query)
			.fetch_all(&self.pool)
			.await
			.map_err(|e| MigrationError::IntrospectionError(e.to_string()))?;

		// Check AUTOINCREMENT by inspecting CREATE TABLE SQL
		let create_sql = Self::get_create_table_sql(&self.pool, table_name).await?;
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

			// AUTOINCREMENT only applies to INTEGER PRIMARY KEY columns
			let is_auto = is_pk && has_autoincrement;

			// Primary key columns are implicitly NOT NULL in SQLite
			let nullable = if is_pk { false } else { row.notnull == 0 };

			// Parse default value - trim surrounding quotes for string defaults
			let default = row.dflt_value.as_ref().map(|v| {
				let trimmed = v.trim();
				if (trimmed.starts_with('\'') && trimmed.ends_with('\''))
					|| (trimmed.starts_with('"') && trimmed.ends_with('"'))
				{
					trimmed[1..trimmed.len() - 1].to_string()
				} else {
					trimmed.to_string()
				}
			});

			columns.insert(
				row.name.clone(),
				ColumnInfo {
					name: row.name.clone(),
					column_type: Self::parse_sqlite_type(&row.r#type),
					nullable,
					default,
					auto_increment: is_auto,
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

#[cfg(test)]
#[cfg(feature = "sqlite")]
mod tests {
	use super::*;
	use crate::migrations::FieldType;

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
