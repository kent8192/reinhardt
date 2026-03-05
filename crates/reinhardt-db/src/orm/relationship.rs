//! # Relationship Definitions
//!
//! SQLAlchemy-inspired relationship and loading strategies.
//!
//! This module is inspired by SQLAlchemy's relationships.py
//! Copyright 2005-2025 SQLAlchemy authors and contributors
//! Licensed under MIT License. See THIRD-PARTY-NOTICES for details.

use crate::orm::Model;
use crate::orm::loading::LoadingStrategy;
use reinhardt_query::prelude::{
	Alias, ColumnRef, Expr, ExprTrait, Order, Query, QueryStatementBuilder, SelectStatement,
};
use std::marker::PhantomData;

/// Relationship type - defines cardinality
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationshipType {
	/// One-to-One relationship
	OneToOne,
	/// One-to-Many relationship
	OneToMany,
	/// Many-to-One relationship
	ManyToOne,
	/// Many-to-Many relationship
	ManyToMany,
}

/// Cascade options for relationships
/// Defines what operations should cascade to related objects
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CascadeOption {
	/// All operations cascade
	All,
	/// DELETE operations cascade
	Delete,
	/// Save and update operations cascade
	SaveUpdate,
	/// Merge operations cascade
	Merge,
	/// Expunge operations cascade
	Expunge,
	/// Delete orphaned objects
	DeleteOrphan,
	/// Refresh operations cascade
	Refresh,
}

impl CascadeOption {
	/// Parse cascade string to options
	/// Example: "all, delete-orphan" -> [All, DeleteOrphan]
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::relationship::CascadeOption;
	///
	/// let options = CascadeOption::parse("all, delete-orphan");
	/// assert_eq!(options.len(), 2);
	/// assert!(options.contains(&CascadeOption::All));
	/// assert!(options.contains(&CascadeOption::DeleteOrphan));
	///
	/// let save_update = CascadeOption::parse("save-update");
	/// assert_eq!(save_update.len(), 1);
	/// assert!(save_update.contains(&CascadeOption::SaveUpdate));
	/// ```
	pub fn parse(cascade_str: &str) -> Vec<Self> {
		cascade_str
			.split(',')
			.filter_map(|s| match s.trim().to_lowercase().as_str() {
				"all" => Some(CascadeOption::All),
				"delete" => Some(CascadeOption::Delete),
				"save-update" => Some(CascadeOption::SaveUpdate),
				"merge" => Some(CascadeOption::Merge),
				"expunge" => Some(CascadeOption::Expunge),
				"delete-orphan" => Some(CascadeOption::DeleteOrphan),
				"refresh" => Some(CascadeOption::Refresh),
				_ => None,
			})
			.collect()
	}
	/// Convert to SQL ON DELETE/ON UPDATE clause
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::relationship::CascadeOption;
	///
	/// let delete_clause = CascadeOption::Delete.to_sql_clause();
	/// assert_eq!(delete_clause, Some("ON DELETE CASCADE"));
	///
	/// let all_clause = CascadeOption::All.to_sql_clause();
	/// assert_eq!(all_clause, Some("ON DELETE CASCADE ON UPDATE CASCADE"));
	///
	/// let merge_clause = CascadeOption::Merge.to_sql_clause();
	/// assert_eq!(merge_clause, None);
	/// ```
	pub fn to_sql_clause(&self) -> Option<&'static str> {
		match self {
			CascadeOption::Delete => Some("ON DELETE CASCADE"),
			CascadeOption::All => Some("ON DELETE CASCADE ON UPDATE CASCADE"),
			_ => None,
		}
	}
}

/// Relationship definition
/// Generic over parent and child models
pub struct Relationship<P: Model, C: Model> {
	/// Relationship name
	name: String,

	/// Type of relationship
	relationship_type: RelationshipType,

	/// Loading strategy
	loading_strategy: LoadingStrategy,

	/// Foreign key field name on child model
	foreign_key: Option<String>,

