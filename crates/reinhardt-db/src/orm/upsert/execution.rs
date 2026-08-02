use crate::orm::connection::{DatabaseBackend, OrmExecutor, Row};
use crate::orm::custom_manager::CustomManager;
use crate::orm::field_codec::{DatabaseValue, FieldCodecError};
use crate::orm::manager::decode_model_row;
use crate::orm::model::Model;
use crate::orm::transaction::AtomicTransaction;
use crate::orm::upsert::assignment::{
	TypedAssignment, UpsertCreate, UpsertWrite, validate_writable_create_assignments,
};
use crate::orm::upsert::plan::UpsertPlan;
use crate::orm::upsert::sql;
use reinhardt_core::exception::{DatabaseErrorKind, Error, Result};
use std::collections::{BTreeMap, HashSet};

pub(crate) async fn execute_get_or_create<C, E>(
	manager: &C,
	mut plan: UpsertPlan<C::Model>,
	executor: &mut E,
) -> Result<(C::Model, bool)>
where
	C: CustomManager,
	E: OrmExecutor + ?Sized,
{
	if !executor.supports_get_or_create_race_recovery() {
		return Err(crate::backends::DatabaseError::new(
			DatabaseErrorKind::Transaction,
			"get_or_create requires an autocommit connection or write-intent atomic transaction",
		)
		.into());
	}
	let backend = executor.backend();
	let select = sql::select_by_lookup(&plan, backend, false)?;
	let rows = executor.fetch_all(&select.sql, select.params).await?;
	if let Some(model) = decode_lookup_rows(rows)? {
		return Ok((model, false));
	}

	manager.before_upsert_write(&mut UpsertWrite::Create(UpsertCreate {
		lookup: &plan.lookup,
		values: &mut plan.create,
	}))?;
	validate_writable_create_assignments::<C::Model>(&plan.create)?;

	let insert = sql::insert(&plan, backend)?;
	if matches!(backend, DatabaseBackend::Postgres | DatabaseBackend::Sqlite) {
		let insert_rows = if backend == DatabaseBackend::Postgres {
			executor
				.fetch_all_in_savepoint(&insert.sql, insert.params)
				.await?
		} else {
			executor.fetch_all(&insert.sql, insert.params).await?
		};
		return match decode_lookup_rows(insert_rows)? {
			Some(model) => Ok((model, true)),
			None => reload_lookup(&plan, executor, false, None).await,
		};
	}

	match executor.execute(&insert.sql, insert.params).await {
		Ok(result) => {
			let created = match result.rows_affected {
				1 => true,
				_ => {
					return Err(Error::Conflict(format!(
						"get_or_create INSERT affected {} rows for {backend:?}; expected one",
						result.rows_affected
					)));
				}
			};
			if backend == DatabaseBackend::MySql {
				if let Some(last_insert_id) = result.last_insert_id {
					return reload_generated_mysql_primary_key::<C::Model, _>(
						last_insert_id,
						executor,
					)
					.await
					.map(|model| (model, created));
				}
			}
			reload_lookup(&plan, executor, created, None).await
		}
		Err(error)
			if backend == DatabaseBackend::MySql
				&& error.database_kind() == Some(DatabaseErrorKind::UniqueViolation) =>
		{
			reload_lookup(&plan, executor, false, Some(error)).await
		}
		Err(error) => Err(error),
	}
}

async fn reload_generated_mysql_primary_key<M, E>(
	last_insert_id: u64,
	executor: &mut E,
) -> Result<M>
where
	M: Model,
	E: OrmExecutor + ?Sized,
{
	let generated_primary_key = i64::try_from(last_insert_id).map_err(|_| {
		Error::Conflict(
			"MySQL generated primary key exceeds the supported integer range".to_owned(),
		)
	})?;
	if generated_primary_key <= 0 {
		return Err(Error::Conflict(
			"MySQL INSERT completed without a generated primary key".to_owned(),
		));
	}
	let select = sql::select_by_generated_mysql_primary_key::<M>(generated_primary_key)?;
	let rows = executor.fetch_all(&select.sql, select.params).await?;
	decode_lookup_rows(rows)?.ok_or_else(|| {
		Error::Conflict(
			"MySQL INSERT completed without a row matching its generated primary key".to_owned(),
		)
	})
}

async fn reload_lookup<M, E>(
	plan: &UpsertPlan<M>,
	executor: &mut E,
	created: bool,
	original_race_error: Option<Error>,
) -> Result<(M, bool)>
where
	M: crate::orm::model::Model,
	E: OrmExecutor + ?Sized,
{
	let select = sql::select_by_lookup(plan, executor.backend(), false)?;
	let rows = executor.fetch_all(&select.sql, select.params).await?;
	match decode_lookup_rows(rows)? {
		Some(model) => Ok((model, created)),
		None => match original_race_error {
			Some(error) => Err(error),
			None => Err(Error::Conflict(
				"get_or_create write completed without exactly one row matching the full lookup"
					.to_owned(),
			)),
		},
	}
}

fn decode_lookup_rows<M: crate::orm::model::Model>(rows: Vec<Row>) -> Result<Option<M>> {
	match rows.len() {
		0 => Ok(None),
		1 => rows.into_iter().next().map(decode_model_row).transpose(),
		count => Err(Error::Conflict(format!(
			"get_or_create full lookup matched {count} rows; expected at most one"
		))),
	}
}

pub(crate) async fn execute_update_or_create<C>(
	manager: &C,
	mut plan: UpsertPlan<C::Model>,
	transaction: &mut AtomicTransaction,
) -> Result<(C::Model, bool)>
where
	C: CustomManager,
{
	if !transaction.has_write_intent() {
		return Err(crate::backends::DatabaseError::new(
			DatabaseErrorKind::Transaction,
			"update_or_create requires a write-intent atomic transaction",
		)
		.into());
	}
	if let Some(locked) = load_locked(&plan, transaction).await? {
		return update_locked(manager, &plan, locked, transaction).await;
	}

	manager.before_upsert_write(&mut UpsertWrite::Create(UpsertCreate {
		lookup: &plan.lookup,
		values: &mut plan.create,
	}))?;
	validate_writable_create_assignments::<C::Model>(&plan.create)?;
	let backend = transaction.backend();
	let insert = sql::insert(&plan, backend)?;
	if matches!(backend, DatabaseBackend::Postgres | DatabaseBackend::Sqlite) {
		let insert_rows = if backend == DatabaseBackend::Postgres {
			fetch_postgres_insert_in_savepoint(transaction, &insert).await
		} else {
			transaction.fetch_all(&insert.sql, insert.params).await
		};
		return resolve_returning_insert(manager, &plan, transaction, insert_rows).await;
	}

	match transaction.execute(&insert.sql, insert.params).await {
		Ok(result) => {
			let created = match result.rows_affected {
				1 => true,
				_ => {
					return Err(Error::Conflict(format!(
						"update_or_create INSERT affected {} rows for {backend:?}; expected one",
						result.rows_affected
					)));
				}
			};
			let Some(model) = load_locked(&plan, transaction).await? else {
				return Err(Error::Conflict(
					"update_or_create write completed without exactly one row matching the full lookup"
						.to_owned(),
				));
			};
			if created {
				Ok((model, true))
			} else {
				update_locked(manager, &plan, model, transaction).await
			}
		}
		Err(error)
			if matches!(backend, DatabaseBackend::MySql | DatabaseBackend::Postgres)
				&& error.database_kind() == Some(DatabaseErrorKind::UniqueViolation) =>
		{
			match load_locked(&plan, transaction).await? {
				Some(model) => update_locked(manager, &plan, model, transaction).await,
				None => Err(error),
			}
		}
		Err(error) => Err(error),
	}
}

