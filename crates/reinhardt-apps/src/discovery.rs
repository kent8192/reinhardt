//! Model and migration discovery
//!
//! This module provides functionality for discovering models and migrations
//! within applications. It includes reverse relation building and migration
//! detection capabilities.
//!
//! # Examples
//!
//! ```rust
//! use reinhardt_apps::discovery::{discover_models, build_reverse_relations};
//!
//! // Discover models for an application
//! let models = discover_models("myapp");
//! println!("Found {} models", models.len());
//!
//! // Build reverse relations (when ORM is fully implemented)
//! // build_reverse_relations();
//! ```

use crate::Apps;
use crate::registry::{
	ModelMetadata, ReverseRelationMetadata, ReverseRelationType, get_models_for_app,
	get_registered_models, register_reverse_relation,
};
use std::borrow::Cow;

/// Discover all models for a given application
///
/// This function retrieves all models that belong to the specified application
/// from the global model registry.
///
/// # Examples
///
/// ```rust
/// use reinhardt_apps::discovery::discover_models;
///
/// let models = discover_models("auth");
/// for model in models {
///     println!("Found model: {}", model.model_name);
/// }
/// ```
pub fn discover_models(app_label: &str) -> Vec<&'static ModelMetadata> {
	get_models_for_app(app_label)
}

/// Discover all models across all applications
///
/// This function retrieves all models that have been registered in the
/// global model registry.
///
/// # Examples
///
/// ```rust
/// use reinhardt_apps::discovery::discover_all_models;
///
/// let models = discover_all_models();
/// println!("Total models: {}", models.len());
/// ```
pub fn discover_all_models() -> &'static [ModelMetadata] {
	get_registered_models()
}

/// Relation metadata for building reverse relations
///
/// This structure contains information about a relationship between two models.
/// It is used to build reverse relations automatically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationMetadata {
	/// The source model (e.g., "Post")
	pub from_model: &'static str,

	/// The target model (e.g., "User")
	pub to_model: &'static str,

	/// The field name in the source model (e.g., "author")
	pub field_name: &'static str,

	/// The related name for the reverse relation (e.g., "posts")
	pub related_name: Option<&'static str>,

	/// The type of relation (OneToMany, ManyToMany, etc.)
	pub relation_type: RelationType,
}

/// Type of relationship between models
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationType {
	/// One-to-many relationship
	OneToMany,
	/// Many-to-many relationship
	ManyToMany,
	/// One-to-one relationship
	OneToOne,
}

impl RelationMetadata {
	/// Create a new relation metadata instance
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_apps::discovery::{RelationMetadata, RelationType};
	///
	/// let relation = RelationMetadata::new(
	///     "Post",
	///     "User",
	///     "author",
	///     Some("posts"),
	///     RelationType::OneToMany,
	/// );
	/// assert_eq!(relation.from_model, "Post");
	/// assert_eq!(relation.to_model, "User");
	/// ```
	pub const fn new(
		from_model: &'static str,
		to_model: &'static str,
		field_name: &'static str,
		related_name: Option<&'static str>,
		relation_type: RelationType,
	) -> Self {
		Self {
			from_model,
			to_model,
			field_name,
			related_name,
			relation_type,
		}
	}

	/// Get the reverse relation name
	///
	/// Returns the related_name if specified, otherwise generates a default name
	/// in the format `{from_model}_set` (e.g., "post_set" for a Post model).
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_apps::discovery::{RelationMetadata, RelationType};
	///
	/// let relation = RelationMetadata::new(
	///     "Post",
	///     "User",
	///     "author",
	///     Some("posts"),
	///     RelationType::OneToMany,
	/// );
	/// assert_eq!(relation.reverse_name(), "posts");
	///
	/// // Without related_name, generates default "{model}_set" format
	/// let relation = RelationMetadata::new(
	///     "Post",
	///     "User",
	///     "author",
	///     None,
	///     RelationType::OneToMany,
	/// );
	/// assert_eq!(relation.reverse_name(), "post_set");
	/// ```
	pub fn reverse_name(&self) -> Cow<'static, str> {
		if let Some(name) = self.related_name {
			Cow::Borrowed(name)
		} else {
			// Generate default name: {model}_set (e.g., post_set)
			Cow::Owned(format!("{}_set", self.from_model.to_lowercase()))
		}
	}
}