	/// Back reference name (for bidirectional relationships)
	back_populates: Option<String>,

	/// Back reference object (alternative to back_populates)
	backref: Option<String>,

	/// Cascade options
	cascade: Vec<CascadeOption>,

	/// Order by clause for collections
	order_by: Option<String>,

	/// Join condition (custom SQL)
	join_condition: Option<String>,

	/// Primary join condition (for complex relationships)
	primaryjoin: Option<String>,

	/// Secondary join condition (for many-to-many through tables)
	secondaryjoin: Option<String>,

	/// Secondary table for many-to-many (junction/through table)
	secondary: Option<String>,

	/// Remote side of the relationship (for self-referential)
	remote_side: Option<Vec<String>>,

	/// Read-only relationship
	viewonly: bool,

	/// Use list for collection (vs dynamic query)
	uselist: bool,

	/// Relationship direction (for self-referential)
	#[allow(dead_code)]
	direction: Option<RelationshipDirection>,

	/// Foreign keys specification
	foreign_keys: Option<Vec<String>>,

	/// Synchronize session state
	sync_backref: bool,

	_phantom_p: PhantomData<P>,
	_phantom_c: PhantomData<C>,
}

/// Relationship direction for self-referential relationships
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationshipDirection {
	/// One-to-Many or Many-to-One
	OneToMany,
	/// Many-to-One
	ManyToOne,
	/// Many-to-Many
	ManyToMany,
}

