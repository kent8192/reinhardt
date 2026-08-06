//! Custom Object Manager support for the Reinhardt ORM.
//!
//! This module provides the [`CustomManager`] trait, enabling Django-style
//! customizable object managers.
//!
//! Issue: <https://github.com/kent8192/reinhardt-web/issues/3980>
//! Unified access: <https://github.com/kent8192/reinhardt-web/issues/3984>
//!
//! # Design
//!
//! The `Model` trait has an associated type `Objects` that determines which
//! manager `Model::objects()` returns. By default (when no custom manager is
//! specified), `type Objects = Manager<Self>`. When a custom manager is
//! configured via `#[model(manager = MyManager)]`, the macro sets
//! `type Objects = MyManager`, so `Model::objects()` returns the custom
//! manager directly.
//!
//! All default implementations delegate to the existing inherent methods on
//! [`Manager<M>`], so the runtime semantics of the standard operations are
//! preserved exactly. The blanket `impl<M: Model> CustomManager for Manager<M>`
//! ensures that the existing manager continues to satisfy the trait, allowing
//! generic functions to accept any compatible manager.
//!
//! # Hooks
//!
//! [`CustomManager`] also exposes three hook methods that default to a no-op
//! and that custom implementations can override:
//!
//! - [`CustomManager::before_save`] — invoked before `create`/`update`
//! - [`CustomManager::before_delete`] — invoked before `delete`
//! - [`CustomManager::before_bulk_update`] — invoked before `bulk_update`
//!
//! Returning `Err(_)` from any hook vetoes the operation, mirroring the event
//! veto behavior already present on `Model::save`/`Model::delete`.
//!
//! # Quick Start
//!
//! Define a custom manager with `Default`, implement [`CustomManager`], and
//! use the `#[model(manager = ...)]` attribute:
//!
//! ```ignore
//! use reinhardt_db::orm::CustomManager;
//! use reinhardt_core::exception::Result;
//!
//! #[derive(Default)]
//! struct ActiveUserManager;
//!
//! impl CustomManager for ActiveUserManager {
//!     type Model = User;
//!
//!     fn new() -> Self { Self }
//!
//!     fn before_save(&self, user: &mut User) -> Result<()> {
//!         if user.username.is_empty() {
//!             return Err(reinhardt_core::exception::Error::Database(
//!                 "username must not be empty".into(),
//!             ));
//!         }
//!         Ok(())
//!     }
//! }
//!
//! #[reinhardt_macros::model(table_name = "users", manager = ActiveUserManager)]
//! struct User { /* ... */ }
//!
//! // objects() now returns ActiveUserManager directly
//! let manager = User::objects();
//! ```
//!
//! # Blanket Implementation
//!
//! The blanket `impl<M: Model> CustomManager for Manager<M>` makes every
//! existing manager — the value returned by `Model::objects()` — satisfy
//! [`CustomManager`] automatically. Generic code can therefore accept any
//! compatible manager:
//!
//! ```
//! use reinhardt_db::orm::custom_manager::CustomManager;
//! use reinhardt_db::orm::manager::Manager;
//! use reinhardt_db::orm::model::{FieldSelector, Model};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
//! struct Article { id: Option<i64>, title: String }
//!
//! #[derive(Clone)]
//! struct ArticleFields;
//! impl FieldSelector for ArticleFields {
//!     fn with_alias(self, _alias: &str) -> Self { self }
//! }
//!
//! impl Model for Article {
//!     type PrimaryKey = i64;
//!     type Fields = ArticleFields;
//!     type Objects = Manager<Self>;
//!     fn table_name() -> &'static str { "articles" }
//!     fn new_fields() -> Self::Fields { ArticleFields }
//!     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
//!     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
//! }
//!
//! // Generic helper accepting any `CustomManager` bound to `Article`.
//! fn count_filters<M: CustomManager<Model = Article>>(m: &M) -> usize {
//!     m.all().filters().len()
//! }
//!
//! // `Manager<Article>` satisfies the trait via blanket impl.
//! let m = Manager::<Article>::new();
//! assert_eq!(count_filters(&m), 0);
//! ```

use std::collections::HashMap;
use std::future::Future;

