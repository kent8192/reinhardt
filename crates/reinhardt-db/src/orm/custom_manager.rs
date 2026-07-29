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
//! [`CustomManager`] also exposes hook methods that default to a no-op
//! and that custom implementations can override:
//!
//! - [`CustomManager::before_save`] — invoked before `create`/`update`
//! - [`CustomManager::before_upsert_write`] — invoked before a typed upsert write
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
//!             return Err(reinhardt_core::exception::DatabaseError::new(
//!                 reinhardt_core::exception::DatabaseErrorKind::Query,
//!                 "username must not be empty",
//!             )
//!             .into());
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
//! ## Upsert Hook
//!
//! [`CustomManager::before_upsert_write`] is separate from
//! [`CustomManager::before_save`]. It can mutate pending typed create values or
//! the locked model that will be updated:
//!
//! ```no_run
//! # #![allow(unexpected_cfgs)]
//! # mod migrations { pub use reinhardt_db::migrations::*; }
//! # mod orm { pub use reinhardt_db::orm::*; }
//! use reinhardt_core::exception::Result;
//! use reinhardt_core::macros::model;
//! use reinhardt_db::orm::custom_manager::CustomManager;
//! use reinhardt_db::orm::upsert::UpsertWrite;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Default)]
//! struct AccountManager;
//!
//! impl CustomManager for AccountManager {
//!     type Model = Account;
//!
//!     fn new() -> Self {
//!         Self
//!     }
//!
//!     fn before_upsert_write(
//!         &self,
//!         write: &mut UpsertWrite<'_, Account>,
//!     ) -> Result<()> {
//!         match write {
//!             UpsertWrite::Create(create) => {
//!                 create.set(Account::field_normalized_name(), "normalized")?;
//!             }
//!             UpsertWrite::Update(account) => {
//!                 account.normalized_name.make_ascii_lowercase();
//!             }
//!         }
//!         Ok(())
//!     }
//! }
//!
//! #[model(
//!     app_label = "custom_manager_docs",
//!     table_name = "custom_manager_accounts",
//!     manager = AccountManager
//! )]
//! #[derive(Serialize, Deserialize)]
//! struct Account {
//!     #[field(primary_key = true)]
//!     id: Option<i64>,
//!     #[field(max_length = 64, unique = true)]
//!     name: String,
//!     #[field(max_length = 64)]
//!     normalized_name: String,
//! }
//! # fn main() {}
//! ```
//!
//! [`crate::orm::upsert::UpsertCreate::set`] cannot replace lookup fields. The
//! update view is a normal mutable model and may change writable lookup fields;
//! persistence still identifies the locked row with its old primary key. A
//! call to [`crate::orm::upsert::UpsertCreate::get`] returns `None` when a
//! pending value is absent, even if the database will later supply a default.
//! Hooks must not perform external side effects: a create hook may run before
//! a concurrent insert race is lost, and either branch may later be rolled
//! back.
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

use reinhardt_query::InsertStatement;

use super::annotation::Annotation;
use super::composite_pk::PkValue;
use super::connection::{DatabaseBackend, OrmExecutor};
use super::cte::CTE;
use super::manager::Manager;
use super::model::Model;
use super::query::{QueryFilterInput, QuerySet, RelationLoadInput};
use super::upsert::{GetOrCreateBuilder, UpdateOrCreateBuilder, UpsertWrite};

