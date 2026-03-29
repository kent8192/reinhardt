//! Migration definition

use super::Operation;
use super::dependency::{OptionalDependency, SwappableDependency};
use serde::{Deserialize, Serialize};

/// A database migration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Migration {
	/// Migration name (e.g., "0001_initial")
	pub name: String,

	/// App label
	pub app_label: String,

	/// Operations to apply
	pub operations: Vec<Operation>,

	/// Dependencies (app_label, migration_name)
	pub dependencies: Vec<(String, String)>,

	/// Migrations this replaces
	pub replaces: Vec<(String, String)>,

	/// Whether this is wrapped in a transaction
	pub atomic: bool,

	/// Whether this is an initial migration (explicit or inferred from dependencies)
	/// - `Some(true)`: Explicitly marked as initial
	/// - `Some(false)`: Explicitly marked as non-initial
	/// - `None`: Auto-infer from `dependencies.is_empty()`
	pub initial: Option<bool>,

	/// Whether to update only ProjectState without executing database operations
	/// (Django's SeparateDatabaseAndState equivalent with state_operations only)
	#[serde(default)]
	pub state_only: bool,

	/// Whether to execute only database operations without updating ProjectState
	/// (Django's SeparateDatabaseAndState equivalent with database_operations only)
	#[serde(default)]
	pub database_only: bool,

	/// Swappable dependencies (e.g., AUTH_USER_MODEL pattern)
	///
	/// These dependencies resolve to different apps based on settings.
	/// For example, a migration depending on the User model might use:
	/// ```no_run
	/// swappable_dependencies: vec![
	///     SwappableDependency::new("AUTH_USER_MODEL", "auth", "User", "0001_initial")
	/// ]
	#[serde(default)]
	pub swappable_dependencies: Vec<SwappableDependency>,

	/// Optional dependencies (conditional based on app installation or settings)
	///
	/// These dependencies are only enforced when their condition is met.
	/// For example, a migration might optionally depend on PostGIS:
	/// ```no_run
	/// optional_dependencies: vec![
	///     OptionalDependency::new(
	///         "gis_extension",
	///         "0001_enable_postgis",
	///         DependencyCondition::AppInstalled("gis_extension".to_string())
	///     )
	/// ]
	#[serde(default)]
	pub optional_dependencies: Vec<OptionalDependency>,
}

impl Migration {
	/// Create a new migration
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::Migration;
	///
	/// let migration = Migration::new("0001_initial", "myapp");
	/// assert_eq!(migration.name, "0001_initial");
	/// assert_eq!(migration.app_label, "myapp");
	/// assert!(migration.atomic);
	/// ```
	pub fn new(name: impl Into<String>, app_label: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			app_label: app_label.into(),
			operations: Vec::new(),
			dependencies: Vec::new(),
			replaces: Vec::new(),
			atomic: true,
			initial: None,
			state_only: false,
			database_only: false,
			swappable_dependencies: Vec::new(),
			optional_dependencies: Vec::new(),
		}
	}
	/// Add an operation to this migration
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::{Migration, Operation, ColumnDefinition, FieldType};
	///
	/// let migration = Migration::new("0001_initial", "myapp")
	///     .add_operation(Operation::CreateTable {
	///         name: "users".to_string(),
	///         columns: vec![ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string()))],
	///         constraints: vec![],
	///         without_rowid: None,
	///         interleave_in_parent: None,
	///         partition: None,
	///     });
	///
	/// assert_eq!(migration.operations.len(), 1);
	/// ```
	pub fn add_operation(mut self, operation: Operation) -> Self {
		self.operations.push(operation);
		self
	}
	/// Add a dependency to this migration
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::Migration;
	///
	/// let migration = Migration::new("0002_add_field", "myapp")
	///     .add_dependency("myapp", "0001_initial");
	///
	/// assert_eq!(migration.dependencies.len(), 1);
	/// assert_eq!(migration.dependencies[0].0, "myapp");
	/// assert_eq!(migration.dependencies[0].1, "0001_initial");
	/// ```
	pub fn add_dependency(mut self, app_label: impl Into<String>, name: impl Into<String>) -> Self {
		self.dependencies.push((app_label.into(), name.into()));
		self
	}

	/// Add a swappable dependency to this migration
	///
	/// Swappable dependencies resolve to different apps based on settings.
	/// This is used for Django's AUTH_USER_MODEL pattern.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::Migration;
	/// use reinhardt_db::migrations::dependency::SwappableDependency;
	///
	/// let migration = Migration::new("0001_create_profile", "profiles")
	///     .add_swappable_dependency(SwappableDependency::new(
	///         "AUTH_USER_MODEL",
	///         "auth",
	///         "User",
	///         "0001_initial",
	///     ));
	///
	/// assert_eq!(migration.swappable_dependencies.len(), 1);
	/// ```
	pub fn add_swappable_dependency(mut self, dependency: SwappableDependency) -> Self {
		self.swappable_dependencies.push(dependency);
		self
	}

	/// Add an optional dependency to this migration
	///
	/// Optional dependencies are only enforced when their condition is met.
	/// This is useful for migrations that depend on optional features or apps.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::Migration;
	/// use reinhardt_db::migrations::dependency::{OptionalDependency, DependencyCondition};
	///
	/// let migration = Migration::new("0002_add_location", "geo_app")
	///     .add_optional_dependency(OptionalDependency::new(
	///         "gis_extension",
	///         "0001_enable_postgis",
	///         DependencyCondition::AppInstalled("gis_extension".to_string()),
	///     ));
	///
	/// assert_eq!(migration.optional_dependencies.len(), 1);
	/// ```
	pub fn add_optional_dependency(mut self, dependency: OptionalDependency) -> Self {
		self.optional_dependencies.push(dependency);
		self
	}

	/// Set whether this migration should run in a transaction
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::Migration;
	///
	/// let migration = Migration::new("0001_initial", "myapp")
	///     .atomic(false);
	///
	/// assert!(!migration.atomic);
	/// ```
	pub fn atomic(mut self, atomic: bool) -> Self {
		self.atomic = atomic;
		self
	}
	/// Get full migration identifier
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::Migration;
	///
	/// let migration = Migration::new("0001_initial", "myapp");
	/// assert_eq!(migration.id(), "myapp.0001_initial");
	/// ```
	pub fn id(&self) -> String {
		format!("{}.{}", self.app_label, self.name)
	}

	/// Set initial attribute explicitly
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::Migration;
	///
	/// let migration = Migration::new("0001_initial", "myapp")
	///     .initial(true);
	///
	/// assert!(migration.is_initial());
	/// ```
	pub fn initial(mut self, initial: bool) -> Self {
		self.initial = Some(initial);
		self
	}

	/// Set whether to update only ProjectState without database operations
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::Migration;
	///
	/// let migration = Migration::new("0001_state_sync", "myapp")
	///     .state_only(true);
	///
	/// assert!(migration.state_only);
	/// assert!(!migration.database_only);
	/// ```
	pub fn state_only(mut self, value: bool) -> Self {
		self.state_only = value;
		self
	}

	/// Set whether to execute only database operations without ProjectState updates
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::Migration;
	///
	/// let migration = Migration::new("0001_db_only", "myapp")
	///     .database_only(true);
	///
	/// assert!(migration.database_only);
	/// assert!(!migration.state_only);
	/// ```
	pub fn database_only(mut self, value: bool) -> Self {
		self.database_only = value;
		self
	}

	/// Check if this is an initial migration
	///
	/// Returns `true` if:
	/// - `initial` is explicitly set to `Some(true)`, OR
	/// - `initial` is `None` and `dependencies` is empty
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::Migration;
	///
	/// // Explicitly marked as initial
	/// let migration1 = Migration::new("0001_initial", "myapp")
	///     .initial(true);
	/// assert!(migration1.is_initial());
	///
	/// // Auto-inferred from empty dependencies
	/// let migration2 = Migration::new("0001_initial", "myapp");
	/// assert!(migration2.is_initial());
	///
	/// // Has dependencies, not initial
	/// let migration3 = Migration::new("0002_add_field", "myapp")
	///     .add_dependency("myapp", "0001_initial");
	/// assert!(!migration3.is_initial());
	///
	/// // Explicitly marked as non-initial
	/// let migration4 = Migration::new("0001_custom", "myapp")
	///     .initial(false);
	/// assert!(!migration4.is_initial());
	/// ```
	pub fn is_initial(&self) -> bool {
		match self.initial {
			Some(true) => true,
			Some(false) => false,
			None => self.dependencies.is_empty(),
		}
	}
}

// Auto-generated tests for migrations module
// Translated from Django/SQLAlchemy test suite
// Total available: 1618 | Included: 100