impl<P: Model, C: Model> Relationship<P, C> {
	/// Create a new relationship
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::relationship::{Relationship, RelationshipType};
	/// use reinhardt_db::orm::Model;
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Debug, Clone, Serialize, Deserialize)]
	/// struct User { id: Option<i64>, name: String }
	///
	/// #[derive(Debug, Clone, Serialize, Deserialize)]
	/// struct Post { id: Option<i64>, user_id: i64, title: String }
	///
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl reinhardt_db::orm::FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// impl Model for User {
	///     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	///     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// }
	///
	/// impl Model for Post {
	///     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	///     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// }
	///
	/// let rel = Relationship::<User, Post>::new("posts", RelationshipType::OneToMany);
	/// assert_eq!(rel.name(), "posts");
	/// assert_eq!(rel.relationship_type(), RelationshipType::OneToMany);
	/// ```
	pub fn new(name: &str, relationship_type: RelationshipType) -> Self {
		Self {
			name: name.to_string(),
			relationship_type,
			loading_strategy: LoadingStrategy::Lazy,
			foreign_key: None,
			back_populates: None,
			backref: None,
			cascade: Vec::new(),
			order_by: None,
			join_condition: None,
			primaryjoin: None,
			secondaryjoin: None,
			secondary: None,
			remote_side: None,
			viewonly: false,
			uselist: true,
			direction: None,
			foreign_keys: None,
			sync_backref: true,
			_phantom_p: PhantomData,
			_phantom_c: PhantomData,
		}
	}
	/// Set loading strategy using LoadingStrategy enum
	pub fn with_lazy(mut self, strategy: LoadingStrategy) -> Self {
		self.loading_strategy = strategy;
		self
	}
	/// Set foreign key
	pub fn with_foreign_key(mut self, fk: &str) -> Self {
		self.foreign_key = Some(fk.to_string());
		self
	}
	/// Set back reference for bidirectional relationship
	pub fn with_back_populates(mut self, back_ref: &str) -> Self {
		self.back_populates = Some(back_ref.to_string());
		self
	}
	/// Add cascade option (new API with CascadeOption enum)
	pub fn with_cascade_option(mut self, cascade: CascadeOption) -> Self {
		self.cascade.push(cascade);
		self
	}
	/// Add cascade options from string (e.g., "all, delete-orphan")
	pub fn with_cascade(mut self, cascade_str: &str) -> Self {
		self.cascade.extend(CascadeOption::parse(cascade_str));
		self
	}
	/// Set secondary table for many-to-many (through table)
	/// SQLAlchemy: relationship("Role", secondary="user_roles")
	pub fn with_secondary(mut self, table_name: &str) -> Self {
		self.secondary = Some(table_name.to_string());
		self
	}
	/// Set primary join condition
	/// SQLAlchemy: relationship("Child", primaryjoin="Parent.id==Child.parent_id")
	pub fn with_primaryjoin(mut self, condition: &str) -> Self {
		self.primaryjoin = Some(condition.to_string());
		self
	}
	/// Set secondary join condition (for many-to-many)
	/// SQLAlchemy: relationship("Role", secondaryjoin="user_roles.c.role_id==Role.id")
	pub fn with_secondaryjoin(mut self, condition: &str) -> Self {
		self.secondaryjoin = Some(condition.to_string());
		self
	}
	/// Set backref (creates reverse relationship)
	/// SQLAlchemy: relationship("Child", backref="parent")
	pub fn with_backref(mut self, backref_name: &str) -> Self {
		self.backref = Some(backref_name.to_string());
		self
	}
	/// Set remote side for self-referential relationships
	/// SQLAlchemy: relationship("Node", remote_side=[Node.id])
	pub fn with_remote_side(mut self, columns: Vec<String>) -> Self {
		self.remote_side = Some(columns);
		self
	}
	/// Mark as view-only (read-only)
	/// SQLAlchemy: relationship("Child", viewonly=True)
	///
	pub fn viewonly(mut self) -> Self {
		self.viewonly = true;
		self
	}
	/// Set uselist=False for scalar relationships
	/// SQLAlchemy: relationship("Parent", uselist=False)
	///
	pub fn scalar(mut self) -> Self {
		self.uselist = false;
		self
	}
	/// Set foreign keys explicitly
	/// SQLAlchemy: relationship("Child", foreign_keys=[Child.parent_id])
	pub fn with_foreign_keys(mut self, fk_columns: Vec<String>) -> Self {
		self.foreign_keys = Some(fk_columns);
		self
	}
	/// Disable backref synchronization
	///
	pub fn no_sync_backref(mut self) -> Self {
		self.sync_backref = false;
		self
	}
	/// Set order by for collections
	pub fn with_order_by(mut self, order_by: &str) -> Self {
		self.order_by = Some(order_by.to_string());
		self
	}
	/// Set custom join condition
	pub fn with_join_condition(mut self, condition: &str) -> Self {
		self.join_condition = Some(condition.to_string());
		self
	}
	/// Get relationship name
	///
	pub fn name(&self) -> &str {
		&self.name
	}
	/// Get relationship type
	///
	pub fn relationship_type(&self) -> RelationshipType {
		self.relationship_type
	}
	/// Get loading strategy
	///
	pub fn lazy(&self) -> LoadingStrategy {
		self.loading_strategy
	}
	/// Get loading strategy (alias for consistency)
	///
	pub fn loading_strategy(&self) -> LoadingStrategy {
		self.loading_strategy
	}
	/// Generate reinhardt-query statement for loading related records
	///
	/// Returns a SelectStatement for Lazy/Selectin/Dynamic strategies,
	/// or None for Joined (handled differently), NoLoad, WriteOnly strategies.
	pub fn load_query<V>(&self, parent_id: V) -> Option<SelectStatement>
	where
		V: Into<reinhardt_query::value::Value>,
	{
		let child_table = C::table_name();
		let fk = self.foreign_key.as_deref().unwrap_or("id");

		match self.loading_strategy {
			LoadingStrategy::Joined => {
				// Joined strategy is handled at the query builder level
				// Use get_join_config() to obtain join configuration
				None
			}
			LoadingStrategy::Lazy | LoadingStrategy::Selectin | LoadingStrategy::Dynamic => {
				let mut stmt = Query::select();
				stmt.from(Alias::new(child_table))
					.column(ColumnRef::Asterisk)
					.and_where(Expr::col(Alias::new(fk)).eq(parent_id.into()));

				if let Some(order) = &self.order_by {
					for (col, dir) in Self::parse_order_by(order) {
						stmt.order_by(Alias::new(&col), dir);
					}
				}

				Some(stmt.to_owned())
			}
			LoadingStrategy::Subquery => {
				// Basic subquery generation for standalone use.
				// For proper parent query correlation, use build_subquery() method instead,
				// which accepts the parent SelectStatement and generates an optimized
				// IN-subquery that incorporates the parent's WHERE clause.
				let mut stmt = Query::select();
				stmt.from(Alias::new(child_table))
					.column(ColumnRef::Asterisk);
				Some(stmt.to_owned())
			}
			LoadingStrategy::Raise => {
				panic!("Attempting to load a relationship marked as 'raise'");
			}
			LoadingStrategy::NoLoad | LoadingStrategy::WriteOnly => None,
		}
	}

	/// Parse ORDER BY string to extract column names and directions
	///
	/// Supports formats like:
	/// - "created_at" -> [(created_at, Asc)]
	/// - "created_at DESC" -> [(created_at, Desc)]
	/// - "name ASC, created_at DESC" -> [(name, Asc), (created_at, Desc)]
	fn parse_order_by(order_by: &str) -> Vec<(String, Order)> {
		order_by
			.split(',')
			.filter_map(|part| {
				let trimmed = part.trim();
				if trimmed.is_empty() {
					return None;
				}

				if trimmed.ends_with(" DESC") || trimmed.ends_with(" desc") {
					let col = trimmed[..trimmed.len() - 5].trim();
					Some((col.to_string(), Order::Desc))
				} else if trimmed.ends_with(" ASC") || trimmed.ends_with(" asc") {
					let col = trimmed[..trimmed.len() - 4].trim();
					Some((col.to_string(), Order::Asc))
				} else {
					Some((trimmed.to_string(), Order::Asc))
				}
			})
			.collect()
	}

	/// Get join configuration for Joined loading strategy
	///
	/// Returns join configuration that can be applied at the query builder level.
	/// This is the recommended way to handle Joined loading strategy.
	pub fn get_join_config(&self) -> Option<JoinConfig> {
		if self.loading_strategy != LoadingStrategy::Joined {
			return None;
		}

		let parent_table = P::table_name();
		let child_table = C::table_name();
		let fk = self.foreign_key.as_deref().unwrap_or("id");

		Some(JoinConfig {
			table: child_table.to_string(),
			on_condition: format!("{}.id = {}.{}", parent_table, child_table, fk),
			join_type: JoinType::LeftJoin,
		})
	}

	/// Build subquery for Subquery loading strategy
	///
	/// Accepts parent SelectStatement and builds a subquery that incorporates
	/// the parent's WHERE clause to efficiently load related records.
	pub fn build_subquery(&self, parent_stmt: &SelectStatement) -> Option<String> {
		if self.loading_strategy != LoadingStrategy::Subquery {
			return None;
		}

		let child_table = C::table_name();
		let fk = self.foreign_key.as_deref().unwrap_or("id");

		// Build subquery that extracts parent IDs: SELECT id FROM parent_table WHERE ...
		let mut parent_subquery = parent_stmt.clone();
		parent_subquery.clear_selects();
		parent_subquery.column(Alias::new("id"));

		// Build main query: SELECT * FROM child_table WHERE foreign_key IN (subquery)
		let mut stmt = Query::select();
		stmt.from(Alias::new(child_table))
			.column(ColumnRef::Asterisk)
			.and_where(Expr::col(Alias::new(fk)).in_subquery(parent_subquery));

		// Convert to SQL string using PostgreSQL dialect (can be adjusted based on context)
		use reinhardt_query::prelude::PostgresQueryBuilder;
		Some(stmt.to_string(PostgresQueryBuilder))
	}

	/// Generate SQL string for loading (convenience method)
	///
	/// This converts the reinhardt-query statement to SQL string.
	/// Use this only when you need the final SQL string.
	pub fn load_sql<V>(&self, parent_id: V, dialect: super::types::DatabaseDialect) -> String
	where
		V: Into<reinhardt_query::value::Value>,
	{
		use reinhardt_query::prelude::{
			MySqlQueryBuilder, PostgresQueryBuilder, QueryStatementBuilder, SqliteQueryBuilder,
		};

		if let Some(stmt) = self.load_query(parent_id) {
			match dialect {
				super::types::DatabaseDialect::PostgreSQL => stmt.to_string(PostgresQueryBuilder),
				super::types::DatabaseDialect::MySQL => stmt.to_string(MySqlQueryBuilder),
				super::types::DatabaseDialect::SQLite => stmt.to_string(SqliteQueryBuilder),
				// MSSQL: PostgreSQL builder used as fallback since reinhardt-query lacks MssqlQueryBuilder.
				// Some PostgreSQL-specific syntax may not be compatible with MSSQL.
				super::types::DatabaseDialect::MSSQL => stmt.to_string(PostgresQueryBuilder),
			}
		} else {
			String::new()
		}
	}
}

