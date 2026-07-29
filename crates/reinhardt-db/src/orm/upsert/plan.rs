use crate::orm::field_codec::DatabaseValue;
use crate::orm::inspection::{ConstraintType, FieldInfo};
use crate::orm::model::Model;
use crate::orm::upsert::assignment::TypedAssignment;
use reinhardt_core::exception::{Error, Result};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpsertMode {
	GetOrCreate,
	UpdateOrCreate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UniqueProofSource {
	PrimaryKey,
	UniqueField,
	Constraint(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UniqueProof {
	pub(crate) logical_fields: Vec<String>,
	pub(crate) column_names: Vec<String>,
	pub(crate) source: UniqueProofSource,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UpsertPlan<M> {
	pub(crate) lookup: Vec<TypedAssignment<M>>,
	pub(crate) create: Vec<TypedAssignment<M>>,
	pub(crate) update: Vec<TypedAssignment<M>>,
	pub(crate) proof: UniqueProof,
	pub(crate) mode: UpsertMode,
}

struct ProofCandidate {
	primary_key_rank: u8,
	metadata_index: usize,
	logical_fields: Vec<String>,
	nulls_distinct: Option<bool>,
	source: UniqueProofSource,
}

pub(crate) fn normalize<M: Model>(
	lookup: Vec<TypedAssignment<M>>,
	create: Vec<TypedAssignment<M>>,
	update: Vec<TypedAssignment<M>>,
	mode: UpsertMode,
) -> Result<UpsertPlan<M>> {
	if lookup.is_empty() {
		return Err(Error::Validation(
			"upsert lookup cannot be empty".to_owned(),
		));
	}

	let field_metadata = M::field_metadata();
	let metadata_by_name = field_metadata
		.iter()
		.map(|field| (field.name.as_str(), field))
		.collect::<HashMap<_, _>>();
	validate_assignments::<M>(&lookup, "lookup", true, &metadata_by_name)?;
	let create_role = match mode {
		UpsertMode::GetOrCreate => "default",
		UpsertMode::UpdateOrCreate => "create_default",
	};
	validate_assignments::<M>(&create, create_role, true, &metadata_by_name)?;
	validate_assignments::<M>(&update, "set", true, &metadata_by_name)?;

	if matches!(mode, UpsertMode::GetOrCreate) && !update.is_empty() {
		return Err(Error::Validation(
			"get_or_create does not accept set assignments".to_owned(),
		));
	}

	let lookup_fields = lookup
		.iter()
		.map(|assignment| assignment.logical_name)
		.collect::<HashSet<_>>();
	reject_lookup_overlap(&lookup_fields, &create, create_role)?;
	reject_lookup_overlap(&lookup_fields, &update, "set")?;

	let proof = select_unique_proof::<M>(&lookup, &field_metadata, &metadata_by_name)?;
	let normalized_create = match mode {
		UpsertMode::GetOrCreate => {
			let mut values = lookup.clone();
			values.extend(create.iter().cloned());
			values
		}
		UpsertMode::UpdateOrCreate => {
			let mut values = lookup.clone();
			for assignment in update.iter().cloned() {
				overlay_assignment(&mut values, assignment);
			}
			for assignment in create.iter().cloned() {
				overlay_assignment(&mut values, assignment);
			}
			values
		}
	};

	Ok(UpsertPlan {
		lookup,
		create: normalized_create,
		update,
		proof,
		mode,
	})
}

fn validate_assignments<M: Model>(
	assignments: &[TypedAssignment<M>],
	role: &str,
	require_writable: bool,
	metadata_by_name: &HashMap<&str, &FieldInfo>,
) -> Result<()> {
	let mut seen = HashSet::new();
	for assignment in assignments {
		if !seen.insert(assignment.logical_name) {
			return Err(Error::Validation(format!(
				"duplicate upsert {role} assignment for field '{}'",
				assignment.logical_name
			)));
		}
		if assignment.logical_name.contains("__")
			|| assignment.logical_name.contains('.')
			|| assignment.column_name.contains("__")
			|| assignment.column_name.contains('.')
		{
			return Err(Error::Validation(format!(
				"upsert field '{}' cannot traverse relations",
				assignment.logical_name
			)));
		}
		let Some(field) = metadata_by_name.get(assignment.logical_name) else {
			return Err(Error::Validation(format!(
				"unknown upsert field '{}'",
				assignment.logical_name
			)));
		};
		let expected_column = field.db_column_name();
		if assignment.column_name != expected_column {
			return Err(Error::Validation(format!(
				"upsert field '{}' expected database column '{}', got '{}'",
				assignment.logical_name, expected_column, assignment.column_name
			)));
		}
		if require_writable {
			if M::generated_field_names().iter().any(|generated| {
				*generated == assignment.logical_name || *generated == assignment.column_name
			}) {
				return Err(Error::Validation(format!(
					"upsert field '{}' is database-generated and not writable",
					assignment.logical_name
				)));
			}
			if !field.editable {
				return Err(Error::Validation(format!(
					"upsert field '{}' is not writable",
					assignment.logical_name
				)));
			}
		}
	}
	Ok(())
}

fn reject_lookup_overlap<M>(
	lookup_fields: &HashSet<&str>,
	assignments: &[TypedAssignment<M>],
	role: &str,
) -> Result<()> {
	if let Some(assignment) = assignments
		.iter()
		.find(|assignment| lookup_fields.contains(assignment.logical_name))
	{
		return Err(Error::Validation(format!(
			"upsert field '{}' cannot be used for both lookup and {role}",
			assignment.logical_name
		)));
	}
	Ok(())
}

fn select_unique_proof<M: Model>(
	lookup: &[TypedAssignment<M>],
	field_metadata: &[FieldInfo],
	metadata_by_name: &HashMap<&str, &FieldInfo>,
) -> Result<UniqueProof> {
	let mut candidates = Vec::new();
	let primary_key_fields = field_metadata
		.iter()
		.filter(|field| field.primary_key)
		.map(|field| field.name.clone())
		.collect::<Vec<_>>();
	if !primary_key_fields.is_empty() {
		candidates.push(ProofCandidate {
			primary_key_rank: 0,
			metadata_index: 0,
			logical_fields: primary_key_fields,
			nulls_distinct: Some(false),
			source: UniqueProofSource::PrimaryKey,
		});
	}
	for (metadata_index, field) in field_metadata.iter().enumerate() {
		if field.unique {
			candidates.push(ProofCandidate {
				primary_key_rank: 1,
				metadata_index,
				logical_fields: vec![field.name.clone()],
				nulls_distinct: None,
				source: UniqueProofSource::UniqueField,
			});
		}
	}
	for (constraint_index, constraint) in M::constraint_metadata().into_iter().enumerate() {
		if constraint.constraint_type != ConstraintType::Unique
			|| constraint.fields.is_empty()
			|| constraint.condition.is_some()
			|| constraint.deferrable
			|| constraint
				.fields
				.iter()
				.any(|field| !metadata_by_name.contains_key(field.as_str()))
		{
			continue;
		}
		candidates.push(ProofCandidate {
			primary_key_rank: 1,
			metadata_index: field_metadata.len() + constraint_index,
			logical_fields: constraint.fields,
			nulls_distinct: constraint.nulls_distinct,
			source: UniqueProofSource::Constraint(constraint.name),
		});
	}
	candidates.sort_by_key(|candidate| {
		(
			candidate.primary_key_rank,
			candidate.logical_fields.len(),
			candidate.metadata_index,
		)
	});

	let candidate = candidates.into_iter().find(|candidate| {
		candidate.logical_fields.iter().all(|field_name| {
			let Some(assignment) = lookup
				.iter()
				.find(|assignment| assignment.logical_name == field_name)
			else {
				return false;
			};
			if assignment.value != DatabaseValue::Null {
				return true;
			}
			matches!(candidate.source, UniqueProofSource::Constraint(_))
				&& candidate.nulls_distinct == Some(false)
				&& metadata_by_name
					.get(field_name.as_str())
					.is_some_and(|field| field.nullable)
		})
	});
	let Some(candidate) = candidate else {
		return Err(Error::Validation(
			"upsert lookup must cover an immediate unconditional unique constraint".to_owned(),
		));
	};
	let column_names = candidate
		.logical_fields
		.iter()
		.map(|field_name| {
			metadata_by_name
				.get(field_name.as_str())
				.expect("proof fields were validated against model metadata")
				.db_column_name()
				.to_owned()
		})
		.collect();

	Ok(UniqueProof {
		logical_fields: candidate.logical_fields,
		column_names,
		source: candidate.source,
	})
}

fn overlay_assignment<M>(
	assignments: &mut Vec<TypedAssignment<M>>,
	assignment: TypedAssignment<M>,
) {
	if let Some(index) = assignments
		.iter()
		.position(|existing| existing.logical_name == assignment.logical_name)
	{
		assignments[index] = assignment;
	} else {
		assignments.push(assignment);
	}
}

#[cfg(test)]
mod tests {
	use super::{UniqueProofSource, UpsertMode, normalize};
	use crate::orm::expressions::FieldRef;
	use crate::orm::inspection::{ConstraintInfo, ConstraintType, FieldInfo};
	use crate::orm::model::{FieldSelector, Model};
	use crate::orm::upsert::assignment::TypedAssignment;
	use crate::orm::{DatabaseValue, Manager};
	use reinhardt_core::macros::model;
	use rstest::*;
	use serde::{Deserialize, Serialize};
	use std::collections::HashMap;

	#[derive(Clone, Debug, Default, Serialize, Deserialize)]
	struct Article {
		id: i64,
	}

	#[derive(Clone)]
	struct ArticleFields;

	impl FieldSelector for ArticleFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	#[model(app_label = "tests", table_name = "nullable_primary_key_articles")]
	#[derive(Clone, Debug, Serialize, Deserialize)]
	struct NullablePrimaryKeyArticle {
		#[field(primary_key = true, auto_increment = false)]
		id: Option<i64>,
	}

	fn field(
		name: &str,
		db_column: Option<&str>,
		primary_key: bool,
		unique: bool,
		nullable: bool,
		editable: bool,
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
			editable,
			default: None,
			db_default: None,
			db_column: db_column.map(str::to_owned),
			choices: None,
			attributes: HashMap::new(),
		}
	}

	fn unique_constraint(
		name: &str,
		fields: &[&str],
		condition: Option<&str>,
		deferrable: bool,
		nulls_distinct: Option<bool>,
	) -> ConstraintInfo {
		ConstraintInfo {
			name: name.to_owned(),
			constraint_type: ConstraintType::Unique,
			definition: format!("UNIQUE ({})", fields.join(", ")),
			fields: fields.iter().map(|field| (*field).to_owned()).collect(),
			condition: condition.map(str::to_owned),
			deferrable,
			nulls_distinct,
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
			Some(self.id)
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = value;
		}

		fn primary_key_column() -> &'static str {
			"article_id"
		}

		fn field_metadata() -> Vec<FieldInfo> {
			vec![
				field("id", Some("article_id"), true, false, false, true),
				field("slug", Some("article_slug"), false, true, false, true),
				field("tenant", Some("tenant_id"), false, false, false, true),
				field("external", Some("external_id"), false, false, false, true),
				field("region", None, false, false, false, true),
				field("legacy", None, false, false, false, true),
				field("nullable_unique", None, false, true, true, true),
				field("nullable_tenant", None, false, false, true, true),
				field("nullable_code", None, false, false, true, true),
				field("title", Some("article_title"), false, false, false, true),
				field("readonly", None, false, false, false, false),
				field("generated", None, false, false, false, true),
			]
		}

		fn constraint_metadata() -> Vec<ConstraintInfo> {
			vec![
				unique_constraint(
					"redundant_primary_key_nulls_not_distinct",
					&["id"],
					None,
					false,
					Some(false),
				),
				unique_constraint(
					"tenant_external_region_unique",
					&["tenant", "external", "region"],
					None,
					false,
					None,
				),
				unique_constraint(
					"tenant_external_unique",
					&["tenant", "external"],
					None,
					false,
					None,
				),
				unique_constraint(
					"tenant_region_unique",
					&["tenant", "region"],
					None,
					false,
					None,
				),
				unique_constraint(
					"conditional_unique",
					&["region", "legacy"],
					Some("legacy IS NOT NULL"),
					false,
					None,
				),
				unique_constraint("deferred_unique", &["external", "legacy"], None, true, None),
				unique_constraint(
					"nullable_pair_unique",
					&["nullable_tenant", "nullable_code"],
					None,
					false,
					Some(false),
				),
			]
		}

		fn generated_field_names() -> &'static [&'static str] {
			&["generated"]
		}
	}

	fn assignment<T, V>(
		logical_name: &'static str,
		column_name: &'static str,
		value: V,
	) -> TypedAssignment<Article>
	where
		T: crate::orm::field_codec::DatabaseField,
		V: crate::orm::field_codec::IntoFieldValue<T>,
	{
		// SAFETY: test descriptors use the matching declared field type unless a test
		// intentionally verifies forged descriptor rejection.
		let field = unsafe { FieldRef::<Article, T>::from_model_field(logical_name, column_name) };
		TypedAssignment::new(field, value).expect("encode test assignment")
	}

	fn string_assignment(
		logical_name: &'static str,
		column_name: &'static str,
		value: &str,
	) -> TypedAssignment<Article> {
		assignment::<String, _>(logical_name, column_name, value)
	}

	fn assert_validation(error: reinhardt_core::exception::Error, expected: &str) {
		match error {
			reinhardt_core::exception::Error::Validation(message) => {
				assert_eq!(message, expected);
			}
			other => panic!("expected validation error, got {other}"),
		}
	}

	#[rstest]
	fn normalize_rejects_an_empty_lookup() {
		let error =
			normalize::<Article>(Vec::new(), Vec::new(), Vec::new(), UpsertMode::GetOrCreate)
				.expect_err("empty lookup must fail");

		assert_validation(error, "upsert lookup cannot be empty");
	}

	#[rstest]
	#[case(
		vec![assignment::<i64, _>("id", "article_id", 1_i64)],
		UniqueProofSource::PrimaryKey,
		vec!["id".to_owned()],
		vec!["article_id".to_owned()]
	)]
	#[case(
		vec![string_assignment("slug", "article_slug", "rust")],
		UniqueProofSource::UniqueField,
		vec!["slug".to_owned()],
		vec!["article_slug".to_owned()]
	)]
	#[case(
		vec![
			string_assignment("tenant", "tenant_id", "acme"),
			string_assignment("external", "external_id", "42"),
		],
		UniqueProofSource::Constraint("tenant_external_unique".to_owned()),
		vec!["tenant".to_owned(), "external".to_owned()],
		vec!["tenant_id".to_owned(), "external_id".to_owned()]
	)]
	fn normalize_accepts_each_supported_unique_proof(
		#[case] lookup: Vec<TypedAssignment<Article>>,
		#[case] source: UniqueProofSource,
		#[case] logical_fields: Vec<String>,
		#[case] column_names: Vec<String>,
	) {
		let plan = normalize::<Article>(lookup, Vec::new(), Vec::new(), UpsertMode::GetOrCreate)
			.expect("unique lookup must normalize");

		assert_eq!(plan.proof.source, source);
		assert_eq!(plan.proof.logical_fields, logical_fields);
		assert_eq!(plan.proof.column_names, column_names);
	}

	#[rstest]
	fn normalize_allows_extra_lookup_fields() {
		let plan = normalize::<Article>(
			vec![
				string_assignment("slug", "article_slug", "rust"),
				string_assignment("title", "article_title", "Rust"),
			],
			Vec::new(),
			Vec::new(),
			UpsertMode::GetOrCreate,
		)
		.expect("extra lookup fields are valid");

		assert_eq!(plan.proof.source, UniqueProofSource::UniqueField);
		assert_eq!(plan.lookup.len(), 2);
	}

	#[rstest]
	fn normalize_selects_primary_key_before_other_proofs() {
		let plan = normalize::<Article>(
			vec![
				string_assignment("slug", "article_slug", "rust"),
				assignment::<i64, _>("id", "article_id", 1_i64),
			],
			Vec::new(),
			Vec::new(),
			UpsertMode::GetOrCreate,
		)
		.expect("covered primary key must normalize");

		assert_eq!(plan.proof.source, UniqueProofSource::PrimaryKey);
	}

	#[rstest]
	fn normalize_selects_smallest_then_first_metadata_proof() {
		let plan = normalize::<Article>(
			vec![
				string_assignment("tenant", "tenant_id", "acme"),
				string_assignment("external", "external_id", "42"),
				string_assignment("region", "region", "eu"),
			],
			Vec::new(),
			Vec::new(),
			UpsertMode::GetOrCreate,
		)
		.expect("covered composite constraints must normalize");

		assert_eq!(
			plan.proof.source,
			UniqueProofSource::Constraint("tenant_external_unique".to_owned())
		);
	}

	#[rstest]
	#[case(vec![
		string_assignment("region", "region", "eu"),
		string_assignment("legacy", "legacy", "old"),
	])]
	#[case(vec![
		string_assignment("external", "external_id", "42"),
		string_assignment("legacy", "legacy", "old"),
	])]
	fn normalize_rejects_conditional_and_deferred_proofs(
		#[case] lookup: Vec<TypedAssignment<Article>>,
	) {
		let error = normalize::<Article>(lookup, Vec::new(), Vec::new(), UpsertMode::GetOrCreate)
			.expect_err("unsupported uniqueness semantics must fail");

		assert_validation(
			error,
			"upsert lookup must cover an immediate unconditional unique constraint",
		);
	}

	#[rstest]
	fn normalize_rejects_nullable_unique_null_without_nulls_not_distinct() {
		let error = normalize::<Article>(
			vec![assignment::<Option<String>, _>(
				"nullable_unique",
				"nullable_unique",
				None::<String>,
			)],
			Vec::new(),
			Vec::new(),
			UpsertMode::GetOrCreate,
		)
		.expect_err("nullable distinct uniqueness cannot prove one row");

		assert_validation(
			error,
			"upsert lookup must cover an immediate unconditional unique constraint",
		);
	}

	#[rstest]
	fn normalize_rejects_null_primary_key_from_generated_accessor() {
		let lookup = TypedAssignment::new(NullablePrimaryKeyArticle::field_id(), None::<i64>)
			.expect("encode nullable primary key");

		let error = normalize::<NullablePrimaryKeyArticle>(
			vec![lookup],
			Vec::new(),
			Vec::new(),
			UpsertMode::GetOrCreate,
		)
		.expect_err("NULL primary key cannot prove one row");

		assert_validation(
			error,
			"upsert lookup must cover an immediate unconditional unique constraint",
		);
	}

	#[rstest]
	fn normalize_rejects_null_for_non_nullable_constraint_field() {
		let error = normalize::<Article>(
			vec![assignment::<Option<i64>, _>(
				"id",
				"article_id",
				None::<i64>,
			)],
			Vec::new(),
			Vec::new(),
			UpsertMode::GetOrCreate,
		)
		.expect_err("NULL cannot satisfy a constraint over a non-nullable field");

		assert_validation(
			error,
			"upsert lookup must cover an immediate unconditional unique constraint",
		);
	}

	#[rstest]
	fn normalize_accepts_nullable_nulls_not_distinct_constraint() {
		let plan = normalize::<Article>(
			vec![
				assignment::<Option<String>, _>(
					"nullable_tenant",
					"nullable_tenant",
					None::<String>,
				),
				assignment::<Option<String>, _>("nullable_code", "nullable_code", None::<String>),
			],
			Vec::new(),
			Vec::new(),
			UpsertMode::GetOrCreate,
		)
		.expect("NULLS NOT DISTINCT proof must normalize");

		assert_eq!(
			plan.proof.source,
			UniqueProofSource::Constraint("nullable_pair_unique".to_owned())
		);
	}

	#[rstest]
	#[case(
		vec![
			string_assignment("slug", "article_slug", "rust"),
			string_assignment("slug", "article_slug", "rust-2"),
		],
		Vec::new(),
		Vec::new(),
		UpsertMode::GetOrCreate,
		"duplicate upsert lookup assignment for field 'slug'"
	)]
	#[case(
		vec![string_assignment("slug", "article_slug", "rust")],
		vec![
			string_assignment("title", "article_title", "one"),
			string_assignment("title", "article_title", "two"),
		],
		Vec::new(),
		UpsertMode::GetOrCreate,
		"duplicate upsert default assignment for field 'title'"
	)]
	#[case(
		vec![string_assignment("slug", "article_slug", "rust")],
		Vec::new(),
		vec![
			string_assignment("title", "article_title", "one"),
			string_assignment("title", "article_title", "two"),
		],
		UpsertMode::UpdateOrCreate,
		"duplicate upsert set assignment for field 'title'"
	)]
	#[case(
		vec![string_assignment("slug", "article_slug", "rust")],
		vec![
			string_assignment("title", "article_title", "one"),
			string_assignment("title", "article_title", "two"),
		],
		Vec::new(),
		UpsertMode::UpdateOrCreate,
		"duplicate upsert create_default assignment for field 'title'"
	)]
	fn normalize_rejects_duplicates_in_every_role(
		#[case] lookup: Vec<TypedAssignment<Article>>,
		#[case] create: Vec<TypedAssignment<Article>>,
		#[case] update: Vec<TypedAssignment<Article>>,
		#[case] mode: UpsertMode,
		#[case] expected: &str,
	) {
		let error = normalize::<Article>(lookup, create, update, mode)
			.expect_err("duplicate role assignment must fail");

		assert_validation(error, expected);
	}

	#[rstest]
	#[case(
		vec![string_assignment("slug", "article_slug", "rust")],
		Vec::new(),
		UpsertMode::GetOrCreate,
		"upsert field 'slug' cannot be used for both lookup and default"
	)]
	#[case(
		Vec::new(),
		vec![string_assignment("slug", "article_slug", "updated")],
		UpsertMode::UpdateOrCreate,
		"upsert field 'slug' cannot be used for both lookup and set"
	)]
	#[case(
		vec![string_assignment("slug", "article_slug", "created")],
		Vec::new(),
		UpsertMode::UpdateOrCreate,
		"upsert field 'slug' cannot be used for both lookup and create_default"
	)]
	fn normalize_rejects_every_lookup_write_overlap(
		#[case] create: Vec<TypedAssignment<Article>>,
		#[case] update: Vec<TypedAssignment<Article>>,
		#[case] mode: UpsertMode,
		#[case] expected: &str,
	) {
		let error = normalize::<Article>(
			vec![string_assignment("slug", "article_slug", "rust")],
			create,
			update,
			mode,
		)
		.expect_err("lookup/write overlap must fail");

		assert_validation(error, expected);
	}

	#[rstest]
	fn normalize_allows_set_plus_create_default_with_create_precedence() {
		let plan = normalize::<Article>(
			vec![string_assignment("slug", "article_slug", "rust")],
			vec![string_assignment("title", "article_title", "created")],
			vec![string_assignment("title", "article_title", "updated")],
			UpsertMode::UpdateOrCreate,
		)
		.expect("set/create_default overlap is valid");

		assert_eq!(
			plan.create
				.iter()
				.map(|assignment| (assignment.logical_name, assignment.value.clone(),))
				.collect::<Vec<_>>(),
			vec![
				("slug", DatabaseValue::String("rust".to_owned())),
				("title", DatabaseValue::String("created".to_owned())),
			]
		);
		assert_eq!(
			plan.update
				.iter()
				.map(|assignment| (assignment.logical_name, assignment.value.clone(),))
				.collect::<Vec<_>>(),
			vec![("title", DatabaseValue::String("updated".to_owned()))]
		);
	}

	#[rstest]
	#[case(
		string_assignment("missing", "missing", "value"),
		"unknown upsert field 'missing'"
	)]
	#[case(
		string_assignment("title", "wrong_title", "value"),
		"upsert field 'title' expected database column 'article_title', got 'wrong_title'"
	)]
	#[case(
		string_assignment("author__email", "author__email", "value"),
		"upsert field 'author__email' cannot traverse relations"
	)]
	fn normalize_rejects_unknown_inconsistent_and_traversal_fields(
		#[case] create: TypedAssignment<Article>,
		#[case] expected: &str,
	) {
		let error = normalize::<Article>(
			vec![string_assignment("slug", "article_slug", "rust")],
			vec![create],
			Vec::new(),
			UpsertMode::GetOrCreate,
		)
		.expect_err("invalid descriptor must fail");

		assert_validation(error, expected);
	}

	#[rstest]
	#[case(
		string_assignment("readonly", "readonly", "value"),
		"upsert field 'readonly' is not writable"
	)]
	#[case(
		string_assignment("generated", "generated", "value"),
		"upsert field 'generated' is database-generated and not writable"
	)]
	fn normalize_rejects_non_writable_fields(
		#[case] create: TypedAssignment<Article>,
		#[case] expected: &str,
	) {
		let error = normalize::<Article>(
			vec![string_assignment("slug", "article_slug", "rust")],
			vec![create],
			Vec::new(),
			UpsertMode::GetOrCreate,
		)
		.expect_err("non-writable assignment must fail");

		assert_validation(error, expected);
	}

	#[rstest]
	#[case(
		string_assignment("readonly", "readonly", "value"),
		"upsert field 'readonly' is not writable"
	)]
	#[case(
		string_assignment("generated", "generated", "value"),
		"upsert field 'generated' is database-generated and not writable"
	)]
	fn normalize_rejects_non_writable_lookup_fields(
		#[case] lookup: TypedAssignment<Article>,
		#[case] expected: &str,
	) {
		let error = normalize::<Article>(
			vec![lookup],
			Vec::new(),
			Vec::new(),
			UpsertMode::GetOrCreate,
		)
		.expect_err("lookup fields are copied into the create write");

		assert_validation(error, expected);
	}

	#[rstest]
	fn normalize_keeps_logical_and_physical_primary_key_names() {
		let plan = normalize::<Article>(
			vec![assignment::<i64, _>("id", "article_id", 7_i64)],
			Vec::new(),
			Vec::new(),
			UpsertMode::GetOrCreate,
		)
		.expect("aliased primary key must normalize");

		assert_eq!(plan.lookup[0].logical_name, "id");
		assert_eq!(plan.lookup[0].column_name, "article_id");
		assert_eq!(plan.proof.logical_fields, vec!["id".to_owned()]);
		assert_eq!(plan.proof.column_names, vec!["article_id".to_owned()]);
	}
}
