use crate::orm::field_codec::{DatabaseValue, FieldCodecError, database_value_to_query_value};
use crate::orm::model::Model;
#[cfg(test)]
use crate::orm::upsert::assignment::TypedAssignment;
use crate::orm::upsert::plan::UpsertPlan;
use crate::orm::{DatabaseBackend, QueryValue};
use reinhardt_core::exception::{DatabaseErrorKind, Error, Result};
use reinhardt_query::prelude::{
	Alias, Expr, ExprTrait, InsertStatement, MySqlQueryBuilder, OnConflict, PostgresQueryBuilder,
	Query, QueryBuilder, SelectStatement, SqliteQueryBuilder, UpdateStatement, Values,
};
use std::collections::BTreeMap;

pub(crate) struct BoundSql {
	pub(crate) sql: String,
	pub(crate) params: Vec<QueryValue>,
}

pub(crate) fn select_by_lookup<M: Model>(
	plan: &UpsertPlan<M>,
	backend: DatabaseBackend,
	lock: bool,
) -> Result<BoundSql> {
	let field_metadata = M::field_metadata();
	if field_metadata.is_empty() {
		return Err(Error::Validation(format!(
			"typed upsert SELECT requires field metadata for '{}'",
			M::table_name()
		)));
	}

	let mut statement = Query::select();
	statement.from(Alias::new(M::table_name())).columns(
		field_metadata
			.iter()
			.map(|field| Alias::new(field.db_column_name())),
	);
	for assignment in &plan.lookup {
		let column = Expr::col(Alias::new(assignment.column_name));
		if assignment.value == DatabaseValue::Null {
			statement.and_where(column.is_null());
		} else {
			statement.and_where(column.eq(database_value_to_query_value(assignment.value.clone())));
		}
	}
	let (mut sql, values) = build_select_sql(&statement, backend);
	// Workaround for reinhardt-query lock and inline LIMIT ordering
	// (tracked in reinhardt-web#5813). Keep LIMIT and FOR UPDATE together here because
	// SelectStatement::limit(2) currently adds a bound parameter, which would change
	// BoundSql.params. Remove both manual suffixes once the builder can express an inline
	// LIMIT followed by a lock clause.
	//
	// Ideal implementation (without workaround):
	//   statement.limit(2);
	//   if lock && matches!(backend, DatabaseBackend::Postgres | DatabaseBackend::MySql) {
	//       statement.lock_exclusive();
	//   }
	//   let (sql, values) = build_select_sql(&statement, backend);
	sql.push_str(" LIMIT 2");
	if lock && matches!(backend, DatabaseBackend::Postgres | DatabaseBackend::MySql) {
		sql.push_str(" FOR UPDATE");
	}
	Ok(bound_sql(sql, values))
}

pub(crate) fn select_by_primary_key<M: Model>(
	model: &M,
	backend: DatabaseBackend,
	lock: bool,
) -> Result<BoundSql> {
	let field_metadata = M::field_metadata();
	if field_metadata.is_empty() {
		return Err(Error::Validation(format!(
			"typed upsert SELECT requires field metadata for '{}'",
			M::table_name()
		)));
	}
	let encoded_fields = model.encode_database_fields().map_err(field_codec_error)?;
	let primary_key_fields = M::composite_primary_key().map_or_else(
		|| vec![M::primary_key_field().to_owned()],
		|primary_key| primary_key.fields().to_vec(),
	);
	let mut statement = Query::select();
	statement.from(Alias::new(M::table_name())).columns(
		field_metadata
			.iter()
			.map(|field| Alias::new(field.db_column_name())),
	);
	for logical_name in primary_key_fields {
		let field = field_metadata
			.iter()
			.find(|field| field.name == logical_name)
			.ok_or_else(|| {
				Error::Validation(format!(
					"typed upsert SELECT primary-key field '{logical_name}' is missing model metadata"
				))
			})?;
		let value = encoded_fields.get(&logical_name).ok_or_else(|| {
			Error::Validation(format!(
				"typed upsert SELECT primary-key field '{logical_name}' is missing encoded model values"
			))
		})?;
		if *value == DatabaseValue::Null {
			return Err(Error::Validation(format!(
				"typed upsert SELECT requires non-null primary-key field '{logical_name}'"
			)));
		}
		statement.and_where(
			Expr::col(Alias::new(field.db_column_name()))
				.eq(database_value_to_query_value(value.clone())),
		);
	}
	let (mut sql, values) = build_select_sql(&statement, backend);
	sql.push_str(" LIMIT 2");
	if lock && matches!(backend, DatabaseBackend::Postgres | DatabaseBackend::MySql) {
		sql.push_str(" FOR UPDATE");
	}
	Ok(bound_sql(sql, values))
}

