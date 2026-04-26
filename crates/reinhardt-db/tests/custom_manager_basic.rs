//! Smoke tests for the [`CustomManager`] trait, the [`HasCustomManager`]
//! opt-in trait, and the `#[model(manager = ...)]` attribute (Issue #3980).
//!
//! These tests verify three properties:
//!
//! 1. The blanket `impl<M: Model> CustomManager for Manager<M>` makes every
//!    existing [`Manager<M>`] satisfy the trait without modification, so
//!    generic functions can rely on `CustomManager<Model = M>` and still
//!    accept `Manager<M>` from `Model::objects()`.
//! 2. A user-defined struct implementing [`CustomManager`] interoperates with
//!    [`QuerySet`] exactly like the canonical [`Manager`].
//! 3. The `#[model(manager = ...)]` macro argument generates an
//!    `impl HasCustomManager` that exposes `Model::custom_manager()` returning
//!    the user-supplied type.
//!
//! Database round-trips are covered by the parity test suite under
//! `tests/integration/tests/orm/custom_manager_*.rs`. This file focuses on
//! type-level wiring and on the SQL builder paths that do not require a live
//! connection, so it can run in any environment.

use std::collections::HashMap;

use rstest::rstest;
use serde::{Deserialize, Serialize};

use reinhardt_db::orm::connection::DatabaseBackend;
use reinhardt_db::orm::custom_manager::{CustomManager, HasCustomManager};
use reinhardt_db::orm::manager::Manager;
use reinhardt_db::orm::model::{FieldSelector, Model};
use reinhardt_db::orm::query::{Filter, FilterOperator, FilterValue, QuerySet};

// -----------------------------------------------------------------------------
// Test fixtures
// -----------------------------------------------------------------------------

/// Minimal `Model` used to exercise the trait wiring without invoking the full
/// `#[model(...)]` macro (which is exercised separately by the trybuild tests
/// and the integration tests).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Article {
	id: Option<i64>,
	title: String,
	is_archived: bool,
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
}

/// User-defined manager that filters out archived rows by default.
///
/// Implements only the two required items (`type Model`, `fn new`) plus an
/// override of `all`. Every other operation comes from the trait's default
/// implementation, demonstrating the boilerplate-minimization promise of the
/// design (DESIGN_PHILOSOPHY #8).
#[derive(Default)]
struct ActiveArticleManager;

impl CustomManager for ActiveArticleManager {
	type Model = Article;

	fn new() -> Self {
		Self
	}

	fn all(&self) -> QuerySet<Article> {
		Manager::<Article>::new().all().filter(Filter::new(
			"is_archived".to_string(),
			FilterOperator::Eq,
			FilterValue::Boolean(false),
		))
	}
}

impl HasCustomManager for Article {
	type Manager = ActiveArticleManager;
}

/// Manager that vetoes any save whose title is empty, exercising the
/// [`CustomManager::before_save`] hook.
#[derive(Default)]
struct GuardedArticleManager;

impl CustomManager for GuardedArticleManager {
	type Model = Article;

	fn new() -> Self {
		Self
	}

	fn before_save(&self, model: &mut Article) -> reinhardt_core::exception::Result<()> {
		if model.title.trim().is_empty() {
			return Err(reinhardt_core::exception::Error::Database(
				"title must not be empty".into(),
			));
		}
		Ok(())
	}
}

// -----------------------------------------------------------------------------
// Tests: blanket impl on Manager<M>
// -----------------------------------------------------------------------------

/// Generic helper that accepts any [`CustomManager`] for `Article`.
///
/// Used to confirm that both the canonical `Manager<Article>` and a
/// user-defined struct can be passed to the same generic API.
fn count_via<M: CustomManager<Model = Article>>(m: &M) -> usize {
	// The default builder methods do not run SQL, so we can call them
	// without a live database and observe the resulting `QuerySet`.
	m.all().filters().len()
}

#[rstest]
fn blanket_impl_lets_existing_manager_satisfy_custom_manager() {
	// Arrange
	let manager = Manager::<Article>::new();

	// Act
	let filter_count = count_via(&manager);

	// Assert
	// `Manager::all()` returns an empty `QuerySet`, so no filters are present.
	assert_eq!(filter_count, 0);
}

#[rstest]
fn user_defined_manager_can_be_passed_through_generic_api() {
	// Arrange
	let manager = ActiveArticleManager;

	// Act
	let filter_count = count_via(&manager);

	// Assert
	// `ActiveArticleManager::all` adds exactly one filter (`is_archived = false`).
	assert_eq!(filter_count, 1);
}