/// Build reverse relations between models
///
/// This function analyzes the relationships defined in models and automatically
/// creates reverse relations. For example, if a Post model has a ForeignKey to User,
/// this will create a reverse relation from User to Post.
///
/// The function performs the following steps:
/// 1. Discovers all registered models
/// 2. Analyzes each model for relationship fields (ForeignKey, ManyToMany)
/// 3. Generates appropriate reverse accessor names
/// 4. Creates reverse relation descriptors
///
/// The `#[model(...)]` macro automatically detects relationship fields
/// (ForeignKeyField, OneToOneField, ManyToManyField) and generates registration
/// code for the global RELATIONSHIPS registry.
///
/// # Examples
///
/// ```rust
/// use reinhardt_apps::discovery::build_reverse_relations;
///
/// // Build reverse relations for all models
/// build_reverse_relations().unwrap();
/// ```
///
/// # Errors
///
/// Returns [`AppError::RegistryState`] if reverse relations have already been finalized.
pub fn build_reverse_relations() -> Result<(), crate::AppError> {
	// Step 1: Get all registered models
	let models = get_registered_models();

	// Step 2: Collect all relationships
	let mut relations = Vec::new();

	for model in models {
		let model_relations = extract_model_relations(model);
		relations.extend(model_relations);
	}

	// Step 3: Build reverse relation descriptors
	for relation in &relations {
		create_reverse_relation(relation)?;
	}

	Ok(())
}

/// Extract relationship metadata from a model
///
/// This function retrieves all relationships originating from the specified model
/// by looking up the model in the global RELATIONSHIPS registry.
///
/// # Implementation
///
/// This implementation uses Approach 1 from the architecture design:
/// A separate distributed slice (`RELATIONSHIPS`) for relationship metadata that is
/// populated at compile time via derive macros.
///
/// # Returns
///
/// A vector of `RelationMetadata` instances representing the relationships from this model.
fn extract_model_relations(model: &ModelMetadata) -> Vec<RelationMetadata> {
	use crate::registry::get_relationships_for_model;

	// Get the qualified model name
	let qualified_name = model.qualified_name();

	// Look up relationships from the global registry
	let relationships = get_relationships_for_model(&qualified_name);

	// Convert RelationshipMetadata to RelationMetadata
	relationships
		.into_iter()
		.map(|rel| {
			// Map RelationshipType to RelationType
			let relation_type = match rel.relationship_type {
				crate::registry::RelationshipType::ForeignKey => RelationType::OneToMany,
				crate::registry::RelationshipType::ManyToMany => RelationType::ManyToMany,
				crate::registry::RelationshipType::OneToOne => RelationType::OneToOne,
			};

			// Extract model name from qualified name (e.g., "auth.User" -> "User")
			let from_model = rel.from_model_name();
			let to_model = rel.to_model_name();

			RelationMetadata::new(
				from_model,
				to_model,
				rel.field_name,
				rel.related_name,
				relation_type,
			)
		})
		.collect()
}

/// Create a reverse relation descriptor and register it
///
/// This function generates the reverse accessor name and creates a reverse
/// relation descriptor that will be added to the target model.
///
/// # Reverse Accessor Naming
///
/// - If `related_name` is specified, use that name
/// - Otherwise, generate default name: `{from_model_lowercase}_set`
///   - Example: For Post.author -> User, reverse name is "post_set"
///
/// # Relation Type Mapping
///
/// - ForeignKey (OneToMany) -> Reverse is ReverseOneToMany (collection)
/// - ManyToMany -> Reverse is ReverseManyToMany (collection)
/// - OneToOne -> Reverse is ReverseOneToOne (single object)
///
/// # Lazy Loading Strategy
///
/// Reverse relations are registered in the global registry for future use:
/// - By default, accessing a reverse relation triggers a lazy query
/// - Eager loading can be enabled via select_related() or prefetch_related()
/// - The QuerySet system handles the actual database queries
///
/// # Examples
///
/// ```rust
/// use reinhardt_apps::discovery::{RelationMetadata, RelationType, create_reverse_relation};
///
/// // For Post.author -> User relationship
/// let relation = RelationMetadata::new(
///     "Post",
///     "User",
///     "author",
///     Some("posts"),
///     RelationType::OneToMany,
/// );
///
/// // This creates User.posts reverse accessor
/// create_reverse_relation(&relation).unwrap();
///
/// // Later: user.posts().all() returns QuerySet<Post>
/// ```
///
/// # Errors
///
/// Returns [`AppError::RegistryState`] if called after reverse relations have been finalized.
pub fn create_reverse_relation(relation: &RelationMetadata) -> Result<(), crate::AppError> {
	// Use reverse_name() which handles both explicit and default naming
	let reverse_name = relation.reverse_name().into_owned();

	// Map forward relation type to reverse relation type
	let reverse_type = match relation.relation_type {
		RelationType::OneToMany => ReverseRelationType::ReverseOneToMany,
		RelationType::ManyToMany => ReverseRelationType::ReverseManyToMany,
		RelationType::OneToOne => ReverseRelationType::ReverseOneToOne,
	};

	// Fields are already &'static str, so use them directly without Box::leak
	let reverse_relation = ReverseRelationMetadata::new(
		relation.to_model,   // Reverse relation goes on the target model
		reverse_name,        // The accessor name (e.g., "posts" or "post_set")
		relation.from_model, // Related model (where the forward relation is defined)
		reverse_type,        // Type of reverse relation
		relation.field_name, // Original field name for join queries
	);

	register_reverse_relation(reverse_relation)
}

