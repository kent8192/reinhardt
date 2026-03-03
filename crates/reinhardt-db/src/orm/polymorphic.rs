//! # Polymorphic Relationships
//!
//! Implements polymorphic associations inspired by Django's Generic Foreign Keys
//! and SQLAlchemy's polymorphic inheritance.
//!
//! A polymorphic relationship allows a model to be associated with multiple
//! different model types through a single relationship. This is achieved using
//! a type discriminator field that identifies which model type is referenced.

use super::{Model, RelationshipType};
use reinhardt_query::prelude::{
	Alias, Condition, Expr, ExprTrait, Query, QueryStatementBuilder, SimpleExpr,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::marker::PhantomData;

/// Inheritance type for polymorphic models
/// Corresponds to SQLAlchemy's polymorphic_on configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InheritanceType {
	/// Single Table Inheritance (all types in one table)
	/// SQLAlchemy: __mapper_args__ = {'polymorphic_on': 'type'}
	SingleTable,

	/// Joined Table Inheritance (each type has its own table)
	/// SQLAlchemy: inherits='base_model'
	JoinedTable,

	/// Concrete Table Inheritance (each type is completely independent)
	ConcreteTable,
}

/// Configuration for polymorphic relationships
/// Defines how types are identified and resolved
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct PolymorphicConfig {
	/// Field name that stores the type identifier
	/// Django: content_type
	/// SQLAlchemy: polymorphic_on
	type_field: String,

	/// Field name that stores the foreign key ID
	/// Django: object_id
	id_field: String,

	/// Inheritance strategy
	inheritance_type: InheritanceType,

	/// Default value for type field
	default_type: Option<String>,
}

impl PolymorphicConfig {
	/// Create a new polymorphic configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::polymorphic::{PolymorphicConfig, InheritanceType};
	///
	/// let config = PolymorphicConfig::new("content_type", "object_id")
	///     .with_inheritance(InheritanceType::SingleTable);
	/// assert_eq!(config.type_field(), "content_type");
	/// assert_eq!(config.id_field(), "object_id");
	/// ```
	pub fn new(type_field: impl Into<String>, id_field: impl Into<String>) -> Self {
		Self {
			type_field: type_field.into(),
			id_field: id_field.into(),
			inheritance_type: InheritanceType::SingleTable,
			default_type: None,
		}
	}

	/// Set inheritance type
	pub fn with_inheritance(mut self, inheritance_type: InheritanceType) -> Self {
		self.inheritance_type = inheritance_type;
		self
	}

	/// Set default type value
	pub fn with_default_type(mut self, default_type: impl Into<String>) -> Self {
		self.default_type = Some(default_type.into());
		self
	}

	/// Get type field name
	pub fn type_field(&self) -> &str {
		&self.type_field
	}

	/// Get ID field name
	pub fn id_field(&self) -> &str {
		&self.id_field
	}

	/// Get inheritance type
	pub fn inheritance_type(&self) -> InheritanceType {
		self.inheritance_type
	}

	/// Get default type value
	pub fn default_type(&self) -> Option<&str> {
		self.default_type.as_deref()
	}
}

/// Identity value for polymorphic types
/// Maps type identifiers to model information
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PolymorphicIdentity {
	/// Type identifier (e.g., "user", "post")
	type_id: String,

	/// Model table name
	table_name: String,

	/// Primary key field name
	pk_field: String,
}

impl PolymorphicIdentity {
	/// Create a new polymorphic identity
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::polymorphic::PolymorphicIdentity;
	///
	/// let identity = PolymorphicIdentity::new("user", "users", "id");
	/// assert_eq!(identity.type_id(), "user");
	/// assert_eq!(identity.table_name(), "users");
	/// assert_eq!(identity.pk_field(), "id");
	/// ```
	pub fn new(
		type_id: impl Into<String>,
		table_name: impl Into<String>,
		pk_field: impl Into<String>,
	) -> Self {
		Self {
			type_id: type_id.into(),
			table_name: table_name.into(),
			pk_field: pk_field.into(),
		}
	}

	/// Get type identifier
	pub fn type_id(&self) -> &str {
		&self.type_id
	}

	/// Get table name
	pub fn table_name(&self) -> &str {
		&self.table_name
	}

	/// Get primary key field
	pub fn pk_field(&self) -> &str {
		&self.pk_field
	}
}

