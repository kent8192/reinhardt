//! Field dependencies and conditional requirements for schema validation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a field dependency relationship
///
/// # Examples
///
/// ```
/// use reinhardt_rest::metadata::{FieldDependency, DependencyType};
///
/// // Field 'country' requires 'address' to also be present
/// let dep = FieldDependency::new(
///     "country",
///     DependencyType::Requires { fields: vec!["address".to_string()] }
/// );
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDependency {
	/// The field that has the dependency
	pub field_name: String,
	/// The type of dependency
	pub dependency_type: DependencyType,
}

impl FieldDependency {
	/// Creates a new field dependency
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{FieldDependency, DependencyType};
	///
	/// let dep = FieldDependency::new(
	///     "zip_code",
	///     DependencyType::Requires { fields: vec!["city".to_string(), "state".to_string()] }
	/// );
	/// assert_eq!(dep.field_name, "zip_code");
	/// ```
	pub fn new(field_name: impl Into<String>, dependency_type: DependencyType) -> Self {
		Self {
			field_name: field_name.into(),
			dependency_type,
		}
	}

	/// Creates a dependency that requires other fields
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::FieldDependency;
	///
	/// let dep = FieldDependency::requires("country", vec!["address"]);
	/// ```
	pub fn requires(
		field_name: impl Into<String>,
		required_fields: Vec<impl Into<String>>,
	) -> Self {
		Self::new(
			field_name,
			DependencyType::Requires {
				fields: required_fields.into_iter().map(|f| f.into()).collect(),
			},
		)
	}

	/// Creates a dependency where only one of the fields is allowed
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::FieldDependency;
	///
	/// let dep = FieldDependency::one_of("payment_method", vec!["credit_card", "paypal"]);
	/// ```
	pub fn one_of(field_name: impl Into<String>, fields: Vec<impl Into<String>>) -> Self {
		Self::new(
			field_name,
			DependencyType::OneOf {
				fields: fields.into_iter().map(|f| f.into()).collect(),
			},
		)
	}

	/// Creates a dependency where all fields must be present together
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::FieldDependency;
	///
	/// let dep = FieldDependency::all_of("shipping", vec!["address", "city", "zip_code"]);
	/// ```
	pub fn all_of(field_name: impl Into<String>, fields: Vec<impl Into<String>>) -> Self {
		Self::new(
			field_name,
			DependencyType::AllOf {
				fields: fields.into_iter().map(|f| f.into()).collect(),
			},
		)
	}

	/// Creates a conditional dependency based on field value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::FieldDependency;
	///
	/// // If 'shipping_method' is 'express', then 'express_fee' is required
	/// let dep = FieldDependency::conditional(
	///     "shipping_method",
	///     "express",
	///     vec!["express_fee"]
	/// );
	/// ```
	pub fn conditional(
		field_name: impl Into<String>,
		condition_value: impl Into<String>,
		required_fields: Vec<impl Into<String>>,
	) -> Self {
		Self::new(
			field_name,
			DependencyType::Conditional {
				value: condition_value.into(),
				requires: required_fields.into_iter().map(|f| f.into()).collect(),
			},
		)
	}
}

/// Types of field dependencies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DependencyType {
	/// Requires all specified fields to be present
	Requires {
		/// List of required field names
		fields: Vec<String>,
	},
	/// Only one of the specified fields can be present
	OneOf {
		/// List of field names where only one can be present
		fields: Vec<String>,
	},
	/// All specified fields must be present together
	AllOf {
		/// List of field names that must all be present
		fields: Vec<String>,
	},
	/// Conditional requirement based on field value
	Conditional {
		/// The value that triggers the dependency
		value: String,
		/// Fields required when the condition is met
		requires: Vec<String>,
	},
}

/// Manages field dependencies for a schema
///
/// # Examples
///
/// ```
/// use reinhardt_rest::metadata::{DependencyManager, FieldDependency};
///
/// let mut manager = DependencyManager::new();
/// manager.add_dependency(FieldDependency::requires("country", vec!["address"]));
/// ```
#[derive(Debug, Clone, Default)]
pub struct DependencyManager {
	dependencies: Vec<FieldDependency>,
}

impl DependencyManager {
	/// Creates a new dependency manager
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::DependencyManager;
	///
	/// let manager = DependencyManager::new();
	/// ```
	pub fn new() -> Self {
		Self {
			dependencies: Vec::new(),
		}
	}