async fn resolve_returning_insert<C>(
	manager: &C,
	plan: &UpsertPlan<C::Model>,
	transaction: &mut AtomicTransaction,
	insert_rows: Result<Vec<Row>>,
) -> Result<(C::Model, bool)>
where
	C: CustomManager,
{
	match insert_rows {
		Ok(rows) => match decode_update_lookup_rows(rows)? {
			Some(model) => Ok((model, true)),
			None => match load_locked(plan, transaction).await? {
				Some(model) => update_locked(manager, plan, model, transaction).await,
				None => Err(Error::Conflict(
					"update_or_create INSERT completed without a returned row or a race winner"
						.to_owned(),
				)),
			},
		},
		Err(error)
			if transaction.backend() == DatabaseBackend::Postgres
				&& error.database_kind() == Some(DatabaseErrorKind::UniqueViolation) =>
		{
			match load_locked(plan, transaction).await? {
				Some(model) => update_locked(manager, plan, model, transaction).await,
				None => Err(error),
			}
		}
		Err(error) => Err(error),
	}
}

async fn fetch_postgres_insert_in_savepoint(
	transaction: &mut AtomicTransaction,
	insert: &sql::BoundSql,
) -> Result<Vec<Row>> {
	let sql = insert.sql.clone();
	let params = insert.params.clone();
	transaction
		.atomic(async move |savepoint| savepoint.fetch_all(&sql, params).await)
		.await
}

async fn load_locked<M: Model>(
	plan: &UpsertPlan<M>,
	transaction: &mut AtomicTransaction,
) -> Result<Option<M>> {
	let select = sql::select_by_lookup(plan, transaction.backend(), true)?;
	let rows = transaction.fetch_all(&select.sql, select.params).await?;
	decode_update_lookup_rows(rows)
}

fn decode_update_lookup_rows<M: Model>(rows: Vec<Row>) -> Result<Option<M>> {
	match rows.len() {
		0 => Ok(None),
		1 => rows.into_iter().next().map(decode_model_row).transpose(),
		count => Err(Error::Conflict(format!(
			"update_or_create full lookup matched {count} rows; expected at most one"
		))),
	}
}

async fn update_locked<C>(
	manager: &C,
	plan: &UpsertPlan<C::Model>,
	locked: C::Model,
	transaction: &mut AtomicTransaction,
) -> Result<(C::Model, bool)>
where
	C: CustomManager,
{
	let (mut candidate, original) = build_update_candidate(&locked, &plan.update)?;
	manager.before_upsert_write(&mut UpsertWrite::Update(&mut candidate))?;
	let final_values = candidate
		.encode_database_fields()
		.map_err(field_codec_error)?;
	validate_writable_update_fields::<C::Model>(&original, &final_values)?;
	let generated = C::Model::generated_field_names();
	let values = C::Model::field_metadata()
		.into_iter()
		.filter(|field| {
			field.editable
				&& !generated.iter().any(|generated| {
					*generated == field.name || *generated == field.db_column_name()
				}) && original.get(&field.name) != final_values.get(&field.name)
		})
		.filter_map(|field| {
			final_values
				.get(&field.name)
				.cloned()
				.map(|value| (field.name, value))
		})
		.collect::<BTreeMap<_, _>>();
	if values.is_empty() {
		return Ok((candidate, false));
	}
	let update = sql::update_values_by_primary_key(&locked, &values, transaction.backend())?;
	let result = transaction.execute(&update.sql, update.params).await?;
	if result.rows_affected != 1
		&& !(transaction.backend() == DatabaseBackend::MySql && result.rows_affected == 0)
	{
		return Err(Error::Conflict(format!(
			"update_or_create UPDATE affected {} rows; expected one",
			result.rows_affected
		)));
	}
	let select = sql::select_by_primary_key(&candidate, transaction.backend(), true)?;
	let rows = transaction.fetch_all(&select.sql, select.params).await?;
	let Some(reloaded) = decode_update_lookup_rows(rows)? else {
		return Err(Error::Conflict(
			"update_or_create UPDATE completed without exactly one row matching its final primary key"
				.to_owned(),
		));
	};
	Ok((reloaded, false))
}

fn validate_writable_update_fields<M: Model>(
	original: &BTreeMap<String, DatabaseValue>,
	updated: &BTreeMap<String, DatabaseValue>,
) -> Result<()> {
	let generated = M::generated_field_names();
	for field in M::field_metadata() {
		let generated_field = generated
			.iter()
			.any(|generated| *generated == field.name || *generated == field.db_column_name());
		if (!field.editable || generated_field)
			&& original.get(&field.name) != updated.get(&field.name)
		{
			let reason = if generated_field {
				"database-generated"
			} else {
				"not writable"
			};
			return Err(Error::Validation(format!(
				"before_upsert_write cannot modify {reason} field `{}` during an update",
				field.name
			)));
		}
	}
	Ok(())
}

fn build_update_candidate<M: Model>(
	locked: &M,
	set: &[TypedAssignment<M>],
) -> Result<(M, BTreeMap<String, DatabaseValue>)> {
	let original = locked.encode_database_fields().map_err(field_codec_error)?;
	let mut requested = original.clone();
	for assignment in set {
		requested.insert(assignment.logical_name.to_owned(), assignment.value.clone());
	}
	let metadata = M::field_metadata();
	let mut row = serde_json::Map::new();
	let mut native_json_fields = HashSet::new();
	let mut json_null_fields = HashSet::new();
	for field in &metadata {
		let Some(value) = requested.get(&field.name).cloned() else {
			continue;
		};
		let column_name = field.db_column_name().to_owned();
		if matches!(value, DatabaseValue::Json(_)) {
			native_json_fields.insert(column_name.clone());
			if matches!(&value, DatabaseValue::Json(json) if json.is_null()) {
				json_null_fields.insert(column_name.clone());
			}
		}
		let decoded = M::decode_database_field(&field.name, value).map_err(field_codec_error)?;
		row.insert(column_name, decoded);
	}
	let candidate = crate::orm::json::deserialize_model_row::<M>(
		serde_json::Value::Object(row),
		json_null_fields,
		native_json_fields,
	)
	.map_err(field_codec_error)?;
	Ok((candidate, original))
}