/// The result of an insert whose database write and model hydration are separate.
///
/// MySQL does not support `RETURNING`, so an insert succeeds before Reinhardt
/// reloads the stored row. Consumers that need retry-safe semantics can use
/// this outcome to avoid repeating an insert after that reload fails.
pub enum CreateWithConnOutcome<M> {
	/// The row was inserted and hydrated successfully.
	Created(M),
	/// The insert did not complete.
	FailedBeforeInsert(reinhardt_core::exception::Error),
	/// The insert completed, but reloading the stored row failed.
	FailedAfterInsert(reinhardt_core::exception::Error),
}

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
/// Hook methods allow custom implementations to validate, mutate, or veto
/// operations before they reach the database. The default implementations are
/// no-ops returning `Ok(())`.
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
	/// Accepts typed and untyped filter inputs. See [`Manager::filter`] for the
	/// recommended fluent builder form (`Model::field_x().eq(value)`) and
	/// composite conditions.
	fn filter(&self, filter: impl QueryFilterInput<Self::Model>) -> QuerySet<Self::Model> {
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
	fn select_related<I>(&self, fields: I) -> QuerySet<Self::Model>
	where
		I: RelationLoadInput<Self::Model>,
	{
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
	fn prefetch_related<I>(&self, fields: I) -> QuerySet<Self::Model>
	where
		I: RelationLoadInput<Self::Model>,
	{
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
	fn filter_in_subquery<R: Model, F>(
		&self,
		field: &str,
		subquery_fn: F,
	) -> reinhardt_core::exception::Result<QuerySet<Self::Model>>
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
	) -> reinhardt_core::exception::Result<QuerySet<Self::Model>>
	where
		F: FnOnce(QuerySet<R>) -> QuerySet<R>,
	{
		Manager::<Self::Model>::new().filter_not_in_subquery(field, subquery_fn)
	}

	/// Filter using a correlated `EXISTS (...)` sub-query.
	fn filter_exists<R: Model, F>(
		&self,
		subquery_fn: F,
	) -> reinhardt_core::exception::Result<QuerySet<Self::Model>>
	where
		F: FnOnce(QuerySet<R>) -> QuerySet<R>,
	{
		Manager::<Self::Model>::new().filter_exists(subquery_fn)
	}

	/// Filter using a correlated `NOT EXISTS (...)` sub-query.
	fn filter_not_exists<R: Model, F>(
		&self,
		subquery_fn: F,
	) -> reinhardt_core::exception::Result<QuerySet<Self::Model>>
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
	fn annotate_subquery<R, F>(
		&self,
		name: &str,
		builder: F,
	) -> reinhardt_core::exception::Result<QuerySet<Self::Model>>
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
	fn create_with_conn<'a, E>(
		&'a self,
		conn: &'a mut E,
		model: &'a Self::Model,
	) -> impl Future<Output = reinhardt_core::exception::Result<Self::Model>> + Send + 'a
	where
		E: OrmExecutor + ?Sized + 'a,
	{
		async move {
			let mut model = model.clone();
			self.before_save(&mut model)?;
			Manager::<Self::Model>::new()
				.create_with_conn(conn, &model)
				.await
		}
	}

	/// Inserts a new record and reports whether a failure occurred after the write.
	///
	/// Custom managers that perform a multi-step insert should override this
	/// method when they can distinguish a failed write from a failed hydration.
	fn create_with_conn_outcome<'a, E>(
		&'a self,
		conn: &'a mut E,
		model: &'a Self::Model,
	) -> impl Future<Output = CreateWithConnOutcome<Self::Model>> + Send + 'a
	where
		E: OrmExecutor + ?Sized + 'a,
	{
		async move {
			let mut model = model.clone();
			if let Err(error) = self.before_save(&mut model) {
				return CreateWithConnOutcome::FailedBeforeInsert(error);
			}
			Manager::<Self::Model>::new()
				.create_with_conn_outcome(conn, &model)
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
	fn update_with_conn<'a, E>(
		&'a self,
		conn: &'a mut E,
		model: &'a Self::Model,
	) -> impl Future<Output = reinhardt_core::exception::Result<Self::Model>> + Send + 'a
	where
		E: OrmExecutor + ?Sized + 'a,
	{
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
			let mut conn = super::manager::get_connection().await?;
			self.delete_with_conn(&mut conn, pk).await
		}
	}

	/// Delete a record by primary key using an explicit connection.
	fn delete_with_conn<'a, E>(
		&'a self,
		conn: &'a mut E,
		pk: <Self::Model as Model>::PrimaryKey,
	) -> impl Future<Output = reinhardt_core::exception::Result<()>> + Send + 'a
	where
		E: OrmExecutor + 'a,
	{
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
	fn count_with_conn<'a, E>(
		&'a self,
		conn: &'a mut E,
	) -> impl Future<Output = reinhardt_core::exception::Result<i64>> + Send + 'a
	where
		E: OrmExecutor + 'a,
	{
		async move { Manager::<Self::Model>::new().count_with_conn(conn).await }
	}

	/// Starts a typed get-or-create operation.
	///
	/// The lookup must cover supported immediate uniqueness. Call
	/// [`GetOrCreateBuilder::execute_with`] to use a caller-owned
	/// [`OrmExecutor`].
	fn get_or_create(self) -> GetOrCreateBuilder<Self> {
		GetOrCreateBuilder::new(self)
	}

	/// Starts a typed update-or-create operation.
	///
	/// Caller-owned execution requires a
	/// [`super::transaction::AtomicTransaction`] created by
	/// [`super::connection::DatabaseConnection::atomic_write`].
	fn update_or_create(self) -> UpdateOrCreateBuilder<Self> {
		UpdateOrCreateBuilder::new(self)
	}

	/// Bulk-insert multiple records (Django: `bulk_create`).
	///
	/// The default implementation returns an empty result before acquiring the
	/// global connection when `models` is empty.
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
			if models.is_empty() {
				return Ok(Vec::new());
			}

			let mut conn = super::manager::get_connection().await?;
			self.bulk_create_with_conn(
				&mut conn,
				models,
				batch_size,
				ignore_conflicts,
				update_conflicts,
			)
			.await
		}
	}

	/// Bulk-insert multiple records through a caller-owned executor.
	fn bulk_create_with_conn<'a, E>(
		&'a self,
		conn: &'a mut E,
		models: Vec<Self::Model>,
		batch_size: Option<usize>,
		ignore_conflicts: bool,
		update_conflicts: bool,
	) -> impl Future<Output = reinhardt_core::exception::Result<Vec<Self::Model>>> + Send + 'a
	where
		E: OrmExecutor + 'a,
		Self::Model: 'a,
	{
		async move {
			Manager::<Self::Model>::new()
				.bulk_create_with_conn(conn, models, batch_size, ignore_conflicts, update_conflicts)
				.await
		}
	}

	/// Bulk-update multiple records (Django: `bulk_update`).
	///
	/// The default implementation skips empty input and runs [`Self::before_bulk_update`] before
	/// acquiring the global connection. It then executes through [`Manager::bulk_update_with_conn`]
	/// with that connection, so a veto never opens a database connection and the hook runs exactly
	/// once.
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
			let mut conn = super::manager::get_connection().await?;
			Manager::<Self::Model>::new()
				.bulk_update_with_conn(&mut conn, models, fields, batch_size)
				.await
		}
	}

	/// Bulk-update multiple records through a caller-owned executor.
	///
	/// The default implementation skips empty input and runs [`Self::before_bulk_update`] exactly
	/// once before it executes through the supplied executor.
	fn bulk_update_with_conn<'a, E>(
		&'a self,
		conn: &'a mut E,
		models: Vec<Self::Model>,
		fields: Vec<String>,
		batch_size: Option<usize>,
	) -> impl Future<Output = reinhardt_core::exception::Result<usize>> + Send + 'a
	where
		E: OrmExecutor + 'a,
		Self::Model: 'a,
	{
		async move {
			if models.is_empty() || fields.is_empty() {
				return Ok(0);
			}

			let mut models = models;
			self.before_bulk_update(&mut models)?;
			Manager::<Self::Model>::new()
				.bulk_update_with_conn(conn, models, fields, batch_size)
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
	) -> reinhardt_core::exception::Result<(String, Vec<String>)> {
		Manager::<Self::Model>::new().update_queryset(queryset, updates)
	}

	/// Build the `DELETE` SQL for a `QuerySet`.
	fn delete_queryset(
		&self,
		queryset: &QuerySet<Self::Model>,
	) -> reinhardt_core::exception::Result<(String, Vec<String>)> {
		Manager::<Self::Model>::new().delete_queryset(queryset)
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
	// Hooks — default to no-op
	// =========================================================================

	/// Hook invoked before a `create` or `update`. Returning `Err(_)` vetoes
	/// the write.
	fn before_save(&self, _model: &mut Self::Model) -> reinhardt_core::exception::Result<()> {
		Ok(())
	}

	/// Hook invoked immediately before an upsert write.
	///
	/// The hook can mutate typed create values, mutate an existing model, or
	/// veto the write. [`crate::orm::upsert::UpsertCreate::set`] cannot replace
	/// lookup fields. By contrast, the update view is a normal mutable model
	/// and may change any writable field, including lookup fields; the update
	/// predicate uses the locked model's old primary key. A missing pending
	/// create value reads as `None`, including a value that a database default
	/// would later supply.
	///
	/// The hook may run for an insert attempt that loses a concurrent race or
	/// for a transaction that later rolls back, so external side effects are
	/// unsupported. This hook is separate from [`Self::before_save`].
	fn before_upsert_write(
		&self,
		_write: &mut UpsertWrite<'_, Self::Model>,
	) -> reinhardt_core::exception::Result<()> {
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

	fn create_with_conn_outcome<'a, E>(
		&'a self,
		conn: &'a mut E,
		model: &'a Self::Model,
	) -> impl Future<Output = CreateWithConnOutcome<Self::Model>> + Send + 'a
	where
		E: OrmExecutor + ?Sized + 'a,
	{
		Manager::create_with_conn_outcome(self, conn, model)
	}
}
