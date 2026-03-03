//! Integration tests for reinhardt-rest filters module
//!
//! Tests covering filter backends, search filters, ordering filters, and field filtering.

use reinhardt_db::orm::{Field, FieldSelector, Model};
use reinhardt_rest::filters::field_extensions::FieldOrderingExt;
use reinhardt_rest::filters::{
	CustomFilterBackend, DatabaseDialect, DateRangeFilter, FilterBackend, FilterError,
	MultiTermSearch, NumericRangeFilter, OrderDirection, OrderingField, QueryFilter, RangeFilter,
	SearchableModel, SimpleOrderingBackend, SimpleSearchBackend,
};
use rstest::rstest;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Test model definitions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Article {
	id: i64,
	title: String,
	content: String,
	author: String,
	created_at: String,
	views: i32,
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
		Some(self.id)
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = value;
	}
}

impl SearchableModel for Article {
	fn searchable_fields() -> Vec<Field<Self, String>> {
		vec![
			Field::<Article, String>::new(vec!["title"]),
			Field::<Article, String>::new(vec!["content"]),
		]
	}

	fn default_ordering() -> Vec<OrderingField<Self>> {
		vec![Field::<Article, String>::new(vec!["created_at"]).desc()]
	}
}

// ---------------------------------------------------------------------------
// CustomFilterBackend tests
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn custom_filter_backend_starts_empty() {
	// Arrange
	let backend = CustomFilterBackend::new();

	// Act
	let count = backend.filter_count();

	// Assert
	assert_eq!(count, 0);
}

#[rstest]
#[tokio::test]
async fn custom_filter_backend_passes_sql_unchanged_when_no_filters() {
	// Arrange
	let backend = CustomFilterBackend::new();
	let params = HashMap::new();
	let sql = "SELECT * FROM articles".to_string();

	// Act
	let result = backend.filter_queryset(&params, sql.clone()).await.unwrap();

	// Assert
	assert_eq!(result, sql);
}

#[rstest]
#[tokio::test]
async fn custom_filter_backend_increments_count_on_add() {
	// Arrange
	let mut backend = CustomFilterBackend::new();

	// Act
	backend.add_filter(Box::new(
		SimpleSearchBackend::new("search").with_field("title"),
	));
	backend.add_filter(Box::new(
		SimpleOrderingBackend::new("ordering").allow_field("created_at"),
	));

	// Assert
	assert_eq!(backend.filter_count(), 2);
}

#[rstest]
#[tokio::test]
async fn custom_filter_backend_applies_filters_in_order() {
	// Arrange
	let mut backend = CustomFilterBackend::new();
	backend.add_filter(Box::new(
		SimpleSearchBackend::new("search").with_field("title"),
	));
	backend.add_filter(Box::new(
		SimpleOrderingBackend::new("ordering").allow_field("views"),
	));

	let mut params = HashMap::new();
	params.insert("search".to_string(), "rust".to_string());
	params.insert("ordering".to_string(), "views".to_string());
	let sql = "SELECT * FROM articles".to_string();

	// Act
	let result = backend.filter_queryset(&params, sql).await.unwrap();

	// Assert
	assert!(result.contains("WHERE"));
	assert!(result.ends_with("ORDER BY views ASC"));
}

