use crate::orm::connection::{DatabaseBackend, OrmExecutor, Row};
use crate::orm::custom_manager::CustomManager;
use crate::orm::field_codec::{DatabaseValue, FieldCodecError};
use crate::orm::manager::decode_model_row;
use crate::orm::model::Model;
use crate::orm::transaction::AtomicTransaction;
use crate::orm::upsert::assignment::TypedAssignment;
use crate::orm::upsert::assignment::{UpsertCreate, UpsertWrite};
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
	let backend = executor.backend();
	let select = sql::select_by_lookup(&plan, backend, false)?;
	let rows = executor.fetch_all(&select.sql, select.params).await?;
	match decode_lookup_rows(rows)? {
		Some(model) => return Ok((model, false)),
		None => {}
	}

	manager.before_upsert_write(&mut UpsertWrite::Create(UpsertCreate {
		lookup: &plan.lookup,
		values: &mut plan.create,
	}))?;

	let insert = sql::insert(&plan, backend)?;
	match executor.execute(&insert.sql, insert.params).await {
		Ok(result) => {
			let created = match (backend, result.rows_affected) {
				(DatabaseBackend::Postgres | DatabaseBackend::Sqlite, 0) => false,
				(_, 1) => true,
				_ => {
					return Err(Error::Conflict(format!(
						"get_or_create INSERT affected {} rows for {backend:?}; expected one",
						result.rows_affected
					)));
				}
			};
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
	if let Some(locked) = load_locked(&plan, transaction).await? {
		return update_locked(manager, &plan, locked, transaction).await;
	}

	manager.before_upsert_write(&mut UpsertWrite::Create(UpsertCreate {
		lookup: &plan.lookup,
		values: &mut plan.create,
	}))?;
	let backend = transaction.backend();
	let insert = sql::insert(&plan, backend)?;
	match transaction.execute(&insert.sql, insert.params).await {
		Ok(result) => {
			let created = match (backend, result.rows_affected) {
				(DatabaseBackend::Postgres | DatabaseBackend::Sqlite, 0) => false,
				(_, 1) => true,
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
			if backend == DatabaseBackend::MySql
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
	let update = sql::update_values_by_primary_key(&candidate, &values, transaction.backend())?;
	let result = transaction.execute(&update.sql, update.params).await?;
	if result.rows_affected != 1 {
		return Err(Error::Conflict(format!(
			"update_or_create UPDATE affected {} rows; expected one",
			result.rows_affected
		)));
	}
	Ok((candidate, false))
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
	use super::execute_update_or_create;
	use crate::backends::error::{DatabaseError, DatabaseErrorKind};
	use crate::backends::types::{DatabaseType, QueryResult, QueryValue, Row, TransactionExecutor};
	use crate::orm::connection::DatabaseBackend;
	use crate::orm::custom_manager::CustomManager;
	use crate::orm::expressions::FieldRef;
	use crate::orm::inspection::FieldInfo;
	use crate::orm::manager::Manager;
	use crate::orm::model::{FieldSelector, Model};
	use crate::orm::transaction::AtomicTransaction;
	use crate::orm::upsert::assignment::TypedAssignment;
	use crate::orm::upsert::assignment::UpsertWrite;
	use crate::orm::upsert::plan::{UpsertMode, normalize};
	use async_trait::async_trait;
	use reinhardt_core::exception::Error;
	use serde::{Deserialize, Serialize};
	use std::collections::{HashMap, VecDeque};
	use std::sync::{Arc, Mutex};

	#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
	struct Article {
		id: Option<i64>,
		slug: String,
		rank: i32,
		headline: String,
		computed: i32,
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
			]
		}

		fn generated_field_names() -> &'static [&'static str] {
			&["computed"]
		}
	}

	impl Article {
		fn slug_field() -> FieldRef<Self, String> {
			// SAFETY: the logical and physical names match Article's string field.
			unsafe { FieldRef::from_model_field("slug", "slug") }
		}

		fn rank_field() -> FieldRef<Self, i32> {
			// SAFETY: the logical and physical names match Article's i32 field.
			unsafe { FieldRef::from_model_field("rank", "rank") }
		}

		fn headline_field() -> FieldRef<Self, String> {
			// SAFETY: the logical and physical names match Article's string field.
			unsafe { FieldRef::from_model_field("headline", "headline") }
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

	#[tokio::test]
	async fn update_or_create_existing_row_locks_and_updates_by_primary_key() {
		let (mut transaction, state) = Recorder::transaction(
			DatabaseType::Postgres,
			vec![Ok(QueryResult {
				rows_affected: 1,
				last_insert_id: None,
			})],
			vec![Ok(vec![article_row(7, "rust", 1, "old", 10)])],
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
					sql: "SELECT \"id\", \"slug\", \"rank\", \"headline\", \"computed\" FROM \
						\"articles\" WHERE \"slug\" = $1 LIMIT 2 FOR UPDATE"
						.to_owned(),
					params: vec![QueryValue::String("rust".to_owned())],
				},
				Call {
					operation: "execute",
					sql: "UPDATE \"articles\" SET \"rank\" = $1 WHERE \"id\" = $2".to_owned(),
					params: vec![QueryValue::Int(2), QueryValue::Int(7)],
				},
			]
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
		assert_eq!(calls[1].operation, "execute");
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
					article.computed += 100;
				}
			}
			Ok(())
		}
	}

	#[tokio::test]
	async fn update_or_create_lost_race_relocks_and_diffs_hook_changes() {
		let (mut transaction, state) = Recorder::transaction(
			DatabaseType::Postgres,
			vec![
				Ok(QueryResult {
					rows_affected: 0,
					last_insert_id: None,
				}),
				Ok(QueryResult {
					rows_affected: 1,
					last_insert_id: None,
				}),
			],
			vec![
				Ok(Vec::new()),
				Ok(vec![article_row(9, "rust", 5, "winner", 12)]),
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
			vec![Ok(vec![article_row(11, "rust", 1, "old", 14)])],
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

	#[test]
	fn update_or_create_backend_lock_contract_is_explicit() {
		assert_eq!(
			[
				DatabaseBackend::Postgres,
				DatabaseBackend::MySql,
				DatabaseBackend::Sqlite,
			]
			.map(|backend| matches!(backend, DatabaseBackend::Postgres | DatabaseBackend::MySql)),
			[true, true, false]
		);
	}
}