/// Migration metadata
///
/// This structure contains information about a migration file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationMetadata {
	/// The application this migration belongs to
	pub app_label: String,

	/// The migration name (e.g., "initial")
	pub name: String,

	/// The migration number (e.g., 1 for "0001_initial")
	pub number: u32,

	/// Path to the migration file
	pub path: std::path::PathBuf,

	/// Dependencies on other migrations
	pub dependencies: Vec<String>,
}

impl MigrationMetadata {
	/// Create a new migration metadata instance
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_apps::discovery::MigrationMetadata;
	/// use std::path::PathBuf;
	///
	/// let migration = MigrationMetadata::new(
	///     "myapp".to_string(),
	///     "initial".to_string(),
	///     1,
	///     PathBuf::from("migrations/0001_initial.rs"),
	///     vec![],
	/// );
	/// assert_eq!(migration.app_label, "myapp");
	/// assert_eq!(migration.name, "initial");
	/// assert_eq!(migration.number, 1);
	/// ```
	pub fn new(
		app_label: String,
		name: String,
		number: u32,
		path: std::path::PathBuf,
		dependencies: Vec<String>,
	) -> Self {
		Self {
			app_label,
			name,
			number,
			path,
			dependencies,
		}
	}

	/// Get the fully qualified migration name
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_apps::discovery::MigrationMetadata;
	/// use std::path::PathBuf;
	///
	/// let migration = MigrationMetadata::new(
	///     "myapp".to_string(),
	///     "initial".to_string(),
	///     1,
	///     PathBuf::from("migrations/0001_initial.rs"),
	///     vec![],
	/// );
	/// assert_eq!(migration.qualified_name(), "myapp.0001_initial");
	/// ```
	pub fn qualified_name(&self) -> String {
		format!("{}.{:04}_{}", self.app_label, self.number, self.name)
	}
}

/// Discover migration files in the project
///
/// This function scans all registered applications for migration files in their
/// `migrations/` directories. It extracts metadata from migration file names.
///
/// # Arguments
///
/// * `apps` - The Apps registry containing registered applications
///
/// # Migration File Naming Convention
///
/// Migration files should follow the pattern: `{number}_{name}.rs`
/// - `number`: 4-digit migration number (e.g., 0001, 0002)
/// - `name`: Descriptive name (e.g., initial, add_user_field)
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_apps::{Apps, discover_migrations};
///
/// let apps = Apps::new(vec!["myapp".to_string()]);
/// apps.populate().unwrap();
/// let migrations = discover_migrations(&apps).unwrap();
/// for migration in migrations {
///     println!("Found migration: {}", migration.qualified_name());
/// }
/// ```
pub fn discover_migrations(apps: &Apps) -> Result<Vec<MigrationMetadata>, String> {
	use std::fs;
	use std::path::PathBuf;

	let mut migrations = Vec::new();

	for app in apps.get_app_configs() {
		// Get the app's base path
		let Some(app_path_str) = app.path else {
			continue;
		};

		let app_path = PathBuf::from(&app_path_str);
		let migrations_dir = app_path.join("migrations");

		// Check if migrations directory exists
		if !migrations_dir.exists() || !migrations_dir.is_dir() {
			continue;
		}

		// Read migration files
		let entries = fs::read_dir(&migrations_dir)
			.map_err(|e| format!("Failed to read migrations directory: {}", e))?;

		for entry in entries {
			let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
			let path = entry.path();

			// Skip non-files
			if !path.is_file() {
				continue;
			}

			// Only process .rs files
			if path.extension().and_then(|s| s.to_str()) != Some("rs") {
				continue;
			}

			let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
				continue;
			};

			// Skip files that don't start with a digit
			if !file_name.starts_with(|c: char| c.is_ascii_digit()) {
				continue;
			}

			// Parse migration file
			match parse_migration_file(&path, &app.label) {
				Ok(migration) => migrations.push(migration),
				Err(_) => continue,
			}
		}
	}

	Ok(migrations)
}

