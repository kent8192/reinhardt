//! # Type System
//!
//! SQLAlchemy-inspired custom type system for ORM models.
//!
//! This module provides:
//! - Custom SQL types (UUID, JSON, ARRAY, etc.)
//! - Type decorators and converters
//! - Database-specific type mapping
//! - Type validation and coercion
//!
//! This module is inspired by SQLAlchemy's types.py
//! Copyright 2005-2025 SQLAlchemy authors and contributors
//! Licensed under MIT License. See THIRD-PARTY-NOTICES for details.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::fmt;
use uuid::Uuid;

/// Core trait for custom SQL types
/// Handles conversion between Rust types and SQL types
pub trait SqlTypeDefinition: Send + Sync {
	/// Rust type that this SQL type maps to
	type RustType;

	/// SQL type name (e.g., "UUID", "JSON", "TEXT[]")
	fn sql_type_name(&self) -> &str;

	/// Convert Rust value to database value
	fn process_bind_param(&self, value: Self::RustType) -> Result<SqlValue, TypeError>;

	/// Convert database value to Rust value
	fn process_result_value(&self, value: SqlValue) -> Result<Self::RustType, TypeError>;

	/// Get DDL type for CREATE TABLE
	fn ddl_type(&self, _dialect: &DatabaseDialect) -> String {
		self.sql_type_name().to_string()
	}
}

/// Database value that can be sent to/from database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SqlValue {
	Null,
	Boolean(bool),
	Integer(i64),
	Float(f64),
	Text(String),
	Bytes(Vec<u8>),
	Json(JsonValue),
	Uuid(String),
	Array(Vec<SqlValue>),
}

/// Type conversion errors
#[derive(Debug, Clone)]
pub struct TypeError {
	message: String,
}

impl TypeError {
	/// Create a new type error with a custom message
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::types::TypeError;
	///
	/// let error = TypeError::new("Invalid UUID format");
	/// assert_eq!(error.to_string(), "Type error: Invalid UUID format");
	/// ```
	pub fn new(message: impl Into<String>) -> Self {
		Self {
			message: message.into(),
		}
	}
}

impl fmt::Display for TypeError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "Type error: {}", self.message)
	}
}

impl std::error::Error for TypeError {}

/// Database dialect for type mapping
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseDialect {
	PostgreSQL,
	MySQL,
	SQLite,
	MSSQL,
}

/// UUID type
pub struct UuidType;

impl SqlTypeDefinition for UuidType {
	type RustType = Uuid;

	fn sql_type_name(&self) -> &str {
		"UUID"
	}

	fn process_bind_param(&self, value: Self::RustType) -> Result<SqlValue, TypeError> {
		Ok(SqlValue::Uuid(value.to_string()))
	}

	fn process_result_value(&self, value: SqlValue) -> Result<Self::RustType, TypeError> {
		match value {
			SqlValue::Uuid(s) | SqlValue::Text(s) => {
				Uuid::parse_str(&s).map_err(|e| TypeError::new(format!("Invalid UUID: {}", e)))
			}
			_ => Err(TypeError::new("Expected UUID or Text value")),
		}
	}

	fn ddl_type(&self, dialect: &DatabaseDialect) -> String {
		match dialect {
			DatabaseDialect::PostgreSQL => "UUID".to_string(),
			DatabaseDialect::MySQL => "CHAR(36)".to_string(),
			DatabaseDialect::SQLite => "TEXT".to_string(),
			DatabaseDialect::MSSQL => "UNIQUEIDENTIFIER".to_string(),
		}
	}
}

/// JSON type
pub struct JsonType;

impl SqlTypeDefinition for JsonType {
	type RustType = JsonValue;

	fn sql_type_name(&self) -> &str {
		"JSON"
	}

	fn process_bind_param(&self, value: Self::RustType) -> Result<SqlValue, TypeError> {
		Ok(SqlValue::Json(value))
	}

	fn process_result_value(&self, value: SqlValue) -> Result<Self::RustType, TypeError> {
		match value {
			SqlValue::Json(j) => Ok(j),
			SqlValue::Text(s) => {
				serde_json::from_str(&s).map_err(|e| TypeError::new(format!("Invalid JSON: {}", e)))
			}
			_ => Err(TypeError::new("Expected JSON value")),
		}
	}