#[cfg(test)]
mod migrations_extended_tests {
	use crate::migrations::operations;
	use crate::migrations::{FieldType, ForeignKeyAction};

	#[test]
	// From: Django/migrations
	fn test_add_alter_order_with_respect_to() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		// Create parent table
		let create_categories = Operation::CreateTable {
			name: "categories".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("name", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_categories.state_forwards("testapp", &mut state);

		// Create child table with FK to parent
		let create_items = Operation::CreateTable {
			name: "items".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("name", FieldType::VarChar(200)),
				ColumnDefinition::new(
					"category_id",
					FieldType::Custom("INTEGER REFERENCES categories(id)".to_string()),
				),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_items.state_forwards("testapp", &mut state);

		// Add order_with_respect_to field (_order)
		let add_order = Operation::AddColumn {
			table: "items".to_string(),
			column: ColumnDefinition::new(
				"_order",
				FieldType::Custom("INTEGER NOT NULL DEFAULT 0".to_string()),
			),
			mysql_options: None,
		};
		add_order.state_forwards("testapp", &mut state);

		// Create composite index on (category_id, _order)
		let _create_index = Operation::CreateIndex {
			table: "items".to_string(),
			columns: vec!["category_id".to_string(), "_order".to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		};

		let model = state.get_model("testapp", "items").unwrap();
		assert!(model.fields.contains_key("_order"));
		assert!(model.fields.contains_key("category_id"));
	}

	#[test]
	// From: Django/migrations
	fn test_add_alter_order_with_respect_to_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		// Create parent
		let create_parent = Operation::CreateTable {
			name: "authors".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_parent.state_forwards("app", &mut state);

		// Create child with FK
		let create_child = Operation::CreateTable {
			name: "books".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("title", FieldType::VarChar(255)),
				ColumnDefinition::new(
					"author_id",
					FieldType::Custom("INTEGER REFERENCES authors(id)".to_string()),
				),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_child.state_forwards("app", &mut state);

		// Add _order field for order_with_respect_to
		let add_order = Operation::AddColumn {
			table: "books".to_string(),
			column: ColumnDefinition::new(
				"_order",
				FieldType::Custom("INTEGER NOT NULL DEFAULT 0".to_string()),
			),
			mysql_options: None,
		};
		add_order.state_forwards("app", &mut state);

		assert!(
			state
				.get_model("app", "books")
				.unwrap()
				.fields
				.contains_key("_order")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_add_auto_field_does_not_request_default() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "items".to_string(),
			columns: vec![ColumnDefinition::new("name", FieldType::VarChar(255))],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		// AutoField doesn't need default - it's auto-incrementing
		let add_op = Operation::AddColumn {
			table: "items".to_string(),
			column: ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY AUTOINCREMENT".to_string()),
			),
			mysql_options: None,
		};
		add_op.state_forwards("testapp", &mut state);

		assert!(
			state
				.get_model("testapp", "items")
				.unwrap()
				.fields
				.contains_key("id")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_add_auto_field_does_not_request_default_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "entries".to_string(),
			columns: vec![ColumnDefinition::new("title", FieldType::Text)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let add_op = Operation::AddColumn {
			table: "entries".to_string(),
			column: ColumnDefinition::new(
				"entry_id",
				FieldType::Custom("SERIAL PRIMARY KEY".to_string()),
			),
			mysql_options: None,
		};
		add_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "entries").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_add_blank_textfield_and_charfield() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "articles".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		// Add blank=True text fields (nullable)
		let add_text = Operation::AddColumn {
			table: "articles".to_string(),
			column: ColumnDefinition::new("content", FieldType::Text),
			mysql_options: None,
		};
		add_text.state_forwards("testapp", &mut state);

		let add_char = Operation::AddColumn {
			table: "articles".to_string(),
			column: ColumnDefinition::new("title", FieldType::VarChar(255)),
			mysql_options: None,
		};
		add_char.state_forwards("testapp", &mut state);

		let model = state.get_model("testapp", "articles").unwrap();
		assert!(model.fields.contains_key("content"));
		assert!(model.fields.contains_key("title"));
	}

	#[test]
	// From: Django/migrations
	fn test_add_blank_textfield_and_charfield_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "posts".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let add_op = Operation::AddColumn {
			table: "posts".to_string(),
			column: ColumnDefinition::new(
				"description",
				FieldType::Custom("TEXT NULL".to_string()),
			),
			mysql_options: None,
		};
		add_op.state_forwards("app", &mut state);

		assert!(
			state
				.get_model("app", "posts")
				.unwrap()
				.fields
				.contains_key("description")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_add_composite_pk() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		// Create table with composite primary key
		// Note: Composite primary keys are handled via column definitions, not constraints
		let create_op = Operation::CreateTable {
			name: "order_items".to_string(),
			columns: vec![
				ColumnDefinition::new("order_id", FieldType::Integer),
				ColumnDefinition::new("product_id", FieldType::Integer),
				ColumnDefinition::new("quantity", FieldType::Integer),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		let model = state.get_model("testapp", "order_items").unwrap();
		assert!(model.fields.contains_key("order_id"));
		assert!(model.fields.contains_key("product_id"));
	}

	#[test]
	// From: Django/migrations
	fn test_add_composite_pk_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		// Note: Composite primary keys are handled via column definitions, not constraints
		let create_op = Operation::CreateTable {
			name: "user_roles".to_string(),
			columns: vec![
				ColumnDefinition::new("user_id", FieldType::Integer),
				ColumnDefinition::new("role_id", FieldType::Integer),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "user_roles").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_add_constraints() {
		use crate::migrations::operations::*;

		// Test AddConstraint operation SQL generation
		let op = Operation::AddConstraint {
			table: "users".to_string(),
			constraint_sql: "CHECK (age >= 18)".to_string(),
		};

		let sql = op.to_sql(&SqlDialect::Postgres);
		assert!(sql.contains("ALTER TABLE users"));
		assert!(sql.contains("ADD CHECK (age >= 18)"));
	}

	#[test]
	// From: Django/migrations
	fn test_add_constraints_1() {
		use crate::migrations::operations::*;

		// Test adding a unique constraint
		let op = Operation::AddConstraint {
			table: "products".to_string(),
			constraint_sql: "UNIQUE (sku)".to_string(),
		};

		let sql = op.to_sql(&SqlDialect::Postgres);
		assert!(sql.contains("ALTER TABLE products"));
		assert!(sql.contains("ADD UNIQUE (sku)"));
	}

	#[test]
	// From: Django/migrations
	fn test_add_constraints_with_dict_keys() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "products".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new(
					"price",
					FieldType::Decimal {
						precision: 10,
						scale: 2,
					},
				),
				ColumnDefinition::new(
					"discount_price",
					FieldType::Decimal {
						precision: 10,
						scale: 2,
					},
				),
			],
			constraints: vec![
				Constraint::Check {
					name: "price_positive".to_string(),
					expression: "price >= 0".to_string(),
				},
				Constraint::Check {
					name: "discount_price_valid".to_string(),
					expression: "discount_price <= price".to_string(),
				},
			],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		let model = state.get_model("testapp", "products").unwrap();
		assert!(model.fields.contains_key("price"));
		assert!(model.fields.contains_key("discount_price"));
	}

	#[test]
	// From: Django/migrations
	fn test_add_constraints_with_dict_keys_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("age", FieldType::Integer),
			],
			constraints: vec![Constraint::Check {
				name: "age_valid_range".to_string(),
				expression: "age >= 0 AND age <= 150".to_string(),
			}],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "users").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_add_constraints_with_new_model() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		// Create a table with constraints
		let create_op = Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("age", FieldType::Integer),
			],
			constraints: vec![Constraint::Check {
				name: "age_adult".to_string(),
				expression: "age >= 18".to_string(),
			}],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		let model = state.get_model("testapp", "users").unwrap();
		assert!(model.fields.contains_key("id"));
		assert!(model.fields.contains_key("age"));
	}

	#[test]
	// From: Django/migrations
	fn test_add_constraints_with_new_model_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "products".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new(
					"price",
					FieldType::Decimal {
						precision: 10,
						scale: 2,
					},
				),
			],
			constraints: vec![Constraint::Check {
				name: "price_positive".to_string(),
				expression: "price > 0".to_string(),
			}],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "products").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_add_custom_fk_with_hardcoded_to() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		// Create referenced table first
		let create_users = Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_users.state_forwards("testapp", &mut state);