pub(crate) fn select_by_generated_mysql_primary_key<M: Model>(
	last_insert_id: i64,
) -> Result<BoundSql> {
	if M::composite_primary_key().is_some() {
		return Err(Error::Validation(format!(
			"typed upsert cannot reload a composite primary key from MySQL last_insert_id for '{}'",
			M::table_name()
		)));
	}
	let field_metadata = M::field_metadata();
	let primary_key = field_metadata
		.iter()
		.find(|field| field.name == M::primary_key_field())
		.ok_or_else(|| {
			Error::Validation(format!(
				"typed upsert SELECT primary-key field '{}' is missing model metadata",
				M::primary_key_field()
			))
		})?;
	let mut statement = Query::select();
	statement.from(Alias::new(M::table_name())).columns(
		field_metadata
			.iter()
			.map(|field| Alias::new(field.db_column_name())),
	);
	statement.and_where(
		Expr::col(Alias::new(primary_key.db_column_name()))
			.eq(reinhardt_query::value::Value::BigInt(Some(last_insert_id))),
	);
	let (mut sql, values) = build_select_sql(&statement, DatabaseBackend::MySql);
	sql.push_str(" LIMIT 2");
	Ok(bound_sql(sql, values))
}

pub(crate) fn insert<M: Model>(plan: &UpsertPlan<M>, backend: DatabaseBackend) -> Result<BoundSql> {
	let mut statement = Query::insert();
	statement.into_table(Alias::new(M::table_name())).columns(
		plan.create
			.iter()
			.map(|assignment| Alias::new(assignment.column_name)),
	);
	statement
		.values(
			plan.create
				.iter()
				.map(|assignment| database_value_to_query_value(assignment.value.clone()))
				.collect(),
		)
		.map_err(|error| {
			Error::Validation(format!(
				"typed upsert INSERT could not align columns and values: {error}"
			))
		})?;
	if matches!(backend, DatabaseBackend::Postgres | DatabaseBackend::Sqlite) {
		statement.on_conflict(
			OnConflict::columns(plan.proof.column_names.iter().cloned().map(Alias::new))
				.do_nothing()
				.to_owned(),
		);
		statement.returning_all();
	}

	let (sql, values) = build_insert_sql(&statement, backend);
	Ok(bound_sql(sql, values))
}

#[cfg(test)]
pub(crate) fn update_by_primary_key<M: Model>(
	model: &M,
	values: &[TypedAssignment<M>],
	backend: DatabaseBackend,
) -> Result<BoundSql> {
	if values.is_empty() {
		return Err(Error::Validation(
			"typed upsert UPDATE requires at least one assignment".to_owned(),
		));
	}

	let mut statement = Query::update();
	statement.table(Alias::new(M::table_name()));
	for assignment in values {
		statement.value(
			Alias::new(assignment.column_name),
			database_value_to_query_value(assignment.value.clone()),
		);
	}

	let encoded_fields = model.encode_database_fields().map_err(field_codec_error)?;
	let field_metadata = M::field_metadata();
	let composite_primary_key = M::composite_primary_key();
	let primary_key_fields = composite_primary_key.as_ref().map_or_else(
		|| vec![M::primary_key_field().to_owned()],
		|primary_key| primary_key.fields().to_vec(),
	);
	for logical_name in primary_key_fields {
		let field = field_metadata
			.iter()
			.find(|field| field.name == logical_name);
		let column_name = match field {
			Some(field) => field.db_column_name(),
			None if composite_primary_key.is_none() && logical_name == M::primary_key_field() => {
				M::primary_key_column()
			}
			None => {
				return Err(Error::Validation(format!(
					"typed upsert UPDATE primary-key field '{logical_name}' is missing model metadata"
				)));
			}
		};
		let value = encoded_fields.get(&logical_name).ok_or_else(|| {
			Error::Validation(format!(
				"typed upsert UPDATE primary-key field '{logical_name}' is missing encoded model values"
			))
		})?;
		if *value == DatabaseValue::Null {
			return Err(Error::Validation(format!(
				"typed upsert UPDATE requires non-null primary-key field '{logical_name}'"
			)));
		}
		statement.and_where(
			Expr::col(Alias::new(column_name)).eq(database_value_to_query_value(value.clone())),
		);
	}

	let (sql, values) = build_update_sql(&statement, backend);
	Ok(bound_sql(sql, values))
}