use reinhardt_query::{InsertStatement, SelectStatement};

use super::annotation::Annotation;
use super::composite_pk::PkValue;
use super::connection::{DatabaseBackend, DatabaseConnection};
use super::cte::CTE;
use super::manager::Manager;
use super::model::Model;
use super::query::{FilterCondition, QuerySet};

/// Trait that exposes the full surface area of an object manager and provides
/// extension hooks for custom behavior.
///
/// All builder methods have default implementations that delegate to the
/// canonical [`Manager<M>`] inherent methods, so implementing this trait only
/// requires defining `type Model` and the `new` constructor; every other
/// method may be left to the default to preserve standard behavior, or
/// overridden to inject custom logic.
///
/// # Hooks
///
/// Three hook methods (`before_save`, `before_delete`, `before_bulk_update`)
/// allow custom implementations to validate or veto operations before they
/// reach the database. The default implementations are no-ops returning
/// `Ok(())`.
///
/// # Bounds
///
/// `CustomManager: Sized + Send + Sync` so that managers can be safely
/// constructed via `Default::default()` and shared across asynchronous tasks
/// without additional bounds at every call site.
pub trait CustomManager: Sized + Send + Sync {
	/// The model this manager operates on.
	type Model: Model;

	/// Construct a fresh manager instance.
	///
	/// Custom managers that hold no runtime state can simply return `Self`,
	/// often via `#[derive(Default)]`.
	fn new() -> Self;

	// =========================================================================
	// QuerySet builders (28 methods) — default impls delegate to Manager<M>
	// =========================================================================

	/// Get all records (Django: `Model.objects.all()`).
	fn all(&self) -> QuerySet<Self::Model> {
		Manager::<Self::Model>::new().all()
	}

	/// Filter records by a typed filter expression.
	///
	/// Accepts any value convertible into [`FilterCondition`]. See
	/// [`Manager::filter`] for the recommended fluent builder form
	/// (`Model::field_x().eq(value)`) and composite conditions.
	fn filter(&self, filter: impl Into<FilterCondition>) -> QuerySet<Self::Model> {
		Manager::<Self::Model>::new().filter(filter)
	}

	/// Get a single record by primary key (returns a `QuerySet` for chaining).
	fn get(&self, pk: <Self::Model as Model>::PrimaryKey) -> QuerySet<Self::Model> {
		Manager::<Self::Model>::new().get(pk)
	}

	/// Set a `LIMIT` clause.
	fn limit(&self, limit: usize) -> QuerySet<Self::Model> {
		Manager::<Self::Model>::new().limit(limit)
	}

	/// Set an `ORDER BY` clause; prefix a field with `-` for descending order.
	fn order_by(&self, fields: &[&str]) -> QuerySet<Self::Model> {
		Manager::<Self::Model>::new().order_by(fields)
	}

	/// Add an annotation (computed field) to the query.
	fn annotate(&self, annotation: Annotation) -> QuerySet<Self::Model> {
		Manager::<Self::Model>::new().annotate(annotation)
	}

	/// Defer loading of the specified fields.
	fn defer(&self, fields: &[&str]) -> QuerySet<Self::Model> {
		Manager::<Self::Model>::new().defer(fields)
	}

	/// Restrict loading to only the specified fields.
	fn only(&self, fields: &[&str]) -> QuerySet<Self::Model> {
		Manager::<Self::Model>::new().only(fields)
	}

	/// Project only the specified fields as values.
	fn values(&self, fields: &[&str]) -> QuerySet<Self::Model> {
		Manager::<Self::Model>::new().values(fields)
	}

	/// Eager-load related objects via SQL `JOIN`.
	fn select_related(&self, fields: &[&str]) -> QuerySet<Self::Model> {
		Manager::<Self::Model>::new().select_related(fields)
	}

	/// Set an `OFFSET` clause.
	fn offset(&self, offset: usize) -> QuerySet<Self::Model> {
		Manager::<Self::Model>::new().offset(offset)
	}

	/// Paginate (1-indexed page, fixed page size).
	fn paginate(&self, page: usize, page_size: usize) -> QuerySet<Self::Model> {
		Manager::<Self::Model>::new().paginate(page, page_size)
	}

