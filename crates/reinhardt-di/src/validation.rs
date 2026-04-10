//! Startup validation for the DI registry
//!
//! Provides [`RegistryValidator`] which checks the [`DependencyRegistry`] for
//! missing dependencies, scope incompatibilities, and circular dependency chains
//! before the application starts serving requests.

use crate::graph::DependencyGraph;
use crate::registry::{DependencyRegistry, DependencyScope};
use std::any::TypeId;
use std::fmt;
use std::sync::Arc;

/// Category of a validation error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationErrorKind {
	/// A dependency referenced by a factory is not registered.
	MissingDependency,
	/// A singleton depends on a request-scoped dependency.
	ScopeIncompatibility,
	/// A circular dependency chain was detected.
	CircularDependency,
}

/// A single validation error discovered during registry validation.
#[derive(Debug, Clone)]
pub struct ValidationError {
	/// The category of the error.
	pub kind: ValidationErrorKind,
	/// The human-readable type name involved.
	pub type_name: String,
	/// The `TypeId` of the type involved.
	pub type_id: TypeId,
	/// A descriptive message explaining the problem.
	pub message: String,
}

impl fmt::Display for ValidationError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let prefix = match self.kind {
			ValidationErrorKind::MissingDependency => "[MISSING]",
			ValidationErrorKind::ScopeIncompatibility => "[SCOPE]",
			ValidationErrorKind::CircularDependency => "[CYCLE]",
		};
		write!(f, "{} {}", prefix, self.message)
	}
}

/// Validates a [`DependencyRegistry`] for integrity at startup.
pub struct RegistryValidator {
	registry: Arc<DependencyRegistry>,
}

impl RegistryValidator {
	/// Create a new validator wrapping the given registry.
	pub fn new(registry: Arc<DependencyRegistry>) -> Self {
		Self { registry }
	}

	/// Run all validation checks against the registry.
	///
	/// Returns `Ok(())` when the registry is consistent, or `Err` with the
	/// list of problems found.
	pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
		let mut errors = Vec::new();

		self.check_missing_dependencies(&mut errors);
		self.check_scope_compatibility(&mut errors);
		self.check_circular_dependencies(&mut errors);

		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
	}

	/// Check that every declared dependency is actually registered.
	fn check_missing_dependencies(&self, errors: &mut Vec<ValidationError>) {
		let all_deps = self.registry.get_all_dependencies();

		for (type_id, dep_ids) in &all_deps {
			let parent_name = self.resolve_type_name(*type_id);

			for &dep_id in dep_ids {
				if !self.registry.is_registered_by_id(dep_id) {
					let dep_name = self.resolve_type_name(dep_id);
					errors.push(ValidationError {
						kind: ValidationErrorKind::MissingDependency,
						type_name: parent_name.clone(),
						type_id: *type_id,
						message: format!(
							"'{}' depends on '{}', which is not registered",
							parent_name, dep_name
						),
					});
				}
			}
		}
	}

	/// Check that no singleton transitively depends on a request-scoped dependency.
	///
	/// Performs a BFS from each singleton through the full dependency graph,
	/// flagging any reachable request-scoped type with the chain that leads to it.
	fn check_scope_compatibility(&self, errors: &mut Vec<ValidationError>) {
		let all_deps = self.registry.get_all_dependencies();

		for type_id in all_deps.keys() {
			if self.registry.get_scope_by_id(*type_id) != Some(DependencyScope::Singleton) {
				continue;
			}

			let parent_name = self.resolve_type_name(*type_id);

			// BFS to find all reachable request-scoped types
			let mut queue = std::collections::VecDeque::new();
			let mut visited = std::collections::HashSet::new();
			// (current_id, chain from singleton to current)
			if let Some(direct_deps) = all_deps.get(type_id) {
				for &dep_id in direct_deps {
					queue.push_back((dep_id, vec![*type_id, dep_id]));
				}
			}
			visited.insert(*type_id);

			while let Some((current_id, chain)) = queue.pop_front() {
				if !visited.insert(current_id) {
					continue;
				}

				if self.registry.get_scope_by_id(current_id) == Some(DependencyScope::Request) {
					let chain_names: Vec<String> =
						chain.iter().map(|id| self.resolve_type_name(*id)).collect();
					let dep_name = chain_names.last().unwrap().clone();
					errors.push(ValidationError {
						kind: ValidationErrorKind::ScopeIncompatibility,
						type_name: parent_name.clone(),
						type_id: *type_id,
						message: format!(
							"Singleton '{}' transitively depends on request-scoped '{}' (chain: {})",
							parent_name,
							dep_name,
							chain_names.join(" -> ")
						),
					});
					continue;
				}

				if let Some(next_deps) = all_deps.get(&current_id) {
					for &next_id in next_deps {
						if !visited.contains(&next_id) {
							let mut next_chain = chain.clone();
							next_chain.push(next_id);
							queue.push_back((next_id, next_chain));
						}
					}
				}
			}
		}
	}

	/// Check for circular dependency chains.
	fn check_circular_dependencies(&self, errors: &mut Vec<ValidationError>) {
		let graph = DependencyGraph::new(Arc::clone(&self.registry));
		let cycles = graph.detect_cycles();

		for cycle in &cycles {
			if cycle.is_empty() {
				continue;
			}

			let names: Vec<String> = cycle.iter().map(|id| self.resolve_type_name(*id)).collect();
			let cycle_desc = format!("{} -> {}", names.join(" -> "), names[0]);
			let first_id = cycle[0];
			let first_name = names[0].clone();

			errors.push(ValidationError {
				kind: ValidationErrorKind::CircularDependency,
				type_name: first_name,
				type_id: first_id,
				message: format!("Circular dependency detected: {}", cycle_desc),
			});
		}
	}

	/// Resolve a human-readable name for a `TypeId`, falling back to debug format.
	fn resolve_type_name(&self, type_id: TypeId) -> String {
		self.registry
			.get_type_name(type_id)
			.map(String::from)
			.unwrap_or_else(|| format!("{:?}", type_id))
	}
}