/// Polymorphic relationship definition
/// Allows referencing multiple model types through a single relationship
pub struct PolymorphicRelation<P: Model> {
	/// Relationship name
	name: String,

	/// Polymorphic configuration
	config: PolymorphicConfig,

	/// Registered type identities
	identities: HashMap<String, PolymorphicIdentity>,

	/// Relationship type
	relationship_type: RelationshipType,

	_phantom: PhantomData<P>,
}

impl<P: Model> PolymorphicRelation<P> {
	/// Create a new polymorphic relationship
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::polymorphic::{PolymorphicRelation, PolymorphicConfig};
	/// use reinhardt_db::orm::Model;
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Debug, Clone, Serialize, Deserialize)]
	/// struct Comment { id: Option<i64>, content_type: String, object_id: i64 }
	///
	/// # #[derive(Clone)]
	/// # struct CommentFields;
	/// # impl reinhardt_db::orm::FieldSelector for CommentFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// impl Model for Comment {
	///     type PrimaryKey = i64;
	/// #     type Fields = CommentFields;
	///     fn table_name() -> &'static str { "comments" }
	/// #     fn new_fields() -> Self::Fields { CommentFields }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// }
	///
	/// let config = PolymorphicConfig::new("content_type", "object_id");
	/// let rel = PolymorphicRelation::<Comment>::new("content_object", config);
	/// assert_eq!(rel.name(), "content_object");
	/// ```
	pub fn new(name: impl Into<String>, config: PolymorphicConfig) -> Self {
		Self {
			name: name.into(),
			config,
			identities: HashMap::new(),
			relationship_type: RelationshipType::ManyToOne,
			_phantom: PhantomData,
		}
	}

	/// Register a model type with this polymorphic relationship
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::polymorphic::{PolymorphicRelation, PolymorphicConfig, PolymorphicIdentity};
	/// use reinhardt_db::orm::Model;
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Debug, Clone, Serialize, Deserialize)]
	/// struct Comment { id: Option<i64>, content_type: String, object_id: i64 }
	///
	/// # #[derive(Clone)]
	/// # struct CommentFields;
	/// # impl reinhardt_db::orm::FieldSelector for CommentFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// impl Model for Comment {
	///     type PrimaryKey = i64;
	/// #     type Fields = CommentFields;
	///     fn table_name() -> &'static str { "comments" }
	/// #     fn new_fields() -> Self::Fields { CommentFields }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// }
	///
	/// let config = PolymorphicConfig::new("content_type", "object_id");
	/// let mut rel = PolymorphicRelation::<Comment>::new("content_object", config);
	/// let identity = PolymorphicIdentity::new("post", "posts", "id");
	/// rel.register_type(identity.clone());
	///
	/// assert!(rel.get_identity("post").is_some());
	/// ```
	pub fn register_type(&mut self, identity: PolymorphicIdentity) {
		self.identities
			.insert(identity.type_id().to_string(), identity);
	}

	/// Get identity for a type
	pub fn get_identity(&self, type_id: &str) -> Option<&PolymorphicIdentity> {
		self.identities.get(type_id)
	}

	/// Get all registered type identifiers
	pub fn type_ids(&self) -> Vec<&str> {
		self.identities.keys().map(|s| s.as_str()).collect()
	}

	/// Get relationship name
	pub fn name(&self) -> &str {
		&self.name
	}

	/// Get polymorphic configuration
	pub fn config(&self) -> &PolymorphicConfig {
		&self.config
	}

	/// Get relationship type
	pub fn relationship_type(&self) -> RelationshipType {
		self.relationship_type
	}

	/// Build a parameterized SQL query for loading a related object.
	///
	/// Returns `(sql, params)` where `sql` contains `$N` placeholders and
	/// `params` holds the corresponding bind values. Returns `None` if the
	/// `type_id` is not registered.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::polymorphic::{PolymorphicRelation, PolymorphicConfig, PolymorphicIdentity};
	/// use reinhardt_db::orm::Model;
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Debug, Clone, Serialize, Deserialize)]
	/// struct Comment { id: Option<i64>, content_type: String, object_id: i64 }
	///
	/// # #[derive(Clone)]
	/// # struct CommentFields;
	/// # impl reinhardt_db::orm::FieldSelector for CommentFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// impl Model for Comment {
	///     type PrimaryKey = i64;
	/// #     type Fields = CommentFields;
	///     fn table_name() -> &'static str { "comments" }
	/// #     fn new_fields() -> Self::Fields { CommentFields }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// }
	///
	/// let config = PolymorphicConfig::new("content_type", "object_id");
	/// let mut rel = PolymorphicRelation::<Comment>::new("content_object", config);
	/// let identity = PolymorphicIdentity::new("post", "posts", "id");
	/// rel.register_type(identity);
	///
	/// let result = rel.build_query("post", "123");
	/// assert!(result.is_some());
	/// let (sql, params) = result.unwrap();
	/// assert!(sql.contains("posts"));
	/// assert!(sql.contains("$1"));
	/// assert_eq!(params, vec!["123"]);
	/// ```
	pub fn build_query(&self, type_id: &str, object_id: &str) -> Option<(String, Vec<String>)> {
		let identity = self.get_identity(type_id)?;

		let sql = format!(
			"SELECT * FROM \"{}\" WHERE \"{}\" = $1",
			identity.table_name(),
			identity.pk_field(),
		);
		Some((sql, vec![object_id.to_string()]))
	}

	/// Generate a `SimpleExpr` condition for filtering by type.
	///
	/// Returns a safe, parameterized expression suitable for use with
	/// `reinhardt_query` condition builders.
	pub fn type_filter(&self, type_id: &str) -> SimpleExpr {
		Expr::col(Alias::new(self.config.type_field())).eq(type_id)
	}

	/// Generate JOIN clause for polymorphic relationship.
	///
	/// Table and column names are properly quoted to prevent injection.
	pub fn join_clause(&self, type_id: &str, parent_alias: &str) -> Option<String> {
		let identity = self.get_identity(type_id)?;

		Some(format!(
			"LEFT JOIN \"{}\" ON \"{}\".\"{}\" = \"{}\".\"{}\"",
			identity.table_name(),
			parent_alias,
			self.config.id_field(),
			identity.table_name(),
			identity.pk_field()
		))
	}
}