	/// Pre-fetch related objects in separate queries.
	fn prefetch_related(&self, fields: &[&str]) -> QuerySet<Self::Model> {
		Manager::<Self::Model>::new().prefetch_related(fields)
	}

	/// Project as tuples of values rather than full models.
	fn values_list(&self, fields: &[&str]) -> QuerySet<Self::Model> {
		Manager::<Self::Model>::new().values_list(fields)
	}

	/// PostgreSQL: filter by array overlap (`&&`).
	fn filter_array_overlap(&self, field: &str, values: &[&str]) -> QuerySet<Self::Model> {
		Manager::<Self::Model>::new().filter_array_overlap(field, values)
	}

	/// PostgreSQL: filter by array contains (`@>`).
	fn filter_array_contains(&self, field: &str, values: &[&str]) -> QuerySet<Self::Model> {
		Manager::<Self::Model>::new().filter_array_contains(field, values)
	}

	/// PostgreSQL: filter by JSONB contains (`@>`).
	fn filter_jsonb_contains(&self, field: &str, json: &str) -> QuerySet<Self::Model> {
		Manager::<Self::Model>::new().filter_jsonb_contains(field, json)
	}

	/// PostgreSQL: filter by JSONB key existence (`?`).
	fn filter_jsonb_key_exists(&self, field: &str, key: &str) -> QuerySet<Self::Model> {
		Manager::<Self::Model>::new().filter_jsonb_key_exists(field, key)
	}

	/// PostgreSQL: filter where a range column contains a value.
	fn filter_range_contains(&self, field: &str, value: &str) -> QuerySet<Self::Model> {
		Manager::<Self::Model>::new().filter_range_contains(field, value)
	}

	/// Filter where a field is `IN` the result of a sub-query.
	fn filter_in_subquery<R: Model, F>(&self, field: &str, subquery_fn: F) -> QuerySet<Self::Model>
	where
		F: FnOnce(QuerySet<R>) -> QuerySet<R>,
	{
		Manager::<Self::Model>::new().filter_in_subquery(field, subquery_fn)
	}

	/// Filter where a field is `NOT IN` the result of a sub-query.
	fn filter_not_in_subquery<R: Model, F>(
		&self,
		field: &str,
		subquery_fn: F,
	) -> QuerySet<Self::Model>
	where
		F: FnOnce(QuerySet<R>) -> QuerySet<R>,
	{
		Manager::<Self::Model>::new().filter_not_in_subquery(field, subquery_fn)
	}

	/// Filter using a correlated `EXISTS (...)` sub-query.
	fn filter_exists<R: Model, F>(&self, subquery_fn: F) -> QuerySet<Self::Model>
	where
		F: FnOnce(QuerySet<R>) -> QuerySet<R>,
	{
		Manager::<Self::Model>::new().filter_exists(subquery_fn)
	}

	/// Filter using a correlated `NOT EXISTS (...)` sub-query.
	fn filter_not_exists<R: Model, F>(&self, subquery_fn: F) -> QuerySet<Self::Model>
	where
		F: FnOnce(QuerySet<R>) -> QuerySet<R>,
	{
		Manager::<Self::Model>::new().filter_not_exists(subquery_fn)
	}

	/// Add a Common Table Expression (`WITH ...`).
	fn with_cte(&self, cte: CTE) -> QuerySet<Self::Model> {
		Manager::<Self::Model>::new().with_cte(cte)
	}

	/// PostgreSQL: full-text search using `to_tsvector` / `to_tsquery`.
	fn full_text_search(&self, field: &str, query: &str) -> QuerySet<Self::Model> {
		Manager::<Self::Model>::new().full_text_search(field, query)
	}

	/// Annotate using a sub-query expression.
	fn annotate_subquery<R, F>(&self, name: &str, builder: F) -> QuerySet<Self::Model>
	where
		R: Model + 'static,
		F: FnOnce(QuerySet<R>) -> QuerySet<R>,
	{
		Manager::<Self::Model>::new().annotate_subquery(name, builder)
	}

	// =========================================================================
	// Async CRUD (8 methods) — default impls delegate to Manager<M>
	// =========================================================================

