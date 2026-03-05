//! SQL to Rust type mapping for code generation.
//!
//! Maps database column types to appropriate Rust types, handling:
//! - Nullable fields (`Option<T>`)
//! - Auto-increment primary keys
//! - Database-specific types (PostgreSQL, MySQL, SQLite)
//! - Custom type overrides

use crate::migrations::fields::FieldType;
use crate::migrations::introspection::ColumnInfo;
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during type mapping.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum TypeMappingError {
	#[error("Unsupported type: {0}")]
	UnsupportedType(String),

	#[error("Invalid type definition: {0}")]
	InvalidTypeDefinition(String),

	#[error("Type override not found: {table}.{column}")]
	OverrideNotFound { table: String, column: String },
}

/// Type mapper for converting SQL types to Rust types.
#[derive(Debug, Clone, Default)]
pub struct TypeMapper {
	/// Custom type overrides: "table.column" -> "RustType"
	overrides: HashMap<String, String>,
}

impl TypeMapper {
	/// Create a new TypeMapper with the given overrides.
	pub fn new(overrides: HashMap<String, String>) -> Self {
		Self { overrides }
	}

	/// Check if there's a type override for the given table and column.
	pub fn get_override(&self, table: &str, column: &str) -> Option<&str> {
		let key = format!("{}.{}", table, column);
		self.overrides.get(&key).map(|s| s.as_str())
	}

