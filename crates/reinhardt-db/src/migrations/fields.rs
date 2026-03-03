//! Field type definitions for migrations

use serde::{Deserialize, Serialize};

/// Represents database field types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FieldType {
	// Integer types
	BigInteger,
	Integer,
	SmallInteger,
	TinyInt,   // MySQL-specific
	MediumInt, // MySQL-specific

	// String types (with parameters)
	Char(u32),
	VarChar(u32),
	Text,
	TinyText,   // MySQL-specific
	MediumText, // MySQL-specific
	LongText,   // MySQL-specific

	// Date/time types
	Date,
	Time,
	DateTime,
	TimestampTz, // PostgreSQL TIMESTAMPTZ

	// Numeric types
	Decimal {
		precision: u32,
		scale: u32,
	},
	Float,
	Double,
	Real,

	// Boolean type
	Boolean,

	// Binary types
	Binary,
	Blob,       // MySQL-specific
	TinyBlob,   // MySQL-specific
	MediumBlob, // MySQL-specific
	LongBlob,   // MySQL-specific
	Bytea,      // PostgreSQL-specific

	// JSON types
	Json,
	JsonBinary, // PostgreSQL JSONB

	// PostgreSQL-specific types
	/// PostgreSQL Array type with inner element type
	Array(Box<FieldType>),
	/// PostgreSQL HStore key-value store
	HStore,
	/// PostgreSQL case-insensitive text
	CIText,
	/// PostgreSQL int4range (integer range)
	Int4Range,
	/// PostgreSQL int8range (bigint range)
	Int8Range,
	/// PostgreSQL numrange (numeric range)
	NumRange,
	/// PostgreSQL daterange
	DateRange,
	/// PostgreSQL tsrange (timestamp range without timezone)
	TsRange,
	/// PostgreSQL tstzrange (timestamp range with timezone)
	TsTzRange,
	/// PostgreSQL tsvector for full-text search
	TsVector,
	/// PostgreSQL tsquery for full-text search queries
	TsQuery,

	// Other types
	Uuid,
	Year, // MySQL-specific

	// MySQL-specific collection types
	Enum {
		values: Vec<String>,
	},
	Set {
		values: Vec<String>,
	},

	// Relationship field types
	/// ForeignKey relationship field
	ForeignKey {
		to_table: String,
		to_field: String,
		on_delete: super::ForeignKeyAction,
	},

	/// OneToOne relationship field
	OneToOne {
		to: String, // "app.Model" format
		on_delete: super::ForeignKeyAction,
		on_update: super::ForeignKeyAction,
	},

	/// ManyToMany relationship field
	ManyToMany {
		to: String,              // "app.Model" format
		through: Option<String>, // Intermediate table name (None for auto-generation)
	},

	// Custom types
	Custom(String),
}