	/// Fetch a single record by composite primary key.
	fn get_composite<'a>(
		&'a self,
		pk_values: &'a HashMap<String, PkValue>,
	) -> impl Future<Output = reinhardt_core::exception::Result<Self::Model>> + Send + 'a
	where
		Self::Model: Clone + serde::de::DeserializeOwned,
	{
		async move { Manager::<Self::Model>::new().get_composite(pk_values).await }
	}

	/// Insert a new record.
	fn create<'a>(
		&'a self,
		model: &'a Self::Model,
	) -> impl Future<Output = reinhardt_core::exception::Result<Self::Model>> + Send + 'a {
		async move {
			let mut model = model.clone();
			self.before_save(&mut model)?;
			Manager::<Self::Model>::new().create(&model).await
		}
	}

	/// Insert a new record using an explicit connection (for transactions).
	fn create_with_conn<'a>(
		&'a self,
		conn: &'a DatabaseConnection,
		model: &'a Self::Model,
	) -> impl Future<Output = reinhardt_core::exception::Result<Self::Model>> + Send + 'a {
		async move {
			let mut model = model.clone();
			self.before_save(&mut model)?;
			Manager::<Self::Model>::new()
				.create_with_conn(conn, &model)
				.await
		}
	}

	/// Update an existing record (must have a primary key set).
	fn update<'a>(
		&'a self,
		model: &'a Self::Model,
	) -> impl Future<Output = reinhardt_core::exception::Result<Self::Model>> + Send + 'a {
		async move {
			let mut model = model.clone();
			self.before_save(&mut model)?;
			Manager::<Self::Model>::new().update(&model).await
		}
	}

	/// Update an existing record using an explicit connection.
	fn update_with_conn<'a>(
		&'a self,
		conn: &'a DatabaseConnection,
		model: &'a Self::Model,
	) -> impl Future<Output = reinhardt_core::exception::Result<Self::Model>> + Send + 'a {
		async move {
			let mut model = model.clone();
			self.before_save(&mut model)?;
			Manager::<Self::Model>::new()
				.update_with_conn(conn, &model)
				.await
		}
	}

	/// Delete a record by primary key.
	fn delete<'a>(
		&'a self,
		pk: <Self::Model as Model>::PrimaryKey,
	) -> impl Future<Output = reinhardt_core::exception::Result<()>> + Send + 'a {
		async move {
			let conn = super::manager::get_connection().await?;
			self.delete_with_conn(&conn, pk).await
		}
	}

	/// Delete a record by primary key using an explicit connection.
	fn delete_with_conn<'a>(
		&'a self,
		conn: &'a DatabaseConnection,
		pk: <Self::Model as Model>::PrimaryKey,
	) -> impl Future<Output = reinhardt_core::exception::Result<()>> + Send + 'a {
		async move {
			let manager = Manager::<Self::Model>::new();
			if let Some(model) = manager.get(pk.clone()).first_with_db(conn).await? {
				self.before_delete(&model)?;
			}
			manager.delete_with_conn(conn, pk).await
		}
	}

	/// Count records.
	fn count<'a>(
		&'a self,
	) -> impl Future<Output = reinhardt_core::exception::Result<i64>> + Send + 'a {
		async move { Manager::<Self::Model>::new().count().await }
	}

	/// Count records using an explicit connection.
	fn count_with_conn<'a>(
		&'a self,
		conn: &'a DatabaseConnection,
	) -> impl Future<Output = reinhardt_core::exception::Result<i64>> + Send + 'a {
		async move { Manager::<Self::Model>::new().count_with_conn(conn).await }
	}

	/// Retrieve a record matching `lookup_fields`, or insert with `defaults`.
	fn get_or_create<'a>(
		&'a self,
		lookup_fields: HashMap<String, String>,
		defaults: Option<HashMap<String, String>>,
	) -> impl Future<Output = reinhardt_core::exception::Result<(Self::Model, bool)>> + Send + 'a {
		async move {
			Manager::<Self::Model>::new()
				.get_or_create(lookup_fields, defaults)
				.await
		}
	}

	/// Bulk-insert multiple records (Django: `bulk_create`).
	fn bulk_create<'a>(
		&'a self,
		models: Vec<Self::Model>,
		batch_size: Option<usize>,
		ignore_conflicts: bool,
		update_conflicts: bool,
	) -> impl Future<Output = reinhardt_core::exception::Result<Vec<Self::Model>>> + Send + 'a
	where
		Self::Model: 'a,
	{
		async move {
			Manager::<Self::Model>::new()
				.bulk_create(models, batch_size, ignore_conflicts, update_conflicts)
				.await
		}
	}

	/// Bulk-update multiple records (Django: `bulk_update`).
	fn bulk_update<'a>(
		&'a self,
		models: Vec<Self::Model>,
		fields: Vec<String>,
		batch_size: Option<usize>,
	) -> impl Future<Output = reinhardt_core::exception::Result<usize>> + Send + 'a
	where
		Self::Model: 'a,
	{
		async move {
			if models.is_empty() || fields.is_empty() {
				return Ok(0);
			}

			let mut models = models;
			self.before_bulk_update(&mut models)?;
			Manager::<Self::Model>::new()
				.bulk_update(models, fields, batch_size)
				.await
		}
	}

	// =========================================================================
	// SQL builder utilities (8 methods) — default impls delegate to Manager<M>
	// =========================================================================

	/// Build the `INSERT` statement for a bulk-create call.
	fn bulk_create_query(&self, models: &[Self::Model]) -> Option<InsertStatement> {
		Manager::<Self::Model>::new().bulk_create_query(models)
	}

	/// Render the bulk-create SQL for a backend.
	fn bulk_create_sql(&self, models: &[Self::Model], backend: DatabaseBackend) -> String {
		Manager::<Self::Model>::new().bulk_create_sql(models, backend)
	}

	/// Build the `UPDATE` SQL for a `QuerySet`.
	fn update_queryset(
		&self,
		queryset: &QuerySet<Self::Model>,
		updates: &[(&str, &str)],
	) -> (String, Vec<String>) {
		Manager::<Self::Model>::new().update_queryset(queryset, updates)
	}

	/// Build the `DELETE` SQL for a `QuerySet`.
	fn delete_queryset(&self, queryset: &QuerySet<Self::Model>) -> (String, Vec<String>) {
		Manager::<Self::Model>::new().delete_queryset(queryset)
	}

	/// Build the `(SELECT, INSERT)` statement pair used by `get_or_create`.
	fn get_or_create_queries(
		&self,
		lookup_fields: &HashMap<String, String>,
		defaults: &HashMap<String, String>,
	) -> (SelectStatement, InsertStatement) {
		Manager::<Self::Model>::new().get_or_create_queries(lookup_fields, defaults)
	}

	/// Build the SQL strings used by `get_or_create`.
	fn get_or_create_sql(
		&self,
		lookup_fields: &HashMap<String, String>,
		defaults: &HashMap<String, String>,
		backend: DatabaseBackend,
	) -> (String, String) {
		Manager::<Self::Model>::new().get_or_create_sql(lookup_fields, defaults, backend)
	}

	/// Build the bulk-create SQL given pre-extracted `field_names` and rows.
	fn bulk_create_sql_detailed(
		&self,
		field_names: &[String],
		value_rows: &[Vec<serde_json::Value>],
		ignore_conflicts: bool,
	) -> String {
		Manager::<Self::Model>::new().bulk_create_sql_detailed(
			field_names,
			value_rows,
			ignore_conflicts,
		)
	}

	/// Build the bulk-update SQL using `CASE` expressions.
	///
	/// The `(PrimaryKey, HashMap<String, Value>)` slice mirrors the shape used
	/// by [`Manager::bulk_update_sql_detailed`]; routing it through an
	/// associated-type projection trips `clippy::type_complexity`, which we
	/// silence here because the signature is fixed by the underlying inherent
	/// method we delegate to.
	#[allow(clippy::type_complexity)]
	fn bulk_update_sql_detailed(
		&self,
		updates: &[(
			<Self::Model as Model>::PrimaryKey,
			HashMap<String, serde_json::Value>,
		)],
		fields: &[String],
		backend: DatabaseBackend,
	) -> String
	where
		<Self::Model as Model>::PrimaryKey: std::fmt::Display + Clone,
	{
		Manager::<Self::Model>::new().bulk_update_sql_detailed(updates, fields, backend)
	}

	// =========================================================================
	// Hooks (3 methods) — default to no-op
	// =========================================================================

	/// Hook invoked before a `create` or `update`. Returning `Err(_)` vetoes
	/// the write.
	fn before_save(&self, _model: &mut Self::Model) -> reinhardt_core::exception::Result<()> {
		Ok(())
	}

	/// Hook invoked before a `delete`. Returning `Err(_)` vetoes the delete.
	fn before_delete(&self, _model: &Self::Model) -> reinhardt_core::exception::Result<()> {
		Ok(())
	}

	/// Hook invoked before a `bulk_update`. Returning `Err(_)` vetoes the
	/// entire batch; mutating `models` in place lets implementations rewrite
	/// records before the update is built.
	fn before_bulk_update(
		&self,
		_models: &mut [Self::Model],
	) -> reinhardt_core::exception::Result<()> {
		Ok(())
	}
}