	fn ddl_type(&self, dialect: &DatabaseDialect) -> String {
		match dialect {
			DatabaseDialect::PostgreSQL => "JSONB".to_string(),
			DatabaseDialect::MySQL => "JSON".to_string(),
			DatabaseDialect::SQLite => "TEXT".to_string(),
			DatabaseDialect::MSSQL => "NVARCHAR(MAX)".to_string(),
		}
	}
}

/// Array type (PostgreSQL)
pub struct ArrayType {
	element_type: String,
}

impl ArrayType {
	/// Create a new array type for PostgreSQL
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::types::ArrayType;
	/// use reinhardt_db::orm::SqlTypeDefinition;
	///
	/// let array_type = ArrayType::new("TEXT");
	/// let values = vec!["tag1".to_string(), "tag2".to_string()];
	/// let sql_value = array_type.process_bind_param(values).unwrap();
	/// ```
	pub fn new(element_type: impl Into<String>) -> Self {
		Self {
			element_type: element_type.into(),
		}
	}
}

impl SqlTypeDefinition for ArrayType {
	type RustType = Vec<String>;

	fn sql_type_name(&self) -> &str {
		"ARRAY"
	}

	fn process_bind_param(&self, value: Self::RustType) -> Result<SqlValue, TypeError> {
		let values: Vec<SqlValue> = value.into_iter().map(SqlValue::Text).collect();
		Ok(SqlValue::Array(values))
	}

	fn process_result_value(&self, value: SqlValue) -> Result<Self::RustType, TypeError> {
		match value {
			SqlValue::Array(arr) => arr
				.into_iter()
				.map(|v| match v {
					SqlValue::Text(s) => Ok(s),
					_ => Err(TypeError::new("Array element is not text")),
				})
				.collect(),
			_ => Err(TypeError::new("Expected Array value")),
		}
	}

	fn ddl_type(&self, dialect: &DatabaseDialect) -> String {
		match dialect {
			DatabaseDialect::PostgreSQL => format!("{}[]", self.element_type),
			_ => "TEXT".to_string(), // Fallback to TEXT for non-Postgres
		}
	}
}

/// HSTORE type (PostgreSQL)
pub struct HstoreType;

impl SqlTypeDefinition for HstoreType {
	type RustType = std::collections::HashMap<String, String>;

	fn sql_type_name(&self) -> &str {
		"HSTORE"
	}

	fn process_bind_param(&self, value: Self::RustType) -> Result<SqlValue, TypeError> {
		let json = serde_json::to_value(value)
			.map_err(|e| TypeError::new(format!("Failed to serialize HSTORE: {}", e)))?;
		Ok(SqlValue::Json(json))
	}

	fn process_result_value(&self, value: SqlValue) -> Result<Self::RustType, TypeError> {
		match value {
			SqlValue::Json(j) => serde_json::from_value(j)
				.map_err(|e| TypeError::new(format!("Failed to deserialize HSTORE: {}", e))),
			_ => Err(TypeError::new("Expected HSTORE value")),
		}
	}

	fn ddl_type(&self, dialect: &DatabaseDialect) -> String {
		match dialect {
			DatabaseDialect::PostgreSQL => "HSTORE".to_string(),
			_ => "TEXT".to_string(),
		}
	}
}

/// INET type (PostgreSQL - IP address)
pub struct InetType;

impl SqlTypeDefinition for InetType {
	type RustType = std::net::IpAddr;

	fn sql_type_name(&self) -> &str {
		"INET"
	}

	fn process_bind_param(&self, value: Self::RustType) -> Result<SqlValue, TypeError> {
		Ok(SqlValue::Text(value.to_string()))
	}

	fn process_result_value(&self, value: SqlValue) -> Result<Self::RustType, TypeError> {
		match value {
			SqlValue::Text(s) => s
				.parse()
				.map_err(|e| TypeError::new(format!("Invalid IP address: {}", e))),
			_ => Err(TypeError::new("Expected INET value")),
		}
	}