#[rstest]
fn blanket_impl_delegates_filter_to_manager_inherent() {
	// Arrange
	let manager = Manager::<Article>::new();

	// Act
	let qs = CustomManager::filter(
		&manager,
		"title",
		FilterOperator::Eq,
		FilterValue::String("rust".into()),
	);

	// Assert
	assert_eq!(qs.filters().len(), 1);
	assert_eq!(qs.filters()[0].field, "title");
}

#[rstest]
fn blanket_impl_get_returns_pk_filtered_queryset() {
	// Arrange
	let manager = Manager::<Article>::new();

	// Act
	let qs = CustomManager::get(&manager, 7_i64);

	// Assert
	assert_eq!(qs.filters().len(), 1);
	assert_eq!(qs.filters()[0].field, "id");
}

// -----------------------------------------------------------------------------
// Tests: HasCustomManager dispatch
// -----------------------------------------------------------------------------

#[rstest]
fn has_custom_manager_returns_user_supplied_type() {
	// Arrange + Act
	let manager: ActiveArticleManager = <Article as HasCustomManager>::custom_manager();

	// Assert: the default-filtered `all()` yields the expected filter.
	assert_eq!(manager.all().filters().len(), 1);
}

#[rstest]
fn has_custom_manager_default_filter_is_applied() {
	// Arrange
	let manager = Article::custom_manager();

	// Act
	let qs = manager.all();

	// Assert: a single filter targeting `is_archived` is in place. The exact
	// `FilterOperator`/`FilterValue` variants do not implement `PartialEq`,
	// so we assert the field name and rely on the `Debug` representation
	// for the operator/value, which is enough to confirm the override took
	// effect (a stricter check is performed by the parity test suite, which
	// compares the rendered SQL byte-for-byte).
	let filters = qs.filters();
	assert_eq!(filters.len(), 1);
	assert_eq!(filters[0].field, "is_archived");
	let dbg = format!("{:?}", filters[0]);
	assert!(dbg.contains("Eq"), "expected Eq operator, got: {dbg}");
	assert!(
		dbg.contains("Boolean(false)"),
		"expected Boolean(false) value, got: {dbg}"
	);
}

// -----------------------------------------------------------------------------
// Tests: hooks
// -----------------------------------------------------------------------------