#[rstest]
#[tokio::test]
async fn custom_filter_backend_propagates_filter_error() {
	// Arrange
	let mut backend = CustomFilterBackend::new();
	// Search backend with no fields configured will fail when query param is present
	backend.add_filter(Box::new(SimpleSearchBackend::new("search")));

	let mut params = HashMap::new();
	params.insert("search".to_string(), "rust".to_string());
	let sql = "SELECT * FROM articles".to_string();

	// Act
	let result = backend.filter_queryset(&params, sql).await;

	// Assert
	assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// SimpleSearchBackend tests
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn search_backend_adds_where_clause_for_single_field() {
	// Arrange
	let backend = SimpleSearchBackend::new("search").with_field("title");
	let mut params = HashMap::new();
	params.insert("search".to_string(), "rust".to_string());
	let sql = "SELECT * FROM articles".to_string();

	// Act
	let result = backend.filter_queryset(&params, sql).await.unwrap();

	// Assert
	assert_eq!(
		result,
		"SELECT * FROM articles WHERE (`title` LIKE '%rust%')"
	);
}

#[rstest]
#[tokio::test]
async fn search_backend_generates_or_clause_for_multiple_fields() {
	// Arrange
	let backend = SimpleSearchBackend::new("search")
		.with_field("title")
		.with_field("content");
	let mut params = HashMap::new();
	params.insert("search".to_string(), "web".to_string());
	let sql = "SELECT * FROM articles".to_string();

	// Act
	let result = backend.filter_queryset(&params, sql).await.unwrap();

	// Assert
	assert_eq!(
		result,
		"SELECT * FROM articles WHERE ((`title` LIKE '%web%' OR `content` LIKE '%web%'))"
	);
}

#[rstest]
#[tokio::test]
async fn search_backend_returns_original_sql_when_no_query_param() {
	// Arrange
	let backend = SimpleSearchBackend::new("search").with_field("title");
	let params = HashMap::new();
	let sql = "SELECT * FROM articles".to_string();

	// Act
	let result = backend.filter_queryset(&params, sql.clone()).await.unwrap();

	// Assert
	assert_eq!(result, sql);
}

#[rstest]
#[tokio::test]
async fn search_backend_returns_error_when_no_fields_configured() {
	// Arrange
	let backend = SimpleSearchBackend::new("search");
	let mut params = HashMap::new();
	params.insert("search".to_string(), "rust".to_string());
	let sql = "SELECT * FROM articles".to_string();

	// Act
	let result = backend.filter_queryset(&params, sql).await;

	// Assert
	assert!(result.is_err());
	match result.unwrap_err() {
		FilterError::InvalidParameter(msg) => {
			assert!(msg.contains("No search fields configured"));
		}
		other => panic!("Expected InvalidParameter, got {:?}", other),
	}
}

#[rstest]
#[tokio::test]
async fn search_backend_uses_mysql_dialect_by_default() {
	// Arrange
	let backend = SimpleSearchBackend::new("q").with_field("name");
	let mut params = HashMap::new();
	params.insert("q".to_string(), "test".to_string());
	let sql = "SELECT * FROM products".to_string();

	// Act
	let result = backend.filter_queryset(&params, sql).await.unwrap();

	// Assert
	assert_eq!(
		result,
		"SELECT * FROM products WHERE (`name` LIKE '%test%')"
	);
}

#[rstest]
#[tokio::test]
async fn search_backend_uses_double_quotes_for_postgresql_dialect() {
	// Arrange
	let backend = SimpleSearchBackend::new("q")
		.with_field("name")
		.with_dialect(DatabaseDialect::PostgreSQL);
	let mut params = HashMap::new();
	params.insert("q".to_string(), "test".to_string());
	let sql = "SELECT * FROM products".to_string();

	// Act
	let result = backend.filter_queryset(&params, sql).await.unwrap();

	// Assert
	assert_eq!(
		result,
		"SELECT * FROM products WHERE (\"name\" LIKE '%test%')"
	);
}

#[rstest]
#[tokio::test]
async fn search_backend_escapes_percent_in_search_query() {
	// Arrange
	let backend = SimpleSearchBackend::new("search").with_field("title");
	let mut params = HashMap::new();
	params.insert("search".to_string(), "100%".to_string());
	let sql = "SELECT * FROM articles".to_string();

	// Act
	let result = backend.filter_queryset(&params, sql).await.unwrap();

	// Assert
	// The literal % should be escaped so it does not act as a wildcard
	assert!(result.contains("LIKE"));
	// Ensure the % from the user input is escaped
	assert!(result.contains("\\%"));
}

#[rstest]
#[tokio::test]
async fn search_backend_sql_injection_produces_balanced_quotes() {
	// Arrange
	let backend = SimpleSearchBackend::new("search").with_field("title");
	let mut params = HashMap::new();
	params.insert("search".to_string(), "' OR '1'='1".to_string());
	let sql = "SELECT * FROM articles".to_string();

	// Act
	let result = backend.filter_queryset(&params, sql).await.unwrap();

	// Assert
	let single_quote_count = result.matches('\'').count();
	assert_eq!(
		single_quote_count % 2,
		0,
		"Single quotes must be balanced to prevent SQL injection. Result: {}",
		result
	);
}

// ---------------------------------------------------------------------------
// SimpleOrderingBackend tests
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn ordering_backend_adds_asc_order_by_clause() {
	// Arrange
	let backend = SimpleOrderingBackend::new("ordering").allow_field("title");
	let mut params = HashMap::new();
	params.insert("ordering".to_string(), "title".to_string());
	let sql = "SELECT * FROM articles".to_string();

	// Act
	let result = backend.filter_queryset(&params, sql).await.unwrap();

	// Assert
	assert_eq!(result, "SELECT * FROM articles ORDER BY title ASC");
}

#[rstest]
#[tokio::test]
async fn ordering_backend_adds_desc_order_by_clause_with_dash_prefix() {
	// Arrange
	let backend = SimpleOrderingBackend::new("ordering").allow_field("created_at");
	let mut params = HashMap::new();
	params.insert("ordering".to_string(), "-created_at".to_string());
	let sql = "SELECT * FROM articles".to_string();

	// Act
	let result = backend.filter_queryset(&params, sql).await.unwrap();

	// Assert
	assert_eq!(result, "SELECT * FROM articles ORDER BY created_at DESC");
}

#[rstest]
#[tokio::test]
async fn ordering_backend_returns_error_for_disallowed_field() {
	// Arrange
	let backend = SimpleOrderingBackend::new("ordering").allow_field("title");
	let mut params = HashMap::new();
	params.insert("ordering".to_string(), "secret_column".to_string());
	let sql = "SELECT * FROM articles".to_string();

	// Act
	let result = backend.filter_queryset(&params, sql).await;

	// Assert
	assert!(result.is_err());
	match result.unwrap_err() {
		FilterError::InvalidParameter(msg) => {
			assert!(msg.contains("secret_column"));
		}
		other => panic!("Expected InvalidParameter, got {:?}", other),
	}
}

#[rstest]
#[tokio::test]
async fn ordering_backend_returns_original_sql_when_no_ordering_param() {
	// Arrange
	let backend = SimpleOrderingBackend::new("ordering").allow_field("title");
	let params = HashMap::new();
	let sql = "SELECT * FROM articles".to_string();

	// Act
	let result = backend.filter_queryset(&params, sql.clone()).await.unwrap();

	// Assert
	assert_eq!(result, sql);
}

#[rstest]
#[tokio::test]
async fn ordering_backend_allows_multiple_fields() {
	// Arrange
	let backend = SimpleOrderingBackend::new("ordering")
		.allow_field("title")
		.allow_field("created_at")
		.allow_field("views");
	let mut params = HashMap::new();
	params.insert("ordering".to_string(), "-views".to_string());
	let sql = "SELECT * FROM articles".to_string();

	// Act
	let result = backend.filter_queryset(&params, sql).await.unwrap();

	// Assert
	assert_eq!(result, "SELECT * FROM articles ORDER BY views DESC");
}

// ---------------------------------------------------------------------------
// RangeFilter tests
// ---------------------------------------------------------------------------

#[rstest]
fn range_filter_created_with_no_bounds() {
	// Act
	let filter: RangeFilter<i32> = RangeFilter::new("price");

	// Assert
	assert_eq!(filter.field_name(), "price");
	assert!(!filter.has_bounds());
}

#[rstest]
fn range_filter_gte_sets_inclusive_lower_bound() {
	// Act
	let filter: RangeFilter<i32> = RangeFilter::new("price").gte(100);

	// Assert
	assert_eq!(filter.gte, Some(100));
	assert!(filter.has_bounds());
}

#[rstest]
fn range_filter_lte_sets_inclusive_upper_bound() {
	// Act
	let filter: RangeFilter<i32> = RangeFilter::new("price").lte(500);

	// Assert
	assert_eq!(filter.lte, Some(500));
	assert!(filter.has_bounds());
}

#[rstest]
fn range_filter_between_sets_both_bounds() {
	// Act
	let filter: RangeFilter<i32> = RangeFilter::new("price").between(100, 500);

	// Assert
	assert_eq!(filter.gte, Some(100));
	assert_eq!(filter.lte, Some(500));
	assert!(filter.has_bounds());
}

#[rstest]
fn range_filter_gt_sets_exclusive_lower_bound() {
	// Act
	let filter: RangeFilter<i32> = RangeFilter::new("age").gt(18);

	// Assert
	assert_eq!(filter.gt, Some(18));
	assert!(filter.has_bounds());
}

#[rstest]
fn range_filter_lt_sets_exclusive_upper_bound() {
	// Act
	let filter: RangeFilter<i32> = RangeFilter::new("age").lt(65);

	// Assert
	assert_eq!(filter.lt, Some(65));
	assert!(filter.has_bounds());
}

#[rstest]
fn range_filter_chained_gt_and_lt() {
	// Act
	let filter: RangeFilter<i32> = RangeFilter::new("age").gt(18).lt(65);

	// Assert
	assert_eq!(filter.gt, Some(18));
	assert_eq!(filter.lt, Some(65));
}

// ---------------------------------------------------------------------------
// DateRangeFilter tests
// ---------------------------------------------------------------------------

#[rstest]
fn date_range_filter_after_sets_gte() {
	// Act
	let filter = DateRangeFilter::new("created_at").after("2024-01-01");

	// Assert
	assert_eq!(filter.inner().gte, Some("2024-01-01".to_string()));
}

#[rstest]
fn date_range_filter_before_sets_lte() {
	// Act
	let filter = DateRangeFilter::new("created_at").before("2024-12-31");

	// Assert
	assert_eq!(filter.inner().lte, Some("2024-12-31".to_string()));
}

#[rstest]
fn date_range_filter_range_sets_both_bounds() {
	// Act
	let filter = DateRangeFilter::new("created_at").range("2024-01-01", "2024-12-31");

	// Assert
	assert_eq!(filter.inner().gte, Some("2024-01-01".to_string()));
	assert_eq!(filter.inner().lte, Some("2024-12-31".to_string()));
}

#[rstest]
fn date_range_filter_field_name_accessible() {
	// Act
	let filter = DateRangeFilter::new("published_at");

	// Assert
	assert_eq!(filter.field_name(), "published_at");
}

// ---------------------------------------------------------------------------
// NumericRangeFilter tests
// ---------------------------------------------------------------------------

#[rstest]
fn numeric_range_filter_min_sets_gte() {
	// Act
	let filter: NumericRangeFilter<i32> = NumericRangeFilter::new("stock").min(10);

	// Assert
	assert_eq!(filter.inner().gte, Some(10));
}

#[rstest]
fn numeric_range_filter_max_sets_lte() {
	// Act
	let filter: NumericRangeFilter<i32> = NumericRangeFilter::new("stock").max(999);

	// Assert
	assert_eq!(filter.inner().lte, Some(999));
}

#[rstest]
fn numeric_range_filter_range_sets_both_bounds() {
	// Act
	let filter: NumericRangeFilter<f64> = NumericRangeFilter::new("price").range(9.99, 99.99);

	// Assert
	assert_eq!(filter.inner().gte, Some(9.99));
	assert_eq!(filter.inner().lte, Some(99.99));
}

// ---------------------------------------------------------------------------
// OrderingField + FieldOrderingExt tests
// ---------------------------------------------------------------------------

#[rstest]
fn field_ordering_ext_asc_produces_asc_direction() {
	// Act
	let order = Field::<Article, String>::new(vec!["title"]).asc();

	// Assert
	assert_eq!(order.direction(), OrderDirection::Asc);
	assert_eq!(order.field_path(), &["title"]);
}

#[rstest]
fn field_ordering_ext_desc_produces_desc_direction() {
	// Act
	let order = Field::<Article, String>::new(vec!["created_at"]).desc();

	// Assert
	assert_eq!(order.direction(), OrderDirection::Desc);
	assert_eq!(order.field_path(), &["created_at"]);
}

#[rstest]
fn ordering_field_to_sql_asc() {
	// Act
	let order = Field::<Article, String>::new(vec!["title"]).asc();

	// Assert
	assert_eq!(order.to_sql(), "title ASC");
}

#[rstest]
fn ordering_field_to_sql_desc() {
	// Act
	let order = Field::<Article, String>::new(vec!["created_at"]).desc();

	// Assert
	assert_eq!(order.to_sql(), "created_at DESC");
}

#[rstest]
fn ordering_field_nested_path_joins_with_dot() {
	// Act
	let order = Field::<Article, String>::new(vec!["author", "username"]).asc();

	// Assert
	assert_eq!(order.to_sql(), "author.username ASC");
}

// ---------------------------------------------------------------------------
// SearchableModel tests
// ---------------------------------------------------------------------------

#[rstest]
fn searchable_model_returns_configured_fields() {
	// Act
	let fields = Article::searchable_fields();

	// Assert
	assert_eq!(fields.len(), 2);
}

#[rstest]
fn searchable_model_field_names_accessible() {
	// Act
	let names = Article::searchable_field_names();

	// Assert
	assert_eq!(names.len(), 2);
	assert!(names.contains(&"title".to_string()));
	assert!(names.contains(&"content".to_string()));
}

#[rstest]
fn searchable_model_default_ordering_returns_configured_fields() {
	// Act
	let ordering = Article::default_ordering();

	// Assert
	assert_eq!(ordering.len(), 1);
	assert_eq!(ordering[0].direction(), OrderDirection::Desc);
	assert_eq!(ordering[0].field_path(), &["created_at"]);
}

// ---------------------------------------------------------------------------
// MultiTermSearch tests
// ---------------------------------------------------------------------------

#[rstest]
fn multi_term_search_creates_lookups_per_term_and_field() {
	// Arrange
	let terms = vec!["rust", "programming"];

	// Act
	let lookups = MultiTermSearch::search_terms::<Article>(terms);

	// Assert
	assert_eq!(lookups.len(), 2); // Two terms
	assert_eq!(lookups[0].len(), 2); // Two fields per term (title, content)
	assert_eq!(lookups[1].len(), 2);
}

#[rstest]
fn multi_term_search_empty_terms_returns_empty_vec() {
	// Arrange
	let terms: Vec<&str> = vec![];

	// Act
	let lookups = MultiTermSearch::search_terms::<Article>(terms);

	// Assert
	assert!(lookups.is_empty());
}

#[rstest]
fn parse_search_terms_splits_by_comma() {
	// Arrange
	let input = "rust, programming, web";

	// Act
	let terms = MultiTermSearch::parse_search_terms(input);

	// Assert
	assert_eq!(terms, vec!["rust", "programming", "web"]);
}

#[rstest]
fn parse_search_terms_handles_quoted_phrases() {
	// Arrange
	let input = r#""machine learning", rust"#;

	// Act
	let terms = MultiTermSearch::parse_search_terms(input);

	// Assert
	assert_eq!(terms, vec!["machine learning", "rust"]);
}

#[rstest]
fn parse_search_terms_single_term_without_comma() {
	// Arrange
	let input = "rust";

	// Act
	let terms = MultiTermSearch::parse_search_terms(input);

	// Assert
	assert_eq!(terms, vec!["rust"]);
}

// ---------------------------------------------------------------------------
// QueryFilter tests
// ---------------------------------------------------------------------------

#[rstest]
fn query_filter_starts_empty() {
	// Act
	let filter = QueryFilter::<Article>::new();

	// Assert
	assert!(filter.lookups().is_empty());
	assert!(filter.ordering().is_empty());
	assert!(filter.or_groups().is_empty());
}

#[rstest]
fn query_filter_with_lookup_adds_lookup() {
	// Arrange
	let filter = QueryFilter::<Article>::new()
		.with_lookup(Field::<Article, String>::new(vec!["title"]).icontains("rust"));

	// Act
	let count = filter.lookups().len();

	// Assert
	assert_eq!(count, 1);
}

#[rstest]
fn query_filter_order_by_adds_ordering() {
	// Arrange
	let filter = QueryFilter::<Article>::new()
		.order_by(Field::<Article, String>::new(vec!["created_at"]).desc());

	// Act
	let count = filter.ordering().len();

	// Assert
	assert_eq!(count, 1);
	assert_eq!(filter.ordering()[0].direction(), OrderDirection::Desc);
}

#[rstest]
fn query_filter_multiple_lookups_and_orderings() {
	// Act
	let filter = QueryFilter::<Article>::new()
		.with_lookup(Field::<Article, String>::new(vec!["title"]).icontains("rust"))
		.with_lookup(Field::<Article, i32>::new(vec!["views"]).gte(100))
		.order_by(Field::<Article, String>::new(vec!["created_at"]).desc())
		.order_by(Field::<Article, String>::new(vec!["title"]).asc());

	// Assert
	assert_eq!(filter.lookups().len(), 2);
	assert_eq!(filter.ordering().len(), 2);
}

#[rstest]
fn query_filter_add_or_group_stores_group() {
	// Act
	let filter = QueryFilter::<Article>::new().add_or_group(vec![
		Field::<Article, String>::new(vec!["title"]).icontains("rust"),
		Field::<Article, String>::new(vec!["content"]).icontains("rust"),
	]);

	// Assert
	assert_eq!(filter.or_groups().len(), 1);
	assert_eq!(filter.or_groups()[0].len(), 2);
}

#[rstest]
fn query_filter_add_or_group_ignores_empty_group() {
	// Act
	let filter = QueryFilter::<Article>::new().add_or_group(vec![]);

	// Assert
	assert!(filter.or_groups().is_empty());
}

#[rstest]
fn query_filter_add_multi_term_stores_multiple_groups() {
	// Arrange
	let term_lookups = vec![
		vec![Field::<Article, String>::new(vec!["title"]).icontains("rust")],
		vec![Field::<Article, String>::new(vec!["title"]).icontains("web")],
	];

	// Act
	let filter = QueryFilter::<Article>::new().add_multi_term(term_lookups);

	// Assert
	assert_eq!(filter.or_groups().len(), 2);
}

#[rstest]
#[tokio::test]
async fn query_filter_generates_where_clause_from_lookups() {
	// Arrange
	let filter = QueryFilter::<Article>::new()
		.with_lookup(Field::<Article, String>::new(vec!["title"]).icontains("rust"));
	let params = HashMap::new();
	let sql = "SELECT * FROM articles".to_string();

	// Act
	let result = filter.filter_queryset(&params, sql).await.unwrap();

	// Assert
	assert!(result.contains("WHERE"));
	assert!(result.contains("title"));
}

#[rstest]
#[tokio::test]
async fn query_filter_generates_order_by_clause_from_ordering() {
	// Arrange
	let filter = QueryFilter::<Article>::new()
		.order_by(Field::<Article, String>::new(vec!["created_at"]).desc());
	let params = HashMap::new();
	let sql = "SELECT * FROM articles".to_string();

	// Act
	let result = filter.filter_queryset(&params, sql).await.unwrap();

	// Assert
	assert_eq!(result, "SELECT * FROM articles ORDER BY created_at DESC");
}

#[rstest]
#[tokio::test]
async fn query_filter_passes_sql_unchanged_when_no_conditions() {
	// Arrange
	let filter = QueryFilter::<Article>::new();
	let params = HashMap::new();
	let sql = "SELECT * FROM articles".to_string();

	// Act
	let result = filter.filter_queryset(&params, sql.clone()).await.unwrap();

	// Assert
	assert_eq!(result, sql);
}

#[rstest]
#[tokio::test]
async fn query_filter_combines_where_and_order_by() {
	// Arrange
	let filter = QueryFilter::<Article>::new()
		.with_lookup(Field::<Article, String>::new(vec!["author"]).icontains("alice"))
		.order_by(Field::<Article, String>::new(vec!["title"]).asc());
	let params = HashMap::new();
	let sql = "SELECT * FROM articles".to_string();

	// Act
	let result = filter.filter_queryset(&params, sql).await.unwrap();

	// Assert
	assert!(result.contains("WHERE"));
	assert!(result.ends_with("ORDER BY title ASC"));
}