pub(crate) fn update_values_by_primary_key<M: Model>(
	model: &M,
	values: &BTreeMap<String, DatabaseValue>,
	backend: DatabaseBackend,
) -> Result<BoundSql> {
	if values.is_empty() {
		return Err(Error::Validation(
			"typed upsert UPDATE requires at least one assignment".to_owned(),
		));
	}
	let metadata = M::field_metadata();
	let assignments = values
		.iter()
		.map(|(logical_name, value)| {
			let field = metadata
				.iter()
				.find(|field| field.name == *logical_name)
				.ok_or_else(|| {
					Error::Validation(format!(
						"typed upsert UPDATE field '{logical_name}' is missing model metadata"
					))
				})?;
			Ok((field.db_column_name().to_owned(), value.clone()))
		})
		.collect::<Result<Vec<_>>>()?;

	let mut statement = Query::update();
	statement.table(Alias::new(M::table_name()));
	for (column_name, value) in assignments {
		statement.value(
			Alias::new(column_name),
			database_value_to_query_value(value),
		);
	}
	add_primary_key_predicate(model, &mut statement)?;
	let (sql, values) = build_update_sql(&statement, backend);
	Ok(bound_sql(sql, values))
}

fn add_primary_key_predicate<M: Model>(model: &M, statement: &mut UpdateStatement) -> Result<()> {
	let encoded_fields = model.encode_database_fields().map_err(field_codec_error)?;
	let field_metadata = M::field_metadata();
	let composite_primary_key = M::composite_primary_key();
	let primary_key_fields = composite_primary_key.as_ref().map_or_else(
		|| vec![M::primary_key_field().to_owned()],
		|primary_key| primary_key.fields().to_vec(),
	);
	for logical_name in primary_key_fields {
		let field = field_metadata
			.iter()
			.find(|field| field.name == logical_name);
		let column_name = match field {
			Some(field) => field.db_column_name(),
			None if composite_primary_key.is_none() && logical_name == M::primary_key_field() => {
				M::primary_key_column()
			}
			None => {
				return Err(Error::Validation(format!(
					"typed upsert UPDATE primary-key field '{logical_name}' is missing model metadata"
				)));
			}
		};
		let value = encoded_fields.get(&logical_name).ok_or_else(|| {
			Error::Validation(format!(
				"typed upsert UPDATE primary-key field '{logical_name}' is missing encoded model values"
			))
		})?;
		if *value == DatabaseValue::Null {
			return Err(Error::Validation(format!(
				"typed upsert UPDATE requires non-null primary-key field '{logical_name}'"
			)));
		}
		statement.and_where(
			Expr::col(Alias::new(column_name)).eq(database_value_to_query_value(value.clone())),
		);
	}
	Ok(())
}

fn build_select_sql(statement: &SelectStatement, backend: DatabaseBackend) -> (String, Values) {
	match backend {
		DatabaseBackend::Postgres => PostgresQueryBuilder.build_select(statement),
		DatabaseBackend::MySql => MySqlQueryBuilder.build_select(statement),
		DatabaseBackend::Sqlite => SqliteQueryBuilder.build_select(statement),
	}
}

fn build_insert_sql(statement: &InsertStatement, backend: DatabaseBackend) -> (String, Values) {
	match backend {
		DatabaseBackend::Postgres => PostgresQueryBuilder.build_insert(statement),
		DatabaseBackend::MySql => MySqlQueryBuilder.build_insert(statement),
		DatabaseBackend::Sqlite => SqliteQueryBuilder.build_insert(statement),
	}
}

fn build_update_sql(statement: &UpdateStatement, backend: DatabaseBackend) -> (String, Values) {
	match backend {
		DatabaseBackend::Postgres => PostgresQueryBuilder.build_update(statement),
		DatabaseBackend::MySql => MySqlQueryBuilder.build_update(statement),
		DatabaseBackend::Sqlite => SqliteQueryBuilder.build_update(statement),
	}
}