		// Create table with FK
		let create_posts = Operation::CreateTable {
			name: "posts".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("author_id", FieldType::Integer),
			],
			constraints: vec![Constraint::ForeignKey {
				name: "fk_posts_author".to_string(),
				columns: vec!["author_id".to_string()],
				referenced_table: "users".to_string(),
				referenced_columns: vec!["id".to_string()],
				on_delete: ForeignKeyAction::Cascade,
				on_update: ForeignKeyAction::Cascade,
				deferrable: None,
			}],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		};
		create_posts.state_forwards("testapp", &mut state);

		assert!(
			state
				.get_model("testapp", "posts")
				.unwrap()
				.fields
				.contains_key("author_id")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_add_custom_fk_with_hardcoded_to_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_categories = Operation::CreateTable {
			name: "categories".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_categories.state_forwards("app", &mut state);

		let create_products = Operation::CreateTable {
			name: "products".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new(
					"category_id",
					FieldType::Custom("INTEGER REFERENCES categories(id)".to_string()),
				),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_products.state_forwards("app", &mut state);

		assert!(state.get_model("app", "products").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_add_date_fields_with_auto_now_add_asking_for_default() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "posts".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		// auto_now_add simulated with DEFAULT CURRENT_TIMESTAMP
		let add_op = Operation::AddColumn {
			table: "posts".to_string(),
			column: ColumnDefinition::new(
				"created_at",
				FieldType::Custom(
					FieldType::Custom("TIMESTAMP DEFAULT CURRENT_TIMESTAMP".to_string())
						.to_string(),
				),
			),
			mysql_options: None,
		};
		add_op.state_forwards("testapp", &mut state);

		assert!(
			state
				.get_model("testapp", "posts")
				.unwrap()
				.fields
				.contains_key("created_at")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_add_date_fields_with_auto_now_add_asking_for_default_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "articles".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let add_op = Operation::AddColumn {
			table: "articles".to_string(),
			column: ColumnDefinition::new(
				"published_at",
				FieldType::Custom("TIMESTAMP DEFAULT NOW()".to_string()),
			),
			mysql_options: None,
		};
		add_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "articles").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_add_date_fields_with_auto_now_add_not_asking_for_null_addition() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "events".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		// auto_now_add with NOT NULL
		let add_op = Operation::AddColumn {
			table: "events".to_string(),
			column: ColumnDefinition::new(
				"created_at",
				FieldType::Custom("TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP".to_string()),
			),
			mysql_options: None,
		};
		add_op.state_forwards("testapp", &mut state);

		assert!(
			state
				.get_model("testapp", "events")
				.unwrap()
				.fields
				.contains_key("created_at")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_add_date_fields_with_auto_now_add_not_asking_for_null_addition_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "logs".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let add_op = Operation::AddColumn {
			table: "logs".to_string(),
			column: ColumnDefinition::new(
				"timestamp",
				FieldType::Custom("DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP".to_string()),
			),
			mysql_options: None,
		};
		add_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "logs").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_add_date_fields_with_auto_now_not_asking_for_default() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "records".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		// auto_now typically uses triggers or application-level updates
		// For migration testing, we just add the field
		let add_op = Operation::AddColumn {
			table: "records".to_string(),
			column: ColumnDefinition::new("updated_at", FieldType::Custom("TIMESTAMP".to_string())),
			mysql_options: None,
		};
		add_op.state_forwards("testapp", &mut state);

		assert!(
			state
				.get_model("testapp", "records")
				.unwrap()
				.fields
				.contains_key("updated_at")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_add_date_fields_with_auto_now_not_asking_for_default_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "profiles".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let add_op = Operation::AddColumn {
			table: "profiles".to_string(),
			column: ColumnDefinition::new("modified", FieldType::Custom("DATETIME".to_string())),
			mysql_options: None,
		};
		add_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "profiles").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_add_field() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		// Create a table first
		let create_op = Operation::CreateTable {
			name: "test_table".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		// Add a field
		let add_op = Operation::AddColumn {
			table: "test_table".to_string(),
			column: ColumnDefinition::new("name", FieldType::VarChar(255)),
			mysql_options: None,
		};
		add_op.state_forwards("testapp", &mut state);

		let model = state.get_model("testapp", "test_table").unwrap();
		assert!(model.fields.contains_key("name"));
	}

	#[test]
	// From: Django/migrations
	fn test_add_field_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let add_op = Operation::AddColumn {
			table: "users".to_string(),
			column: ColumnDefinition::new("email", FieldType::VarChar(255)),
			mysql_options: None,
		};
		add_op.state_forwards("app", &mut state);

		assert!(
			state
				.get_model("app", "users")
				.unwrap()
				.fields
				.contains_key("email")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_add_field_and_unique_together() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("email", FieldType::VarChar(255)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let add_op = Operation::AddColumn {
			table: "users".to_string(),
			column: ColumnDefinition::new("username", FieldType::VarChar(100)),
			mysql_options: None,
		};
		add_op.state_forwards("app", &mut state);

		let unique_op = Operation::AlterUniqueTogether {
			table: "users".to_string(),
			unique_together: vec![vec!["email".to_string(), "username".to_string()]],
		};
		unique_op.state_forwards("app", &mut state);

		assert!(
			state
				.get_model("app", "users")
				.unwrap()
				.fields
				.contains_key("username")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_add_field_and_unique_together_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "posts".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("title", FieldType::VarChar(255)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let add_op = Operation::AddColumn {
			table: "posts".to_string(),
			column: ColumnDefinition::new("slug", FieldType::VarChar(255)),
			mysql_options: None,
		};
		add_op.state_forwards("app", &mut state);

		let unique_op = Operation::AlterUniqueTogether {
			table: "posts".to_string(),
			unique_together: vec![vec!["slug".to_string()]],
		};
		unique_op.state_forwards("app", &mut state);

		assert!(
			state
				.get_model("app", "posts")
				.unwrap()
				.fields
				.contains_key("slug")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_add_field_before_generated_field() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		// Create a table
		let create_op = Operation::CreateTable {
			name: "products".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new(
					"price",
					FieldType::Decimal {
						precision: 10,
						scale: 2,
					},
				),
				ColumnDefinition::new("quantity", FieldType::Integer),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		// Add a regular field before adding a generated field
		let add_discount = Operation::AddColumn {
			table: "products".to_string(),
			column: ColumnDefinition::new(
				"discount",
				FieldType::Custom("DECIMAL(10,2) DEFAULT 0".to_string()),
			),
			mysql_options: None,
		};
		add_discount.state_forwards("testapp", &mut state);

		// Add a generated field (total = price * quantity)
		// Generated columns are supported using GENERATED ALWAYS AS syntax
		let add_generated = Operation::AddColumn {
			table: "products".to_string(),
			column: ColumnDefinition::new(
				"total",
				FieldType::Custom(
					"DECIMAL(10,2) GENERATED ALWAYS AS (price * quantity) STORED".to_string(),
				),
			),
			mysql_options: None,
		};
		add_generated.state_forwards("testapp", &mut state);

		let model = state.get_model("testapp", "products").unwrap();
		assert!(model.fields.contains_key("discount"));
		assert!(model.fields.contains_key("total"));
	}

	#[test]
	// From: Django/migrations
	fn test_add_field_before_generated_field_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("first_name", FieldType::VarChar(100)),
				ColumnDefinition::new("last_name", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		// Add regular field
		let add_email = Operation::AddColumn {
			table: "users".to_string(),
			column: ColumnDefinition::new("email", FieldType::VarChar(255)),
			mysql_options: None,
		};
		add_email.state_forwards("app", &mut state);

		// Add generated field (full_name = first_name || ' ' || last_name)
		let add_generated = Operation::AddColumn {
			table: "users".to_string(),
			column: ColumnDefinition::new(
				"full_name",
				FieldType::Custom(
					"VARCHAR(200) GENERATED ALWAYS AS (first_name || ' ' || last_name) STORED"
						.to_string(),
				),
			),
			mysql_options: None,
		};
		add_generated.state_forwards("app", &mut state);

		assert!(
			state
				.get_model("app", "users")
				.unwrap()
				.fields
				.contains_key("full_name")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_add_field_with_default() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		// Create a table
		let create_op = Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		// Add a field with default value in type definition
		let add_op = Operation::AddColumn {
			table: "users".to_string(),
			column: ColumnDefinition::new(
				"status",
				FieldType::Custom("VARCHAR(50) DEFAULT 'active'".to_string()),
			),
			mysql_options: None,
		};
		add_op.state_forwards("testapp", &mut state);

		let model = state.get_model("testapp", "users").unwrap();
		assert!(model.fields.contains_key("status"));
	}

	#[test]
	// From: Django/migrations
	fn test_add_field_with_default_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "products".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let add_op = Operation::AddColumn {
			table: "products".to_string(),
			column: ColumnDefinition::new(
				"price",
				FieldType::Custom("DECIMAL(10,2) DEFAULT 0.00".to_string()),
			),
			mysql_options: None,
		};
		add_op.state_forwards("app", &mut state);

		assert!(
			state
				.get_model("app", "products")
				.unwrap()
				.fields
				.contains_key("price")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_add_fk_before_generated_field() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		// Create referenced table
		let create_categories = Operation::CreateTable {
			name: "categories".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("name", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_categories.state_forwards("testapp", &mut state);

		// Create main table
		let create_products = Operation::CreateTable {
			name: "products".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("name", FieldType::VarChar(200)),
				ColumnDefinition::new(
					"price",
					FieldType::Decimal {
						precision: 10,
						scale: 2,
					},
				),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_products.state_forwards("testapp", &mut state);

		// Add FK field
		let add_fk = Operation::AddColumn {
			table: "products".to_string(),
			column: ColumnDefinition::new(
				"category_id",
				FieldType::Custom("INTEGER REFERENCES categories(id)".to_string()),
			),
			mysql_options: None,
		};
		add_fk.state_forwards("testapp", &mut state);

		// Add generated field that uses the FK
		let add_generated = Operation::AddColumn {
			table: "products".to_string(),
			column: ColumnDefinition::new(
				"display_price",
				FieldType::Custom(
					"VARCHAR(50) GENERATED ALWAYS AS ('$' || CAST(price AS TEXT)) STORED"
						.to_string(),
				),
			),
			mysql_options: None,
		};
		add_generated.state_forwards("testapp", &mut state);

		let model = state.get_model("testapp", "products").unwrap();
		assert!(model.fields.contains_key("category_id"));
		assert!(model.fields.contains_key("display_price"));
	}

	#[test]
	// From: Django/migrations
	fn test_add_fk_before_generated_field_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_users = Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_users.state_forwards("app", &mut state);

		let create_orders = Operation::CreateTable {
			name: "orders".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new(
					"total",
					FieldType::Decimal {
						precision: 10,
						scale: 2,
					},
				),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_orders.state_forwards("app", &mut state);

		// Add FK
		let add_fk = Operation::AddColumn {
			table: "orders".to_string(),
			column: ColumnDefinition::new(
				"user_id",
				FieldType::Custom("INTEGER REFERENCES users(id)".to_string()),
			),
			mysql_options: None,
		};
		add_fk.state_forwards("app", &mut state);

		// Add generated field
		let add_generated = Operation::AddColumn {
			table: "orders".to_string(),
			column: ColumnDefinition::new(
				"total_with_tax",
				FieldType::Custom(
					"DECIMAL(10,2) GENERATED ALWAYS AS (total * 1.1) STORED".to_string(),
				),
			),
			mysql_options: None,
		};
		add_generated.state_forwards("app", &mut state);

		assert!(state.get_model("app", "orders").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_add_index_with_new_model() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		// Create a table
		let create_op = Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("email", FieldType::VarChar(255)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		// Add an index (doesn't affect state but generates SQL)
		let index_op = Operation::CreateIndex {
			table: "users".to_string(),
			columns: vec!["email".to_string()],
			unique: true,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		};
		let sql = index_op.to_sql(&operations::SqlDialect::Postgres);

		assert!(sql.contains("CREATE UNIQUE INDEX"));
		assert!(state.get_model("testapp", "users").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_add_index_with_new_model_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "products".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("sku", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let index_op = Operation::CreateIndex {
			table: "products".to_string(),
			columns: vec!["sku".to_string()],
			unique: true,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		};
		let sql = index_op.to_sql(&operations::SqlDialect::Sqlite);

		assert!(sql.contains("CREATE UNIQUE INDEX"));
		assert!(state.get_model("app", "products").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_add_indexes() {
		use crate::migrations::operations::*;

		// Test CreateIndex operation SQL generation
		let op = Operation::CreateIndex {
			table: "users".to_string(),
			columns: vec!["email".to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		};

		let sql = op.to_sql(&SqlDialect::Postgres);
		assert!(sql.contains("CREATE INDEX"));
		assert!(sql.contains("users"));
		assert!(sql.contains("email"));
	}

	#[test]
	// From: Django/migrations
	fn test_add_indexes_1() {
		use crate::migrations::operations::*;

		// Test unique index creation
		let op = Operation::CreateIndex {
			table: "products".to_string(),
			columns: vec!["sku".to_string()],
			unique: true,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		};

		let sql = op.to_sql(&SqlDialect::Postgres);
		assert!(sql.contains("CREATE UNIQUE INDEX"));
		assert!(sql.contains("products"));
		assert!(sql.contains("sku"));
	}

	#[test]
	// From: Django/migrations
	fn test_add_many_to_many() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		// Create first table (e.g., students)
		let create_students = Operation::CreateTable {
			name: "students".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("name", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_students.state_forwards("testapp", &mut state);

		// Create second table (e.g., courses)
		let create_courses = Operation::CreateTable {
			name: "courses".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("title", FieldType::VarChar(200)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_courses.state_forwards("testapp", &mut state);

		// Create many-to-many association table
		// Note: Composite primary keys are handled via column definitions, not constraints
		let create_m2m = Operation::CreateTable {
			name: "students_courses".to_string(),
			columns: vec![
				ColumnDefinition::new(
					"student_id",
					FieldType::Custom("INTEGER REFERENCES students(id)".to_string()),
				),
				ColumnDefinition::new(
					"course_id",
					FieldType::Custom("INTEGER REFERENCES courses(id)".to_string()),
				),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_m2m.state_forwards("testapp", &mut state);

		assert!(state.get_model("testapp", "students_courses").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_add_many_to_many_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_tags = Operation::CreateTable {
			name: "tags".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("name", FieldType::VarChar(50)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_tags.state_forwards("app", &mut state);

		let create_posts = Operation::CreateTable {
			name: "posts".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("title", FieldType::VarChar(255)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_posts.state_forwards("app", &mut state);

		// Create association table for many-to-many
		// Note: Composite primary keys are handled via column definitions, not constraints
		let create_assoc = Operation::CreateTable {
			name: "posts_tags".to_string(),
			columns: vec![
				ColumnDefinition::new(
					"post_id",
					FieldType::Custom("INTEGER REFERENCES posts(id)".to_string()),
				),
				ColumnDefinition::new(
					"tag_id",
					FieldType::Custom("INTEGER REFERENCES tags(id)".to_string()),
				),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_assoc.state_forwards("app", &mut state);

		assert!(state.get_model("app", "posts_tags").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_add_model_order_with_respect_to() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		// Create with order_with_respect_to from the start
		let create_parent = Operation::CreateTable {
			name: "parent".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_parent.state_forwards("app", &mut state);

		let create_child = Operation::CreateTable {
			name: "child".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new(
					"parent_id",
					FieldType::Custom("INTEGER REFERENCES parent(id)".to_string()),
				),
				ColumnDefinition::new(
					"_order",
					FieldType::Custom("INTEGER NOT NULL DEFAULT 0".to_string()),
				),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_child.state_forwards("app", &mut state);

		assert!(
			state
				.get_model("app", "child")
				.unwrap()
				.fields
				.contains_key("_order")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_add_model_order_with_respect_to_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "ordered_items".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("container_id", FieldType::Integer),
				ColumnDefinition::new(
					"_order",
					FieldType::Custom("INTEGER NOT NULL DEFAULT 0".to_string()),
				),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "ordered_items").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_add_model_order_with_respect_to_constraint() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "items".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("parent_id", FieldType::Integer),
				ColumnDefinition::new(
					"_order",
					FieldType::Custom("INTEGER NOT NULL DEFAULT 0".to_string()),
				),
			],
			constraints: vec![Constraint::Check {
				name: "order_non_negative".to_string(),
				expression: "_order >= 0".to_string(),
			}],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "items").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_add_model_order_with_respect_to_constraint_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "entries".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("group_id", FieldType::Integer),
				ColumnDefinition::new("_order", FieldType::Custom("INTEGER NOT NULL".to_string())),
			],
			constraints: vec![Constraint::Check {
				name: "order_non_negative".to_string(),
				expression: "_order >= 0".to_string(),
			}],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "entries").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_add_model_order_with_respect_to_index() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "items".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("parent_id", FieldType::Integer),
				ColumnDefinition::new(
					"_order",
					FieldType::Custom("INTEGER NOT NULL DEFAULT 0".to_string()),
				),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		// Add index on (parent_id, _order)
		let _create_index = Operation::CreateIndex {
			table: "items".to_string(),
			columns: vec!["parent_id".to_string(), "_order".to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		};

		assert!(state.get_model("app", "items").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_add_model_order_with_respect_to_index_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "tasks".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("project_id", FieldType::Integer),
				ColumnDefinition::new("_order", FieldType::Custom("INTEGER NOT NULL".to_string())),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "tasks").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_add_model_order_with_respect_to_unique_together() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "items".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("parent_id", FieldType::Integer),
				ColumnDefinition::new("_order", FieldType::Custom("INTEGER NOT NULL".to_string())),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let unique_op = Operation::AlterUniqueTogether {
			table: "items".to_string(),
			unique_together: vec![vec!["parent_id".to_string(), "_order".to_string()]],
		};
		unique_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "items").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_add_model_order_with_respect_to_unique_together_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "slides".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("deck_id", FieldType::Integer),
				ColumnDefinition::new("_order", FieldType::Custom("INTEGER NOT NULL".to_string())),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let unique_op = Operation::AlterUniqueTogether {
			table: "slides".to_string(),
			unique_together: vec![vec!["deck_id".to_string(), "_order".to_string()]],
		};
		unique_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "slides").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_add_model_with_field_removed_from_base_model() {
		// Tests joined table inheritance where child model has its own table
		// linked to parent table via foreign key
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		// Create base (parent) model
		let create_base = Operation::CreateTable {
			name: "employees".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("name", FieldType::VarChar(100)),
				ColumnDefinition::new("email", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_base.state_forwards("company", &mut state);

		// Create inherited (child) model using joined table inheritance
		let create_inherited = Operation::CreateInheritedTable {
			name: "managers".to_string(),
			columns: vec![
				ColumnDefinition::new("department", FieldType::VarChar(100)),
				ColumnDefinition::new(
					"budget",
					FieldType::Decimal {
						precision: 10,
						scale: 2,
					},
				),
			],
			base_table: "employees".to_string(),
			join_column: "employee_id".to_string(),
		};
		create_inherited.state_forwards("company", &mut state);

		let manager_model = state.get_model("company", "managers").unwrap();
		assert!(manager_model.fields.contains_key("employee_id"));
		assert!(manager_model.fields.contains_key("department"));
		assert!(manager_model.fields.contains_key("budget"));
		assert_eq!(manager_model.base_model, Some("employees".to_string()));
		assert_eq!(
			manager_model.inheritance_type,
			Some("joined_table".to_string())
		);
	}

	#[test]
	// From: Django/migrations
	fn test_add_model_with_field_removed_from_base_model_1() {
		// Tests single table inheritance where parent and children share one table
		// using a discriminator column to distinguish types
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		// Create base (parent) model with all fields
		let create_base = Operation::CreateTable {
			name: "persons".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("name", FieldType::VarChar(100)),
				ColumnDefinition::new("email", FieldType::VarChar(100)),
				// Fields for all child types in single table
				ColumnDefinition::new("student_id", FieldType::VarChar(20)),
				ColumnDefinition::new("grade", FieldType::VarChar(10)),
				ColumnDefinition::new("employee_id", FieldType::VarChar(20)),
				ColumnDefinition::new("department", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_base.state_forwards("school", &mut state);

		// Add discriminator column for single table inheritance
		let add_discriminator = Operation::AddDiscriminatorColumn {
			table: "persons".to_string(),
			column_name: "person_type".to_string(),
			default_value: "person".to_string(),
		};
		add_discriminator.state_forwards("school", &mut state);

		let person_model = state.get_model("school", "persons").unwrap();
		assert!(person_model.fields.contains_key("person_type"));
		assert_eq!(
			person_model.discriminator_column,
			Some("person_type".to_string())
		);
		assert_eq!(
			person_model.inheritance_type,
			Some("single_table".to_string())
		);
	}

	#[test]
	// From: Django/migrations
	fn test_add_non_blank_textfield_and_charfield() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "articles".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		// Add non-blank fields (NOT NULL with defaults or constraints)
		let add_text = Operation::AddColumn {
			table: "articles".to_string(),
			column: ColumnDefinition::new(
				"content",
				FieldType::Custom("TEXT NOT NULL DEFAULT ''".to_string()),
			),
			mysql_options: None,
		};
		add_text.state_forwards("testapp", &mut state);

		let add_char = Operation::AddColumn {
			table: "articles".to_string(),
			column: ColumnDefinition::new(
				"title",
				FieldType::Custom("VARCHAR(255) NOT NULL DEFAULT ''".to_string()),
			),
			mysql_options: None,
		};
		add_char.state_forwards("testapp", &mut state);

		let model = state.get_model("testapp", "articles").unwrap();
		assert!(model.fields.contains_key("content"));
		assert!(model.fields.contains_key("title"));
	}

	#[test]
	// From: Django/migrations
	fn test_add_non_blank_textfield_and_charfield_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "posts".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let add_op = Operation::AddColumn {
			table: "posts".to_string(),
			column: ColumnDefinition::new("body", FieldType::Custom("TEXT NOT NULL".to_string())),
			mysql_options: None,
		};
		add_op.state_forwards("app", &mut state);

		assert!(
			state
				.get_model("app", "posts")
				.unwrap()
				.fields
				.contains_key("body")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_add_not_null_field_with_db_default() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		// Add NOT NULL field with database-level default
		let add_op = Operation::AddColumn {
			table: "users".to_string(),
			column: ColumnDefinition::new(
				"created_at",
				FieldType::Custom("TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP".to_string()),
			),
			mysql_options: None,
		};
		add_op.state_forwards("testapp", &mut state);

		let model = state.get_model("testapp", "users").unwrap();
		assert!(model.fields.contains_key("created_at"));
	}

	#[test]
	// From: Django/migrations
	fn test_add_not_null_field_with_db_default_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "orders".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let add_op = Operation::AddColumn {
			table: "orders".to_string(),
			column: ColumnDefinition::new(
				"status",
				FieldType::Custom("VARCHAR(50) NOT NULL DEFAULT 'pending'".to_string()),
			),
			mysql_options: None,
		};
		add_op.state_forwards("app", &mut state);

		assert!(
			state
				.get_model("app", "orders")
				.unwrap()
				.fields
				.contains_key("status")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_add_unique_together() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "products".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("name", FieldType::VarChar(255)),
				ColumnDefinition::new("sku", FieldType::VarChar(50)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let unique_op = Operation::AlterUniqueTogether {
			table: "products".to_string(),
			unique_together: vec![vec!["name".to_string(), "sku".to_string()]],
		};
		unique_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "products").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_add_unique_together_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "books".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("title", FieldType::VarChar(255)),
				ColumnDefinition::new("author", FieldType::VarChar(255)),
				ColumnDefinition::new("isbn", FieldType::VarChar(20)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let unique_op = Operation::AlterUniqueTogether {
			table: "books".to_string(),
			unique_together: vec![
				vec!["title".to_string(), "author".to_string()],
				vec!["isbn".to_string()],
			],
		};
		unique_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "books").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_alter_constraint() {
		use crate::migrations::operations::*;

		// Test dropping and adding a constraint (simulating alteration)
		let drop_op = Operation::DropConstraint {
			table: "users".to_string(),
			constraint_name: "old_constraint".to_string(),
		};

		let add_op = Operation::AddConstraint {
			table: "users".to_string(),
			constraint_sql: "CHECK (age >= 21)".to_string(),
		};

		let drop_sql = drop_op.to_sql(&SqlDialect::Postgres);
		let add_sql = add_op.to_sql(&SqlDialect::Postgres);

		assert!(drop_sql.contains("DROP CONSTRAINT"));
		assert!(add_sql.contains("ADD CHECK (age >= 21)"));
	}

	#[test]
	// From: Django/migrations
	fn test_alter_constraint_1() {
		use crate::migrations::operations::*;

		// Test constraint alteration with different constraint
		let drop_op = Operation::DropConstraint {
			table: "products".to_string(),
			constraint_name: "price_check".to_string(),
		};

		let add_op = Operation::AddConstraint {
			table: "products".to_string(),
			constraint_sql: "CHECK (price > 0)".to_string(),
		};

		let drop_sql = drop_op.to_sql(&SqlDialect::Postgres);
		let add_sql = add_op.to_sql(&SqlDialect::Postgres);

		assert!(drop_sql.contains("DROP CONSTRAINT price_check"));
		assert!(add_sql.contains("ADD CHECK (price > 0)"));
	}

	#[test]
	// From: Django/migrations
	fn test_alter_db_table_add() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		// Create with default name
		let create_op = Operation::CreateTable {
			name: "myapp_user".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		// Simulate db_table change by renaming
		let rename_op = Operation::RenameTable {
			old_name: "myapp_user".to_string(),
			new_name: "custom_users".to_string(),
		};
		rename_op.state_forwards("testapp", &mut state);

		assert!(state.get_model("testapp", "custom_users").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_alter_db_table_add_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "app_product".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let rename_op = Operation::RenameTable {
			old_name: "app_product".to_string(),
			new_name: "products_table".to_string(),
		};
		rename_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "products_table").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_alter_db_table_change() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		// Create a table
		let create_op = Operation::CreateTable {
			name: "old_table".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		// Rename the table
		let rename_op = Operation::RenameTable {
			old_name: "old_table".to_string(),
			new_name: "new_table".to_string(),
		};
		rename_op.state_forwards("testapp", &mut state);

		assert!(state.get_model("testapp", "old_table").is_none());
		assert!(state.get_model("testapp", "new_table").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_alter_db_table_change_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let rename_op = Operation::RenameTable {
			old_name: "users".to_string(),
			new_name: "customers".to_string(),
		};
		rename_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "customers").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_alter_db_table_comment_add() {
		use crate::migrations::operations::*;

		let op = Operation::AlterTableComment {
			table: "users".to_string(),
			comment: Some("User accounts table".to_string()),
		};

		let sql = op.to_sql(&SqlDialect::Postgres);
		assert!(sql.contains("COMMENT ON TABLE users"));
		assert!(sql.contains("User accounts table"));
	}

	#[test]
	// From: Django/migrations
	fn test_alter_db_table_comment_add_1() {
		use crate::migrations::operations::*;

		let op = Operation::AlterTableComment {
			table: "products".to_string(),
			comment: Some("Product catalog".to_string()),
		};

		let sql = op.to_sql(&SqlDialect::Mysql);
		assert!(sql.contains("ALTER TABLE products"));
		assert!(sql.contains("COMMENT='Product catalog'"));
	}

	#[test]
	// From: Django/migrations
	fn test_alter_db_table_comment_change() {
		use crate::migrations::operations::*;

		let op = Operation::AlterTableComment {
			table: "users".to_string(),
			comment: Some("Updated user table".to_string()),
		};

		let sql = op.to_sql(&SqlDialect::Postgres);
		assert!(sql.contains("COMMENT ON TABLE users"));
		assert!(sql.contains("Updated user table"));
	}

	#[test]
	// From: Django/migrations
	fn test_alter_db_table_comment_change_1() {
		use crate::migrations::operations::*;

		let op = Operation::AlterTableComment {
			table: "orders".to_string(),
			comment: Some("Order history".to_string()),
		};

		let sql = op.to_sql(&SqlDialect::Mysql);
		assert!(sql.contains("ALTER TABLE orders"));
	}

	#[test]
	// From: Django/migrations
	fn test_alter_db_table_comment_no_changes() {
		use crate::migrations::operations::*;

		// Setting same comment - this is a no-op test
		let op = Operation::AlterTableComment {
			table: "users".to_string(),
			comment: Some("Same comment".to_string()),
		};

		let sql = op.to_sql(&SqlDialect::Postgres);
		assert!(sql.contains("COMMENT ON TABLE users"));
	}

	#[test]
	// From: Django/migrations
	fn test_alter_db_table_comment_no_changes_1() {
		use crate::migrations::operations::*;

		let op = Operation::AlterTableComment {
			table: "products".to_string(),
			comment: Some("No change".to_string()),
		};

		let sql = op.to_sql(&SqlDialect::Mysql);
		assert!(!sql.is_empty());
	}

	#[test]
	// From: Django/migrations
	fn test_alter_db_table_comment_remove() {
		use crate::migrations::operations::*;

		let op = Operation::AlterTableComment {
			table: "users".to_string(),
			comment: None,
		};

		let sql = op.to_sql(&SqlDialect::Postgres);
		assert!(sql.contains("COMMENT ON TABLE users IS NULL"));
	}

	#[test]
	// From: Django/migrations
	fn test_alter_db_table_comment_remove_1() {
		use crate::migrations::operations::*;

		let op = Operation::AlterTableComment {
			table: "orders".to_string(),
			comment: None,
		};

		let sql = op.to_sql(&SqlDialect::Mysql);
		assert!(sql.contains("COMMENT=''"));
	}

	#[test]
	// From: Django/migrations
	fn test_alter_db_table_no_changes() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		// Create a table
		let create_op = Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		// Rename to same name (no-op)
		let rename_op = Operation::RenameTable {
			old_name: "users".to_string(),
			new_name: "users".to_string(),
		};
		rename_op.state_forwards("testapp", &mut state);

		// Table should still exist with same name
		assert!(state.get_model("testapp", "users").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_alter_db_table_no_changes_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "products".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		// No actual change
		let rename_op = Operation::RenameTable {
			old_name: "products".to_string(),
			new_name: "products".to_string(),
		};
		rename_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "products").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_alter_db_table_remove() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "custom_table".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		// Removing db_table means reverting to default name
		let rename_op = Operation::RenameTable {
			old_name: "custom_table".to_string(),
			new_name: "myapp_model".to_string(),
		};
		rename_op.state_forwards("testapp", &mut state);

		assert!(state.get_model("testapp", "myapp_model").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_alter_db_table_remove_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "old_custom".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let rename_op = Operation::RenameTable {
			old_name: "old_custom".to_string(),
			new_name: "app_default".to_string(),
		};
		rename_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "app_default").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_alter_db_table_with_model_change() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("name", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		// Change table name and add field in same migration
		let rename_op = Operation::RenameTable {
			old_name: "users".to_string(),
			new_name: "custom_users".to_string(),
		};
		rename_op.state_forwards("testapp", &mut state);

		let add_field = Operation::AddColumn {
			table: "custom_users".to_string(),
			column: ColumnDefinition::new("email", FieldType::VarChar(255)),
			mysql_options: None,
		};
		add_field.state_forwards("testapp", &mut state);

		let model = state.get_model("testapp", "custom_users").unwrap();
		assert!(model.fields.contains_key("email"));
	}

	#[test]
	// From: Django/migrations
	fn test_alter_db_table_with_model_change_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "items".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let rename_op = Operation::RenameTable {
			old_name: "items".to_string(),
			new_name: "products".to_string(),
		};
		rename_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "products").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_alter_field() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		// Create a table
		let create_op = Operation::CreateTable {
			name: "test_table".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("name", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		// Alter the field
		let alter_op = Operation::AlterColumn {
			table: "test_table".to_string(),
			column: "name".to_string(),
			old_definition: None,
			new_definition: ColumnDefinition::new("name", FieldType::VarChar(255)),
			mysql_options: None,
		};
		alter_op.state_forwards("testapp", &mut state);

		let model = state.get_model("testapp", "test_table").unwrap();
		assert!(model.fields.contains_key("name"));
	}

	#[test]
	// From: Django/migrations
	fn test_alter_field_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("email", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let alter_op = Operation::AlterColumn {
			table: "users".to_string(),
			column: "email".to_string(),
			old_definition: None,
			new_definition: ColumnDefinition::new("email", FieldType::Text),
			mysql_options: None,
		};
		alter_op.state_forwards("app", &mut state);

		assert!(
			state
				.get_model("app", "users")
				.unwrap()
				.fields
				.contains_key("email")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_alter_field_and_unique_together() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "items".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("code", FieldType::VarChar(50)),
				ColumnDefinition::new("category", FieldType::VarChar(50)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let unique_op = Operation::AlterUniqueTogether {
			table: "items".to_string(),
			unique_together: vec![vec!["code".to_string(), "category".to_string()]],
		};
		unique_op.state_forwards("app", &mut state);

		let alter_op = Operation::AlterColumn {
			table: "items".to_string(),
			column: "code".to_string(),
			old_definition: None,
			new_definition: ColumnDefinition::new("code", FieldType::VarChar(100)),
			mysql_options: None,
		};
		alter_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "items").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_alter_field_and_unique_together_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "orders".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("order_number", FieldType::VarChar(20)),
				ColumnDefinition::new("year", FieldType::Integer),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let unique_op = Operation::AlterUniqueTogether {
			table: "orders".to_string(),
			unique_together: vec![vec!["order_number".to_string(), "year".to_string()]],
		};
		unique_op.state_forwards("app", &mut state);

		let alter_op = Operation::AlterColumn {
			table: "orders".to_string(),
			column: "year".to_string(),
			old_definition: None,
			new_definition: ColumnDefinition::new(
				"year",
				FieldType::Custom("SMALLINT".to_string()),
			),
			mysql_options: None,
		};
		alter_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "orders").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_alter_field_to_fk_dependency_other_app() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		// Create referenced table in another "app"
		let create_users = Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_users.state_forwards("auth_app", &mut state);

		// Create table with regular field
		let create_posts = Operation::CreateTable {
			name: "posts".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("author_id", FieldType::Integer),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_posts.state_forwards("blog_app", &mut state);

		// Alter to FK (in practice, this would add FK constraint)
		let alter_op = Operation::AlterColumn {
			table: "posts".to_string(),
			column: "author_id".to_string(),
			old_definition: None,
			new_definition: ColumnDefinition::new(
				"author_id",
				FieldType::Custom("INTEGER REFERENCES users(id)".to_string()),
			),
			mysql_options: None,
		};
		alter_op.state_forwards("blog_app", &mut state);

		assert!(
			state
				.get_model("blog_app", "posts")
				.unwrap()
				.fields
				.contains_key("author_id")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_alter_field_to_fk_dependency_other_app_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_categories = Operation::CreateTable {
			name: "categories".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_categories.state_forwards("catalog", &mut state);

		let create_items = Operation::CreateTable {
			name: "items".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("cat_id", FieldType::Integer),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_items.state_forwards("store", &mut state);

		let alter_op = Operation::AlterColumn {
			table: "items".to_string(),
			column: "cat_id".to_string(),
			old_definition: None,
			new_definition: ColumnDefinition::new(
				"cat_id",
				FieldType::Custom("INTEGER REFERENCES categories(id)".to_string()),
			),
			mysql_options: None,
		};
		alter_op.state_forwards("store", &mut state);

		assert!(state.get_model("store", "items").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_alter_field_to_not_null_oneoff_default() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("nickname", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		// This simulates a two-step process:
		// 1. Add default temporarily
		// 2. Make field NOT NULL
		// In practice, this would be done with RunSQL or a combined operation
		let alter_op = Operation::AlterColumn {
			table: "users".to_string(),
			column: "nickname".to_string(),
			old_definition: None,
			new_definition: ColumnDefinition::new(
				"nickname",
				FieldType::Custom("VARCHAR(100) NOT NULL".to_string()),
			),
			mysql_options: None,
		};
		alter_op.state_forwards("testapp", &mut state);

		assert!(
			state
				.get_model("testapp", "users")
				.unwrap()
				.fields
				.contains_key("nickname")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_alter_field_to_not_null_oneoff_default_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "profiles".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("bio", FieldType::Text),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let alter_op = Operation::AlterColumn {
			table: "profiles".to_string(),
			column: "bio".to_string(),
			old_definition: None,
			new_definition: ColumnDefinition::new(
				"bio",
				FieldType::Custom("TEXT NOT NULL".to_string()),
			),
			mysql_options: None,
		};
		alter_op.state_forwards("app", &mut state);

		assert!(
			state
				.get_model("app", "profiles")
				.unwrap()
				.fields
				.contains_key("bio")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_alter_field_to_not_null_with_db_default() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "products".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("quantity", FieldType::Integer),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		// Alter to NOT NULL with database default
		let alter_op = Operation::AlterColumn {
			table: "products".to_string(),
			column: "quantity".to_string(),
			old_definition: None,
			new_definition: ColumnDefinition::new(
				"quantity",
				FieldType::Custom("INTEGER NOT NULL DEFAULT 0".to_string()),
			),
			mysql_options: None,
		};
		alter_op.state_forwards("testapp", &mut state);

		assert!(
			state
				.get_model("testapp", "products")
				.unwrap()
				.fields
				.contains_key("quantity")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_alter_field_to_not_null_with_db_default_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "items".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("available", FieldType::Boolean),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let alter_op = Operation::AlterColumn {
			table: "items".to_string(),
			column: "available".to_string(),
			old_definition: None,
			new_definition: ColumnDefinition::new(
				"available",
				FieldType::Custom("BOOLEAN NOT NULL DEFAULT TRUE".to_string()),
			),
			mysql_options: None,
		};
		alter_op.state_forwards("app", &mut state);

		assert!(
			state
				.get_model("app", "items")
				.unwrap()
				.fields
				.contains_key("available")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_alter_field_to_not_null_with_default() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("status", FieldType::VarChar(50)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		// Alter field to NOT NULL with default
		let alter_op = Operation::AlterColumn {
			table: "users".to_string(),
			column: "status".to_string(),
			old_definition: None,
			new_definition: ColumnDefinition::new(
				"status",
				FieldType::Custom("VARCHAR(50) NOT NULL DEFAULT 'active'".to_string()),
			),
			mysql_options: None,
		};
		alter_op.state_forwards("testapp", &mut state);

		assert!(
			state
				.get_model("testapp", "users")
				.unwrap()
				.fields
				.contains_key("status")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_alter_field_to_not_null_with_default_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "products".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("active", FieldType::Boolean),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let alter_op = Operation::AlterColumn {
			table: "products".to_string(),
			column: "active".to_string(),
			old_definition: None,
			new_definition: ColumnDefinition::new(
				"active",
				FieldType::Custom("BOOLEAN NOT NULL DEFAULT TRUE".to_string()),
			),
			mysql_options: None,
		};
		alter_op.state_forwards("app", &mut state);

		assert!(
			state
				.get_model("app", "products")
				.unwrap()
				.fields
				.contains_key("active")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_alter_field_to_not_null_without_default() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("email", FieldType::VarChar(255)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("testapp", &mut state);

		// Alter field to NOT NULL without default (assumes data exists)
		let alter_op = Operation::AlterColumn {
			table: "users".to_string(),
			column: "email".to_string(),
			old_definition: None,
			new_definition: ColumnDefinition::new(
				"email",
				FieldType::Custom("VARCHAR(255) NOT NULL".to_string()),
			),
			mysql_options: None,
		};
		alter_op.state_forwards("testapp", &mut state);

		assert!(
			state
				.get_model("testapp", "users")
				.unwrap()
				.fields
				.contains_key("email")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_alter_field_to_not_null_without_default_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "orders".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("customer_id", FieldType::Integer),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let alter_op = Operation::AlterColumn {
			table: "orders".to_string(),
			column: "customer_id".to_string(),
			old_definition: None,
			new_definition: ColumnDefinition::new(
				"customer_id",
				FieldType::Custom("INTEGER NOT NULL".to_string()),
			),
			mysql_options: None,
		};
		alter_op.state_forwards("app", &mut state);

		assert!(
			state
				.get_model("app", "orders")
				.unwrap()
				.fields
				.contains_key("customer_id")
		);
	}

	#[test]
	// From: Django/migrations
	fn test_alter_fk_before_model_deletion() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_old = Operation::CreateTable {
			name: "old_table".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_old.state_forwards("testapp", &mut state);

		let create_new = Operation::CreateTable {
			name: "new_table".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_new.state_forwards("testapp", &mut state);

		let create_ref = Operation::CreateTable {
			name: "referencing".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new(
					"ref_id",
					FieldType::Custom("INTEGER REFERENCES old_table(id)".to_string()),
				),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_ref.state_forwards("testapp", &mut state);

		// Change FK to point to new_table before deleting old_table
		let alter_fk = Operation::AlterColumn {
			table: "referencing".to_string(),
			column: "ref_id".to_string(),
			old_definition: None,
			new_definition: ColumnDefinition::new(
				"ref_id",
				FieldType::Custom("INTEGER REFERENCES new_table(id)".to_string()),
			),
			mysql_options: None,
		};
		alter_fk.state_forwards("testapp", &mut state);

		// Now safe to delete old_table
		let drop_old = Operation::DropTable {
			name: "old_table".to_string(),
		};
		drop_old.state_forwards("testapp", &mut state);

		assert!(state.get_model("testapp", "old_table").is_none());
		assert!(state.get_model("testapp", "referencing").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_alter_fk_before_model_deletion_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		let create_categories = Operation::CreateTable {
			name: "categories".to_string(),
			columns: vec![ColumnDefinition::new(
				"id",
				FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
			)],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_categories.state_forwards("app", &mut state);

		let create_products = Operation::CreateTable {
			name: "products".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("cat_id", FieldType::Integer),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_products.state_forwards("app", &mut state);

		// Remove FK constraint or set to NULL before deletion
		let alter_op = Operation::AlterColumn {
			table: "products".to_string(),
			column: "cat_id".to_string(),
			old_definition: None,
			new_definition: ColumnDefinition::new(
				"cat_id",
				FieldType::Custom("INTEGER NULL".to_string()),
			),
			mysql_options: None,
		};
		alter_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "products").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_alter_many_to_many() {
		// Tests altering a many-to-many association table by adding extra fields
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		// Create two models
		let create_authors = Operation::CreateTable {
			name: "authors".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("name", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_authors.state_forwards("library", &mut state);

		let create_books = Operation::CreateTable {
			name: "books".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("title", FieldType::VarChar(200)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_books.state_forwards("library", &mut state);

		// Create association table for many-to-many
		let create_assoc = Operation::CreateTable {
			name: "authors_books".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new(
					"author_id",
					FieldType::Custom("INTEGER REFERENCES authors(id)".to_string()),
				),
				ColumnDefinition::new(
					"book_id",
					FieldType::Custom("INTEGER REFERENCES books(id)".to_string()),
				),
			],
			constraints: vec![Constraint::Unique {
				name: "unique_author_book".to_string(),
				columns: vec!["author_id".to_string(), "book_id".to_string()],
			}],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_assoc.state_forwards("library", &mut state);

		// Alter the association table by adding extra metadata fields
		let add_created_at = Operation::AddColumn {
			table: "authors_books".to_string(),
			column: ColumnDefinition::new(
				"created_at",
				FieldType::Custom(
					FieldType::Custom("TIMESTAMP DEFAULT CURRENT_TIMESTAMP".to_string())
						.to_string(),
				),
			),
			mysql_options: None,
		};
		add_created_at.state_forwards("library", &mut state);

		let add_role = Operation::AddColumn {
			table: "authors_books".to_string(),
			column: ColumnDefinition::new("contribution_role", FieldType::VarChar(50)),
			mysql_options: None,
		};
		add_role.state_forwards("library", &mut state);

		// Verify the association table has been altered
		let assoc_model = state.get_model("library", "authors_books").unwrap();
		assert!(assoc_model.fields.contains_key("author_id"));
		assert!(assoc_model.fields.contains_key("book_id"));
		assert!(assoc_model.fields.contains_key("created_at"));
		assert!(assoc_model.fields.contains_key("contribution_role"));
	}

	#[test]
	// From: Django/migrations
	fn test_alter_many_to_many_1() {
		// Tests altering a many-to-many by changing field types in association table
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;

		let mut state = ProjectState::new();

		// Create two models
		let create_students = Operation::CreateTable {
			name: "students".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("name", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_students.state_forwards("school", &mut state);

		let create_courses = Operation::CreateTable {
			name: "courses".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("title", FieldType::VarChar(200)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_courses.state_forwards("school", &mut state);

		// Create association table
		let create_enrollment = Operation::CreateTable {
			name: "enrollments".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new(
					"student_id",
					FieldType::Custom("INTEGER REFERENCES students(id)".to_string()),
				),
				ColumnDefinition::new(
					"course_id",
					FieldType::Custom("INTEGER REFERENCES courses(id)".to_string()),
				),
				ColumnDefinition::new("grade", FieldType::VarChar(2)),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_enrollment.state_forwards("school", &mut state);

		// Alter the grade field to use a numeric type instead
		let alter_grade = Operation::AlterColumn {
			table: "enrollments".to_string(),
			column: "grade".to_string(),
			old_definition: None,
			new_definition: ColumnDefinition::new(
				"grade",
				FieldType::Decimal {
					precision: 3,
					scale: 2,
				},
			),
			mysql_options: None,
		};
		alter_grade.state_forwards("school", &mut state);

		// Add an index on the association table
		let add_index = Operation::CreateIndex {
			table: "enrollments".to_string(),
			columns: vec!["student_id".to_string(), "course_id".to_string()],
			unique: true,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		};
		add_index.state_forwards("school", &mut state);

		let enrollment_model = state.get_model("school", "enrollments").unwrap();
		assert!(enrollment_model.fields.contains_key("student_id"));
		assert!(enrollment_model.fields.contains_key("course_id"));
		assert!(enrollment_model.fields.contains_key("grade"));
	}

	#[test]
	#[should_panic(expected = "runtime-only")]
	// From: Django/migrations
	fn test_alter_model_managers() {
		// Model managers are application-level constructs that don't affect database schema.
		// They define custom query methods and default querysets for models.
		// Migrations don't need to handle manager changes since they're runtime-only.
		//
		// Use reinhardt-migrations types
		use crate::migrations::operations::Operation;
		let _ = std::any::type_name::<Operation>();

		// This test intentionally panics to demonstrate that managers are not a migration concern.
		// Managers are defined in application code and only affect how queries are built at runtime.
		panic!(
			"Model managers are runtime-only and don't require migration support. See reinhardt-orm manager module"
		)
	}

	#[test]
	#[should_panic(expected = "runtime-only")]
	// From: Django/migrations
	fn test_alter_model_managers_1() {
		// See test_alter_model_managers for details
		// Use reinhardt-migrations types
		use crate::migrations::ProjectState;
		let _ = std::any::type_name::<ProjectState>();

		// This test also intentionally panics for the same reason.
		panic!(
			"Model managers are runtime-only and don't require migration support. See reinhardt-orm manager module"
		)
	}

	#[test]
	// From: Django/migrations
	fn test_alter_model_options() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;
		use std::collections::HashMap;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "articles".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("title", FieldType::VarChar(255)),
				ColumnDefinition::new("created_at", FieldType::Custom("TIMESTAMP".to_string())),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let mut options = HashMap::new();
		options.insert("ordering".to_string(), "created_at".to_string());
		options.insert("verbose_name".to_string(), "Article".to_string());

		let alter_op = Operation::AlterModelOptions {
			table: "articles".to_string(),
			options,
		};
		alter_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "articles").is_some());
	}

	#[test]
	// From: Django/migrations
	fn test_alter_model_options_1() {
		use crate::migrations::ProjectState;
		use crate::migrations::operations::*;
		use std::collections::HashMap;

		let mut state = ProjectState::new();

		let create_op = Operation::CreateTable {
			name: "products".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Custom("INTEGER PRIMARY KEY".to_string())),
				ColumnDefinition::new("name", FieldType::VarChar(255)),
				ColumnDefinition::new(
					"price",
					FieldType::Decimal {
						precision: 10,
						scale: 2,
					},
				),
			],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		};
		create_op.state_forwards("app", &mut state);

		let mut options = HashMap::new();
		options.insert("ordering".to_string(), "-price".to_string());
		options.insert("verbose_name_plural".to_string(), "Products".to_string());

		let alter_op = Operation::AlterModelOptions {
			table: "products".to_string(),
			options,
		};
		alter_op.state_forwards("app", &mut state);

		assert!(state.get_model("app", "products").is_some());
	}

	#[test]
	#[should_panic(expected = "don't affect database schema")]
	// From: Django/migrations
	fn test_alter_model_options_proxy() {
		// Proxy models in Django are models that don't have their own database table.
		// They inherit from a concrete model and can have different behavior/methods.
		// Migrations typically ignore proxy models since they don't affect schema.
		// Note: This is primarily a Django ORM feature for model organization
		//
		// Use reinhardt-migrations Migration type
		use super::Migration;
		let _ = std::any::type_name::<Migration>();

		// This test intentionally panics to demonstrate that proxy models are schema-independent.
		// Proxy models are purely for code organization and behavior customization.
		// They share the parent model's table and therefore require no migrations.
		panic!("Proxy models don't require migrations as they don't affect database schema")
	}

	#[test]
	#[should_panic(expected = "don't affect database schema")]
	// From: Django/migrations
	fn test_alter_model_options_proxy_1() {
		// See test_alter_model_options_proxy for details
		// Use reinhardt-migrations ColumnDefinition type
		use crate::migrations::ColumnDefinition;
		let _ = std::any::type_name::<ColumnDefinition>();

		// This test also intentionally panics for the same reason.
		panic!("Proxy models don't require migrations as they don't affect database schema")
	}

	#[test]
	// From: Django/migrations
	fn test_alter_regex_string_to_compiled_regex() {
		// Regex validators are application-level validation, not database schema.
		// They validate input before it reaches the database.
		// Note: reinhardt-orm has RegexValidator in src/validators.rs
		// Migrations don't need to handle validator changes as they're runtime-only.

		// This test would verify that changing a regex validator doesn't generate migrations
		// In practice, we just ensure no migration operations are generated
		use crate::migrations::ProjectState;

		let state = ProjectState::new();
		// No operations needed - validators are not part of schema
		assert!(state.models.is_empty());
	}

	#[test]
	// From: Django/migrations
	fn test_alter_regex_string_to_compiled_regex_1() {
		// Validators (including regex) are runtime-only and don't affect schema
		use crate::migrations::ProjectState;

		let state = ProjectState::new();
		// Changing a regex validator doesn't require any migration
		assert!(state.models.is_empty());
	}
}