/// Registry for polymorphic types
/// Global registry mapping type identifiers to model information
#[derive(Debug, Default)]
pub struct PolymorphicRegistry {
	/// Type identifier -> Identity mapping
	identities: HashMap<String, PolymorphicIdentity>,
}

impl PolymorphicRegistry {
	/// Create a new registry
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::polymorphic::PolymorphicRegistry;
	///
	/// let registry = PolymorphicRegistry::new();
	/// assert_eq!(registry.count(), 0);
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Register a polymorphic identity
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::polymorphic::{PolymorphicRegistry, PolymorphicIdentity};
	///
	/// let mut registry = PolymorphicRegistry::new();
	/// let identity = PolymorphicIdentity::new("user", "users", "id");
	/// registry.register(identity.clone());
	/// assert_eq!(registry.count(), 1);
	/// assert!(registry.get("user").is_some());
	/// ```
	pub fn register(&mut self, identity: PolymorphicIdentity) {
		self.identities
			.insert(identity.type_id().to_string(), identity);
	}

	/// Get identity by type ID
	pub fn get(&self, type_id: &str) -> Option<&PolymorphicIdentity> {
		self.identities.get(type_id)
	}

	/// Get all registered type IDs
	pub fn type_ids(&self) -> Vec<&str> {
		self.identities.keys().map(|s| s.as_str()).collect()
	}

	/// Get count of registered types
	pub fn count(&self) -> usize {
		self.identities.len()
	}

	/// Clear all registrations
	pub fn clear(&mut self) {
		self.identities.clear();
	}
}

/// Query builder for polymorphic relationships
/// Handles complex queries across multiple model types
pub struct PolymorphicQuery<P: Model> {
	/// Base model
	_phantom: PhantomData<P>,

	/// Polymorphic relation being queried
	relation: PolymorphicRelation<P>,

	/// Active filters as type-safe expressions
	filters: Vec<SimpleExpr>,

	/// Selected type ID
	selected_type: Option<String>,
}

impl<P: Model> PolymorphicQuery<P> {
	/// Create a new polymorphic query
	pub fn new(relation: PolymorphicRelation<P>) -> Self {
		Self {
			_phantom: PhantomData,
			relation,
			filters: Vec::new(),
			selected_type: None,
		}
	}