	/// Adds a field dependency
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{DependencyManager, FieldDependency};
	///
	/// let mut manager = DependencyManager::new();
	/// manager.add_dependency(FieldDependency::requires("zip_code", vec!["city"]));
	/// ```
	pub fn add_dependency(&mut self, dependency: FieldDependency) {
		self.dependencies.push(dependency);
	}

	/// Gets all dependencies
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{DependencyManager, FieldDependency};
	///
	/// let mut manager = DependencyManager::new();
	/// manager.add_dependency(FieldDependency::requires("country", vec!["address"]));
	/// assert_eq!(manager.get_dependencies().len(), 1);
	/// ```
	pub fn get_dependencies(&self) -> &[FieldDependency] {
		&self.dependencies
	}

	/// Gets dependencies for a specific field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{DependencyManager, FieldDependency};
	///
	/// let mut manager = DependencyManager::new();
	/// manager.add_dependency(FieldDependency::requires("country", vec!["address"]));
	/// manager.add_dependency(FieldDependency::requires("city", vec!["address"]));
	///
	/// let country_deps = manager.get_field_dependencies("country");
	/// assert_eq!(country_deps.len(), 1);
	/// ```
	pub fn get_field_dependencies(&self, field_name: &str) -> Vec<&FieldDependency> {
		self.dependencies
			.iter()
			.filter(|dep| dep.field_name == field_name)
			.collect()
	}

	/// Converts dependencies to OpenAPI schema format
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{DependencyManager, FieldDependency};
	///
	/// let mut manager = DependencyManager::new();
	/// manager.add_dependency(FieldDependency::requires("country", vec!["address"]));
	///
	/// let openapi = manager.to_openapi_dependencies();
	/// assert!(openapi.contains_key("country"));
	/// ```
	pub fn to_openapi_dependencies(&self) -> HashMap<String, serde_json::Value> {
		let mut result = HashMap::new();

		for dep in &self.dependencies {
			match &dep.dependency_type {
				DependencyType::Requires { fields } => {
					result.insert(dep.field_name.clone(), serde_json::json!(fields));
				}
				DependencyType::OneOf { fields } => {
					result.insert(
						dep.field_name.clone(),
						serde_json::json!({
							"oneOf": fields.iter().map(|f| {
								serde_json::json!({
									"required": [f]
								})
							}).collect::<Vec<_>>()
						}),
					);
				}
				DependencyType::AllOf { fields } => {
					result.insert(
						dep.field_name.clone(),
						serde_json::json!({
							"allOf": fields.iter().map(|f| {
								serde_json::json!({
									"required": [f]
								})
							}).collect::<Vec<_>>()
						}),
					);
				}
				DependencyType::Conditional { value, requires } => {
					result.insert(
						dep.field_name.clone(),
						serde_json::json!({
							"if": {
								"properties": {
									dep.field_name.clone(): {
										"const": value
									}
								}
							},
							"then": {
								"required": requires
							}
						}),
					);
				}
			}
		}

		result
	}

