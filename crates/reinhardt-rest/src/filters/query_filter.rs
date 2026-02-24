//! Type-safe query filter using Field and Lookup
//!
//! Provides a completely type-safe filtering system using reinhardt-orm's
//! Field<M, T> and Lookup types.

use super::ordering_field::OrderingField;
use super::{FilterBackend, FilterResult};
use async_trait::async_trait;
use reinhardt_db::orm::{
	Cond, Expr, Lookup, Model, MySqlQueryBuilder, Query, QueryFieldCompiler, QueryStatementBuilder,
};
use std::collections::HashMap;
use std::marker::PhantomData;

/// Type-safe query filter
///
/// Combines Field-based lookups and ordering into SQL WHERE and ORDER BY clauses.
/// All field access is compile-time checked for correctness.
///
/// # Examples
///
/// ```rust
/// # use reinhardt_rest::filters::QueryFilter;
/// # use reinhardt_db::orm::{Field, FieldSelector, Model};
/// # use reinhardt_rest::filters::field_extensions::FieldOrderingExt;
/// # #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
/// # struct Post {
/// #     id: i64,
/// #     title: String,
/// #     created_at: String,
/// # }
/// # #[derive(Clone)]
/// # struct PostFields;
/// # impl FieldSelector for PostFields {
/// #     fn with_alias(self, _alias: &str) -> Self { self }
/// # }
/// # impl Model for Post {
/// #     type PrimaryKey = i64;
/// #     type Fields = PostFields;
/// #     fn table_name() -> &'static str { "posts" }
/// #     fn new_fields() -> Self::Fields { PostFields }
/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { Some(self.id) }
/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = value; }
/// # }
/// let filter = QueryFilter::<Post>::new()
///     .with_lookup(Field::new(vec!["title"]).icontains("rust"))
///     .with_lookup(Field::<Post, i32>::new(vec!["created_at"]).gte(2024))
///     .order_by(Field::<Post, String>::new(vec!["title"]).asc());
/// // Verify filter was constructed successfully
/// assert_eq!(filter.lookups().len(), 2);
/// assert_eq!(filter.ordering().len(), 1);
/// ```
pub struct QueryFilter<M: Model> {
	lookups: Vec<Lookup<M>>,
	or_groups: Vec<Vec<Lookup<M>>>, // Each inner Vec is OR'd together, outer Vec is AND'd
	ordering: Vec<OrderingField<M>>,
	_phantom: PhantomData<M>,
}

impl<M: Model> QueryFilter<M> {
	/// Create a new empty filter
	pub fn new() -> Self {
		Self {
			lookups: Vec::new(),
			or_groups: Vec::new(),
			ordering: Vec::new(),
			_phantom: PhantomData,
		}
	}