impl FieldType {
	/// Convert FieldType to SQL string for a specific dialect
	///
	/// This method returns database-specific SQL types.
	/// Use this method when generating SQL for a specific database.
	pub fn to_sql_for_dialect(&self, dialect: &super::operations::SqlDialect) -> String {
		use super::operations::SqlDialect;

		match self {
			FieldType::DateTime => match dialect {
				SqlDialect::Postgres | SqlDialect::Cockroachdb => "TIMESTAMP".to_string(),
				SqlDialect::Mysql | SqlDialect::Sqlite => "DATETIME".to_string(),
			},
			FieldType::TimestampTz => match dialect {
				SqlDialect::Postgres | SqlDialect::Cockroachdb => "TIMESTAMPTZ".to_string(),
				SqlDialect::Mysql => "DATETIME".to_string(), // MySQL doesn't have TIMESTAMPTZ
				SqlDialect::Sqlite => "DATETIME".to_string(), // SQLite doesn't have TIMESTAMPTZ
			},
			// Use "BOOLEAN" for SQLite instead of "INTEGER" to ensure sqlx's
			// type_info().name() returns "BOOLEAN". This allows our convert_row
			// function to properly detect boolean columns and convert integer 0/1
			// values to bool. SQLite will still store values as integers due to
			// type affinity, but the declared type name will be "BOOLEAN".
			FieldType::Boolean => match dialect {
				SqlDialect::Postgres | SqlDialect::Cockroachdb => "BOOLEAN".to_string(),
				SqlDialect::Mysql => "TINYINT(1)".to_string(), // MySQL uses TINYINT for boolean
				SqlDialect::Sqlite => "BOOLEAN".to_string(),   // SQLite - use BOOLEAN for type detection
			},
			FieldType::Uuid => match dialect {
				SqlDialect::Postgres | SqlDialect::Cockroachdb => "UUID".to_string(),
				SqlDialect::Mysql => "CHAR(36)".to_string(), // MySQL doesn't have native UUID
				SqlDialect::Sqlite => "TEXT".to_string(),    // SQLite doesn't have native UUID
			},
			FieldType::JsonBinary => match dialect {
				SqlDialect::Postgres | SqlDialect::Cockroachdb => "JSONB".to_string(),
				SqlDialect::Mysql | SqlDialect::Sqlite => "JSON".to_string(), // Fallback to JSON
			},
			// PostgreSQL-specific types with dialect handling
			FieldType::Array(inner) => match dialect {
				SqlDialect::Postgres | SqlDialect::Cockroachdb => {
					format!("{}[]", inner.to_sql_for_dialect(dialect))
				}
				// MySQL and SQLite don't support native arrays, fallback to JSON
				SqlDialect::Mysql | SqlDialect::Sqlite => "JSON".to_string(),
			},
			FieldType::HStore => match dialect {
				SqlDialect::Postgres | SqlDialect::Cockroachdb => "HSTORE".to_string(),
				// Fallback to JSON for other databases
				SqlDialect::Mysql | SqlDialect::Sqlite => "JSON".to_string(),
			},
			FieldType::CIText => match dialect {
				SqlDialect::Postgres | SqlDialect::Cockroachdb => "CITEXT".to_string(),
				// Fallback to TEXT for other databases
				SqlDialect::Mysql | SqlDialect::Sqlite => "TEXT".to_string(),
			},
			FieldType::Int4Range => match dialect {
				SqlDialect::Postgres | SqlDialect::Cockroachdb => "INT4RANGE".to_string(),
				// No native range support in other databases
				SqlDialect::Mysql | SqlDialect::Sqlite => "VARCHAR(50)".to_string(),
			},
			FieldType::Int8Range => match dialect {
				SqlDialect::Postgres | SqlDialect::Cockroachdb => "INT8RANGE".to_string(),
				SqlDialect::Mysql | SqlDialect::Sqlite => "VARCHAR(50)".to_string(),
			},
			FieldType::NumRange => match dialect {
				SqlDialect::Postgres | SqlDialect::Cockroachdb => "NUMRANGE".to_string(),
				SqlDialect::Mysql | SqlDialect::Sqlite => "VARCHAR(100)".to_string(),
			},
			FieldType::DateRange => match dialect {
				SqlDialect::Postgres | SqlDialect::Cockroachdb => "DATERANGE".to_string(),
				SqlDialect::Mysql | SqlDialect::Sqlite => "VARCHAR(50)".to_string(),
			},
			FieldType::TsRange => match dialect {
				SqlDialect::Postgres | SqlDialect::Cockroachdb => "TSRANGE".to_string(),
				SqlDialect::Mysql | SqlDialect::Sqlite => "VARCHAR(100)".to_string(),
			},
			FieldType::TsTzRange => match dialect {
				SqlDialect::Postgres | SqlDialect::Cockroachdb => "TSTZRANGE".to_string(),
				SqlDialect::Mysql | SqlDialect::Sqlite => "VARCHAR(100)".to_string(),
			},
			FieldType::TsVector => match dialect {
				SqlDialect::Postgres | SqlDialect::Cockroachdb => "TSVECTOR".to_string(),
				// No native full-text search vector in other databases
				SqlDialect::Mysql | SqlDialect::Sqlite => "TEXT".to_string(),
			},
			FieldType::TsQuery => match dialect {
				SqlDialect::Postgres | SqlDialect::Cockroachdb => "TSQUERY".to_string(),
				SqlDialect::Mysql | SqlDialect::Sqlite => "TEXT".to_string(),
			},
			FieldType::Float => match dialect {
				SqlDialect::Postgres | SqlDialect::Cockroachdb => "REAL".to_string(),
				_ => self.to_sql_string(),
			},
			FieldType::Double => match dialect {
				SqlDialect::Postgres | SqlDialect::Cockroachdb => "DOUBLE PRECISION".to_string(),
				_ => self.to_sql_string(),
			},
			// For all other types, use the generic SQL type
			_ => self.to_sql_string(),
		}
	}