	/// Filter by type ID
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::polymorphic::{PolymorphicQuery, PolymorphicRelation, PolymorphicConfig};
	/// use reinhardt_db::orm::Model;
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Debug, Clone, Serialize, Deserialize)]
	/// struct Comment { id: Option<i64>, content_type: String, object_id: i64 }
	///
	/// # #[derive(Clone)]
	/// # struct CommentFields;
	/// # impl reinhardt_db::orm::FieldSelector for CommentFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// impl Model for Comment {
	///     type PrimaryKey = i64;
	/// #     type Fields = CommentFields;
	///     fn table_name() -> &'static str { "comments" }
	/// #     fn new_fields() -> Self::Fields { CommentFields }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// }
	///
	/// let config = PolymorphicConfig::new("content_type", "object_id");
	/// let rel = PolymorphicRelation::<Comment>::new("content_object", config);
	/// let query = PolymorphicQuery::new(rel).filter_type("post");
	/// assert_eq!(query.selected_type(), Some("post"));
	/// ```
	pub fn filter_type(mut self, type_id: impl Into<String>) -> Self {
		let type_id = type_id.into();
		self.filters.push(self.relation.type_filter(&type_id));
		self.selected_type = Some(type_id);
		self
	}

	/// Add a type-safe filter condition.
	///
	/// Accepts a `SimpleExpr` from `reinhardt_query` to ensure all conditions
	/// are parameterized and safe from SQL injection.
	pub fn filter(mut self, condition: SimpleExpr) -> Self {
		self.filters.push(condition);
		self
	}

	/// Build parameterized SQL query.
	///
	/// Returns `(sql, values)` where `sql` contains parameterized placeholders.
	pub fn build_sql(&self) -> (String, reinhardt_query::value::Values) {
		let base_table = P::table_name();
		let mut stmt = Query::select()
			.column(Alias::new("*"))
			.from(Alias::new(base_table))
			.to_owned();

		if !self.filters.is_empty() {
			let mut cond = Condition::all();
			for f in &self.filters {
				cond = cond.add(f.clone());
			}
			stmt = stmt.cond_where(cond).to_owned();
		}

		stmt.build_any(&reinhardt_query::prelude::PostgresQueryBuilder)
	}

	/// Get selected type ID
	pub fn selected_type(&self) -> Option<&str> {
		self.selected_type.as_deref()
	}

	/// Get relation
	pub fn relation(&self) -> &PolymorphicRelation<P> {
		&self.relation
	}
}

/// Global polymorphic registry instance
static POLYMORPHIC_REGISTRY: once_cell::sync::Lazy<parking_lot::RwLock<PolymorphicRegistry>> =
	once_cell::sync::Lazy::new(|| parking_lot::RwLock::new(PolymorphicRegistry::new()));

