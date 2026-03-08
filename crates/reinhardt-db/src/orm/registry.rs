use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Table metadata information
#[derive(Debug, Clone)]
pub struct TableInfo {
	/// The name.
	pub name: String,
	/// The columns.
	pub columns: Vec<ColumnInfo>,
	/// The primary key.
	pub primary_key: Vec<String>,
}

impl TableInfo {
	/// Creates a new instance.
	pub fn new(name: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			columns: Vec::new(),
			primary_key: Vec::new(),
		}
	}

	/// Performs the schema operation.
	pub fn schema(self, _schema: &str) -> Self {
		// Schema information would be stored here
		self
	}

	/// Adds column.
	pub fn add_column(mut self, column: ColumnInfo) -> Self {
		self.columns.push(column);
		self
	}
}

/// Column metadata information
#[derive(Debug, Clone)]
pub struct ColumnInfo {
	/// The name.
	pub name: String,
	/// The column type.
	pub column_type: String,
	/// The nullable.
	pub nullable: bool,
	/// The default.
	pub default: Option<String>,
}

impl ColumnInfo {
	/// Creates a new instance.
	pub fn new(name: impl Into<String>, column_type: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			column_type: column_type.into(),
			nullable: true,
			default: None,
		}
	}
}

/// Entity mapper (alias for EntityMapper)
pub type Mapper = EntityMapper;

/// Global registry instance
pub fn registry() -> &'static MapperRegistry {
	use once_cell::sync::Lazy;
	static REGISTRY: Lazy<MapperRegistry> = Lazy::new(MapperRegistry::new);
	&REGISTRY
}

/// Mapper registry for managing entity mappings
#[derive(Debug, Clone)]
pub struct MapperRegistry {
	mappers: Arc<RwLock<HashMap<String, EntityMapper>>>,
}