/// Format a validation report grouped by error kind.
pub fn format_validation_report(errors: &[ValidationError]) -> String {
	let mut report = String::from("DI Registry Validation Failed\n");
	report.push_str(&format!("  {} error(s) found:\n\n", errors.len()));

	let missing: Vec<_> = errors
		.iter()
		.filter(|e| e.kind == ValidationErrorKind::MissingDependency)
		.collect();
	let scope: Vec<_> = errors
		.iter()
		.filter(|e| e.kind == ValidationErrorKind::ScopeIncompatibility)
		.collect();
	let cycle: Vec<_> = errors
		.iter()
		.filter(|e| e.kind == ValidationErrorKind::CircularDependency)
		.collect();

	if !missing.is_empty() {
		report.push_str("Missing Dependencies:\n");
		for err in &missing {
			report.push_str(&format!("  - {}\n", err.message));
		}
		report.push('\n');
	}

	if !scope.is_empty() {
		report.push_str("Scope Incompatibilities:\n");
		for err in &scope {
			report.push_str(&format!("  - {}\n", err.message));
		}
		report.push('\n');
	}

	if !cycle.is_empty() {
		report.push_str("Circular Dependencies:\n");
		for err in &cycle {
			report.push_str(&format!("  - {}\n", err.message));
		}
		report.push('\n');
	}

	report
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::registry::DependencyScope;
	use rstest::*;
	use std::any::TypeId;

	// Dummy types for testing
	struct TypeA;
	struct TypeB;
	struct TypeC;

	/// Helper to create a registry with a factory for a given type.
	fn register_dummy<T: Send + Sync + 'static>(
		registry: &DependencyRegistry,
		scope: DependencyScope,
	) {
		registry.register_async::<T, _, _>(scope, |_ctx| async { unreachable!() });
	}

	#[rstest]
	fn validate_empty_registry_passes() {
		// Arrange
		let registry = Arc::new(DependencyRegistry::new());
		let validator = RegistryValidator::new(registry);

		// Act
		let result = validator.validate();

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn validate_complete_registry_passes() {
		// Arrange
		let registry = Arc::new(DependencyRegistry::new());
		let type_a = TypeId::of::<TypeA>();
		let type_b = TypeId::of::<TypeB>();

		register_dummy::<TypeA>(&registry, DependencyScope::Singleton);
		register_dummy::<TypeB>(&registry, DependencyScope::Singleton);
		registry.register_type_name(type_a, "TypeA");
		registry.register_type_name(type_b, "TypeB");
		registry.register_dependencies(type_a, vec![type_b]);
		registry.register_dependencies(type_b, vec![]);

		let validator = RegistryValidator::new(registry);

		// Act
		let result = validator.validate();

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn validate_missing_dependency() {
		// Arrange
		let registry = Arc::new(DependencyRegistry::new());
		let type_a = TypeId::of::<TypeA>();
		let type_b = TypeId::of::<TypeB>();

		register_dummy::<TypeA>(&registry, DependencyScope::Singleton);
		registry.register_type_name(type_a, "TypeA");
		registry.register_type_name(type_b, "TypeB");
		// TypeA depends on TypeB, but TypeB has no factory registered
		registry.register_dependencies(type_a, vec![type_b]);

		let validator = RegistryValidator::new(registry);

		// Act
		let result = validator.validate();

		// Assert
		let errors = result.unwrap_err();
		assert_eq!(errors.len(), 1);
		assert_eq!(errors[0].kind, ValidationErrorKind::MissingDependency);
		assert!(errors[0].message.contains("TypeB"));
	}

	#[rstest]
	fn validate_scope_singleton_depends_on_request() {
		// Arrange
		let registry = Arc::new(DependencyRegistry::new());
		let type_a = TypeId::of::<TypeA>();
		let type_b = TypeId::of::<TypeB>();

		register_dummy::<TypeA>(&registry, DependencyScope::Singleton);
		register_dummy::<TypeB>(&registry, DependencyScope::Request);
		registry.register_type_name(type_a, "TypeA");
		registry.register_type_name(type_b, "TypeB");
		registry.register_dependencies(type_a, vec![type_b]);
		registry.register_dependencies(type_b, vec![]);

		let validator = RegistryValidator::new(registry);

		// Act
		let result = validator.validate();

		// Assert
		let errors = result.unwrap_err();
		assert!(
			errors
				.iter()
				.any(|e| e.kind == ValidationErrorKind::ScopeIncompatibility)
		);
	}

	#[rstest]
	fn validate_scope_singleton_transitively_depends_on_request() {
		// Arrange: Singleton(A) -> Transient(B) -> Request(C)
		let registry = Arc::new(DependencyRegistry::new());
		let type_a = TypeId::of::<TypeA>();
		let type_b = TypeId::of::<TypeB>();
		let type_c = TypeId::of::<TypeC>();

		register_dummy::<TypeA>(&registry, DependencyScope::Singleton);
		register_dummy::<TypeB>(&registry, DependencyScope::Transient);
		register_dummy::<TypeC>(&registry, DependencyScope::Request);
		registry.register_type_name(type_a, "TypeA");
		registry.register_type_name(type_b, "TypeB");
		registry.register_type_name(type_c, "TypeC");
		registry.register_dependencies(type_a, vec![type_b]);
		registry.register_dependencies(type_b, vec![type_c]);
		registry.register_dependencies(type_c, vec![]);

		let validator = RegistryValidator::new(registry);

		// Act
		let result = validator.validate();

		// Assert
		let errors = result.unwrap_err();
		assert_eq!(errors.len(), 1);
		assert_eq!(errors[0].kind, ValidationErrorKind::ScopeIncompatibility);
		assert!(errors[0].message.contains("TypeA"));
		assert!(errors[0].message.contains("TypeC"));
		assert!(errors[0].message.contains("TypeA -> TypeB -> TypeC"));
	}

	#[rstest]
	fn validate_scope_request_depends_on_singleton_ok() {
		// Arrange
		let registry = Arc::new(DependencyRegistry::new());
		let type_a = TypeId::of::<TypeA>();
		let type_b = TypeId::of::<TypeB>();

		register_dummy::<TypeA>(&registry, DependencyScope::Request);
		register_dummy::<TypeB>(&registry, DependencyScope::Singleton);
		registry.register_type_name(type_a, "TypeA");
		registry.register_type_name(type_b, "TypeB");
		registry.register_dependencies(type_a, vec![type_b]);
		registry.register_dependencies(type_b, vec![]);

		let validator = RegistryValidator::new(registry);

		// Act
		let result = validator.validate();

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn validate_circular_dependency() {
		// Arrange
		let registry = Arc::new(DependencyRegistry::new());
		let type_a = TypeId::of::<TypeA>();
		let type_b = TypeId::of::<TypeB>();

		register_dummy::<TypeA>(&registry, DependencyScope::Singleton);
		register_dummy::<TypeB>(&registry, DependencyScope::Singleton);
		registry.register_type_name(type_a, "TypeA");
		registry.register_type_name(type_b, "TypeB");
		registry.register_dependencies(type_a, vec![type_b]);
		registry.register_dependencies(type_b, vec![type_a]);

		let validator = RegistryValidator::new(registry);

		// Act
		let result = validator.validate();

		// Assert
		let errors = result.unwrap_err();
		assert!(
			errors
				.iter()
				.any(|e| e.kind == ValidationErrorKind::CircularDependency)
		);
	}

	#[rstest]
	fn validate_multiple_errors() {
		// Arrange
		let registry = Arc::new(DependencyRegistry::new());
		let type_a = TypeId::of::<TypeA>();
		let type_b = TypeId::of::<TypeB>();
		let type_c = TypeId::of::<TypeC>();

		// TypeA (Singleton) depends on TypeB (Request) and TypeC (not registered)
		register_dummy::<TypeA>(&registry, DependencyScope::Singleton);
		register_dummy::<TypeB>(&registry, DependencyScope::Request);
		registry.register_type_name(type_a, "TypeA");
		registry.register_type_name(type_b, "TypeB");
		registry.register_type_name(type_c, "TypeC");
		registry.register_dependencies(type_a, vec![type_b, type_c]);
		registry.register_dependencies(type_b, vec![]);

		let validator = RegistryValidator::new(registry);

		// Act
		let result = validator.validate();

		// Assert
		let errors = result.unwrap_err();
		assert!(errors.len() >= 2);
		assert!(
			errors
				.iter()
				.any(|e| e.kind == ValidationErrorKind::MissingDependency)
		);
		assert!(
			errors
				.iter()
				.any(|e| e.kind == ValidationErrorKind::ScopeIncompatibility)
		);
	}

	// --- Additional edge-case tests ---

	struct TypeD;
	struct TypeE;

	#[rstest]
	fn validate_transient_depends_on_request_ok() {
		// Arrange
		let registry = Arc::new(DependencyRegistry::new());
		let type_a = TypeId::of::<TypeA>();
		let type_b = TypeId::of::<TypeB>();

		register_dummy::<TypeA>(&registry, DependencyScope::Transient);
		register_dummy::<TypeB>(&registry, DependencyScope::Request);
		registry.register_type_name(type_a, "TypeA");
		registry.register_type_name(type_b, "TypeB");
		registry.register_dependencies(type_a, vec![type_b]);
		registry.register_dependencies(type_b, vec![]);

		let validator = RegistryValidator::new(registry);

		// Act
		let result = validator.validate();

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn validate_singleton_depends_on_transient_ok() {
		// Arrange
		let registry = Arc::new(DependencyRegistry::new());
		let type_a = TypeId::of::<TypeA>();
		let type_b = TypeId::of::<TypeB>();

		register_dummy::<TypeA>(&registry, DependencyScope::Singleton);
		register_dummy::<TypeB>(&registry, DependencyScope::Transient);
		registry.register_type_name(type_a, "TypeA");
		registry.register_type_name(type_b, "TypeB");
		registry.register_dependencies(type_a, vec![type_b]);
		registry.register_dependencies(type_b, vec![]);

		let validator = RegistryValidator::new(registry);

		// Act
		let result = validator.validate();

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn validate_missing_transitive_dependency() {
		// Arrange: A -> B -> C, C is not registered
		let registry = Arc::new(DependencyRegistry::new());
		let type_a = TypeId::of::<TypeA>();
		let type_b = TypeId::of::<TypeB>();
		let type_c = TypeId::of::<TypeC>();

		register_dummy::<TypeA>(&registry, DependencyScope::Singleton);
		register_dummy::<TypeB>(&registry, DependencyScope::Singleton);
		// TypeC has NO factory
		registry.register_type_name(type_a, "TypeA");
		registry.register_type_name(type_b, "TypeB");
		registry.register_type_name(type_c, "TypeC");
		registry.register_dependencies(type_a, vec![type_b]);
		registry.register_dependencies(type_b, vec![type_c]);

		let validator = RegistryValidator::new(registry);

		// Act
		let result = validator.validate();

		// Assert
		let errors = result.unwrap_err();
		assert_eq!(errors.len(), 1);
		assert_eq!(errors[0].kind, ValidationErrorKind::MissingDependency);
		assert!(errors[0].message.contains("TypeC"));
		assert!(errors[0].message.contains("TypeB"));
	}

	#[rstest]
	fn validate_three_way_circular_dependency() {
		// Arrange: A -> B -> C -> A
		let registry = Arc::new(DependencyRegistry::new());
		let type_a = TypeId::of::<TypeA>();
		let type_b = TypeId::of::<TypeB>();
		let type_c = TypeId::of::<TypeC>();

		register_dummy::<TypeA>(&registry, DependencyScope::Singleton);
		register_dummy::<TypeB>(&registry, DependencyScope::Singleton);
		register_dummy::<TypeC>(&registry, DependencyScope::Singleton);
		registry.register_type_name(type_a, "TypeA");
		registry.register_type_name(type_b, "TypeB");
		registry.register_type_name(type_c, "TypeC");
		registry.register_dependencies(type_a, vec![type_b]);
		registry.register_dependencies(type_b, vec![type_c]);
		registry.register_dependencies(type_c, vec![type_a]);

		let validator = RegistryValidator::new(registry);

		// Act
		let result = validator.validate();

		// Assert
		let errors = result.unwrap_err();
		assert!(
			errors
				.iter()
				.any(|e| e.kind == ValidationErrorKind::CircularDependency)
		);
	}

	#[rstest]
	fn validate_type_without_registered_name_uses_fallback() {
		// Arrange: TypeD depends on TypeE, but neither has a registered type name
		let registry = Arc::new(DependencyRegistry::new());
		let type_d = TypeId::of::<TypeD>();
		let type_e = TypeId::of::<TypeE>();

		register_dummy::<TypeD>(&registry, DependencyScope::Singleton);
		// TypeE has no factory — triggers MissingDependency
		registry.register_dependencies(type_d, vec![type_e]);

		let validator = RegistryValidator::new(registry);

		// Act
		let result = validator.validate();

		// Assert — should not panic; falls back to TypeId debug format
		let errors = result.unwrap_err();
		assert_eq!(errors.len(), 1);
		assert_eq!(errors[0].kind, ValidationErrorKind::MissingDependency);
	}

	#[rstest]
	fn validate_leaf_node_with_no_dependencies() {
		// Arrange: single type with no dependencies declared
		let registry = Arc::new(DependencyRegistry::new());
		register_dummy::<TypeA>(&registry, DependencyScope::Singleton);
		// No register_dependencies call — leaf node

		let validator = RegistryValidator::new(registry);

		// Act
		let result = validator.validate();

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn validate_error_display_formatting() {
		// Arrange
		let err = ValidationError {
			kind: ValidationErrorKind::MissingDependency,
			type_name: "TestType".to_string(),
			type_id: TypeId::of::<TypeA>(),
			message: "'TestType' depends on 'Missing', which is not registered".to_string(),
		};

		// Act
		let display = format!("{}", err);

		// Assert
		assert!(display.starts_with("[MISSING]"));
		assert!(display.contains("TestType"));
	}

	#[rstest]
	fn validate_format_report_groups_errors() {
		// Arrange
		let errors = vec![
			ValidationError {
				kind: ValidationErrorKind::MissingDependency,
				type_name: "A".to_string(),
				type_id: TypeId::of::<TypeA>(),
				message: "'A' depends on 'X', which is not registered".to_string(),
			},
			ValidationError {
				kind: ValidationErrorKind::ScopeIncompatibility,
				type_name: "B".to_string(),
				type_id: TypeId::of::<TypeB>(),
				message: "Singleton 'B' depends on request-scoped 'Y'".to_string(),
			},
		];

		// Act
		let report = format_validation_report(&errors);

		// Assert
		assert!(report.contains("Missing Dependencies:"));
		assert!(report.contains("Scope Incompatibilities:"));
		assert!(report.contains("2 error(s) found"));
	}
}