	fn ddl_type(&self, dialect: &DatabaseDialect) -> String {
		match dialect {
			DatabaseDialect::PostgreSQL => "INET".to_string(),
			_ => "VARCHAR(45)".to_string(), // IPv6 max length
		}
	}
}

// Type alias for coerce function
type CoerceFunction = Box<dyn Fn(SqlValue) -> Result<SqlValue, TypeError> + Send + Sync>;

/// Type decorator - wraps another type with custom behavior
pub struct TypeDecorator<T: SqlTypeDefinition> {
	// Allow dead_code: inner value stored for type decoration pattern
	#[allow(dead_code)]
	inner: T,
	coerce_fn: Option<CoerceFunction>,
}

impl<T: SqlTypeDefinition> TypeDecorator<T> {
	/// Create a new type decorator wrapping an inner type
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::types::{TypeDecorator, UuidType};
	///
	/// let decorated = TypeDecorator::new(UuidType);
	/// // Verify the decorator is created successfully (type check passes)
	/// let _: TypeDecorator<UuidType> = decorated;
	/// ```
	pub fn new(inner: T) -> Self {
		Self {
			inner,
			coerce_fn: None,
		}
	}
	/// Add a custom coercion function to the type decorator
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::types::{TypeDecorator, UuidType, SqlValue, TypeError};
	///
	/// let decorated = TypeDecorator::new(UuidType).with_coercion(|value| {
	///     // Custom coercion logic
	///     Ok(value)
	/// });
	/// // Verify the decorator with coercion is created successfully
	/// let _: TypeDecorator<UuidType> = decorated;
	/// ```
	pub fn with_coercion<F>(mut self, coerce: F) -> Self
	where
		F: Fn(SqlValue) -> Result<SqlValue, TypeError> + Send + Sync + 'static,
	{
		self.coerce_fn = Some(Box::new(coerce));
		self
	}
}

/// Type registry for custom types
#[derive(Default)]
pub struct TypeRegistry {
	types: std::collections::HashMap<String, String>,
}