impl MapperRegistry {
	/// Creates a new instance.
	pub fn new() -> Self {
		Self {
			mappers: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Performs the register operation.
	pub fn register(&self, name: impl Into<String>, mapper: EntityMapper) {
		let name = name.into();
		if let Ok(mut mappers) = self.mappers.write() {
			mappers.insert(name, mapper);
		}
	}

	/// Performs the get operation.
	pub fn get(&self, name: &str) -> Option<EntityMapper> {
		if let Ok(mappers) = self.mappers.read() {
			mappers.get(name).cloned()
		} else {
			None
		}
	}

	/// Performs the remove operation.
	pub fn remove(&self, name: &str) -> bool {
		if let Ok(mut mappers) = self.mappers.write() {
			mappers.remove(name).is_some()
		} else {
			false
		}
	}

	/// Performs the clear operation.
	pub fn clear(&self) {
		if let Ok(mut mappers) = self.mappers.write() {
			mappers.clear();
		}
	}

	/// Performs the count operation.
	pub fn count(&self) -> usize {
		if let Ok(mappers) = self.mappers.read() {
			mappers.len()
		} else {
			0
		}
	}
}

impl Default for MapperRegistry {
	fn default() -> Self {
		Self::new()
	}
}

/// Entity mapper
#[derive(Debug, Clone)]
pub struct EntityMapper {
	/// The table name.
	pub table_name: String,
	/// The columns.
	pub columns: Vec<ColumnMapping>,
	/// The primary key.
	pub primary_key: Vec<String>,
}

impl EntityMapper {
	/// Creates a new instance.
	pub fn new(table_name: impl Into<String>) -> Self {
		Self {
			table_name: table_name.into(),
			columns: Vec::new(),
			primary_key: Vec::new(),
		}
	}

	/// Adds column.
	pub fn add_column(&mut self, mapping: ColumnMapping) {
		self.columns.push(mapping);
	}

	/// Sets the primary key.
	pub fn set_primary_key(&mut self, columns: Vec<String>) {
		self.primary_key = columns;
	}
}

/// Column mapping
#[derive(Debug, Clone)]
pub struct ColumnMapping {
	/// The property name.
	pub property_name: String,
	/// The column name.
	pub column_name: String,
	/// The column type.
	pub column_type: String,
	/// The nullable.
	pub nullable: bool,
}

impl ColumnMapping {
	/// Creates a new instance.
	pub fn new(
		property_name: impl Into<String>,
		column_name: impl Into<String>,
		column_type: impl Into<String>,
	) -> Self {
		Self {
			property_name: property_name.into(),
			column_name: column_name.into(),
			column_type: column_type.into(),
			nullable: true,
		}
	}

	/// Performs the not null operation.
	pub fn not_null(mut self) -> Self {
		self.nullable = false;
		self
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_mapper_registry_new() {
		let registry = MapperRegistry::new();
		assert_eq!(registry.count(), 0);
	}

	#[test]
	fn test_mapper_registry_register() {
		let registry = MapperRegistry::new();
		let mapper = EntityMapper::new("users");
		registry.register("User", mapper);
		assert_eq!(registry.count(), 1);
	}

	#[test]
	fn test_mapper_registry_get() {
		let registry = MapperRegistry::new();
		let mapper = EntityMapper::new("users");
		registry.register("User", mapper);

		let retrieved = registry.get("User");
		assert!(retrieved.is_some());
		assert_eq!(retrieved.unwrap().table_name, "users");
	}

	#[test]
	fn test_mapper_registry_remove() {
		let registry = MapperRegistry::new();
		let mapper = EntityMapper::new("users");
		registry.register("User", mapper);

		assert!(registry.remove("User"));
		assert_eq!(registry.count(), 0);
	}

	#[test]
	fn test_mapper_registry_clear() {
		let registry = MapperRegistry::new();
		registry.register("User", EntityMapper::new("users"));
		registry.register("Post", EntityMapper::new("posts"));

		registry.clear();
		assert_eq!(registry.count(), 0);
	}

	#[test]
	fn test_entity_mapper_new() {
		let mapper = EntityMapper::new("users");
		assert_eq!(mapper.table_name, "users");
		assert_eq!(mapper.columns.len(), 0);
		assert_eq!(mapper.primary_key.len(), 0);
	}

	#[test]
	fn test_entity_mapper_add_column() {
		let mut mapper = EntityMapper::new("users");
		let column = ColumnMapping::new("id", "id", "INTEGER");
		mapper.add_column(column);

		assert_eq!(mapper.columns.len(), 1);
		assert_eq!(mapper.columns[0].property_name, "id");
	}

	#[test]
	fn test_entity_mapper_set_primary_key() {
		let mut mapper = EntityMapper::new("users");
		mapper.set_primary_key(vec!["id".to_string()]);

		assert_eq!(mapper.primary_key.len(), 1);
		assert_eq!(mapper.primary_key[0], "id");
	}

	#[test]
	fn test_column_mapping_new() {
		let column = ColumnMapping::new("username", "user_name", "VARCHAR");
		assert_eq!(column.property_name, "username");
		assert_eq!(column.column_name, "user_name");
		assert_eq!(column.column_type, "VARCHAR");
		assert!(column.nullable);
	}

	#[test]
	fn test_column_mapping_not_null() {
		let column = ColumnMapping::new("id", "id", "INTEGER").not_null();
		assert!(!column.nullable);
	}
}

// ============================================================================
// Type-safe entity registry (compile-time checked)
// ============================================================================

/// Trait for entities that can be registered in the ORM
///
/// Implement this trait for each entity/model in your application.
/// The compiler will ensure that only valid entity types can be used.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::orm::registry::EntityType;
///
/// pub struct User {
///     pub id: i64,
///     pub name: String,
/// }
///
/// impl EntityType for User {
///     const NAME: &'static str = "User";
///     const TABLE_NAME: &'static str = "users";
/// }
///
/// // Verify the trait implementation works correctly
/// assert_eq!(User::NAME, "User");
/// assert_eq!(User::TABLE_NAME, "users");
/// ```
pub trait EntityType {
	/// The entity name
	const NAME: &'static str;

	/// The table name
	const TABLE_NAME: &'static str;
}

// ============================================================================
// Global Model Registry for foreign_key resolution
// ============================================================================

/// Model information for foreign key resolution
#[derive(Debug, Clone)]
pub struct ModelInfo {
	/// The app label.
	pub app_label: String,
	/// The model name.
	pub model_name: String,
	/// The type path.
	pub type_path: String,
	/// The table name.
	pub table_name: String,
}

/// Global model registry for resolving foreign_key references
pub struct GlobalModelRegistry {
	models: Arc<RwLock<HashMap<String, ModelInfo>>>,
}

impl GlobalModelRegistry {
	/// Creates a new instance.
	pub fn new() -> Self {
		Self {
			models: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Register a model in the global registry
	pub fn register(&self, model_info: ModelInfo) {
		let key = format!("{}.{}", model_info.app_label, model_info.model_name);
		if let Ok(mut models) = self.models.write() {
			models.insert(key, model_info);
		}
	}

	/// Get model info by app_label.model_name
	pub fn get(&self, app_label: &str, model_name: &str) -> Option<ModelInfo> {
		let key = format!("{}.{}", app_label, model_name);
		if let Ok(models) = self.models.read() {
			models.get(&key).cloned()
		} else {
			None
		}
	}

	/// Find models by type name (may return multiple results)
	pub fn find_by_type_name(&self, type_name: &str) -> Vec<ModelInfo> {
		if let Ok(models) = self.models.read() {
			models
				.values()
				.filter(|m| m.model_name == type_name)
				.cloned()
				.collect()
		} else {
			Vec::new()
		}
	}

	/// Get all registered models
	pub fn all(&self) -> Vec<ModelInfo> {
		if let Ok(models) = self.models.read() {
			models.values().cloned().collect()
		} else {
			Vec::new()
		}
	}

	/// Clear all registered models
	pub fn clear(&self) {
		if let Ok(mut models) = self.models.write() {
			models.clear();
		}
	}

	/// Get the count of registered models
	pub fn count(&self) -> usize {
		if let Ok(models) = self.models.read() {
			models.len()
		} else {
			0
		}
	}
}

impl Default for GlobalModelRegistry {
	fn default() -> Self {
		Self::new()
	}
}

/// Global model registry instance
pub fn global_model_registry() -> &'static GlobalModelRegistry {
	use once_cell::sync::Lazy;
	static REGISTRY: Lazy<GlobalModelRegistry> = Lazy::new(GlobalModelRegistry::new);
	&REGISTRY
}

impl MapperRegistry {
	/// Type-safe get method
	///
	/// Get an entity mapper using compile-time verified entity types.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::registry::{MapperRegistry, EntityType};
	///
	/// pub struct User;
	/// impl EntityType for User {
	///     const NAME: &'static str = "User";
	///     const TABLE_NAME: &'static str = "users";
	/// }
	///
	/// let registry = MapperRegistry::new();
	/// let mapper = registry.get_typed::<User>();
	/// // Verify the get method executes successfully (returns None for unregistered entity)
	/// assert!(mapper.is_none());
	/// ```
	pub fn get_typed<E: EntityType>(&self) -> Option<EntityMapper> {
		self.get(E::NAME)
	}

	/// Type-safe register method
	///
	/// Register an entity mapper using compile-time verified entity types.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::registry::{MapperRegistry, EntityType, EntityMapper};
	///
	/// pub struct Post;
	/// impl EntityType for Post {
	///     const NAME: &'static str = "Post";
	///     const TABLE_NAME: &'static str = "posts";
	/// }
	///
	/// let registry = MapperRegistry::new();
	/// let mapper = EntityMapper::new(Post::TABLE_NAME);
	/// registry.register_typed::<Post>(mapper);
	/// // Verify the register method executed successfully
	/// assert!(registry.get_typed::<Post>().is_some());
	/// ```
	pub fn register_typed<E: EntityType>(&self, mapper: EntityMapper) {
		self.register(E::NAME, mapper);
	}

	/// Type-safe remove method
	///
	/// Remove an entity mapper using compile-time verified entity types.
	pub fn remove_typed<E: EntityType>(&self) -> bool {
		self.remove(E::NAME)
	}
}

#[cfg(test)]
mod typed_tests {
	use super::*;

	// Test entity types
	struct User;
	impl EntityType for User {
		const NAME: &'static str = "User";
		const TABLE_NAME: &'static str = "users";
	}

	struct Post;
	impl EntityType for Post {
		const NAME: &'static str = "Post";
		const TABLE_NAME: &'static str = "posts";
	}

	struct Comment;
	impl EntityType for Comment {
		const NAME: &'static str = "Comment";
		const TABLE_NAME: &'static str = "comments";
	}

	#[test]
	fn test_registry_typed_register() {
		let registry = MapperRegistry::new();
		let mapper = EntityMapper::new(User::TABLE_NAME);
		registry.register_typed::<User>(mapper);

		assert_eq!(registry.count(), 1);
	}

	#[test]
	fn test_registry_typed_get() {
		let registry = MapperRegistry::new();
		let mapper = EntityMapper::new(Post::TABLE_NAME);
		registry.register_typed::<Post>(mapper);

		let retrieved = registry.get_typed::<Post>();
		assert!(retrieved.is_some());
		assert_eq!(retrieved.unwrap().table_name, "posts");
	}

	#[test]
	fn test_registry_typed_get_not_found() {
		let registry = MapperRegistry::new();

		let result = registry.get_typed::<Comment>();
		assert!(result.is_none());
	}

	#[test]
	fn test_typed_remove() {
		let registry = MapperRegistry::new();
		let mapper = EntityMapper::new(User::TABLE_NAME);
		registry.register_typed::<User>(mapper);

		assert_eq!(registry.count(), 1);

		let removed = registry.remove_typed::<User>();
		assert!(removed);
		assert_eq!(registry.count(), 0);
	}

	#[test]
	fn test_registry_typed_and_regular_mixed() {
		let registry = MapperRegistry::new();

		// Register using typed method
		let mapper = EntityMapper::new(User::TABLE_NAME);
		registry.register_typed::<User>(mapper);

		// Can access using both methods
		let typed = registry.get_typed::<User>();
		let regular = registry.get("User");

		assert!(typed.is_some());
		assert!(regular.is_some());
		assert_eq!(typed.unwrap().table_name, regular.unwrap().table_name);
	}
}