/// Get global polymorphic registry
pub fn polymorphic_registry() -> &'static parking_lot::RwLock<PolymorphicRegistry> {
	&POLYMORPHIC_REGISTRY
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_core::validators::TableName;
	use serde::{Deserialize, Serialize};

	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct Comment {
		id: Option<i64>,
		content_type: String,
		object_id: i64,
		text: String,
	}

	const COMMENT_TABLE: TableName = TableName::new_const("comments");

	#[derive(Debug, Clone)]
	struct CommentFields;

	impl crate::orm::model::FieldSelector for CommentFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for Comment {
		type PrimaryKey = i64;
		type Fields = CommentFields;

		fn table_name() -> &'static str {
			COMMENT_TABLE.as_str()
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn primary_key_field() -> &'static str {
			"id"
		}

		fn new_fields() -> Self::Fields {
			CommentFields
		}
	}

	#[allow(dead_code)]
	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct Post {
		id: Option<i64>,
		title: String,
	}

	#[allow(dead_code)]
	const POST_TABLE: TableName = TableName::new_const("posts");

	#[derive(Debug, Clone)]
	struct PostFields;

	impl crate::orm::model::FieldSelector for PostFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for Post {
		type PrimaryKey = i64;
		type Fields = PostFields;

		fn table_name() -> &'static str {
			POST_TABLE.as_str()
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn primary_key_field() -> &'static str {
			"id"
		}

		fn new_fields() -> Self::Fields {
			PostFields
		}
	}

	#[test]
	fn test_polymorphic_config_creation() {
		let config = PolymorphicConfig::new("content_type", "object_id");
		assert_eq!(config.type_field(), "content_type");
		assert_eq!(config.id_field(), "object_id");
		assert_eq!(config.inheritance_type(), InheritanceType::SingleTable);
	}

	#[test]
	fn test_polymorphic_config_with_inheritance() {
		let config =
			PolymorphicConfig::new("type", "id").with_inheritance(InheritanceType::JoinedTable);
		assert_eq!(config.inheritance_type(), InheritanceType::JoinedTable);
	}

	#[test]
	fn test_polymorphic_config_with_default_type() {
		let config = PolymorphicConfig::new("type", "id").with_default_type("post");
		assert_eq!(config.default_type(), Some("post"));
	}

	#[test]
	fn test_polymorphic_identity_creation() {
		let identity = PolymorphicIdentity::new("user", "users", "id");
		assert_eq!(identity.type_id(), "user");
		assert_eq!(identity.table_name(), "users");
		assert_eq!(identity.pk_field(), "id");
	}

	#[test]
	fn test_polymorphic_identity_serialization() {
		let identity = PolymorphicIdentity::new("post", "posts", "id");
		let json = serde_json::to_string(&identity).unwrap();
		let deserialized: PolymorphicIdentity = serde_json::from_str(&json).unwrap();
		assert_eq!(identity, deserialized);
	}

	#[test]
	fn test_polymorphic_relation_creation() {
		let config = PolymorphicConfig::new("content_type", "object_id");
		let rel = PolymorphicRelation::<Comment>::new("content_object", config);
		assert_eq!(rel.name(), "content_object");
		assert_eq!(rel.relationship_type(), RelationshipType::ManyToOne);
	}

	#[test]
	fn test_polymorphic_relation_register_type() {
		let config = PolymorphicConfig::new("content_type", "object_id");
		let mut rel = PolymorphicRelation::<Comment>::new("content_object", config);

		let post_identity = PolymorphicIdentity::new("post", "posts", "id");
		rel.register_type(post_identity);

		assert!(rel.get_identity("post").is_some());
		assert_eq!(rel.type_ids(), vec!["post"]);
	}

	#[test]
	fn test_polymorphic_relation_multiple_types() {
		let config = PolymorphicConfig::new("content_type", "object_id");
		let mut rel = PolymorphicRelation::<Comment>::new("content_object", config);

		rel.register_type(PolymorphicIdentity::new("post", "posts", "id"));
		rel.register_type(PolymorphicIdentity::new("user", "users", "id"));

		assert_eq!(rel.type_ids().len(), 2);
		assert!(rel.get_identity("post").is_some());
		assert!(rel.get_identity("user").is_some());
	}

	#[test]
	fn test_polymorphic_relation_build_query() {
		let config = PolymorphicConfig::new("content_type", "object_id");
		let mut rel = PolymorphicRelation::<Comment>::new("content_object", config);

		rel.register_type(PolymorphicIdentity::new("post", "posts", "id"));

		let result = rel.build_query("post", "123");
		let (sql, params) = result.unwrap();
		assert!(sql.contains("posts"));
		assert!(sql.contains("$1"));
		assert_eq!(params, vec!["123"]);
	}

	#[test]
	fn test_polymorphic_relation_build_query_unknown_type() {
		let config = PolymorphicConfig::new("content_type", "object_id");
		let rel = PolymorphicRelation::<Comment>::new("content_object", config);

		let sql = rel.build_query("unknown", "123");
		assert!(sql.is_none());
	}

	#[test]
	fn test_polymorphic_relation_type_filter() {
		use reinhardt_query::prelude::PostgresQueryBuilder;

		let config = PolymorphicConfig::new("content_type", "object_id");
		let rel = PolymorphicRelation::<Comment>::new("content_object", config);

		// type_filter now returns a SimpleExpr; verify it builds correctly
		let filter = rel.type_filter("post");
		let mut stmt = Query::select()
			.column(Alias::new("*"))
			.from(Alias::new("comments"))
			.cond_where(Condition::all().add(filter))
			.to_owned();
		let (sql, _) = stmt.build_any(&PostgresQueryBuilder);
		assert!(sql.contains("\"content_type\""));
		assert!(sql.contains("$1"));
	}

	#[test]
	fn test_polymorphic_relation_join_clause() {
		let config = PolymorphicConfig::new("content_type", "object_id");
		let mut rel = PolymorphicRelation::<Comment>::new("content_object", config);

		rel.register_type(PolymorphicIdentity::new("post", "posts", "id"));

		let join = rel.join_clause("post", "comments");
		let join = join.unwrap();
		assert!(join.contains("LEFT JOIN \"posts\""));
		assert!(join.contains("\"comments\".\"object_id\" = \"posts\".\"id\""));
	}

	#[test]
	fn test_polymorphic_registry_creation() {
		let registry = PolymorphicRegistry::new();
		assert_eq!(registry.count(), 0);
	}

	#[test]
	fn test_polymorphic_registry_register() {
		let mut registry = PolymorphicRegistry::new();
		let identity = PolymorphicIdentity::new("user", "users", "id");
		registry.register(identity);

		assert_eq!(registry.count(), 1);
		assert!(registry.get("user").is_some());
	}

	#[test]
	fn test_polymorphic_registry_multiple_types() {
		let mut registry = PolymorphicRegistry::new();
		registry.register(PolymorphicIdentity::new("user", "users", "id"));
		registry.register(PolymorphicIdentity::new("post", "posts", "id"));

		assert_eq!(registry.count(), 2);
		assert_eq!(registry.type_ids().len(), 2);
	}

	#[test]
	fn test_polymorphic_registry_clear() {
		let mut registry = PolymorphicRegistry::new();
		registry.register(PolymorphicIdentity::new("user", "users", "id"));
		assert_eq!(registry.count(), 1);

		registry.clear();
		assert_eq!(registry.count(), 0);
	}

	#[test]
	fn test_polymorphic_registry_get_unknown() {
		let registry = PolymorphicRegistry::new();
		assert!(registry.get("unknown").is_none());
	}

	#[test]
	fn test_polymorphic_query_creation() {
		let config = PolymorphicConfig::new("content_type", "object_id");
		let rel = PolymorphicRelation::<Comment>::new("content_object", config);
		let query = PolymorphicQuery::new(rel);

		assert!(query.selected_type().is_none());
	}

	#[test]
	fn test_polymorphic_query_filter_type() {
		let config = PolymorphicConfig::new("content_type", "object_id");
		let rel = PolymorphicRelation::<Comment>::new("content_object", config);
		let query = PolymorphicQuery::new(rel).filter_type("post");

		assert_eq!(query.selected_type(), Some("post"));
	}

	#[test]
	fn test_polymorphic_query_build_sql() {
		let config = PolymorphicConfig::new("content_type", "object_id");
		let rel = PolymorphicRelation::<Comment>::new("content_object", config);
		let query = PolymorphicQuery::new(rel).filter_type("post");

		let (sql, values) = query.build_sql();
		assert!(sql.contains("\"comments\""));
		assert!(sql.contains("\"content_type\""));
		assert!(sql.contains("$1"));
		assert_eq!(values.0.len(), 1);
	}

	#[test]
	fn test_polymorphic_query_multiple_filters() {
		let config = PolymorphicConfig::new("content_type", "object_id");
		let rel = PolymorphicRelation::<Comment>::new("content_object", config);
		let query = PolymorphicQuery::new(rel)
			.filter_type("post")
			.filter(Expr::col(Alias::new("object_id")).gt(100));

		let (sql, values) = query.build_sql();
		assert!(sql.contains("\"content_type\""));
		assert!(sql.contains("\"object_id\""));
		assert!(sql.contains("AND"));
		assert_eq!(values.0.len(), 2);
	}

	#[test]
	fn test_global_registry_access() {
		let registry = polymorphic_registry();
		let mut reg = registry.write();
		let initial_count = reg.count();

		reg.register(PolymorphicIdentity::new("test", "test_table", "id"));
		assert_eq!(reg.count(), initial_count + 1);

		reg.clear();
	}

	#[test]
	fn test_inheritance_type_equality() {
		assert_eq!(InheritanceType::SingleTable, InheritanceType::SingleTable);
		assert_ne!(InheritanceType::SingleTable, InheritanceType::JoinedTable);
		assert_ne!(InheritanceType::JoinedTable, InheritanceType::ConcreteTable);
	}
}