fn field_codec_error(error: FieldCodecError) -> Error {
	let kind = match &error {
		FieldCodecError::TypeMismatch { .. } | FieldCodecError::InvalidEnumValue { .. } => {
			DatabaseErrorKind::Type
		}
		FieldCodecError::Serialization(_) => DatabaseErrorKind::Serialization,
	};
	Error::database_with_source(
		kind,
		format!("typed upsert field codec failed: {error}"),
		error,
	)
}

#[cfg(test)]
mod tests {
	use super::{build_update_candidate, execute_get_or_create, execute_update_or_create};
	use crate::backends::error::{DatabaseError, DatabaseErrorKind};
	use crate::backends::types::{DatabaseType, QueryResult, QueryValue, Row, TransactionExecutor};
	use crate::orm::composite_pk::CompositePrimaryKey;
	#[cfg(feature = "sqlite")]
	use crate::orm::connection::{BackendsConnection, DatabaseConnectionLease};
	use crate::orm::custom_manager::CustomManager;
	use crate::orm::expressions::{FieldRef, GeneratedModelField};
	use crate::orm::field_codec::{DatabaseStorageKind, DatabaseValue, FieldCodecError};
	use crate::orm::inspection::FieldInfo;
	use crate::orm::json::Json;
	use crate::orm::manager::Manager;
	use crate::orm::model::{FieldSelector, Model};
	use crate::orm::transaction::AtomicTransaction;
	use crate::orm::upsert::assignment::TypedAssignment;
	use crate::orm::upsert::assignment::UpsertWrite;
	use crate::orm::upsert::plan::{UpsertMode, normalize};
	use async_trait::async_trait;
	use reinhardt_core::exception::Error;
	use rstest::rstest;
	use serde::{Deserialize, Serialize};
	use std::collections::{BTreeMap, HashMap, VecDeque};
	use std::sync::{Arc, Mutex};
	#[cfg(feature = "sqlite")]
	use std::time::Duration;
	#[cfg(feature = "sqlite")]
	use tokio::sync::Notify;

	#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
	struct Article {
		id: Option<i64>,
		slug: String,
		rank: i32,
		headline: String,
		computed: i32,
		readonly: String,
	}

	#[derive(Clone)]
	struct ArticleFields;

	impl FieldSelector for ArticleFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for Article {
		type PrimaryKey = i64;
		type Fields = ArticleFields;
		type Objects = Manager<Self>;

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
			vec![
				field("id", true, false),
				field("slug", false, true),
				field("rank", false, false),
				field("headline", false, false),
				field("computed", false, false),
				noneditable_field("readonly"),
			]
		}