/// Blanket implementation: every existing [`Manager<M>`] is also a
/// [`CustomManager`].
///
/// This means functions generic over `CustomManager<Model = M>` can accept the
/// vanilla manager that `Model::objects()` returns today, preserving full
/// backward compatibility. Custom implementations can be substituted in via
/// the `#[model(manager = MyManager)]` attribute.
impl<M: Model> CustomManager for Manager<M> {
	type Model = M;

	fn new() -> Self {
		Manager::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::orm::fields::{CharField, Field};
	use crate::orm::inspection::FieldInfo;
	use crate::orm::model::FieldSelector;
	use crate::orm::query::{Filter, FilterOperator, FilterValue};
	use serde::{Deserialize, Serialize};
	use std::collections::HashMap;

	#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
	struct Article {
		id: Option<i64>,
		title: String,
	}

	#[derive(Clone, Debug)]
	struct ArticleFields;

	impl FieldSelector for ArticleFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for Article {
		type PrimaryKey = i64;
		type Fields = ArticleFields;
		type Objects = ArticleManager;

		fn table_name() -> &'static str {
			"articles"
		}

		fn new_fields() -> Self::Fields {
			ArticleFields
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn field_metadata() -> Vec<FieldInfo> {
			let mut id = CharField::new(20);
			id.set_attributes_from_name("id");
			let mut title = CharField::new(255);
			title.set_attributes_from_name("title");
			vec![FieldInfo::from_field(&id), FieldInfo::from_field(&title)]
		}
	}

	#[derive(Default)]
	struct ArticleManager;

	impl CustomManager for ArticleManager {
		type Model = Article;

		fn new() -> Self {
			Self
		}
	}

	#[derive(Default)]
	struct VetoArticleManager;

	impl CustomManager for VetoArticleManager {
		type Model = Article;

		fn new() -> Self {
			Self
		}

		fn before_save(&self, _model: &mut Article) -> reinhardt_core::exception::Result<()> {
			Err(reinhardt_core::exception::Error::Database(
				"save vetoed".to_string(),
			))
		}

		fn before_delete(&self, _model: &Article) -> reinhardt_core::exception::Result<()> {
			Err(reinhardt_core::exception::Error::Database(
				"delete vetoed".to_string(),
			))
		}

		fn before_bulk_update(
			&self,
			_models: &mut [Article],
		) -> reinhardt_core::exception::Result<()> {
			Err(reinhardt_core::exception::Error::Database(
				"bulk update vetoed".to_string(),
			))
		}
	}

	#[test]
	fn custom_manager_get_preserves_the_primary_key_filter() {
		let query = ArticleManager::new().get(42);
		assert_eq!(query.filters().len(), 1);
		assert!(matches!(query.filters()[0].value, FilterValue::Integer(42)));
	}

	#[test]
	fn custom_manager_builder_delegation_preserves_query_output() {
		let manager = ArticleManager::new();
		let filter = manager.filter(Filter::new(
			"title",
			FilterOperator::Eq,
			FilterValue::String("Rust".to_string()),
		));

		assert_eq!(manager.all().to_sql(), "SELECT * FROM \"articles\"");
		assert_eq!(filter.filters().len(), 1);
		assert_eq!(
			filter.to_sql(),
			"SELECT * FROM \"articles\" WHERE \"title\" = 'Rust'"
		);
		assert_eq!(
			manager.limit(3).to_sql(),
			"SELECT * FROM \"articles\" LIMIT 3"
		);
		assert_eq!(
			manager.offset(2).to_sql(),
			"SELECT * FROM \"articles\" OFFSET 2"
		);
		assert_eq!(
			manager.paginate(2, 5).to_sql(),
			"SELECT * FROM \"articles\" LIMIT 5 OFFSET 5"
		);
		assert_eq!(
			manager.order_by(&["-title", "id"]).to_sql(),
			"SELECT * FROM \"articles\" ORDER BY \"title\" DESC, \"id\" ASC"
		);
		assert_eq!(
			manager.defer(&["title"]).to_sql(),
			"SELECT \"id\" FROM \"articles\""
		);
		assert_eq!(
			manager.only(&["id", "title"]).to_sql(),
			"SELECT \"id\", \"title\" FROM \"articles\""
		);
		assert_eq!(
			manager.values(&["title"]).to_sql(),
			"SELECT \"title\" FROM \"articles\""
		);
		assert_eq!(
			manager.values_list(&["id", "title"]).to_sql(),
			"SELECT \"id\", \"title\" FROM \"articles\""
		);
		assert_eq!(
			manager
				.filter_array_overlap("tags", &["rust", "orm"])
				.to_sql(),
			"SELECT * FROM \"articles\" WHERE \"tags\" && ARRAY['rust', 'orm']"
		);
		assert_eq!(
			manager
				.filter_array_contains("tags", &["rust", "orm"])
				.to_sql(),
			"SELECT * FROM \"articles\" WHERE \"tags\" @> ARRAY['rust', 'orm']"
		);
		assert_eq!(
			manager
				.filter_jsonb_contains("metadata", r#"{"published":true}"#)
				.to_sql(),
			"SELECT * FROM \"articles\" WHERE \"metadata\" @> '{\"published\":true}'::jsonb"
		);
		assert_eq!(
			manager
				.filter_jsonb_key_exists("metadata", "published")
				.to_sql(),
			"SELECT * FROM \"articles\" WHERE \"metadata\" ? 'published'"
		);
		assert_eq!(
			manager
				.filter_range_contains("published_range", "2026-08-06")
				.to_sql(),
			"SELECT * FROM \"articles\" WHERE \"published_range\" @> '2026-08-06'"
		);
		assert_eq!(
			manager
				.filter_in_subquery::<Article, _>("id", |query| query.only(&["id"]))
				.to_sql(),
			"SELECT * FROM \"articles\" WHERE \"id\" IN (SELECT \"id\" FROM \"articles\")"
		);
		assert_eq!(
			manager
				.filter_not_in_subquery::<Article, _>("id", |query| query.only(&["id"]))
				.to_sql(),
			"SELECT * FROM \"articles\" WHERE \"id\" NOT IN (SELECT \"id\" FROM \"articles\")"
		);
		assert_eq!(
			manager
				.filter_exists::<Article, _>(|query| query.filter(Filter::new(
					"title",
					FilterOperator::Eq,
					FilterValue::String("Rust".to_string()),
				)))
				.to_sql(),
			"SELECT * FROM \"articles\" WHERE EXISTS (SELECT * FROM \"articles\" WHERE \"title\" = 'Rust')"
		);
		assert_eq!(
			manager
				.filter_not_exists::<Article, _>(|query| query.filter(Filter::new(
					"title",
					FilterOperator::Eq,
					FilterValue::String("Rust".to_string()),
				)))
				.to_sql(),
			"SELECT * FROM \"articles\" WHERE NOT EXISTS (SELECT * FROM \"articles\" WHERE \"title\" = 'Rust')"
		);
		assert_eq!(
			manager
				.with_cte(CTE::new("published_articles", "SELECT id FROM articles"))
				.to_sql(),
			"WITH published_articles AS (SELECT id FROM articles) SELECT * FROM \"articles\""
		);
		assert_eq!(
			manager.full_text_search("title", "rust orm").to_sql(),
			"SELECT * FROM \"articles\" WHERE \"title\" @@ plainto_tsquery('english', 'rust orm')"
		);
	}

	#[test]
	fn custom_manager_sql_utilities_preserve_complete_statements() {
		let manager = ArticleManager::new();
		let queryset = manager.filter(Filter::new(
			"title",
			FilterOperator::Eq,
			FilterValue::String("Rust".to_string()),
		));
		let mut lookup_fields = HashMap::new();
		lookup_fields.insert("title".to_string(), "Rust".to_string());
		let mut defaults = HashMap::new();
		defaults.insert("title".to_string(), "Rust ORM".to_string());

		assert_eq!(
			manager.update_queryset(&queryset, &[("title", "Rust ORM")]),
			(
				"UPDATE \"articles\" SET \"title\" = $1 WHERE \"title\" = $2".to_string(),
				vec!["Rust ORM".to_string(), "Rust".to_string()],
			)
		);
		assert_eq!(
			manager.delete_queryset(&queryset),
			(
				"DELETE FROM \"articles\" WHERE \"title\" = $1".to_string(),
				vec!["Rust".to_string()],
			)
		);
		assert_eq!(
			manager.get_or_create_sql(&lookup_fields, &defaults, DatabaseBackend::Sqlite),
			(
				"SELECT * FROM \"articles\" WHERE \"title\" = ?".to_string(),
				"INSERT INTO \"articles\" (\"title\") VALUES (?)".to_string(),
			)
		);
	}

	#[test]
	fn custom_manager_default_hooks_are_noops() {
		let manager = ArticleManager::new();
		let mut article = Article {
			id: Some(1),
			title: "unchanged".to_string(),
		};

		assert!(manager.before_save(&mut article).is_ok());
		assert!(manager.before_delete(&article).is_ok());
		assert!(
			manager
				.before_bulk_update(std::slice::from_mut(&mut article))
				.is_ok()
		);
	}

	#[tokio::test]
	async fn custom_manager_vetoes_create_before_writing_to_sqlite() {
		let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
			.await
			.expect("in-memory SQLite connection should be available");
		connection
			.execute(
				"CREATE TABLE articles (id INTEGER PRIMARY KEY, title TEXT NOT NULL)",
				Vec::new(),
			)
			.await
			.expect("articles table should be created");
		let article = Article {
			id: None,
			title: "blocked".to_string(),
		};

		let result = VetoArticleManager::new()
			.create_with_conn(&connection, &article)
			.await;

		assert!(matches!(
			result,
			Err(reinhardt_core::exception::Error::Database(message)) if message == "save vetoed"
		));
		let row = connection
			.query_one("SELECT COUNT(*) AS count FROM articles", Vec::new())
			.await
			.expect("article count should be queryable");
		assert_eq!(row.get::<i64>("count"), Some(0));
	}

	#[tokio::test]
	async fn custom_manager_vetoes_delete_without_removing_the_sqlite_row() {
		let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
			.await
			.expect("in-memory SQLite connection should be available");
		connection
			.execute(
				"CREATE TABLE articles (id INTEGER PRIMARY KEY, title TEXT NOT NULL)",
				Vec::new(),
			)
			.await
			.expect("articles table should be created");
		connection
			.execute(
				"INSERT INTO articles (id, title) VALUES (1, 'retained')",
				Vec::new(),
			)
			.await
			.expect("article should be inserted");

		let result = VetoArticleManager::new()
			.delete_with_conn(&connection, 1)
			.await;

		assert!(matches!(
			result,
			Err(reinhardt_core::exception::Error::Database(message)) if message == "delete vetoed"
		));
		let row = connection
			.query_one("SELECT COUNT(*) AS count FROM articles", Vec::new())
			.await
			.expect("article count should be queryable");
		assert_eq!(row.get::<i64>("count"), Some(1));
	}

	#[tokio::test]
	async fn custom_manager_vetoes_bulk_update_before_global_database_lookup() {
		let articles = vec![Article {
			id: Some(1),
			title: "blocked".to_string(),
		}];

		let result = VetoArticleManager::new()
			.bulk_update(articles, vec!["title".to_string()], None)
			.await;

		assert!(matches!(
			result,
			Err(reinhardt_core::exception::Error::Database(message)) if message == "bulk update vetoed"
		));
	}
}