	/// Convert FieldType to SQL string
	///
	/// This method returns generic SQL types that may not be compatible with all databases.
	/// For database-specific SQL generation, use `to_sql_for_dialect()` instead.
	pub fn to_sql_string(&self) -> String {
		match self {
			FieldType::BigInteger => "BIGINT".to_string(),
			FieldType::Integer => "INTEGER".to_string(),
			FieldType::SmallInteger => "SMALLINT".to_string(),
			FieldType::TinyInt => "TINYINT".to_string(),
			FieldType::MediumInt => "MEDIUMINT".to_string(),
			FieldType::Char(max_length) => format!("CHAR({})", max_length),
			FieldType::VarChar(max_length) => format!("VARCHAR({})", max_length),
			FieldType::Text => "TEXT".to_string(),
			FieldType::TinyText => "TINYTEXT".to_string(),
			FieldType::MediumText => "MEDIUMTEXT".to_string(),
			FieldType::LongText => "LONGTEXT".to_string(),
			FieldType::Date => "DATE".to_string(),
			FieldType::Time => "TIME".to_string(),
			FieldType::DateTime => "DATETIME".to_string(),
			FieldType::TimestampTz => "TIMESTAMPTZ".to_string(),
			FieldType::Decimal { precision, scale } => format!("DECIMAL({}, {})", precision, scale),
			FieldType::Float => "FLOAT".to_string(),
			FieldType::Double => "DOUBLE".to_string(),
			FieldType::Real => "REAL".to_string(),
			FieldType::Boolean => "BOOLEAN".to_string(),
			FieldType::Binary => "BINARY".to_string(),
			FieldType::Blob => "BLOB".to_string(),
			FieldType::TinyBlob => "TINYBLOB".to_string(),
			FieldType::MediumBlob => "MEDIUMBLOB".to_string(),
			FieldType::LongBlob => "LONGBLOB".to_string(),
			FieldType::Bytea => "BYTEA".to_string(),
			FieldType::Json => "JSON".to_string(),
			FieldType::JsonBinary => "JSONB".to_string(),
			// PostgreSQL-specific types
			FieldType::Array(inner) => format!("{}[]", inner.to_sql_string()),
			FieldType::HStore => "HSTORE".to_string(),
			FieldType::CIText => "CITEXT".to_string(),
			FieldType::Int4Range => "INT4RANGE".to_string(),
			FieldType::Int8Range => "INT8RANGE".to_string(),
			FieldType::NumRange => "NUMRANGE".to_string(),
			FieldType::DateRange => "DATERANGE".to_string(),
			FieldType::TsRange => "TSRANGE".to_string(),
			FieldType::TsTzRange => "TSTZRANGE".to_string(),
			FieldType::TsVector => "TSVECTOR".to_string(),
			FieldType::TsQuery => "TSQUERY".to_string(),
			FieldType::Uuid => "UUID".to_string(),
			FieldType::Year => "YEAR".to_string(),
			FieldType::Enum { values } => {
				let values_str = values
					.iter()
					.map(|v| format!("'{}'", v))
					.collect::<Vec<_>>()
					.join(",");
				format!("ENUM({})", values_str)
			}
			FieldType::Set { values } => {
				let values_str = values
					.iter()
					.map(|v| format!("'{}'", v))
					.collect::<Vec<_>>()
					.join(",");
				format!("SET({})", values_str)
			}
			FieldType::ForeignKey { to_table, .. } => {
				format!("-- ForeignKey to {}", to_table)
			}
			FieldType::OneToOne { to, .. } => {
				format!("-- OneToOne relationship to {}", to)
			}
			FieldType::ManyToMany { to, through } => match through {
				Some(table) => format!("-- ManyToMany through {}", table),
				None => format!("-- ManyToMany to {} (auto-generated)", to),
			},
			FieldType::Custom(custom_type) => custom_type.clone(),
		}
	}