	/// Validates that field dependencies are satisfied
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::metadata::{DependencyManager, FieldDependency};
	/// use std::collections::HashSet;
	///
	/// let mut manager = DependencyManager::new();
	/// manager.add_dependency(FieldDependency::requires("country", vec!["address"]));
	///
	/// let mut present_fields = HashSet::new();
	/// present_fields.insert("country".to_string());
	/// present_fields.insert("address".to_string());
	///
	/// let errors = manager.validate_dependencies(&present_fields);
	/// assert!(errors.is_empty());
	/// ```
	pub fn validate_dependencies(
		&self,
		present_fields: &std::collections::HashSet<String>,
	) -> Vec<String> {
		let mut errors = Vec::new();

		for dep in &self.dependencies {
			if !present_fields.contains(&dep.field_name) {
				continue;
			}

			match &dep.dependency_type {
				DependencyType::Requires { fields } => {
					for required_field in fields {
						if !present_fields.contains(required_field) {
							errors.push(format!(
								"Field '{}' requires field '{}' to be present",
								dep.field_name, required_field
							));
						}
					}
				}
				DependencyType::OneOf { fields } => {
					let count = fields
						.iter()
						.filter(|f| present_fields.contains(*f))
						.count();
					if count != 1 {
						errors.push(format!(
							"Field '{}' requires exactly one of: {:?}",
							dep.field_name, fields
						));
					}
				}
				DependencyType::AllOf { fields } => {
					let missing: Vec<_> = fields
						.iter()
						.filter(|f| !present_fields.contains(*f))
						.collect();
					if !missing.is_empty() {
						errors.push(format!(
							"Field '{}' requires all of: {:?}, missing: {:?}",
							dep.field_name, fields, missing
						));
					}
				}
				DependencyType::Conditional { requires, .. } => {
					// For conditional dependencies, we only validate if the condition is met
					// This would require actual field values, not just presence
					for required_field in requires {
						if !present_fields.contains(required_field) {
							errors.push(format!(
								"Field '{}' conditionally requires field '{}'",
								dep.field_name, required_field
							));
						}
					}
				}
			}
		}

		errors
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::collections::HashSet;

	#[rstest]
	fn test_create_requires_dependency() {
		let dep = FieldDependency::requires("country", vec!["address"]);
		assert_eq!(dep.field_name, "country");
		match dep.dependency_type {
			DependencyType::Requires { fields } => {
				assert_eq!(fields.len(), 1);
				assert_eq!(fields[0], "address");
			}
			_ => panic!("Wrong dependency type"),
		}
	}

	#[rstest]
	fn test_create_one_of_dependency() {
		let dep = FieldDependency::one_of("payment", vec!["credit_card", "paypal"]);
		match dep.dependency_type {
			DependencyType::OneOf { fields } => {
				assert_eq!(fields.len(), 2);
				assert!(fields.contains(&"credit_card".to_string()));
				assert!(fields.contains(&"paypal".to_string()));
			}
			_ => panic!("Wrong dependency type"),
		}
	}

	#[rstest]
	fn test_create_all_of_dependency() {
		let dep = FieldDependency::all_of("shipping", vec!["address", "city", "zip"]);
		match dep.dependency_type {
			DependencyType::AllOf { fields } => {
				assert_eq!(fields.len(), 3);
			}
			_ => panic!("Wrong dependency type"),
		}
	}

	#[rstest]
	fn test_create_conditional_dependency() {
		let dep = FieldDependency::conditional("shipping_method", "express", vec!["express_fee"]);
		match dep.dependency_type {
			DependencyType::Conditional { value, requires } => {
				assert_eq!(value, "express");
				assert_eq!(requires.len(), 1);
				assert_eq!(requires[0], "express_fee");
			}
			_ => panic!("Wrong dependency type"),
		}
	}

	#[rstest]
	fn test_dependency_manager_add_and_get() {
		let mut manager = DependencyManager::new();
		manager.add_dependency(FieldDependency::requires("country", vec!["address"]));
		manager.add_dependency(FieldDependency::requires("zip_code", vec!["city"]));

		assert_eq!(manager.get_dependencies().len(), 2);
	}

	#[rstest]
	fn test_get_field_dependencies() {
		let mut manager = DependencyManager::new();
		manager.add_dependency(FieldDependency::requires("country", vec!["address"]));
		manager.add_dependency(FieldDependency::requires("city", vec!["address"]));
		manager.add_dependency(FieldDependency::requires("country", vec!["phone"]));

		let country_deps = manager.get_field_dependencies("country");
		assert_eq!(country_deps.len(), 2);

		let city_deps = manager.get_field_dependencies("city");
		assert_eq!(city_deps.len(), 1);
	}

	#[rstest]
	fn test_to_openapi_dependencies_requires() {
		let mut manager = DependencyManager::new();
		manager.add_dependency(FieldDependency::requires("country", vec!["address"]));

		let openapi = manager.to_openapi_dependencies();
		assert!(openapi.contains_key("country"));

		let country_deps = &openapi["country"];
		assert!(country_deps.is_array());
		assert_eq!(country_deps.as_array().unwrap().len(), 1);
	}

	#[rstest]
	fn test_validate_dependencies_success() {
		let mut manager = DependencyManager::new();
		manager.add_dependency(FieldDependency::requires("country", vec!["address"]));

		let mut present_fields = HashSet::new();
		present_fields.insert("country".to_string());
		present_fields.insert("address".to_string());

		let errors = manager.validate_dependencies(&present_fields);
		assert!(errors.is_empty());
	}

	#[rstest]
	fn test_validate_dependencies_missing_required() {
		let mut manager = DependencyManager::new();
		manager.add_dependency(FieldDependency::requires("country", vec!["address"]));

		let mut present_fields = HashSet::new();
		present_fields.insert("country".to_string());
		// address is missing

		let errors = manager.validate_dependencies(&present_fields);
		assert_eq!(errors.len(), 1);
		assert!(errors[0].contains("requires"));
	}

	#[rstest]
	fn test_validate_one_of_success() {
		let mut manager = DependencyManager::new();
		manager.add_dependency(FieldDependency::one_of(
			"payment",
			vec!["credit_card", "paypal"],
		));

		let mut present_fields = HashSet::new();
		present_fields.insert("payment".to_string());
		present_fields.insert("credit_card".to_string());

		let errors = manager.validate_dependencies(&present_fields);
		assert!(errors.is_empty());
	}

	#[rstest]
	fn test_validate_one_of_multiple_present() {
		let mut manager = DependencyManager::new();
		manager.add_dependency(FieldDependency::one_of(
			"payment",
			vec!["credit_card", "paypal"],
		));

		let mut present_fields = HashSet::new();
		present_fields.insert("payment".to_string());
		present_fields.insert("credit_card".to_string());
		present_fields.insert("paypal".to_string());

		let errors = manager.validate_dependencies(&present_fields);
		assert_eq!(errors.len(), 1);
		assert!(errors[0].contains("exactly one"));
	}

	#[rstest]
	fn test_validate_all_of_success() {
		let mut manager = DependencyManager::new();
		manager.add_dependency(FieldDependency::all_of(
			"shipping",
			vec!["address", "city", "zip"],
		));

		let mut present_fields = HashSet::new();
		present_fields.insert("shipping".to_string());
		present_fields.insert("address".to_string());
		present_fields.insert("city".to_string());
		present_fields.insert("zip".to_string());

		let errors = manager.validate_dependencies(&present_fields);
		assert!(errors.is_empty());
	}

	#[rstest]
	fn test_validate_all_of_missing_some() {
		let mut manager = DependencyManager::new();
		manager.add_dependency(FieldDependency::all_of(
			"shipping",
			vec!["address", "city", "zip"],
		));

		let mut present_fields = HashSet::new();
		present_fields.insert("shipping".to_string());
		present_fields.insert("address".to_string());
		// city and zip are missing

		let errors = manager.validate_dependencies(&present_fields);
		assert_eq!(errors.len(), 1);
		assert!(errors[0].contains("requires all of"));
	}

	#[rstest]
	fn test_dependency_type_serialization() {
		let dep = FieldDependency::requires("country", vec!["address"]);
		let json = serde_json::to_string(&dep).unwrap();
		let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

		assert_eq!(
			parsed["field_name"], "country",
			"Metadata field name mismatch. Expected 'country', got: {:?}",
			parsed["field_name"]
		);
		assert_eq!(
			parsed["dependency_type"]["type"], "requires",
			"Dependency type mismatch. Expected 'requires', got: {:?}",
			parsed["dependency_type"]["type"]
		);
		assert_eq!(
			parsed["dependency_type"]["fields"][0], "address",
			"Required field mismatch. Expected 'address', got: {:?}",
			parsed["dependency_type"]["fields"][0]
		);
	}

	#[rstest]
	fn test_conditional_dependency_serialization() {
		let dep = FieldDependency::conditional("shipping_method", "express", vec!["express_fee"]);
		let json = serde_json::to_string(&dep).unwrap();
		let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

		assert_eq!(
			parsed["field_name"], "shipping_method",
			"Metadata field name mismatch. Expected 'shipping_method', got: {:?}",
			parsed["field_name"]
		);
		assert_eq!(
			parsed["dependency_type"]["type"], "conditional",
			"Dependency type mismatch. Expected 'conditional', got: {:?}",
			parsed["dependency_type"]["type"]
		);
		assert_eq!(
			parsed["dependency_type"]["value"], "express",
			"Conditional value mismatch. Expected 'express', got: {:?}",
			parsed["dependency_type"]["value"]
		);
		assert_eq!(
			parsed["dependency_type"]["requires"][0], "express_fee",
			"Required field mismatch. Expected 'express_fee', got: {:?}",
			parsed["dependency_type"]["requires"][0]
		);
	}
}