	/// Map a column to its Rust type.
	///
	/// # Arguments
	///
	/// * `table_name` - Name of the table
	/// * `column` - Column information
	///
	/// # Returns
	///
	/// TokenStream representing the Rust type (e.g., `i64`, `Option<String>`)
	pub fn map_column(
		&self,
		table_name: &str,
		column: &ColumnInfo,
	) -> Result<TokenStream, TypeMappingError> {
		// Check for custom override first
		if let Some(override_type) = self.get_override(table_name, &column.name) {
			let type_ident: TokenStream = override_type
				.parse()
				.map_err(|_| TypeMappingError::InvalidTypeDefinition(override_type.to_string()))?;

			return if column.nullable && !column.auto_increment {
				Ok(quote! { Option<#type_ident> })
			} else {
				Ok(type_ident)
			};
		}

		// Map the field type to Rust type
		let base_type = self.field_type_to_rust(&column.column_type)?;

		// Wrap in Option if nullable and not auto-increment
		// Auto-increment fields are never Option (they're set by DB)
		if column.nullable && !column.auto_increment {
			Ok(quote! { Option<#base_type> })
		} else {
			Ok(base_type)
		}
	}

	/// Map a FieldType to its Rust type TokenStream.
	#[allow(clippy::only_used_in_recursion)]
	pub fn field_type_to_rust(
		&self,
		field_type: &FieldType,
	) -> Result<TokenStream, TypeMappingError> {
		let tokens = match field_type {
			// Integer types
			FieldType::BigInteger => quote! { i64 },
			FieldType::Integer => quote! { i32 },
			FieldType::SmallInteger => quote! { i16 },
			FieldType::TinyInt => quote! { i8 },
			FieldType::MediumInt => quote! { i32 },

			// String types
			FieldType::Char(_) => quote! { String },
			FieldType::VarChar(_) => quote! { String },
			FieldType::Text => quote! { String },
			FieldType::TinyText => quote! { String },
			FieldType::MediumText => quote! { String },
			FieldType::LongText => quote! { String },

			// Date/time types
			FieldType::Date => quote! { chrono::NaiveDate },
			FieldType::Time => quote! { chrono::NaiveTime },
			FieldType::DateTime => quote! { chrono::NaiveDateTime },
			FieldType::TimestampTz => quote! { chrono::DateTime<chrono::Utc> },

			// Numeric types
			FieldType::Decimal { .. } => quote! { rust_decimal::Decimal },
			FieldType::Float => quote! { f32 },
			FieldType::Double => quote! { f64 },
			FieldType::Real => quote! { f32 },

			// Boolean
			FieldType::Boolean => quote! { bool },

			// Binary types
			FieldType::Binary => quote! { Vec<u8> },
			FieldType::Blob => quote! { Vec<u8> },
			FieldType::TinyBlob => quote! { Vec<u8> },
			FieldType::MediumBlob => quote! { Vec<u8> },
			FieldType::LongBlob => quote! { Vec<u8> },
			FieldType::Bytea => quote! { Vec<u8> },

			// JSON types
			FieldType::Json => quote! { serde_json::Value },
			FieldType::JsonBinary => quote! { serde_json::Value },

			// PostgreSQL-specific types
			FieldType::Array(inner) => {
				let inner_type = self.field_type_to_rust(inner)?;
				quote! { Vec<#inner_type> }
			}
			FieldType::HStore => quote! { std::collections::HashMap<String, String> },
			FieldType::CIText => quote! { String },
			FieldType::Int4Range => quote! { (i32, i32) },
			FieldType::Int8Range => quote! { (i64, i64) },
			FieldType::NumRange => quote! { (rust_decimal::Decimal, rust_decimal::Decimal) },
			FieldType::DateRange => quote! { (chrono::NaiveDate, chrono::NaiveDate) },
			FieldType::TsRange => quote! { (chrono::NaiveDateTime, chrono::NaiveDateTime) },
			FieldType::TsTzRange => {
				quote! { (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) }
			}
			FieldType::TsVector => quote! { String },
			FieldType::TsQuery => quote! { String },

			// UUID
			FieldType::Uuid => quote! { uuid::Uuid },

			// Year (MySQL)
			FieldType::Year => quote! { i16 },

			// Enum/Set (MySQL)
			FieldType::Enum { .. } => quote! { String },
			FieldType::Set { .. } => quote! { Vec<String> },

			// Relationship types - these are handled specially by the generator
			FieldType::ForeignKey { .. } => {
				// Return the FK column type (usually i64 for the foreign key ID)
				quote! { i64 }
			}
			FieldType::OneToOne { .. } => quote! { i64 },
			FieldType::ManyToMany { .. } => {
				// ManyToMany is not a column, skip
				return Err(TypeMappingError::UnsupportedType(
					"ManyToMany relationships are not stored as columns".to_string(),
				));
			}

			// Custom type
			FieldType::Custom(type_name) => {
				// Try to parse as Rust type, fallback to String
				type_name.parse().unwrap_or_else(|_| quote! { String })
			}
		};

		Ok(tokens)
	}

	/// Get the Rust type as a string for display purposes.
	pub fn field_type_to_rust_string(
		&self,
		field_type: &FieldType,
		nullable: bool,
		auto_increment: bool,
	) -> Result<String, TypeMappingError> {
		let base_type = match field_type {
			FieldType::BigInteger => "i64",
			FieldType::Integer => "i32",
			FieldType::SmallInteger => "i16",
			FieldType::TinyInt => "i8",
			FieldType::MediumInt => "i32",
			FieldType::Char(_) | FieldType::VarChar(_) | FieldType::Text => "String",
			FieldType::TinyText | FieldType::MediumText | FieldType::LongText => "String",
			FieldType::Date => "chrono::NaiveDate",
			FieldType::Time => "chrono::NaiveTime",
			FieldType::DateTime => "chrono::NaiveDateTime",
			FieldType::TimestampTz => "chrono::DateTime<chrono::Utc>",
			FieldType::Decimal { .. } => "rust_decimal::Decimal",
			FieldType::Float | FieldType::Real => "f32",
			FieldType::Double => "f64",
			FieldType::Boolean => "bool",
			FieldType::Binary | FieldType::Blob | FieldType::Bytea => "Vec<u8>",
			FieldType::TinyBlob | FieldType::MediumBlob | FieldType::LongBlob => "Vec<u8>",
			FieldType::Json | FieldType::JsonBinary => "serde_json::Value",
			FieldType::Uuid => "uuid::Uuid",
			FieldType::Year => "i16",
			FieldType::HStore => "std::collections::HashMap<String, String>",
			FieldType::CIText => "String",
			FieldType::TsVector | FieldType::TsQuery => "String",
			FieldType::ForeignKey { .. } | FieldType::OneToOne { .. } => "i64",
			FieldType::Array(_) => "Vec<_>",
			FieldType::Enum { .. } => "String",
			FieldType::Set { .. } => "Vec<String>",
			FieldType::Int4Range => "(i32, i32)",
			FieldType::Int8Range => "(i64, i64)",
			FieldType::NumRange => "(Decimal, Decimal)",
			FieldType::DateRange => "(NaiveDate, NaiveDate)",
			FieldType::TsRange => "(NaiveDateTime, NaiveDateTime)",
			FieldType::TsTzRange => "(DateTime<Utc>, DateTime<Utc>)",
			FieldType::Custom(name) => name.as_str(),
			FieldType::ManyToMany { .. } => {
				return Err(TypeMappingError::UnsupportedType("ManyToMany".to_string()));
			}
		};

		if nullable && !auto_increment {
			Ok(format!("Option<{}>", base_type))
		} else {
			Ok(base_type.to_string())
		}
	}
}

/// Parse VARCHAR length from type definition.
///
/// # Examples
///
/// ```rust,ignore
/// assert_eq!(parse_varchar_length("VARCHAR(255)"), Ok(255));
/// ```
#[allow(dead_code)] // Utility function for future type parsing features
pub(super) fn parse_varchar_length(type_def: &str) -> Result<u32, TypeMappingError> {
	let upper = type_def.to_uppercase();
	if !upper.starts_with("VARCHAR(") || !upper.ends_with(')') {
		return Err(TypeMappingError::InvalidTypeDefinition(
			type_def.to_string(),
		));
	}

	let inner = &type_def[8..type_def.len() - 1];
	inner
		.parse()
		.map_err(|_| TypeMappingError::InvalidTypeDefinition(type_def.to_string()))
}

/// Parse DECIMAL precision and scale from type definition.
///
/// # Examples
///
/// ```rust,ignore
/// assert_eq!(parse_decimal_precision("DECIMAL(10,2)"), Ok((10, 2)));
/// ```
#[allow(dead_code)] // Utility function for future type parsing features
pub(super) fn parse_decimal_precision(type_def: &str) -> Result<(u32, u32), TypeMappingError> {
	let upper = type_def.to_uppercase();
	if !upper.starts_with("DECIMAL(") || !upper.ends_with(')') {
		return Err(TypeMappingError::InvalidTypeDefinition(
			type_def.to_string(),
		));
	}

	let inner = &type_def[8..type_def.len() - 1];
	let parts: Vec<&str> = inner.split(',').collect();

	if parts.len() != 2 {
		return Err(TypeMappingError::InvalidTypeDefinition(
			type_def.to_string(),
		));
	}

	let precision: u32 = parts[0]
		.trim()
		.parse()
		.map_err(|_| TypeMappingError::InvalidTypeDefinition(type_def.to_string()))?;

	let scale: u32 = parts[1]
		.trim()
		.parse()
		.map_err(|_| TypeMappingError::InvalidTypeDefinition(type_def.to_string()))?;

	// Validate: scale cannot be greater than precision
	if scale > precision {
		return Err(TypeMappingError::InvalidTypeDefinition(format!(
			"Scale ({}) cannot be greater than precision ({}) in {}",
			scale, precision, type_def
		)));
	}

	Ok((precision, scale))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_integer_type_mapping() {
		let mapper = TypeMapper::default();

		let result = mapper.field_type_to_rust_string(&FieldType::BigInteger, false, false);
		assert_eq!(result.unwrap(), "i64");

		let result = mapper.field_type_to_rust_string(&FieldType::Integer, false, false);
		assert_eq!(result.unwrap(), "i32");

		let result = mapper.field_type_to_rust_string(&FieldType::SmallInteger, false, false);
		assert_eq!(result.unwrap(), "i16");
	}

	#[test]
	fn test_nullable_type_mapping() {
		let mapper = TypeMapper::default();

		let result = mapper.field_type_to_rust_string(&FieldType::Integer, true, false);
		assert_eq!(result.unwrap(), "Option<i32>");

		// Auto-increment should not be Option even if nullable
		let result = mapper.field_type_to_rust_string(&FieldType::Integer, true, true);
		assert_eq!(result.unwrap(), "i32");
	}

	#[test]
	fn test_string_type_mapping() {
		let mapper = TypeMapper::default();

		let result = mapper.field_type_to_rust_string(&FieldType::VarChar(255), false, false);
		assert_eq!(result.unwrap(), "String");

		let result = mapper.field_type_to_rust_string(&FieldType::Text, false, false);
		assert_eq!(result.unwrap(), "String");
	}

	#[test]
	fn test_datetime_type_mapping() {
		let mapper = TypeMapper::default();

		let result = mapper.field_type_to_rust_string(&FieldType::DateTime, false, false);
		assert_eq!(result.unwrap(), "chrono::NaiveDateTime");

		let result = mapper.field_type_to_rust_string(&FieldType::TimestampTz, false, false);
		assert_eq!(result.unwrap(), "chrono::DateTime<chrono::Utc>");
	}

	#[test]
	fn test_parse_varchar_length() {
		assert_eq!(parse_varchar_length("VARCHAR(255)").unwrap(), 255);
		assert_eq!(parse_varchar_length("varchar(100)").unwrap(), 100);
		assert!(parse_varchar_length("VARCHAR").is_err());
		assert!(parse_varchar_length("VARCHAR(abc)").is_err());
	}

	#[test]
	fn test_parse_decimal_precision() {
		assert_eq!(parse_decimal_precision("DECIMAL(10,2)").unwrap(), (10, 2));
		assert_eq!(parse_decimal_precision("decimal(5, 3)").unwrap(), (5, 3));
		assert!(parse_decimal_precision("DECIMAL(10)").is_err());
		assert!(parse_decimal_precision("DECIMAL(5,10)").is_err()); // scale > precision
	}

	#[test]
	fn test_type_override() {
		let mut overrides = HashMap::new();
		overrides.insert("users.status".to_string(), "UserStatus".to_string());

		let mapper = TypeMapper::new(overrides);

		assert_eq!(mapper.get_override("users", "status"), Some("UserStatus"));
		assert_eq!(mapper.get_override("users", "name"), None);
	}
}