impl TypeRegistry {
	/// Create a new type registry for custom database types
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::types::TypeRegistry;
	///
	/// let mut registry = TypeRegistry::new();
	/// registry.register("custom_id", "UUID");
	/// assert_eq!(registry.get("custom_id"), Some(&"UUID".to_string()));
	/// ```
	pub fn new() -> Self {
		Self::default()
	}
	/// Register a custom type mapping
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::types::TypeRegistry;
	///
	/// let mut registry = TypeRegistry::new();
	/// registry.register("user_status", "VARCHAR(20)");
	/// assert_eq!(registry.get("user_status"), Some(&"VARCHAR(20)".to_string()));
	/// ```
	pub fn register(&mut self, name: impl Into<String>, sql_type: impl Into<String>) {
		self.types.insert(name.into(), sql_type.into());
	}
	/// Get a registered type by name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::types::TypeRegistry;
	///
	/// let mut registry = TypeRegistry::new();
	/// registry.register("email", "VARCHAR(255)");
	/// assert_eq!(registry.get("email"), Some(&"VARCHAR(255)".to_string()));
	/// assert_eq!(registry.get("nonexistent"), None);
	/// ```
	pub fn get(&self, name: &str) -> Option<&String> {
		self.types.get(name)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::collections::HashMap;

	#[test]
	fn test_uuid_type() {
		let uuid_type = UuidType;
		let uuid = Uuid::new_v4();

		let bound = uuid_type.process_bind_param(uuid).unwrap();
		let uuid_str = match &bound {
			SqlValue::Uuid(s) => s.clone(),
			_ => panic!("Expected UUID value"),
		};
		assert_eq!(uuid_str, uuid.to_string());

		let result = uuid_type.process_result_value(bound).unwrap();
		assert_eq!(result, uuid);
	}

	#[test]
	fn test_uuid_ddl_types() {
		let uuid_type = UuidType;

		assert_eq!(uuid_type.ddl_type(&DatabaseDialect::PostgreSQL), "UUID");
		assert_eq!(uuid_type.ddl_type(&DatabaseDialect::MySQL), "CHAR(36)");
		assert_eq!(uuid_type.ddl_type(&DatabaseDialect::SQLite), "TEXT");
		assert_eq!(
			uuid_type.ddl_type(&DatabaseDialect::MSSQL),
			"UNIQUEIDENTIFIER"
		);
	}

	#[test]
	fn test_orm_types_json() {
		let json_type = JsonType;
		let json_val = serde_json::json!({"key": "value", "number": 42});

		let bound = json_type.process_bind_param(json_val.clone()).unwrap();
		let result = json_type.process_result_value(bound).unwrap();

		assert_eq!(result, json_val);
	}

	#[test]
	fn test_json_ddl_types() {
		let json_type = JsonType;

		assert_eq!(json_type.ddl_type(&DatabaseDialect::PostgreSQL), "JSONB");
		assert_eq!(json_type.ddl_type(&DatabaseDialect::MySQL), "JSON");
		assert_eq!(json_type.ddl_type(&DatabaseDialect::SQLite), "TEXT");
	}

	#[test]
	fn test_array_type() {
		let array_type = ArrayType::new("TEXT");
		let arr = vec!["a".to_string(), "b".to_string(), "c".to_string()];

		let bound = array_type.process_bind_param(arr.clone()).unwrap();
		let result = array_type.process_result_value(bound).unwrap();

		assert_eq!(result, arr);
	}

	#[test]
	fn test_array_ddl_type() {
		let array_type = ArrayType::new("INTEGER");

		assert_eq!(
			array_type.ddl_type(&DatabaseDialect::PostgreSQL),
			"INTEGER[]"
		);
		assert_eq!(array_type.ddl_type(&DatabaseDialect::MySQL), "TEXT");
	}

	#[test]
	fn test_hstore_type() {
		let hstore_type = HstoreType;
		let mut map = HashMap::new();
		map.insert("key1".to_string(), "value1".to_string());
		map.insert("key2".to_string(), "value2".to_string());

		let bound = hstore_type.process_bind_param(map.clone()).unwrap();
		let result = hstore_type.process_result_value(bound).unwrap();

		assert_eq!(result.len(), 2);
		assert_eq!(result.get("key1"), Some(&"value1".to_string()));
	}

	#[test]
	fn test_inet_type() {
		let inet_type = InetType;
		let ip: std::net::IpAddr = "192.168.1.1".parse().unwrap();

		let bound = inet_type.process_bind_param(ip).unwrap();
		let result = inet_type.process_result_value(bound).unwrap();

		assert_eq!(result, ip);
	}

	#[test]
	fn test_inet_ipv6() {
		let inet_type = InetType;
		let ip: std::net::IpAddr = "2001:0db8::1".parse().unwrap();

		let bound = inet_type.process_bind_param(ip).unwrap();
		let result = inet_type.process_result_value(bound).unwrap();

		assert_eq!(result, ip);
	}

	#[test]
	fn test_type_registry() {
		let mut registry = TypeRegistry::new();
		registry.register("custom_uuid", "UUID");
		registry.register("custom_json", "JSONB");

		assert_eq!(registry.get("custom_uuid"), Some(&"UUID".to_string()));
		assert_eq!(registry.get("custom_json"), Some(&"JSONB".to_string()));
		assert_eq!(registry.get("nonexistent"), None);
	}

	#[test]
	fn test_type_error() {
		let error = TypeError::new("Test error");
		assert_eq!(error.to_string(), "Type error: Test error");
	}

	#[test]
	fn test_sql_value_variants() {
		let null = SqlValue::Null;
		let bool_val = SqlValue::Boolean(true);
		let int_val = SqlValue::Integer(42);
		let float_val = SqlValue::Float(3.15);
		let text_val = SqlValue::Text("hello".to_string());
		let bytes_val = SqlValue::Bytes(vec![1, 2, 3]);

		assert!(matches!(null, SqlValue::Null));
		assert!(matches!(bool_val, SqlValue::Boolean(true)));
		assert!(matches!(int_val, SqlValue::Integer(42)));
		assert!(matches!(float_val, SqlValue::Float(_)));
		assert!(matches!(text_val, SqlValue::Text(_)));
		assert!(matches!(bytes_val, SqlValue::Bytes(_)));
	}
}