#[rstest]
fn before_save_default_is_a_noop() {
	// Arrange
	let manager = Manager::<Article>::new();
	let mut article = Article {
		id: None,
		title: String::new(),
		is_archived: false,
	};

	// Act
	let result = CustomManager::before_save(&manager, &mut article);

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn custom_before_save_can_veto_with_error() {
	// Arrange
	let manager = GuardedArticleManager;
	let mut article = Article {
		id: None,
		title: "   ".into(),
		is_archived: false,
	};

	// Act
	let result = manager.before_save(&mut article);

	// Assert
	assert!(result.is_err());
}

#[rstest]
fn custom_before_save_passes_for_valid_input() {
	// Arrange
	let manager = GuardedArticleManager;
	let mut article = Article {
		id: None,
		title: "Custom Managers in Reinhardt".into(),
		is_archived: false,
	};

	// Act
	let result = manager.before_save(&mut article);

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn before_delete_default_is_a_noop() {
	// Arrange
	let manager = Manager::<Article>::new();
	let article = Article {
		id: Some(1),
		title: "to delete".into(),
		is_archived: false,
	};

	// Act
	let result = CustomManager::before_delete(&manager, &article);

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn before_bulk_update_default_is_a_noop_and_keeps_models_unchanged() {
	// Arrange
	let manager = Manager::<Article>::new();
	let mut models = vec![
		Article {
			id: Some(1),
			title: "first".into(),
			is_archived: false,
		},
		Article {
			id: Some(2),
			title: "second".into(),
			is_archived: false,
		},
	];
	let snapshot = models.clone();

	// Act
	let result = CustomManager::before_bulk_update(&manager, &mut models);

	// Assert
	assert!(result.is_ok());
	assert_eq!(models, snapshot);
}

// -----------------------------------------------------------------------------
// Tests: SQL parity (delegation to Manager<M>)
// -----------------------------------------------------------------------------

#[rstest]
fn bulk_create_sql_via_trait_matches_inherent_method() {
	// Arrange
	let manager = Manager::<Article>::new();
	let models = vec![
		Article {
			id: None,
			title: "alpha".into(),
			is_archived: false,
		},
		Article {
			id: None,
			title: "beta".into(),
			is_archived: true,
		},
	];

	// Act
	let inherent_sql = manager.bulk_create_sql(&models, DatabaseBackend::Postgres);
	let trait_sql = CustomManager::bulk_create_sql(&manager, &models, DatabaseBackend::Postgres);

	// Assert
	assert_eq!(inherent_sql, trait_sql);
	assert!(inherent_sql.contains("INSERT INTO"));
	assert!(inherent_sql.contains("articles"));
}

#[rstest]
fn get_or_create_sql_via_trait_matches_inherent_method() {
	// Arrange
	let manager = Manager::<Article>::new();
	let mut lookup = HashMap::new();
	lookup.insert("title".into(), "Reinhardt Custom Managers".into());
	let defaults = HashMap::new();

	// Act
	let (inherent_select, inherent_insert) =
		manager.get_or_create_sql(&lookup, &defaults, DatabaseBackend::Postgres);
	let (trait_select, trait_insert) =
		CustomManager::get_or_create_sql(&manager, &lookup, &defaults, DatabaseBackend::Postgres);

	// Assert: trait dispatch produces identical SQL to the inherent path.
	assert_eq!(inherent_select, trait_select);
	assert_eq!(inherent_insert, trait_insert);
}

#[rstest]
#[case::postgres(DatabaseBackend::Postgres)]
#[case::mysql(DatabaseBackend::MySql)]
#[case::sqlite(DatabaseBackend::Sqlite)]
fn bulk_create_sql_parity_across_backends(#[case] backend: DatabaseBackend) {
	// Arrange
	let manager = Manager::<Article>::new();
	let models = vec![Article {
		id: None,
		title: "parity".into(),
		is_archived: false,
	}];

	// Act
	let inherent_sql = manager.bulk_create_sql(&models, backend);
	let trait_sql = CustomManager::bulk_create_sql(&manager, &models, backend);

	// Assert: trait path matches inherent path on every supported backend.
	assert_eq!(inherent_sql, trait_sql);
}

#[rstest]
fn get_or_create_sql_parity_with_defaults() {
	// Arrange
	let manager = Manager::<Article>::new();
	let mut lookup = HashMap::new();
	lookup.insert("title".into(), "Reinhardt Custom Managers".into());
	let mut defaults = HashMap::new();
	defaults.insert("is_archived".into(), "false".into());

	// Act
	let (inherent_select, inherent_insert) =
		manager.get_or_create_sql(&lookup, &defaults, DatabaseBackend::Postgres);
	let (trait_select, trait_insert) =
		CustomManager::get_or_create_sql(&manager, &lookup, &defaults, DatabaseBackend::Postgres);

	// Assert: defaults map is preserved through trait dispatch.
	assert_eq!(inherent_select, trait_select);
	assert_eq!(inherent_insert, trait_insert);
}

#[rstest]
fn delete_queryset_sql_via_trait_matches_inherent_method() {
	// Arrange
	let manager = Manager::<Article>::new();
	let qs = manager.filter(
		"is_archived",
		FilterOperator::Eq,
		FilterValue::Boolean(true),
	);

	// Act
	let (inherent_sql, inherent_params) = manager.delete_queryset(&qs);
	let (trait_sql, trait_params) = CustomManager::delete_queryset(&manager, &qs);

	// Assert
	assert_eq!(inherent_sql, trait_sql);
	assert_eq!(inherent_params, trait_params);
}

#[rstest]
fn update_queryset_sql_via_trait_matches_inherent_method() {
	// Arrange
	let manager = Manager::<Article>::new();
	let qs = manager.filter(
		"is_archived",
		FilterOperator::Eq,
		FilterValue::Boolean(false),
	);
	let updates: &[(&str, &str)] = &[("title", "renamed")];

	// Act
	let (inherent_sql, inherent_params) = manager.update_queryset(&qs, updates);
	let (trait_sql, trait_params) = CustomManager::update_queryset(&manager, &qs, updates);

	// Assert
	assert_eq!(inherent_sql, trait_sql);
	assert_eq!(inherent_params, trait_params);
}

#[rstest]
fn user_defined_manager_inherits_sql_builders_via_default_impl() {
	// Arrange: ActiveArticleManager only overrides `all()` and inherits
	// every other method from the trait's default implementation. Verify the
	// default impl reaches the same Manager<M>-backed SQL builder.
	let active = ActiveArticleManager;
	let manager = Manager::<Article>::new();
	let models = vec![Article {
		id: None,
		title: "via custom".into(),
		is_archived: false,
	}];

	// Act
	let custom_sql = active.bulk_create_sql(&models, DatabaseBackend::Postgres);
	let canonical_sql = manager.bulk_create_sql(&models, DatabaseBackend::Postgres);

	// Assert: bit-exact SQL output regardless of dispatch path.
	assert_eq!(custom_sql, canonical_sql);
}