	/// Add a lookup condition
	///
	/// All conditions are combined with AND.
	///
	/// # Examples
	///
	/// ```rust
	/// # use reinhardt_rest::filters::QueryFilter;
	/// # use reinhardt_db::orm::{Field, FieldSelector, Model};
	/// # #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
	/// # struct Post {
	/// #     id: i64,
	/// #     title: String,
	/// #     age: i32,
	/// # }
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { Some(self.id) }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = value; }
	/// # }
	/// let filter = QueryFilter::<Post>::new()
	///     .with_lookup(Field::new(vec!["title"]).icontains("rust"))
	///     .with_lookup(Field::new(vec!["age"]).gte(18));
	/// assert_eq!(filter.lookups().len(), 2);
	/// ```
	pub fn with_lookup(mut self, lookup: Lookup<M>) -> Self {
		self.lookups.push(lookup);
		self
	}

	/// Add multiple lookups at once
	pub fn add_all(mut self, lookups: Vec<Lookup<M>>) -> Self {
		self.lookups.extend(lookups);
		self
	}

	/// Add an ordering field
	///
	/// Multiple ordering fields are applied in the order they are added.
	///
	/// # Examples
	///
	/// ```rust
	/// # use reinhardt_rest::filters::QueryFilter;
	/// # use reinhardt_db::orm::{Field, FieldSelector, Model};
	/// # use reinhardt_rest::filters::field_extensions::FieldOrderingExt;
	/// # #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
	/// # struct Post {
	/// #     id: i64,
	/// #     title: String,
	/// #     created_at: String,
	/// # }
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { Some(self.id) }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = value; }
	/// # }
	/// let filter = QueryFilter::<Post>::new()
	///     .order_by(Field::<Post, String>::new(vec!["created_at"]).desc())
	///     .order_by(Field::<Post, String>::new(vec!["title"]).asc());
	/// assert_eq!(filter.ordering().len(), 2);
	/// ```
	pub fn order_by(mut self, field: OrderingField<M>) -> Self {
		self.ordering.push(field);
		self
	}

	/// Add multiple ordering fields at once
	pub fn order_by_all(mut self, fields: Vec<OrderingField<M>>) -> Self {
		self.ordering.extend(fields);
		self
	}

	/// Add an OR group (lookups within the group are OR'd together)
	///
	/// # Examples
	///
	/// ```rust
	/// # use reinhardt_rest::filters::QueryFilter;
	/// # use reinhardt_db::orm::{Field, FieldSelector, Model};
	/// # #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
	/// # struct Post {
	/// #     id: i64,
	/// #     title: String,
	/// #     content: String,
	/// # }
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { Some(self.id) }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = value; }
	/// # }
	/// // (title ICONTAINS 'rust' OR content ICONTAINS 'rust')
	/// let filter = QueryFilter::<Post>::new()
	///     .add_or_group(vec![
	///         Field::new(vec!["title"]).icontains("rust"),
	///         Field::new(vec!["content"]).icontains("rust"),
	///     ]);
	/// assert_eq!(filter.or_groups().len(), 1);
	/// assert_eq!(filter.or_groups()[0].len(), 2);
	/// ```
	pub fn add_or_group(mut self, lookups: Vec<Lookup<M>>) -> Self {
		if !lookups.is_empty() {
			self.or_groups.push(lookups);
		}
		self
	}

	/// Add multiple OR groups from multi-term search
	///
	/// Each term becomes an OR group, and all groups are AND'd together.
	///
	/// # Examples
	///
	/// ```rust
	/// # use reinhardt_rest::filters::QueryFilter;
	/// # use reinhardt_db::orm::{Field, FieldSelector, Model};
	/// # #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
	/// # struct Post {
	/// #     id: i64,
	/// #     title: String,
	/// # }
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { Some(self.id) }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = value; }
	/// # }
	/// // Simulate multi-term search
	/// let term_lookups = vec![
	///     vec![Field::new(vec!["title"]).icontains("rust")],
	///     vec![Field::new(vec!["title"]).icontains("web")],
	/// ];
	/// let filter = QueryFilter::<Post>::new()
	///     .add_multi_term(term_lookups);
	/// assert_eq!(filter.or_groups().len(), 2);
	/// ```
	pub fn add_multi_term(mut self, term_lookups: Vec<Vec<Lookup<M>>>) -> Self {
		for lookups in term_lookups {
			if !lookups.is_empty() {
				self.or_groups.push(lookups);
			}
		}
		self
	}

	/// Get the list of lookups
	pub fn lookups(&self) -> &[Lookup<M>] {
		&self.lookups
	}

	/// Get the list of OR groups
	pub fn or_groups(&self) -> &[Vec<Lookup<M>>] {
		&self.or_groups
	}

	/// Get the list of ordering fields
	pub fn ordering(&self) -> &[OrderingField<M>] {
		&self.ordering
	}

	/// Compile lookups to SQL WHERE clause using reinhardt-query
	fn compile_where_clause(&self) -> Option<String> {
		if self.lookups.is_empty() && self.or_groups.is_empty() {
			return None;
		}

		// Build the main AND condition using reinhardt-query
		let mut main_cond = Cond::all();

		// Add regular AND conditions
		for lookup in &self.lookups {
			main_cond = main_cond.add(QueryFieldCompiler::compile_to_expr(lookup));
		}

		// Add OR groups (each group is AND'd with others)
		for or_group in &self.or_groups {
			if or_group.is_empty() {
				continue;
			}

			if or_group.len() == 1 {
				// Single condition, add directly
				main_cond = main_cond.add(QueryFieldCompiler::compile_to_expr(&or_group[0]));
			} else {
				// Multiple conditions, create OR group
				let mut or_cond = Cond::any();
				for lookup in or_group {
					or_cond = or_cond.add(QueryFieldCompiler::compile_to_expr(lookup));
				}
				main_cond = main_cond.add(or_cond);
			}
		}

		// Build a dummy query to extract just the WHERE clause
		let query = Query::select()
			.expr(Expr::val(1))
			.cond_where(main_cond)
			.to_string(MySqlQueryBuilder);

		// Extract just the WHERE portion (after "WHERE ")
		query.find("WHERE ").map(|idx| query[idx + 6..].to_string())
	}

	/// Compile ordering to SQL ORDER BY clause
	fn compile_order_clause(&self) -> Option<String> {
		if self.ordering.is_empty() {
			return None;
		}

		let order_parts: Vec<String> = self.ordering.iter().map(|field| field.to_sql()).collect();

		Some(order_parts.join(", "))
	}

	/// Append order clause to existing ORDER BY
	///
	/// Supports:
	/// - Appending to existing ORDER BY clause
	/// - Merging multiple ordering fields
	///
	/// # Examples
	///
	/// ```ignore
	/// // Input: "SELECT * FROM posts ORDER BY created_at DESC"
	/// // New ordering: "title ASC"
	/// // Output: "SELECT * FROM posts ORDER BY created_at DESC, title ASC"
	/// ```
	fn append_order_clause(&self, sql: &str, new_order: &str) -> String {
		// Find ORDER BY position
		if let Some(order_by_pos) = sql.find("ORDER BY") {
			let before_order = &sql[..order_by_pos + 8]; // Include "ORDER BY"
			let after_order = &sql[order_by_pos + 8..];

			// Find end of ORDER BY clause (before LIMIT, OFFSET, or end of string)
			let end_markers = ["LIMIT", "OFFSET", ";"];
			let mut end_pos = after_order.len();

			for marker in &end_markers {
				if let Some(pos) = after_order.find(marker) {
					end_pos = end_pos.min(pos);
				}
			}

			let existing_order = after_order[..end_pos].trim();
			let remaining = &after_order[end_pos..];

			// Merge existing and new order clauses
			// Ensure space before remaining clause if it exists
			if remaining.is_empty() {
				format!("{} {}, {}", before_order, existing_order, new_order)
			} else {
				format!(
					"{} {}, {} {}",
					before_order,
					existing_order,
					new_order,
					remaining.trim()
				)
			}
		} else {
			// No ORDER BY found, append it
			format!("{} ORDER BY {}", sql, new_order)
		}
	}
}

impl<M: Model> Default for QueryFilter<M> {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl<M: Model> FilterBackend for QueryFilter<M> {
	async fn filter_queryset(
		&self,
		_params: &HashMap<String, String>,
		mut sql: String,
	) -> FilterResult<String> {
		// Add WHERE clause if we have lookups
		if let Some(where_clause) = self.compile_where_clause() {
			sql = if sql.contains("WHERE") {
				// Already has WHERE, add with AND
				sql.replace("WHERE", &format!("WHERE {} AND", where_clause))
			} else {
				// No WHERE yet, add it
				format!("{} WHERE {}", sql, where_clause)
			};
		}

		// Add ORDER BY clause if we have ordering
		if let Some(order_clause) = self.compile_order_clause() {
			if sql.contains("ORDER BY") {
				// Already has ORDER BY, append to it
				sql = self.append_order_clause(&sql, &order_clause);
			} else {
				sql = format!("{} ORDER BY {}", sql, order_clause);
			}
		}

		Ok(sql)
	}
}