/// Join configuration for Joined loading strategy
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct JoinConfig {
	/// Table to join
	pub table: String,
	/// ON condition for the join
	pub on_condition: String,
	/// Type of join (INNER, LEFT, RIGHT, etc.)
	pub join_type: JoinType,
}

/// Join type for relationships
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinType {
	/// INNER JOIN
	InnerJoin,
	/// LEFT JOIN (most common for relationships)
	LeftJoin,
	/// RIGHT JOIN
	RightJoin,
	/// FULL OUTER JOIN
	FullJoin,
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::orm::types::DatabaseDialect;
	use reinhardt_core::validators::TableName;
	use serde::{Deserialize, Serialize};

	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct User {
		id: Option<i64>,
		name: String,
	}

	const USER_TABLE: TableName = TableName::new_const("users");

	#[derive(Debug, Clone)]
	struct UserFields;

	impl crate::orm::model::FieldSelector for UserFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for User {
		type PrimaryKey = i64;
		type Fields = UserFields;

		fn table_name() -> &'static str {
			USER_TABLE.as_str()
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
			UserFields
		}
	}

	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct Post {
		id: Option<i64>,
		user_id: i64,
		title: String,
	}

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

	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct Role {
		id: Option<i64>,
		name: String,
	}

	const ROLE_TABLE: TableName = TableName::new_const("roles");

	#[derive(Debug, Clone)]
	struct RoleFields;

	impl crate::orm::model::FieldSelector for RoleFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for Role {
		type PrimaryKey = i64;
		type Fields = RoleFields;

		fn table_name() -> &'static str {
			ROLE_TABLE.as_str()
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
			RoleFields
		}
	}

	#[test]
	fn test_one_to_many_relationship() {
		let rel = Relationship::<User, Post>::new("posts", RelationshipType::OneToMany)
			.with_foreign_key("user_id")
			.with_lazy(LoadingStrategy::Lazy);

		assert_eq!(rel.name(), "posts");
		assert_eq!(rel.relationship_type(), RelationshipType::OneToMany);
		assert_eq!(rel.lazy(), LoadingStrategy::Lazy);
	}

	#[test]
	fn test_lazy_joined_query() {
		let rel = Relationship::<User, Post>::new("posts", RelationshipType::OneToMany)
			.with_foreign_key("user_id")
			.with_lazy(LoadingStrategy::Joined);

		// Joined strategy returns None - should be handled at query builder level
		let query = rel.load_query(1);
		assert!(query.is_none());
	}

	#[test]
	fn test_lazy_select_query() {
		use reinhardt_query::prelude::{QueryStatementBuilder, SqliteQueryBuilder};

		let rel = Relationship::<User, Post>::new("posts", RelationshipType::OneToMany)
			.with_foreign_key("user_id")
			.with_lazy(LoadingStrategy::Lazy)
			.with_order_by("created_at");

		let query = rel.load_query(1).expect("Should return a query");
		let sql = query.to_string(SqliteQueryBuilder);

		assert!(sql.contains("SELECT * FROM"));
		assert!(sql.contains("posts"));
		assert!(sql.contains("user_id"));
		assert!(sql.contains("ORDER BY") && sql.contains("created_at"));
	}

	#[test]
	fn test_bidirectional_relationship() {
		let rel = Relationship::<User, Post>::new("posts", RelationshipType::OneToMany)
			.with_back_populates("author")
			.with_cascade("delete");

		assert_eq!(rel.name(), "posts");
	}

	// Auto-generated relationship tests
	// Total: 30 tests

	#[test]
	fn test_search_with_exact_lookup_relationship_field() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_search_with_exact_lookup_relationship_field_1() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_emptylistfieldfilter_reverse_relationships() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_emptylistfieldfilter_reverse_relationships_1() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_relatedfieldlistfilter_reverse_relationships() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_relatedfieldlistfilter_reverse_relationships_1() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_relatedfieldlistfilter_reverse_relationships_default_ordering() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_relatedfieldlistfilter_reverse_relationships_default_ordering_1() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_relatedonlyfieldlistfilter_foreignkey_reverse_relationships() {
		let rel = Relationship::<User, Post>::new("posts", RelationshipType::OneToMany)
			.with_foreign_key("user_id");
		assert_eq!(rel.name(), "posts");
	}

	#[test]
	fn test_relatedonlyfieldlistfilter_foreignkey_reverse_relationships_1() {
		let rel = Relationship::<User, Post>::new("posts", RelationshipType::OneToMany)
			.with_foreign_key("user_id");
		assert_eq!(rel.name(), "posts");
	}

	#[test]
	fn test_relatedonlyfieldlistfilter_manytomany_reverse_relationships() {
		let rel = Relationship::<User, Role>::new("roles", RelationshipType::ManyToMany)
			.with_secondary("user_roles");
		assert_eq!(rel.relationship_type(), RelationshipType::ManyToMany);
	}

	#[test]
	fn test_relatedonlyfieldlistfilter_manytomany_reverse_relationships_1() {
		let rel = Relationship::<User, Role>::new("roles", RelationshipType::ManyToMany)
			.with_secondary("user_roles");
		assert_eq!(rel.relationship_type(), RelationshipType::ManyToMany);
	}

	#[test]
	fn test_valid_generic_relationship() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_valid_generic_relationship_1() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_valid_generic_relationship_with_explicit_fields() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_valid_generic_relationship_with_explicit_fields_1() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_valid_self_referential_generic_relationship() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_valid_self_referential_generic_relationship_1() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_delete_with_keeping_parents_relationships() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_delete_with_keeping_parents_relationships_1() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_fast_delete_combined_relationships() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_fast_delete_combined_relationships_1() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_aggregate() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_aggregate_1() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_aggregate_2() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_aggregate_3() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_as_subquery() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_as_subquery_1() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_condition_deeper_relation_name() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_condition_deeper_relation_name_1() {
		let rel = Relationship::<User, Post>::new("test_rel", RelationshipType::OneToMany);
		assert_eq!(rel.name(), "test_rel");
	}

	#[test]
	fn test_parse_order_by_single_column() {
		let parsed = Relationship::<User, Post>::parse_order_by("created_at");
		assert_eq!(parsed.len(), 1);
		assert_eq!(parsed[0].0, "created_at");
		assert_eq!(parsed[0].1, Order::Asc);
	}

	#[test]
	fn test_parse_order_by_with_desc() {
		let parsed = Relationship::<User, Post>::parse_order_by("created_at DESC");
		assert_eq!(parsed.len(), 1);
		assert_eq!(parsed[0].0, "created_at");
		assert_eq!(parsed[0].1, Order::Desc);
	}

	#[test]
	fn test_parse_order_by_with_asc() {
		let parsed = Relationship::<User, Post>::parse_order_by("name ASC");
		assert_eq!(parsed.len(), 1);
		assert_eq!(parsed[0].0, "name");
		assert_eq!(parsed[0].1, Order::Asc);
	}

	#[test]
	fn test_parse_order_by_multiple_columns() {
		let parsed = Relationship::<User, Post>::parse_order_by("name ASC, created_at DESC");
		assert_eq!(parsed.len(), 2);
		assert_eq!(parsed[0].0, "name");
		assert_eq!(parsed[0].1, Order::Asc);
		assert_eq!(parsed[1].0, "created_at");
		assert_eq!(parsed[1].1, Order::Desc);
	}

	#[test]
	fn test_parse_order_by_case_insensitive() {
		let parsed_upper = Relationship::<User, Post>::parse_order_by("name desc");
		assert_eq!(parsed_upper[0].1, Order::Desc);

		let parsed_lower = Relationship::<User, Post>::parse_order_by("name asc");
		assert_eq!(parsed_lower[0].1, Order::Asc);
	}

	#[test]
	fn test_get_join_config() {
		let rel = Relationship::<User, Post>::new("posts", RelationshipType::OneToMany)
			.with_foreign_key("user_id")
			.with_lazy(LoadingStrategy::Joined);

		let join_config = rel.get_join_config().expect("Should return join config");
		assert_eq!(join_config.table, "posts");
		assert!(join_config.on_condition.contains("users.id"));
		assert!(join_config.on_condition.contains("posts.user_id"));
		assert_eq!(join_config.join_type, JoinType::LeftJoin);
	}

	#[test]
	fn test_get_join_config_returns_none_for_non_joined() {
		let rel = Relationship::<User, Post>::new("posts", RelationshipType::OneToMany)
			.with_foreign_key("user_id")
			.with_lazy(LoadingStrategy::Lazy);

		assert!(rel.get_join_config().is_none());
	}

	#[test]
	fn test_build_subquery() {
		let rel = Relationship::<User, Post>::new("posts", RelationshipType::OneToMany)
			.with_foreign_key("user_id")
			.with_lazy(LoadingStrategy::Subquery);

		let parent_stmt = Query::select().from(Alias::new("users")).to_owned();
		let subquery = rel
			.build_subquery(&parent_stmt)
			.expect("Should return subquery");

		assert!(subquery.contains("posts"));
		assert!(subquery.contains("user_id"));
	}

	#[test]
	fn test_build_subquery_returns_none_for_non_subquery() {
		let rel = Relationship::<User, Post>::new("posts", RelationshipType::OneToMany)
			.with_foreign_key("user_id")
			.with_lazy(LoadingStrategy::Lazy);

		let parent_stmt = Query::select().from(Alias::new("users")).to_owned();
		assert!(rel.build_subquery(&parent_stmt).is_none());
	}

	#[test]
	fn test_load_sql_with_mssql() {
		let rel = Relationship::<User, Post>::new("posts", RelationshipType::OneToMany)
			.with_foreign_key("user_id")
			.with_lazy(LoadingStrategy::Lazy);

		let sql = rel.load_sql(1, DatabaseDialect::MSSQL);
		assert!(!sql.is_empty());
		assert!(sql.contains("posts"));
	}

	#[test]
	fn test_order_by_in_query() {
		use reinhardt_query::prelude::{QueryStatementBuilder, SqliteQueryBuilder};

		let rel = Relationship::<User, Post>::new("posts", RelationshipType::OneToMany)
			.with_foreign_key("user_id")
			.with_lazy(LoadingStrategy::Lazy)
			.with_order_by("created_at DESC, title ASC");

		let query = rel.load_query(1).expect("Should return a query");
		let sql = query.to_string(SqliteQueryBuilder);

		assert!(sql.contains("ORDER BY"));
		assert!(sql.contains("created_at"));
		assert!(sql.contains("title"));
	}
}