/// Parse a migration file and extract metadata
///
/// This function extracts migration information from the file name.
/// Expected format: `{number}_{name}.rs` (e.g., `0001_initial.rs`)
fn parse_migration_file(
	path: &std::path::Path,
	app_label: &str,
) -> Result<MigrationMetadata, String> {
	let filename = path
		.file_stem()
		.and_then(|s| s.to_str())
		.ok_or_else(|| "Invalid filename".to_string())?;

	// Extract migration number and name
	// Expected format: {number}_{name}
	let parts: Vec<&str> = filename.splitn(2, '_').collect();

	if parts.len() != 2 {
		return Err(format!("Invalid migration filename format: {}", filename));
	}

	let number = parts[0]
		.parse::<u32>()
		.map_err(|_| format!("Invalid migration number: {}", parts[0]))?;

	let name = parts[1].to_string();

	Ok(MigrationMetadata::new(
		app_label.to_string(),
		name,
		number,
		path.to_path_buf(),
		vec![], // Dependencies are not extracted from file name
	))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::registry::{MODELS, ModelMetadata, ReverseRelationType};
	use linkme::distributed_slice;
	use rstest::*;
	use serial_test::serial;

	// Test models for discovery
	#[distributed_slice(MODELS)]
	static DISCOVERY_TEST_USER: ModelMetadata = ModelMetadata {
		app_label: "discovery_test",
		model_name: "User",
		table_name: "discovery_test_users",
	};

	#[distributed_slice(MODELS)]
	static DISCOVERY_TEST_POST: ModelMetadata = ModelMetadata {
		app_label: "discovery_test",
		model_name: "Post",
		table_name: "discovery_test_posts",
	};

	#[test]
	fn test_relation_metadata_new() {
		let relation = RelationMetadata::new(
			"Post",
			"User",
			"author",
			Some("posts"),
			RelationType::OneToMany,
		);

		assert_eq!(relation.from_model, "Post");
		assert_eq!(relation.to_model, "User");
		assert_eq!(relation.field_name, "author");
		assert_eq!(relation.related_name, Some("posts"));
		assert_eq!(relation.relation_type, RelationType::OneToMany);
	}

	#[test]
	fn test_relation_metadata_reverse_name() {
		// With explicit related_name
		let relation = RelationMetadata::new(
			"Post",
			"User",
			"author",
			Some("posts"),
			RelationType::OneToMany,
		);
		assert_eq!(relation.reverse_name(), "posts");

		// Without related_name, generates default "{model}_set" format
		let relation =
			RelationMetadata::new("Post", "User", "author", None, RelationType::OneToMany);
		assert_eq!(relation.reverse_name(), "post_set");
	}

	#[test]
	fn test_relation_types() {
		assert_eq!(RelationType::OneToMany, RelationType::OneToMany);
		assert_ne!(RelationType::OneToMany, RelationType::ManyToMany);
		assert_ne!(RelationType::OneToMany, RelationType::OneToOne);
	}

	#[test]
	fn test_migration_metadata_new() {
		use std::path::PathBuf;

		let migration = MigrationMetadata::new(
			"myapp".to_string(),
			"initial".to_string(),
			1,
			PathBuf::from("/tmp/migrations/0001_initial.rs"),
			vec![],
		);

		assert_eq!(migration.app_label, "myapp");
		assert_eq!(migration.name, "initial");
		assert_eq!(migration.number, 1);
		assert_eq!(migration.dependencies.len(), 0);
	}

	#[test]
	fn test_migration_metadata_qualified_name() {
		use std::path::PathBuf;

		let migration = MigrationMetadata::new(
			"myapp".to_string(),
			"initial".to_string(),
			1,
			PathBuf::from("/tmp/migrations/0001_initial.rs"),
			vec![],
		);
		assert_eq!(migration.qualified_name(), "myapp.0001_initial");
	}

	#[test]
	fn test_migration_metadata_with_dependencies() {
		use std::path::PathBuf;

		let migration = MigrationMetadata::new(
			"myapp".to_string(),
			"add_field".to_string(),
			2,
			PathBuf::from("/tmp/migrations/0002_add_field.rs"),
			vec![
				"myapp.0001_initial".to_string(),
				"auth.0001_initial".to_string(),
			],
		);

		assert_eq!(migration.dependencies.len(), 2);
		assert_eq!(migration.dependencies[0], "myapp.0001_initial");
		assert_eq!(migration.dependencies[1], "auth.0001_initial");
	}

	#[test]
	fn test_parse_migration_file() {
		use std::path::PathBuf;

		let path = PathBuf::from("/tmp/migrations/0001_initial.rs");
		let result = parse_migration_file(&path, "myapp");

		let migration = result.unwrap();
		assert_eq!(migration.app_label, "myapp");
		assert_eq!(migration.name, "initial");
		assert_eq!(migration.number, 1);
	}

	#[test]
	fn test_parse_migration_file_with_underscores() {
		use std::path::PathBuf;

		let path = PathBuf::from("/tmp/migrations/0002_add_user_field.rs");
		let result = parse_migration_file(&path, "myapp");

		let migration = result.unwrap();
		assert_eq!(migration.app_label, "myapp");
		assert_eq!(migration.name, "add_user_field");
		assert_eq!(migration.number, 2);
	}

	#[test]
	fn test_parse_migration_file_invalid_format() {
		use std::path::PathBuf;

		// No underscore separator
		let path = PathBuf::from("/tmp/migrations/0001initial.rs");
		let result = parse_migration_file(&path, "myapp");
		assert!(result.is_err());

		// Invalid number
		let path = PathBuf::from("/tmp/migrations/abc_initial.rs");
		let result = parse_migration_file(&path, "myapp");
		assert!(result.is_err());
	}

	#[rstest]
	#[case(RelationType::OneToMany, ReverseRelationType::ReverseOneToMany)]
	#[case(RelationType::ManyToMany, ReverseRelationType::ReverseManyToMany)]
	#[case(RelationType::OneToOne, ReverseRelationType::ReverseOneToOne)]
	#[serial(apps_registry)]
	fn test_create_reverse_relation_uses_static_fields_directly(
		#[case] relation_type: RelationType,
		#[case] expected_reverse_type: ReverseRelationType,
	) {
		// Arrange - Reset global state before test
		crate::registry::reset_global_registry();

		// Arrange: fields are &'static str literals (no heap allocation needed)
		let relation = RelationMetadata::new(
			"Article",
			"Author",
			"writer",
			Some("articles"),
			relation_type,
		);

		// Act: create_reverse_relation uses fields directly without Box::leak
		create_reverse_relation(&relation).expect("reverse relation registration should succeed");

		// Assert: the reverse relation is registered with the correct values
		// from the original &'static str fields (pointer equality confirms
		// no intermediate String allocation was leaked)
		let reverse_relations = crate::registry::get_reverse_relations_for_model("Author");
		let found = reverse_relations
			.iter()
			.find(|r| r.accessor_name == "articles");

		if let Some(rev) = found {
			assert_eq!(rev.on_model, "Author");
			assert_eq!(rev.related_model, "Article");
			assert_eq!(rev.through_field, "writer");
			assert_eq!(rev.relation_type, expected_reverse_type);
			// Verify pointer identity: the stored &'static str should point to
			// the same string literal as the input, confirming no Box::leak copy
			assert!(std::ptr::eq(rev.on_model, relation.to_model));
			assert!(std::ptr::eq(rev.related_model, relation.from_model));
			assert!(std::ptr::eq(rev.through_field, relation.field_name));
		}
	}

	#[rstest]
	#[serial(apps_registry)]
	fn test_create_reverse_relation_default_accessor_name() {
		// Arrange - Reset global state before test
		crate::registry::reset_global_registry();

		// Arrange: no explicit related_name, should generate "{model}_set"
		let relation =
			RelationMetadata::new("Comment", "BlogPost", "post", None, RelationType::OneToMany);

		// Act
		create_reverse_relation(&relation).expect("reverse relation registration should succeed");

		// Assert: default accessor name follows "{from_model_lowercase}_set" pattern
		let reverse_relations = crate::registry::get_reverse_relations_for_model("BlogPost");
		let found = reverse_relations
			.iter()
			.find(|r| r.accessor_name == "comment_set");

		if let Some(rev) = found {
			assert_eq!(rev.on_model, "BlogPost");
			assert_eq!(rev.accessor_name, "comment_set");
			assert_eq!(rev.related_model, "Comment");
			assert_eq!(rev.relation_type, ReverseRelationType::ReverseOneToMany);
		}
	}
}