		fn generated_field_names() -> &'static [&'static str] {
			&["computed"]
		}
	}

	impl Article {
		fn id_field() -> FieldRef<Self, Option<i64>, GeneratedModelField> {
			// SAFETY: the logical and physical names match Article's nullable primary key.
			unsafe {
				FieldRef::<Self, _, GeneratedModelField>::from_generated_model_field_with_names(
					"id", "id",
				)
			}
		}

		fn slug_field() -> FieldRef<Self, String, GeneratedModelField> {
			// SAFETY: the logical and physical names match Article's string field.
			unsafe {
				FieldRef::<Self, _, GeneratedModelField>::from_generated_model_field_with_names(
					"slug", "slug",
				)
			}
		}

		fn rank_field() -> FieldRef<Self, i32, GeneratedModelField> {
			// SAFETY: the logical and physical names match Article's i32 field.
			unsafe {
				FieldRef::<Self, _, GeneratedModelField>::from_generated_model_field_with_names(
					"rank", "rank",
				)
			}
		}

		fn headline_field() -> FieldRef<Self, String, GeneratedModelField> {
			// SAFETY: the logical and physical names match Article's string field.
			unsafe {
				FieldRef::<Self, _, GeneratedModelField>::from_generated_model_field_with_names(
					"headline", "headline",
				)
			}
		}

		fn computed_field() -> FieldRef<Self, i32, GeneratedModelField> {
			// SAFETY: the logical and physical names match Article's generated i32 field.
			unsafe {
				FieldRef::<Self, _, GeneratedModelField>::from_generated_model_field_with_names(
					"computed", "computed",
				)
			}
		}

		fn readonly_field() -> FieldRef<Self, String, GeneratedModelField> {
			// SAFETY: the logical and physical names match Article's noneditable string field.
			unsafe {
				FieldRef::<Self, _, GeneratedModelField>::from_generated_model_field_with_names(
					"readonly", "readonly",
				)
			}
		}
	}

	#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
	struct JsonArticle {
		id: Option<i64>,
		rank: i32,
		json_null: Option<Json<serde_json::Value>>,
		sql_null: Option<Json<serde_json::Value>>,
	}

	impl Model for JsonArticle {
		type PrimaryKey = i64;
		type Fields = ArticleFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"json_articles"
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

		fn field_is_none(&self, field_name: &str) -> bool {
			match field_name {
				"id" => self.id.is_none(),
				"json_null" => self.json_null.is_none(),
				"sql_null" => self.sql_null.is_none(),
				_ => false,
			}
		}

		fn encode_database_fields(
			&self,
		) -> std::result::Result<BTreeMap<String, DatabaseValue>, FieldCodecError> {
			Ok(BTreeMap::from([
				(
					"id".to_owned(),
					self.id.map_or(DatabaseValue::Null, DatabaseValue::I64),
				),
				("rank".to_owned(), DatabaseValue::I32(self.rank)),
				(
					"json_null".to_owned(),
					self.json_null
						.as_ref()
						.map_or(DatabaseValue::Null, |value| {
							DatabaseValue::Json(value.as_inner().clone())
						}),
				),
				(
					"sql_null".to_owned(),
					self.sql_null.as_ref().map_or(DatabaseValue::Null, |value| {
						DatabaseValue::Json(value.as_inner().clone())
					}),
				),
			]))
		}

		fn field_metadata() -> Vec<FieldInfo> {
			vec![
				field("id", true, false),
				field("rank", false, false),
				json_field("json_null"),
				json_field("sql_null"),
			]
		}
	}

	impl JsonArticle {
		fn rank_field() -> FieldRef<Self, i32, GeneratedModelField> {
			// SAFETY: the logical and physical names match JsonArticle's i32 field.
			unsafe {
				FieldRef::<Self, _, GeneratedModelField>::from_generated_model_field_with_names(
					"rank", "rank",
				)
			}
		}
	}

	#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
	struct CompositeArticle {
		tenant_id: i64,
		article_id: i64,
		slug: String,
		rank: i32,
	}

	impl Model for CompositeArticle {
		type PrimaryKey = String;
		type Fields = ArticleFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"composite_articles"
		}

		fn new_fields() -> Self::Fields {
			ArticleFields
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			None
		}

		fn set_primary_key(&mut self, _value: Self::PrimaryKey) {}

		fn composite_primary_key() -> Option<CompositePrimaryKey> {
			CompositePrimaryKey::new(vec!["tenant_id".to_owned(), "article_id".to_owned()]).ok()
		}

		fn field_metadata() -> Vec<FieldInfo> {
			vec![
				field_with_column("tenant_id", "tenant_key", true, false),
				field_with_column("article_id", "article_key", true, false),
				field("slug", false, true),
				field("rank", false, false),
			]
		}
	}

	impl CompositeArticle {
		fn slug_field() -> FieldRef<Self, String, GeneratedModelField> {
			// SAFETY: the logical and physical names match CompositeArticle's string field.
			unsafe {
				FieldRef::<Self, _, GeneratedModelField>::from_generated_model_field_with_names(
					"slug", "slug",
				)
			}
		}

		fn rank_field() -> FieldRef<Self, i32, GeneratedModelField> {
			// SAFETY: the logical and physical names match CompositeArticle's i32 field.
			unsafe {
				FieldRef::<Self, _, GeneratedModelField>::from_generated_model_field_with_names(
					"rank", "rank",
				)
			}
		}
	}

	fn field(name: &str, primary_key: bool, unique: bool) -> FieldInfo {
		FieldInfo {
			name: name.to_owned(),
			field_type: "test".to_owned(),
			nullable: false,
			primary_key,
			unique,
			blank: false,
			editable: true,
			storage_kind: None,
			domain: None,
			default: None,
			db_default: None,
			db_column: None,
			choices: None,
			attributes: HashMap::new(),
		}
	}

	fn noneditable_field(name: &str) -> FieldInfo {
		let mut field = field(name, false, false);
		field.editable = false;
		field
	}

	fn field_with_column(name: &str, column: &str, primary_key: bool, unique: bool) -> FieldInfo {
		let mut field = field(name, primary_key, unique);
		field.db_column = Some(column.to_owned());
		field
	}

	fn json_field(name: &str) -> FieldInfo {
		let mut field = field(name, false, false);
		field.field_type = "JsonField".to_owned();
		field.nullable = true;
		field.storage_kind = Some(DatabaseStorageKind::Json);
		field
	}

	fn article_row(id: i64, slug: &str, rank: i32, headline: &str, computed: i32) -> Row {
		Row {
			data: HashMap::from([
				("id".to_owned(), QueryValue::Int(id)),
				("slug".to_owned(), QueryValue::String(slug.to_owned())),
				("rank".to_owned(), QueryValue::Int(i64::from(rank))),
				(
					"headline".to_owned(),
					QueryValue::String(headline.to_owned()),
				),
				("computed".to_owned(), QueryValue::Int(i64::from(computed))),
				(
					"readonly".to_owned(),
					QueryValue::String("fixed".to_owned()),
				),
			]),
		}
	}

	fn composite_article_row(tenant_id: i64, article_id: i64, slug: &str, rank: i32) -> Row {
		Row {
			data: HashMap::from([
				("tenant_key".to_owned(), QueryValue::Int(tenant_id)),
				("article_key".to_owned(), QueryValue::Int(article_id)),
				("slug".to_owned(), QueryValue::String(slug.to_owned())),
				("rank".to_owned(), QueryValue::Int(i64::from(rank))),
			]),
		}
	}

	#[derive(Clone, Debug, PartialEq)]
	struct Call {
		operation: &'static str,
		sql: String,
		params: Vec<QueryValue>,
	}

	#[derive(Default)]
	struct State {
		calls: Vec<Call>,
		execute_results: VecDeque<crate::backends::error::Result<QueryResult>>,
		fetch_results: VecDeque<crate::backends::error::Result<Vec<Row>>>,
	}

	struct Recorder {
		state: Arc<Mutex<State>>,
		backend: DatabaseType,
	}

	impl Recorder {
		fn transaction(
			backend: DatabaseType,
			execute_results: Vec<crate::backends::error::Result<QueryResult>>,
			fetch_results: Vec<crate::backends::error::Result<Vec<Row>>>,
		) -> (AtomicTransaction, Arc<Mutex<State>>) {
			let state = Arc::new(Mutex::new(State {
				calls: Vec::new(),
				execute_results: execute_results.into(),
				fetch_results: fetch_results.into(),
			}));
			let transaction = AtomicTransaction::new_write(Box::new(Self {
				state: Arc::clone(&state),
				backend,
			}));
			(transaction, state)
		}

		fn ordinary_transaction(backend: DatabaseType) -> (AtomicTransaction, Arc<Mutex<State>>) {
			let state = Arc::new(Mutex::new(State::default()));
			let transaction = AtomicTransaction::new(Box::new(Self {
				state: Arc::clone(&state),
				backend,
			}));
			(transaction, state)
		}

		fn missing(operation: &str) -> Error {
			DatabaseError::new(
				DatabaseErrorKind::Query,
				format!("no queued {operation} result"),
			)
			.into()
		}
	}

	#[async_trait]
	impl TransactionExecutor for Recorder {
		fn backend(&self) -> DatabaseType {
			self.backend
		}

		async fn execute(
			&mut self,
			sql: &str,
			params: Vec<QueryValue>,
		) -> crate::backends::error::Result<QueryResult> {
			let mut state = self.state.lock().unwrap();
			state.calls.push(Call {
				operation: "execute",
				sql: sql.to_owned(),
				params,
			});
			state
				.execute_results
				.pop_front()
				.unwrap_or_else(|| Err(Self::missing("execute")))
		}

		async fn fetch_one(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> crate::backends::error::Result<Row> {
			Err(Self::missing("fetch_one"))
		}

		async fn fetch_all(
			&mut self,
			sql: &str,
			params: Vec<QueryValue>,
		) -> crate::backends::error::Result<Vec<Row>> {
			let mut state = self.state.lock().unwrap();
			state.calls.push(Call {
				operation: "fetch_all",
				sql: sql.to_owned(),
				params,
			});
			state
				.fetch_results
				.pop_front()
				.unwrap_or_else(|| Err(Self::missing("fetch_all")))
		}

		async fn fetch_optional(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> crate::backends::error::Result<Option<Row>> {
			Err(Self::missing("fetch_optional"))
		}

		async fn commit(self: Box<Self>) -> crate::backends::error::Result<()> {
			self.state.lock().unwrap().calls.push(Call {
				operation: "commit",
				sql: "COMMIT".to_owned(),
				params: Vec::new(),
			});
			Ok(())
		}

		async fn rollback(self: Box<Self>) -> crate::backends::error::Result<()> {
			self.state.lock().unwrap().calls.push(Call {
				operation: "rollback",
				sql: "ROLLBACK".to_owned(),
				params: Vec::new(),
			});
			Ok(())
		}

		async fn savepoint(&mut self, _name: &str) -> crate::backends::error::Result<()> {
			Ok(())
		}

		async fn release_savepoint(&mut self, _name: &str) -> crate::backends::error::Result<()> {
			Ok(())
		}

		async fn rollback_to_savepoint(
			&mut self,
			_name: &str,
		) -> crate::backends::error::Result<()> {
			Ok(())
		}
	}

	fn plan(rank: i32, create_headline: Option<&str>) -> super::UpsertPlan<Article> {
		let lookup =
			vec![TypedAssignment::new(Article::slug_field(), "rust").expect("encode lookup")];
		let update =
			vec![TypedAssignment::new(Article::rank_field(), rank).expect("encode update")];
		let create = create_headline
			.map(|headline| {
				vec![
					TypedAssignment::new(Article::headline_field(), headline)
						.expect("encode create default"),
				]
			})
			.unwrap_or_default();
		normalize(lookup, create, update, UpsertMode::UpdateOrCreate)
			.expect("normalize update-or-create plan")
	}

	fn get_plan() -> super::UpsertPlan<Article> {
		normalize(
			vec![TypedAssignment::new(Article::slug_field(), "rust").expect("encode lookup")],
			vec![TypedAssignment::new(Article::rank_field(), 1).expect("encode create default")],
			Vec::new(),
			UpsertMode::GetOrCreate,
		)
		.expect("normalize get-or-create plan")
	}

	fn composite_plan() -> super::UpsertPlan<CompositeArticle> {
		normalize(
			vec![
				TypedAssignment::new(CompositeArticle::slug_field(), "rust")
					.expect("encode composite lookup"),
			],
			Vec::new(),
			vec![
				TypedAssignment::new(CompositeArticle::rank_field(), 2)
					.expect("encode composite update"),
			],
			UpsertMode::UpdateOrCreate,
		)
		.expect("normalize composite update-or-create plan")
	}

	struct CompositePkHookManager;

	impl CustomManager for CompositePkHookManager {
		type Model = CompositeArticle;

		fn new() -> Self {
			Self
		}

		fn before_upsert_write(
			&self,
			write: &mut UpsertWrite<'_, CompositeArticle>,
		) -> reinhardt_core::exception::Result<()> {
			if let UpsertWrite::Update(article) = write {
				article.tenant_id = 8;
				article.article_id = 10;
			}
			Ok(())
		}
	}

	#[derive(Clone, Copy)]
	enum InvalidCreateField {
		Generated,
		Noneditable,
	}

	struct InvalidCreateHookManager {
		field: InvalidCreateField,
	}

	impl CustomManager for InvalidCreateHookManager {
		type Model = Article;

		fn new() -> Self {
			Self {
				field: InvalidCreateField::Generated,
			}
		}

		fn before_upsert_write(
			&self,
			write: &mut UpsertWrite<'_, Article>,
		) -> reinhardt_core::exception::Result<()> {
			if let UpsertWrite::Create(create) = write {
				match self.field {
					InvalidCreateField::Generated => {
						create.set(Article::computed_field(), 99)?;
					}
					InvalidCreateField::Noneditable => {
						create.set(Article::readonly_field(), "changed")?;
					}
				}
			}
			Ok(())
		}
	}

	struct InvalidUpdateHookManager {
		field: InvalidCreateField,
	}

	impl CustomManager for InvalidUpdateHookManager {
		type Model = Article;

		fn new() -> Self {
			Self {
				field: InvalidCreateField::Generated,
			}
		}

		fn before_upsert_write(
			&self,
			write: &mut UpsertWrite<'_, Article>,
		) -> reinhardt_core::exception::Result<()> {
			if let UpsertWrite::Update(article) = write {
				match self.field {
					InvalidCreateField::Generated => article.computed += 1,
					InvalidCreateField::Noneditable => article.readonly = "changed".to_owned(),
				}
			}
			Ok(())
		}
	}

	#[tokio::test]
	async fn get_or_create_rejects_invalid_create_hook_fields_before_write() {
		for (field, diagnostic) in [
			(InvalidCreateField::Generated, "database-generated"),
			(InvalidCreateField::Noneditable, "not writable"),
		] {
			let (mut transaction, state) =
				Recorder::transaction(DatabaseType::Postgres, Vec::new(), vec![Ok(Vec::new())]);
			let manager = InvalidCreateHookManager { field };

			let error = execute_get_or_create(&manager, get_plan(), &mut transaction)
				.await
				.expect_err("invalid create hook assignment must fail");

			assert!(error.to_string().contains(diagnostic));
			assert_eq!(
				state
					.lock()
					.unwrap()
					.calls
					.iter()
					.map(|call| call.operation)
					.collect::<Vec<_>>(),
				["fetch_all"]
			);
		}
	}

	#[tokio::test]
	async fn update_or_create_rejects_invalid_create_hook_fields_before_write() {
		for (field, diagnostic) in [
			(InvalidCreateField::Generated, "database-generated"),
			(InvalidCreateField::Noneditable, "not writable"),
		] {
			let (mut transaction, state) =
				Recorder::transaction(DatabaseType::Postgres, Vec::new(), vec![Ok(Vec::new())]);
			let manager = InvalidCreateHookManager { field };

			let error = execute_update_or_create(&manager, plan(2, None), &mut transaction)
				.await
				.expect_err("invalid update-or-create hook assignment must fail");

			assert!(error.to_string().contains(diagnostic));
			assert_eq!(
				state
					.lock()
					.unwrap()
					.calls
					.iter()
					.map(|call| call.operation)
					.collect::<Vec<_>>(),
				["fetch_all"]
			);
		}
	}

	#[tokio::test]
	async fn update_or_create_rejects_invalid_update_hook_fields_before_write() {
		for (field, diagnostic) in [
			(InvalidCreateField::Generated, "database-generated"),
			(InvalidCreateField::Noneditable, "not writable"),
		] {
			let (mut transaction, state) = Recorder::transaction(
				DatabaseType::Postgres,
				Vec::new(),
				vec![Ok(vec![article_row(1, "rust", 1, "old", 10)])],
			);
			let manager = InvalidUpdateHookManager { field };

			let error = execute_update_or_create(&manager, plan(2, None), &mut transaction)
				.await
				.expect_err("invalid update hook mutation must fail");

			assert!(error.to_string().contains(diagnostic));
			assert_eq!(
				state
					.lock()
					.unwrap()
					.calls
					.iter()
					.map(|call| call.operation)
					.collect::<Vec<_>>(),
				["fetch_all"]
			);
		}
	}

	#[tokio::test]
	async fn update_or_create_rejects_non_write_intent_transaction_before_sql() {
		let (mut transaction, state) = Recorder::ordinary_transaction(DatabaseType::Sqlite);

		let error = Manager::<Article>::new()
			.update_or_create()
			.lookup(Article::slug_field(), "rust")
			.set(Article::rank_field(), 2)
			.execute_with(&mut transaction)
			.await
			.expect_err("ordinary atomic transactions must not run update_or_create");

		assert_eq!(
			error.to_string(),
			"Database error: update_or_create requires a write-intent atomic transaction"
		);
		assert_eq!(state.lock().unwrap().calls, Vec::<Call>::new());
	}

	#[rstest]
	#[tokio::test]
	async fn get_or_create_rejects_non_write_intent_transaction_before_sql() {
		let (mut transaction, state) = Recorder::ordinary_transaction(DatabaseType::Mysql);

		let error = execute_get_or_create(&Manager::<Article>::new(), get_plan(), &mut transaction)
			.await
			.expect_err("ordinary transactions must not promise race recovery");

		assert_eq!(
			error.to_string(),
			"Database error: get_or_create requires an autocommit connection or write-intent atomic transaction"
		);
		assert!(state.lock().unwrap().calls.is_empty());
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn update_or_create_sqlite_execute_with_serializes_real_writers() {
		let directory = tempfile::tempdir().expect("create SQLite transaction test directory");
		let database_path = directory.path().join("update-or-create.sqlite3");
		let owner = BackendsConnection::connect_sqlite(
			database_path
				.to_str()
				.expect("temporary database path must be valid UTF-8"),
		)
		.await
		.expect("connect SQLite update-or-create database");
		let lease = DatabaseConnectionLease::register(owner).expect("register SQLite connection");
		let connection = lease.handle();
		connection
			.execute(
				"CREATE TABLE articles (id INTEGER PRIMARY KEY, slug TEXT NOT NULL UNIQUE, \
				 rank INTEGER NOT NULL, headline TEXT NOT NULL, computed INTEGER NOT NULL, \
				 readonly TEXT NOT NULL)",
				Vec::new(),
			)
			.await
			.expect("create update-or-create table");
		connection
			.execute(
				"INSERT INTO articles \
				 (id, slug, rank, headline, computed, readonly) VALUES (1, 'rust', 1, 'old', 10, 'fixed')",
				Vec::new(),
			)
			.await
			.expect("insert update-or-create fixture");

		let ordinary_error = connection
			.atomic(async |transaction| {
				Manager::<Article>::new()
					.update_or_create()
					.lookup(Article::slug_field(), "rust")
					.set(Article::rank_field(), 2)
					.execute_with(transaction)
					.await
			})
			.await
			.expect_err("ordinary atomic must reject update_or_create");
		assert!(ordinary_error.to_string().contains("write-intent"));

		let first_ready = Arc::new(Notify::new());
		let release_first = Arc::new(Notify::new());
		let second_entered = Arc::new(Notify::new());
		let first_connection = connection;
		let first_ready_task = Arc::clone(&first_ready);
		let release_first_task = Arc::clone(&release_first);
		let first = tokio::spawn(async move {
			first_connection
				.atomic_write(async |transaction| {
					Manager::<Article>::new()
						.update_or_create()
						.lookup(Article::slug_field(), "rust")
						.set(Article::rank_field(), 2)
						.execute_with(transaction)
						.await?;
					first_ready_task.notify_one();
					release_first_task.notified().await;
					Ok::<_, Error>(())
				})
				.await
		});
		first_ready.notified().await;

		let second_connection = connection;
		let second_entered_task = Arc::clone(&second_entered);
		let second = tokio::spawn(async move {
			second_connection
				.atomic_write(async |transaction| {
					second_entered_task.notify_one();
					Manager::<Article>::new()
						.update_or_create()
						.lookup(Article::slug_field(), "rust")
						.set(Article::rank_field(), 3)
						.execute_with(transaction)
						.await
				})
				.await
		});
		assert!(
			tokio::time::timeout(Duration::from_millis(100), second_entered.notified())
				.await
				.is_err(),
			"second execute_with callback must wait for SQLite write intent"
		);

		release_first.notify_one();
		first
			.await
			.expect("first writer task must not panic")
			.expect("first writer must commit");
		let (_, created) = tokio::time::timeout(Duration::from_secs(2), second)
			.await
			.expect("second writer must enter after first commit")
			.expect("second writer task must not panic")
			.expect("second writer must commit");
		assert!(!created);
	}

	#[test]
	fn update_candidate_preserves_native_json_null_and_sql_null() {
		let locked = JsonArticle {
			id: Some(1),
			rank: 1,
			json_null: Some(Json::new(serde_json::Value::Null)),
			sql_null: None,
		};
		let set =
			vec![TypedAssignment::new(JsonArticle::rank_field(), 2).expect("encode rank update")];

		let (candidate, _) =
			build_update_candidate(&locked, &set).expect("build JSON update candidate");

		assert_eq!(candidate.rank, 2);
		assert_eq!(
			candidate.json_null,
			Some(Json::new(serde_json::Value::Null))
		);
		assert_eq!(candidate.sql_null, None);
	}

	#[tokio::test]
	async fn update_or_create_existing_row_locks_and_updates_by_primary_key() {
		let (mut transaction, state) = Recorder::transaction(
			DatabaseType::Postgres,
			vec![Ok(QueryResult {
				rows_affected: 1,
				last_insert_id: None,
			})],
			vec![
				Ok(vec![article_row(7, "rust", 1, "old", 10)]),
				Ok(vec![article_row(7, "rust", 2, "old", 10)]),
			],
		);

		let (article, created) =
			execute_update_or_create(&Manager::<Article>::new(), plan(2, None), &mut transaction)
				.await
				.expect("update existing row");

		assert_eq!(article.rank, 2);
		assert!(!created);
		assert_eq!(
			state.lock().unwrap().calls,
			vec![
				Call {
					operation: "fetch_all",
					sql: "SELECT \"id\", \"slug\", \"rank\", \"headline\", \"computed\", \"readonly\" FROM \
						\"articles\" WHERE \"slug\" = $1 LIMIT 2 FOR UPDATE"
						.to_owned(),
					params: vec![QueryValue::String("rust".to_owned())],
				},
				Call {
					operation: "execute",
					sql: "UPDATE \"articles\" SET \"rank\" = $1 WHERE \"id\" = $2".to_owned(),
					params: vec![QueryValue::Int(2), QueryValue::Int(7)],
				},
				Call {
					operation: "fetch_all",
					sql: concat!(
						"SELECT \"id\", \"slug\", \"rank\", \"headline\", \"computed\", \"readonly\" FROM ",
						"\"articles\" WHERE \"id\" = $1 LIMIT 2 FOR UPDATE",
					)
					.to_owned(),
					params: vec![QueryValue::Int(7)],
				},
			]
		);
	}

	#[tokio::test]
	async fn update_or_create_primary_key_set_targets_the_locked_primary_key() {
		let (mut transaction, state) = Recorder::transaction(
			DatabaseType::Postgres,
			vec![Ok(QueryResult {
				rows_affected: 1,
				last_insert_id: None,
			})],
			vec![
				Ok(vec![article_row(7, "rust", 1, "old", 10)]),
				Ok(vec![article_row(8, "rust", 2, "old", 10)]),
			],
		);
		let mut mutation_plan = plan(2, None);
		mutation_plan.update.push(
			TypedAssignment::new(Article::id_field(), Some(8)).expect("encode primary-key update"),
		);

		let (article, created) =
			execute_update_or_create(&Manager::<Article>::new(), mutation_plan, &mut transaction)
				.await
				.expect("mutate primary key using locked row predicate");

		assert_eq!(article.id, Some(8));
		assert!(!created);
		assert_eq!(
			state.lock().unwrap().calls[1],
			Call {
				operation: "execute",
				sql: "UPDATE \"articles\" SET \"id\" = $1, \"rank\" = $2 WHERE \"id\" = $3"
					.to_owned(),
				params: vec![QueryValue::Int(8), QueryValue::Int(2), QueryValue::Int(7)],
			}
		);
		assert_eq!(
			state.lock().unwrap().calls[2],
			Call {
				operation: "fetch_all",
				sql: "SELECT \"id\", \"slug\", \"rank\", \"headline\", \"computed\", \"readonly\" FROM \
					\"articles\" WHERE \"id\" = $1 LIMIT 2 FOR UPDATE"
					.to_owned(),
				params: vec![QueryValue::Int(8)],
			}
		);
	}

	#[tokio::test]
	async fn update_or_create_hook_composite_pk_mutation_targets_locked_components() {
		let (mut transaction, state) = Recorder::transaction(
			DatabaseType::Postgres,
			vec![Ok(QueryResult {
				rows_affected: 1,
				last_insert_id: None,
			})],
			vec![
				Ok(vec![composite_article_row(7, 9, "rust", 1)]),
				Ok(vec![composite_article_row(8, 10, "rust", 2)]),
			],
		);

		let (article, created) =
			execute_update_or_create(&CompositePkHookManager, composite_plan(), &mut transaction)
				.await
				.expect("mutate composite primary key using locked predicate");

		assert_eq!((article.tenant_id, article.article_id), (8, 10));
		assert!(!created);
		assert_eq!(
			state.lock().unwrap().calls[1],
			Call {
				operation: "execute",
				sql: "UPDATE \"composite_articles\" SET \"article_key\" = $1, \"rank\" = $2, \
					\"tenant_key\" = $3 WHERE \"tenant_key\" = $4 AND \"article_key\" = $5"
					.to_owned(),
				params: vec![
					QueryValue::Int(10),
					QueryValue::Int(2),
					QueryValue::Int(8),
					QueryValue::Int(7),
					QueryValue::Int(9),
				],
			}
		);
	}

	#[tokio::test]
	async fn update_or_create_create_defaults_apply_only_to_create() {
		let (mut transaction, state) = Recorder::transaction(
			DatabaseType::Postgres,
			vec![Ok(QueryResult {
				rows_affected: 1,
				last_insert_id: None,
			})],
			vec![
				Ok(Vec::new()),
				Ok(vec![article_row(8, "rust", 2, "created", 11)]),
			],
		);

		let (article, created) = execute_update_or_create(
			&Manager::<Article>::new(),
			plan(2, Some("created")),
			&mut transaction,
		)
		.await
		.expect("create absent row");

		assert!(created);
		assert_eq!(article.headline, "created");
		let calls = &state.lock().unwrap().calls;
		assert_eq!(calls.len(), 2);
		assert_eq!(calls[1].operation, "fetch_all");
		assert!(calls[1].sql.ends_with("RETURNING *"));
		assert_eq!(
			calls[1].params,
			vec![
				QueryValue::String("rust".to_owned()),
				QueryValue::Int(2),
				QueryValue::String("created".to_owned()),
			]
		);
	}

	#[derive(Default)]
	struct RaceHookManager {
		counts: Arc<Mutex<(usize, usize)>>,
	}

	impl CustomManager for RaceHookManager {
		type Model = Article;

		fn new() -> Self {
			Self::default()
		}

		fn before_upsert_write(
			&self,
			write: &mut UpsertWrite<'_, Article>,
		) -> reinhardt_core::exception::Result<()> {
			let mut counts = self.counts.lock().unwrap();
			match write {
				UpsertWrite::Create(_) => counts.0 += 1,
				UpsertWrite::Update(article) => {
					counts.1 += 1;
					article.headline = "hooked".to_owned();
				}
			}
			Ok(())
		}
	}

	#[tokio::test]
	async fn update_or_create_lost_race_relocks_and_diffs_hook_changes() {
		let (mut transaction, state) = Recorder::transaction(
			DatabaseType::Postgres,
			vec![Ok(QueryResult {
				rows_affected: 1,
				last_insert_id: None,
			})],
			vec![
				Ok(Vec::new()),
				Ok(Vec::new()),
				Ok(vec![article_row(9, "rust", 5, "winner", 12)]),
				Ok(vec![article_row(9, "rust", 2, "hooked", 12)]),
			],
		);
		let manager = RaceHookManager::default();

		let (article, created) =
			execute_update_or_create(&manager, plan(2, Some("create only")), &mut transaction)
				.await
				.expect("update race winner");

		assert!(!created);
		assert_eq!(article.rank, 2);
		assert_eq!(article.headline, "hooked");
		assert_eq!(*manager.counts.lock().unwrap(), (1, 1));
		let calls = &state.lock().unwrap().calls;
		assert_eq!(calls[2].operation, "fetch_all");
		assert!(calls[2].sql.ends_with("LIMIT 2 FOR UPDATE"));
		assert_eq!(
			calls[3],
			Call {
				operation: "execute",
				sql: "UPDATE \"articles\" SET \"headline\" = $1, \"rank\" = $2 WHERE \"id\" = $3"
					.to_owned(),
				params: vec![
					QueryValue::String("hooked".to_owned()),
					QueryValue::Int(2),
					QueryValue::Int(9),
				],
			}
		);
		assert!(
			!calls[3]
				.params
				.contains(&QueryValue::String("create only".to_owned()))
		);
		assert!(!calls[3].sql.contains("computed"));
	}

	#[tokio::test]
	async fn update_or_create_postgres_unique_violation_reloads_after_savepoint() {
		let (mut transaction, state) = Recorder::transaction(
			DatabaseType::Postgres,
			vec![Ok(QueryResult {
				rows_affected: 1,
				last_insert_id: None,
			})],
			vec![
				Ok(Vec::new()),
				Err(DatabaseError::new(
					DatabaseErrorKind::UniqueViolation,
					"duplicate alternate unique field",
				)
				.into()),
				Ok(vec![article_row(10, "rust", 1, "winner", 13)]),
				Ok(vec![article_row(10, "rust", 2, "winner", 13)]),
			],
		);

		let (article, created) =
			execute_update_or_create(&Manager::<Article>::new(), plan(2, None), &mut transaction)
				.await
				.expect("unique race winner should be reloaded and updated");

		assert!(!created);
		assert_eq!(article.rank, 2);
		let calls = &state.lock().unwrap().calls;
		assert_eq!(
			calls.iter().map(|call| call.operation).collect::<Vec<_>>(),
			[
				"fetch_all",
				"fetch_all",
				"fetch_all",
				"execute",
				"fetch_all"
			]
		);
		assert!(calls[2].sql.ends_with("LIMIT 2 FOR UPDATE"));
		assert_eq!(
			calls[3].params,
			vec![QueryValue::Int(2), QueryValue::Int(10)]
		);
	}

	#[tokio::test]
	async fn update_or_create_postgres_unique_violation_without_lookup_match_preserves_error() {
		let (mut transaction, state) = Recorder::transaction(
			DatabaseType::Postgres,
			Vec::new(),
			vec![
				Ok(Vec::new()),
				Err(DatabaseError::new(
					DatabaseErrorKind::UniqueViolation,
					"duplicate alternate unique field",
				)
				.into()),
				Ok(Vec::new()),
			],
		);

		let error =
			execute_update_or_create(&Manager::<Article>::new(), plan(2, None), &mut transaction)
				.await
				.expect_err("an unrelated unique violation must not be converted into a conflict");

		assert_eq!(
			error.database_kind(),
			Some(DatabaseErrorKind::UniqueViolation)
		);
		assert_eq!(
			state
				.lock()
				.unwrap()
				.calls
				.iter()
				.map(|call| call.operation)
				.collect::<Vec<_>>(),
			["fetch_all", "fetch_all", "fetch_all"]
		);
	}

	#[tokio::test]
	async fn update_or_create_unchanged_fields_skip_update_and_caller_completion() {
		let (mut transaction, state) = Recorder::transaction(
			DatabaseType::Sqlite,
			Vec::new(),
			vec![Ok(vec![article_row(10, "rust", 2, "same", 13)])],
		);

		let (article, created) =
			execute_update_or_create(&Manager::<Article>::new(), plan(2, None), &mut transaction)
				.await
				.expect("unchanged row");

		assert_eq!(article.rank, 2);
		assert!(!created);
		let calls = &state.lock().unwrap().calls;
		assert_eq!(calls.len(), 1);
		assert!(!calls[0].sql.contains("FOR UPDATE"));
		assert_eq!(calls[0].operation, "fetch_all");
	}

	#[tokio::test]
	async fn get_or_create_mysql_reloads_by_generated_primary_key() {
		let (mut transaction, state) = Recorder::transaction(
			DatabaseType::Mysql,
			vec![Ok(QueryResult {
				rows_affected: 1,
				last_insert_id: Some(17),
			})],
			vec![
				Ok(Vec::new()),
				Ok(vec![article_row(17, "stored-slug", 1, "created", 13)]),
			],
		);

		let (article, created) =
			execute_get_or_create(&Manager::<Article>::new(), get_plan(), &mut transaction)
				.await
				.expect("reload MySQL insert by generated primary key");

		assert!(created);
		assert_eq!(article.slug, "stored-slug");
		let calls = &state.lock().unwrap().calls;
		assert_eq!(calls.len(), 3);
		assert_eq!(calls[2].operation, "fetch_all");
		assert!(
			calls[2].sql.contains("WHERE `id` = ? LIMIT 2"),
			"{}",
			calls[2].sql
		);
		assert!(
			!calls[2].sql.contains("WHERE `slug` = ?"),
			"{}",
			calls[2].sql
		);
	}

	#[tokio::test]
	async fn update_or_create_accepts_a_matched_but_unchanged_mysql_update() {
		let (mut transaction, state) = Recorder::transaction(
			DatabaseType::Mysql,
			vec![Ok(QueryResult {
				rows_affected: 0,
				last_insert_id: None,
			})],
			vec![
				Ok(vec![article_row(10, "rust", 1, "old", 13)]),
				Ok(vec![article_row(10, "rust", 2, "old", 13)]),
			],
		);

		let (article, created) =
			execute_update_or_create(&Manager::<Article>::new(), plan(2, None), &mut transaction)
				.await
				.expect("a locked MySQL row may report no changed values");

		assert_eq!(article.rank, 2);
		assert!(!created);
		assert_eq!(
			state
				.lock()
				.unwrap()
				.calls
				.iter()
				.map(|call| call.operation)
				.collect::<Vec<_>>(),
			["fetch_all", "execute", "fetch_all"]
		);
	}

	#[derive(Default)]
	struct VetoUpdateManager;

	impl CustomManager for VetoUpdateManager {
		type Model = Article;

		fn new() -> Self {
			Self
		}

		fn before_upsert_write(
			&self,
			write: &mut UpsertWrite<'_, Article>,
		) -> reinhardt_core::exception::Result<()> {
			if matches!(write, UpsertWrite::Update(_)) {
				return Err(Error::Validation("update vetoed".to_owned()));
			}
			Ok(())
		}
	}

	#[tokio::test]
	async fn update_or_create_owned_scope_commits_or_rolls_back() {
		let (success, success_state) = Recorder::transaction(
			DatabaseType::Postgres,
			vec![Ok(QueryResult {
				rows_affected: 1,
				last_insert_id: None,
			})],
			vec![
				Ok(vec![article_row(11, "rust", 1, "old", 14)]),
				Ok(vec![article_row(11, "rust", 2, "old", 14)]),
			],
		);
		let success = success
			.run(async |transaction| {
				execute_update_or_create(&Manager::<Article>::new(), plan(2, None), transaction)
					.await
			})
			.await;
		assert!(success.is_ok());
		assert_eq!(
			success_state
				.lock()
				.unwrap()
				.calls
				.last()
				.unwrap()
				.operation,
			"commit"
		);

		let (failure, failure_state) = Recorder::transaction(
			DatabaseType::Postgres,
			Vec::new(),
			vec![Ok(vec![article_row(12, "rust", 1, "old", 15)])],
		);
		let failure = failure
			.run(async |transaction| {
				execute_update_or_create(&VetoUpdateManager, plan(2, None), transaction).await
			})
			.await;
		assert!(matches!(failure, Err(Error::Validation(_))));
		assert_eq!(
			failure_state
				.lock()
				.unwrap()
				.calls
				.last()
				.unwrap()
				.operation,
			"rollback"
		);

		let (database_failure, database_failure_state) = Recorder::transaction(
			DatabaseType::Postgres,
			vec![Err(DatabaseError::new(
				DatabaseErrorKind::Query,
				"UPDATE failed",
			)
			.into())],
			vec![Ok(vec![article_row(13, "rust", 1, "old", 16)])],
		);
		let database_failure = database_failure
			.run(async |transaction| {
				execute_update_or_create(&Manager::<Article>::new(), plan(2, None), transaction)
					.await
			})
			.await;
		assert!(database_failure.is_err());
		assert_eq!(
			database_failure_state
				.lock()
				.unwrap()
				.calls
				.last()
				.unwrap()
				.operation,
			"rollback"
		);
	}

	#[tokio::test]
	async fn update_or_create_mysql_lookup_compiles_for_update() {
		let (mut transaction, state) = Recorder::transaction(
			DatabaseType::Mysql,
			Vec::new(),
			vec![Ok(vec![article_row(14, "rust", 2, "same", 17)])],
		);

		execute_update_or_create(&Manager::<Article>::new(), plan(2, None), &mut transaction)
			.await
			.expect("load unchanged MySQL row");

		assert_eq!(
			state.lock().unwrap().calls[0],
			Call {
				operation: "fetch_all",
				sql: "SELECT `id`, `slug`, `rank`, `headline`, `computed`, `readonly` FROM \
					`articles` WHERE `slug` = ? LIMIT 2 FOR UPDATE"
					.to_owned(),
				params: vec![QueryValue::String("rust".to_owned())],
			}
		);
	}
}