fn bound_sql(sql: String, values: Values) -> BoundSql {
	BoundSql {
		sql,
		params: crate::orm::execution::convert_values(values),
	}
}

fn field_codec_error(error: FieldCodecError) -> Error {
	let kind = match &error {
		FieldCodecError::TypeMismatch { .. }
		| FieldCodecError::InvalidEnumValue { .. }
		| FieldCodecError::MissingFieldMetadata { .. }
		| FieldCodecError::FieldPolicyMismatch { .. } => DatabaseErrorKind::Type,
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
	use super::{
		field_codec_error, insert, select_by_lookup, update_by_primary_key,
		update_values_by_primary_key,
	};
	use crate::orm::composite_pk::CompositePrimaryKey;
	use crate::orm::expressions::FieldRef;
	use crate::orm::field_codec::{DatabaseValue, FieldCodecContext, FieldCodecError};
	use crate::orm::inspection::FieldInfo;
	use crate::orm::model::{FieldSelector, Model};
	use crate::orm::upsert::assignment::TypedAssignment;
	use crate::orm::upsert::plan::{UniqueProof, UniqueProofSource, UpsertMode, UpsertPlan};
	use crate::orm::{DatabaseBackend, Manager, QueryValue};
	use rstest::*;
	use serde::{Deserialize, Serialize};
	use std::collections::{BTreeMap, HashMap};

	#[derive(Clone, Debug, Serialize, Deserialize)]
	struct Article {
		id: Option<i64>,
		tenant_id: i64,
		slug: Option<String>,
		headline: String,
	}

	#[test]
	fn field_policy_mismatch_is_a_typed_upsert_sql_error() {
		let error = field_codec_error(FieldCodecError::FieldPolicyMismatch {
			context: FieldCodecContext::new("Article", "attachment", "attachment_path"),
			key: "file_storage".to_owned(),
			expected: "private_uploads".to_owned(),
			actual: "default".to_owned(),
		});

		assert_eq!(
			error.database_kind(),
			Some(reinhardt_core::exception::DatabaseErrorKind::Type)
		);
		assert!(
			std::error::Error::source(&error)
				.unwrap()
				.downcast_ref::<FieldCodecError>()
				.is_some()
		);
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
				field("id", None, true, false, true),
				field("tenant_id", None, false, false, false),
				field("slug", Some("article_slug"), false, false, true),
				field("headline", None, false, false, false),
			]
		}
	}

	impl Article {
		fn tenant_field() -> FieldRef<Self, i64> {
			// SAFETY: the logical and physical names match Article's declared i64 field.
			unsafe { FieldRef::from_model_field_with_names("tenant_id", "tenant_id") }
		}

		fn slug_field() -> FieldRef<Self, Option<String>> {
			// SAFETY: the names match Article's declared optional string field and db_column.
			unsafe { FieldRef::from_model_field_with_names("slug", "article_slug") }
		}

		fn headline_field() -> FieldRef<Self, String> {
			// SAFETY: the logical and physical names match Article's declared string field.
			unsafe { FieldRef::from_model_field_with_names("headline", "headline") }
		}
	}

	#[derive(Clone, Debug, Serialize, Deserialize)]
	struct ArticleRevision {
		tenant_id: i64,
		article_id: i64,
		headline: String,
	}

	#[derive(Clone)]
	struct ArticleRevisionFields;

	impl FieldSelector for ArticleRevisionFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for ArticleRevision {
		type PrimaryKey = String;
		type Fields = ArticleRevisionFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"article_revisions"
		}

		fn new_fields() -> Self::Fields {
			ArticleRevisionFields
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
				field("tenant_id", Some("tenant_key"), true, false, false),
				field("article_id", Some("article_key"), true, false, false),
				field("headline", Some("display_headline"), false, false, false),
			]
		}
	}

	impl ArticleRevision {
		fn headline_field() -> FieldRef<Self, String> {
			// SAFETY: the names match ArticleRevision's declared string field and db_column.
			unsafe { FieldRef::from_model_field_with_names("headline", "display_headline") }
		}
	}

	#[derive(Clone, Debug, Serialize, Deserialize)]
	struct MissingCompositeMetadata {
		tenant_id: i64,
		article_id: i64,
		headline: String,
	}

	impl Model for MissingCompositeMetadata {
		type PrimaryKey = String;
		type Fields = ArticleFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"missing_composite_metadata"
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
				field("tenant_id", None, true, false, false),
				field("headline", None, false, false, false),
			]
		}
	}

	impl MissingCompositeMetadata {
		fn headline_field() -> FieldRef<Self, String> {
			// SAFETY: the names match the model's declared headline field.
			unsafe { FieldRef::from_model_field_with_names("headline", "headline") }
		}
	}

	#[derive(Clone, Debug, Serialize, Deserialize)]
	struct MissingPrimaryKeyValue {
		other_id: i64,
		headline: String,
	}

	impl Model for MissingPrimaryKeyValue {
		type PrimaryKey = i64;
		type Fields = ArticleFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"missing_primary_key_values"
		}

		fn new_fields() -> Self::Fields {
			ArticleFields
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			None
		}

		fn set_primary_key(&mut self, _value: Self::PrimaryKey) {}

		fn field_metadata() -> Vec<FieldInfo> {
			vec![
				field("id", None, true, false, false),
				field("headline", None, false, false, false),
			]
		}
	}

	impl MissingPrimaryKeyValue {
		fn headline_field() -> FieldRef<Self, String> {
			// SAFETY: the names match the model's declared headline field.
			unsafe { FieldRef::from_model_field_with_names("headline", "headline") }
		}
	}

	#[derive(Clone, Debug, Serialize, Deserialize)]
	struct MetadataFreeModel {
		id: Option<i64>,
	}

	impl Model for MetadataFreeModel {
		type PrimaryKey = i64;
		type Fields = ArticleFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"metadata_free_models"
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
	}

	fn field(
		name: &str,
		db_column: Option<&str>,
		primary_key: bool,
		unique: bool,
		nullable: bool,
	) -> FieldInfo {
		FieldInfo {
			name: name.to_owned(),
			field_type: "reinhardt.orm.models.CharField".to_owned(),
			storage_kind: None,
			domain: None,
			nullable,
			primary_key,
			unique,
			blank: false,
			editable: true,
			default: None,
			db_default: None,
			db_column: db_column.map(str::to_owned),
			choices: None,
			attributes: HashMap::new(),
		}
	}

	fn article_plan(slug: Option<&str>) -> UpsertPlan<Article> {
		let lookup = vec![
			TypedAssignment::new(Article::tenant_field(), 7_i64).expect("encode tenant"),
			TypedAssignment::new(Article::slug_field(), slug.map(str::to_owned))
				.expect("encode slug"),
		];
		let mut create = lookup.clone();
		create.push(
			TypedAssignment::new(Article::headline_field(), "A quoted headline")
				.expect("encode headline"),
		);
		UpsertPlan {
			lookup,
			create,
			update: Vec::new(),
			proof: UniqueProof {
				logical_fields: vec!["tenant_id".to_owned(), "slug".to_owned()],
				column_names: vec!["tenant_id".to_owned(), "article_slug".to_owned()],
				source: UniqueProofSource::Constraint("articles_tenant_slug_key".to_owned()),
			},
			mode: UpsertMode::GetOrCreate,
		}
	}

	#[rstest]
	#[case(
		DatabaseBackend::Postgres,
		"SELECT \"id\", \"tenant_id\", \"article_slug\", \"headline\" FROM \"articles\" WHERE \"tenant_id\" = $1 AND \"article_slug\" = $2 LIMIT 2 FOR UPDATE"
	)]
	#[case(
		DatabaseBackend::MySql,
		"SELECT `id`, `tenant_id`, `article_slug`, `headline` FROM `articles` WHERE `tenant_id` = ? AND `article_slug` = ? LIMIT 2 FOR UPDATE"
	)]
	#[case(
		DatabaseBackend::Sqlite,
		"SELECT \"id\", \"tenant_id\", \"article_slug\", \"headline\" FROM \"articles\" WHERE \"tenant_id\" = ? AND \"article_slug\" = ? LIMIT 2"
	)]
	fn select_binds_composite_lookup_and_uses_backend_locking(
		#[case] backend: DatabaseBackend,
		#[case] expected_sql: &str,
	) {
		let quote_bearing_slug = "rust's \"systems\"";
		let plan = article_plan(Some(quote_bearing_slug));

		let compiled = select_by_lookup(&plan, backend, true).expect("compile SELECT");

		assert_eq!(compiled.sql, expected_sql);
		assert_eq!(
			compiled.params,
			vec![
				QueryValue::Int(7),
				QueryValue::String(quote_bearing_slug.to_owned()),
			]
		);
		assert_eq!(compiled.sql.contains(quote_bearing_slug), false);
	}

	#[rstest]
	#[case(DatabaseBackend::Postgres, "$1")]
	#[case(DatabaseBackend::MySql, "?")]
	#[case(DatabaseBackend::Sqlite, "?")]
	fn select_renders_null_lookup_without_a_bound_slot(
		#[case] backend: DatabaseBackend,
		#[case] tenant_placeholder: &str,
	) {
		let plan = article_plan(None);

		let compiled = select_by_lookup(&plan, backend, false).expect("compile SELECT");

		assert_eq!(
			compiled.sql,
			format!(
				"SELECT {id}, {tenant}, {slug}, {headline} FROM {table} \
				 WHERE {tenant} = {tenant_placeholder} AND {slug} IS NULL LIMIT 2",
				id = quoted(backend, "id"),
				tenant = quoted(backend, "tenant_id"),
				slug = quoted(backend, "article_slug"),
				headline = quoted(backend, "headline"),
				table = quoted(backend, "articles"),
			)
		);
		assert_eq!(compiled.params, vec![QueryValue::Int(7)]);
	}

	#[rstest]
	#[case(
		DatabaseBackend::Postgres,
		"INSERT INTO \"articles\" (\"tenant_id\", \"article_slug\", \"headline\") VALUES ($1, $2, $3) ON CONFLICT (\"tenant_id\", \"article_slug\") DO NOTHING RETURNING *"
	)]
	#[case(
		DatabaseBackend::MySql,
		"INSERT INTO `articles` (`tenant_id`, `article_slug`, `headline`) VALUES (?, ?, ?)"
	)]
	#[case(
		DatabaseBackend::Sqlite,
		"INSERT INTO \"articles\" (\"tenant_id\", \"article_slug\", \"headline\") VALUES (?, ?, ?) ON CONFLICT (\"tenant_id\", \"article_slug\") DO NOTHING RETURNING *"
	)]
	fn insert_uses_backend_conflict_handling(
		#[case] backend: DatabaseBackend,
		#[case] expected_sql: &str,
	) {
		let plan = article_plan(Some("rust"));

		let compiled = insert(&plan, backend).expect("compile INSERT");

		assert_eq!(compiled.sql, expected_sql);
		assert_eq!(
			compiled.params,
			vec![
				QueryValue::Int(7),
				QueryValue::String("rust".to_owned()),
				QueryValue::String("A quoted headline".to_owned()),
			]
		);
	}

	#[rstest]
	#[case(
		DatabaseBackend::Postgres,
		"UPDATE \"article_revisions\" SET \"display_headline\" = $1 WHERE \"tenant_key\" = $2 AND \"article_key\" = $3"
	)]
	#[case(
		DatabaseBackend::MySql,
		"UPDATE `article_revisions` SET `display_headline` = ? WHERE `tenant_key` = ? AND `article_key` = ?"
	)]
	#[case(
		DatabaseBackend::Sqlite,
		"UPDATE \"article_revisions\" SET \"display_headline\" = ? WHERE \"tenant_key\" = ? AND \"article_key\" = ?"
	)]
	fn update_uses_every_composite_primary_key_column_and_alias(
		#[case] backend: DatabaseBackend,
		#[case] expected_sql: &str,
	) {
		let model = ArticleRevision {
			tenant_id: 7,
			article_id: 9,
			headline: "old".to_owned(),
		};
		let quote_bearing_headline = "reader's \"choice\"";
		let values = vec![
			TypedAssignment::new(ArticleRevision::headline_field(), quote_bearing_headline)
				.expect("encode headline"),
		];

		let compiled = update_by_primary_key(&model, &values, backend).expect("compile UPDATE");

		assert_eq!(compiled.sql, expected_sql);
		assert_eq!(
			compiled.params,
			vec![
				QueryValue::String(quote_bearing_headline.to_owned()),
				QueryValue::Int(7),
				QueryValue::Int(9),
			]
		);
		assert_eq!(compiled.sql.contains(quote_bearing_headline), false);
	}

	#[test]
	fn update_value_map_uses_every_composite_primary_key_column() {
		let model = ArticleRevision {
			tenant_id: 7,
			article_id: 9,
			headline: "old".to_owned(),
		};
		let values = BTreeMap::from([(
			"headline".to_owned(),
			DatabaseValue::String("new".to_owned()),
		)]);

		let compiled = update_values_by_primary_key(&model, &values, DatabaseBackend::Postgres)
			.expect("compile composite primary-key UPDATE");

		assert_eq!(
			compiled.sql,
			"UPDATE \"article_revisions\" SET \"display_headline\" = $1 \
			 WHERE \"tenant_key\" = $2 AND \"article_key\" = $3"
		);
		assert_eq!(
			compiled.params,
			vec![
				QueryValue::String("new".to_owned()),
				QueryValue::Int(7),
				QueryValue::Int(9),
			]
		);
	}

	#[rstest]
	fn update_rejects_empty_assignments() {
		let model = ArticleRevision {
			tenant_id: 7,
			article_id: 9,
			headline: "old".to_owned(),
		};

		let error = expect_error(
			update_by_primary_key(&model, &[], DatabaseBackend::Postgres),
			"empty assignments must fail",
		);

		assert_validation(
			error,
			"typed upsert UPDATE requires at least one assignment",
		);
	}

	#[rstest]
	fn update_rejects_missing_composite_primary_key_metadata() {
		let model = MissingCompositeMetadata {
			tenant_id: 7,
			article_id: 9,
			headline: "old".to_owned(),
		};
		let values = vec![
			TypedAssignment::new(MissingCompositeMetadata::headline_field(), "new")
				.expect("encode headline"),
		];

		let error = expect_error(
			update_by_primary_key(&model, &values, DatabaseBackend::Postgres),
			"missing composite primary-key metadata must fail",
		);

		assert_validation(
			error,
			"typed upsert UPDATE primary-key field 'article_id' is missing model metadata",
		);
	}

	#[rstest]
	fn update_rejects_missing_primary_key_value() {
		let model = MissingPrimaryKeyValue {
			other_id: 7,
			headline: "old".to_owned(),
		};
		let values = vec![
			TypedAssignment::new(MissingPrimaryKeyValue::headline_field(), "new")
				.expect("encode headline"),
		];

		let error = expect_error(
			update_by_primary_key(&model, &values, DatabaseBackend::Postgres),
			"missing primary-key value must fail",
		);

		assert_validation(
			error,
			"typed upsert UPDATE primary-key field 'id' is missing encoded model values",
		);
	}

	#[rstest]
	fn update_rejects_null_primary_key_value() {
		let model = Article {
			id: None,
			tenant_id: 7,
			slug: Some("rust".to_owned()),
			headline: "old".to_owned(),
		};
		let values =
			vec![TypedAssignment::new(Article::headline_field(), "new").expect("encode headline")];

		let error = expect_error(
			update_by_primary_key(&model, &values, DatabaseBackend::Postgres),
			"null primary-key value must fail",
		);

		assert_validation(
			error,
			"typed upsert UPDATE requires non-null primary-key field 'id'",
		);
	}

	#[rstest]
	fn select_rejects_empty_model_field_metadata() {
		let plan = UpsertPlan::<MetadataFreeModel> {
			lookup: Vec::new(),
			create: Vec::new(),
			update: Vec::new(),
			proof: UniqueProof {
				logical_fields: Vec::new(),
				column_names: Vec::new(),
				source: UniqueProofSource::PrimaryKey,
			},
			mode: UpsertMode::GetOrCreate,
		};

		let error = expect_error(
			select_by_lookup(&plan, DatabaseBackend::Postgres, false),
			"empty field metadata must fail",
		);

		assert_validation(
			error,
			"typed upsert SELECT requires field metadata for 'metadata_free_models'",
		);
	}

	fn assert_validation(error: reinhardt_core::exception::Error, expected: &str) {
		match error {
			reinhardt_core::exception::Error::Validation(message) => {
				assert_eq!(message, expected);
			}
			other => panic!("expected validation error, got {other:?}"),
		}
	}

	fn expect_error<T>(
		result: reinhardt_core::exception::Result<T>,
		message: &str,
	) -> reinhardt_core::exception::Error {
		match result {
			Ok(_) => panic!("{message}"),
			Err(error) => error,
		}
	}

	fn quoted(backend: DatabaseBackend, identifier: &str) -> String {
		match backend {
			DatabaseBackend::MySql => format!("`{identifier}`"),
			DatabaseBackend::Postgres | DatabaseBackend::Sqlite => format!("\"{identifier}\""),
		}
	}
}