	/// Get max_length if this type has one
	pub fn max_length(&self) -> Option<u32> {
		match self {
			FieldType::Char(max_length) | FieldType::VarChar(max_length) => Some(*max_length),
			_ => None,
		}
	}
}

/// Trait for field types that provide their type name as a compile-time constant
pub trait FieldTypeName {
	const NAME: &'static str;
}

// Type-safe field type markers
pub struct BigIntegerField;
impl FieldTypeName for BigIntegerField {
	const NAME: &'static str = "BigIntegerField";
}

pub struct IntegerField;
impl FieldTypeName for IntegerField {
	const NAME: &'static str = "IntegerField";
}

pub struct SmallIntegerField;
impl FieldTypeName for SmallIntegerField {
	const NAME: &'static str = "SmallIntegerField";
}

pub struct CharField;
impl FieldTypeName for CharField {
	const NAME: &'static str = "CharField";
}

pub struct TextField;
impl FieldTypeName for TextField {
	const NAME: &'static str = "TextField";
}

pub struct DateTimeField;
impl FieldTypeName for DateTimeField {
	const NAME: &'static str = "DateTimeField";
}

pub struct DateField;
impl FieldTypeName for DateField {
	const NAME: &'static str = "DateField";
}

pub struct TimeField;
impl FieldTypeName for TimeField {
	const NAME: &'static str = "TimeField";
}

pub struct BooleanField;
impl FieldTypeName for BooleanField {
	const NAME: &'static str = "BooleanField";
}

pub struct DecimalField;
impl FieldTypeName for DecimalField {
	const NAME: &'static str = "DecimalField";
}

pub struct BinaryField;
impl FieldTypeName for BinaryField {
	const NAME: &'static str = "BinaryField";
}

pub struct JSONField;
impl FieldTypeName for JSONField {
	const NAME: &'static str = "JSONField";
}

pub struct UUIDField;
impl FieldTypeName for UUIDField {
	const NAME: &'static str = "UUIDField";
}

// PostgreSQL-specific field type markers
pub struct ArrayField;
impl FieldTypeName for ArrayField {
	const NAME: &'static str = "ArrayField";
}

pub struct HStoreField;
impl FieldTypeName for HStoreField {
	const NAME: &'static str = "HStoreField";
}

pub struct CITextField;
impl FieldTypeName for CITextField {
	const NAME: &'static str = "CITextField";
}

pub struct Int4RangeField;
impl FieldTypeName for Int4RangeField {
	const NAME: &'static str = "Int4RangeField";
}

pub struct Int8RangeField;
impl FieldTypeName for Int8RangeField {
	const NAME: &'static str = "Int8RangeField";
}

pub struct NumRangeField;
impl FieldTypeName for NumRangeField {
	const NAME: &'static str = "NumRangeField";
}

pub struct DateRangeField;
impl FieldTypeName for DateRangeField {
	const NAME: &'static str = "DateRangeField";
}

pub struct TsRangeField;
impl FieldTypeName for TsRangeField {
	const NAME: &'static str = "TsRangeField";
}

pub struct TsTzRangeField;
impl FieldTypeName for TsTzRangeField {
	const NAME: &'static str = "TsTzRangeField";
}

pub struct TsVectorField;
impl FieldTypeName for TsVectorField {
	const NAME: &'static str = "TsVectorField";
}

pub struct TsQueryField;
impl FieldTypeName for TsQueryField {
	const NAME: &'static str = "TsQueryField";
}

pub mod prelude {
	pub use super::{
		// PostgreSQL-specific field types
		ArrayField,
		// Standard field types
		BigIntegerField,
		BinaryField,
		BooleanField,
		CITextField,
		CharField,
		DateField,
		DateRangeField,
		DateTimeField,
		DecimalField,
		FieldTypeName,
		HStoreField,
		Int4RangeField,
		Int8RangeField,
		IntegerField,
		JSONField,
		NumRangeField,
		SmallIntegerField,
		TextField,
		TimeField,
		TsQueryField,
		TsRangeField,
		TsTzRangeField,
		TsVectorField,
		UUIDField,
	};
}

/// Display implementation for FieldType
impl std::fmt::Display for FieldType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.to_sql_string())
	}
}
