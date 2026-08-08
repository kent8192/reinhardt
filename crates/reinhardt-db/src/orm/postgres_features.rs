//! PostgreSQL-specific advanced features
//!
//! This module provides PostgreSQL-specific advanced query features inspired by
//! Django's `django/contrib/postgres/aggregates/` and `django/contrib/postgres/search/`.
//!
//! # Available Features
//!
//! - **ArrayAgg**: Array aggregation function
//! - **JsonbBuildObject**: JSONB object construction
//! - **FullTextSearch**: Full-text search functionality
//! - **ArrayOverlap**: Array overlap operations
//!
//! # Example
//!
//! ```rust
//! use reinhardt_db::orm::{ArrayAgg, FullTextSearch};
//!
//! // Aggregate values into an array
//! let agg = ArrayAgg::<String>::new("tags".to_string()).distinct();
//! assert!(agg.to_sql().contains("ARRAY_AGG(DISTINCT"));
//!
//! // Full-text search
//! let search = FullTextSearch::new("content".to_string(), "rust programming".to_string());
//! assert!(search.to_sql().contains("to_tsvector"));
//! ```

use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

/// A PostgreSQL-only annotation retained outside the portable ORM expression tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendAnnotation {
	label: String,
	value: BackendAnnotationValue,
}

/// PostgreSQL-specific projection values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackendAnnotationValue {
	/// PostgreSQL array aggregation.
	ArrayAgg(ArrayAgg<serde_json::Value>),
	/// PostgreSQL string aggregation.
	StringAgg(StringAgg),
	/// PostgreSQL JSONB aggregation.
	JsonbAgg(JsonbAgg),
	/// PostgreSQL JSONB object construction.
	JsonbBuildObject(JsonbBuildObject),
	/// PostgreSQL full-text rank.
	TsRank(TsRank),
}

impl BackendAnnotation {
	/// Creates a PostgreSQL annotation after validating its label.
	pub fn new(
		label: impl Into<String>,
		value: BackendAnnotationValue,
	) -> reinhardt_core::exception::Result<Self> {
		let label = label.into();
		crate::orm::query::validate_annotation_label(&label)?;
		Ok(Self { label, value })
	}

	/// Returns the validated projection label.
	pub(crate) fn label(&self) -> &str {
		&self.label
	}

	/// Returns whether this projection changes the cardinality of the query.
	pub(crate) fn is_aggregate(&self) -> bool {
		matches!(
			self.value,
			BackendAnnotationValue::ArrayAgg(_)
				| BackendAnnotationValue::StringAgg(_)
				| BackendAnnotationValue::JsonbAgg(_)
		)
	}

	/// Renders this projection with a field mapper for the queryset root alias.
	pub(crate) fn to_sql_with_field_mapper<F>(&self, map_field: F) -> String
	where
		F: Fn(&str) -> String + Copy,
	{
		let expression = match &self.value {
			BackendAnnotationValue::ArrayAgg(value) => value.to_sql_with_field_mapper(map_field),
			BackendAnnotationValue::StringAgg(value) => value.to_sql_with_field_mapper(map_field),
			BackendAnnotationValue::JsonbAgg(value) => value.to_sql_with_field_mapper(map_field),
			BackendAnnotationValue::JsonbBuildObject(value) => {
				value.to_sql_with_field_mapper(map_field)
			}
			BackendAnnotationValue::TsRank(value) => value.to_sql_with_field_mapper(map_field),
		};
		format!(
			"{expression} AS {}",
			crate::orm::query::quote_identifier(&self.label)
		)
	}
}

fn mapped_ordering_sql<F>(ordering: &[String], mapper: F) -> String
where
	F: Fn(&str) -> String,
{
	let mut mapped_orderings = Vec::with_capacity(ordering.len());
	for ordering in ordering {
		let field_end = ordering
			.char_indices()
			.find_map(|(index, character)| character.is_whitespace().then_some(index))
			.unwrap_or(ordering.len());
		let (field, suffix) = ordering.split_at(field_end);
		mapped_orderings.push(format!("{}{}", mapper(field), suffix));
	}
	mapped_orderings.join(", ")
}

/// PostgreSQL ARRAY_AGG aggregation function
///
/// Aggregates values into a PostgreSQL array.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::orm::ArrayAgg;
///
/// let agg = ArrayAgg::<i32>::new("score".to_string());
/// assert_eq!(agg.to_sql(), "ARRAY_AGG(score)");
///
/// let distinct_agg = ArrayAgg::<String>::new("category".to_string()).distinct();
/// assert_eq!(distinct_agg.to_sql(), "ARRAY_AGG(DISTINCT category)");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrayAgg<T> {
	field: String,
	distinct: bool,
	ordering: Option<Vec<String>>,
	_phantom: PhantomData<T>,
}

impl<T> ArrayAgg<T> {
	/// Create a new ArrayAgg for the specified field
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::ArrayAgg;
	///
	/// let agg = ArrayAgg::<String>::new("name".to_string());
	/// assert_eq!(agg.to_sql(), "ARRAY_AGG(name)");
	/// ```
	pub fn new(field: String) -> Self {
		Self {
			field,
			distinct: false,
			ordering: None,
			_phantom: PhantomData,
		}
	}

	/// Apply DISTINCT to the aggregation
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::ArrayAgg;
	///
	/// let agg = ArrayAgg::<i32>::new("id".to_string()).distinct();
	/// assert!(agg.to_sql().contains("DISTINCT"));
	/// ```
	pub fn distinct(mut self) -> Self {
		self.distinct = true;
		self
	}

	/// Add ORDER BY clause to the aggregation
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::ArrayAgg;
	///
	/// let agg = ArrayAgg::<String>::new("name".to_string())
	///     .order_by(vec!["created_at DESC".to_string()]);
	/// assert!(agg.to_sql().contains("ORDER BY"));
	/// ```
	pub fn order_by(mut self, fields: Vec<String>) -> Self {
		self.ordering = Some(fields);
		self
	}

	/// Generate SQL for this aggregation
	pub fn to_sql(&self) -> String {
		let mut sql = String::from("ARRAY_AGG(");

		if self.distinct {
			sql.push_str("DISTINCT ");
		}

		sql.push_str(&self.field);

		if let Some(ref ordering) = self.ordering {
			sql.push_str(" ORDER BY ");
			sql.push_str(&ordering.join(", "));
		}

		sql.push(')');
		sql
	}

	pub(crate) fn to_sql_with_field_mapper<F>(&self, map_field: F) -> String
	where
		F: Fn(&str) -> String,
	{
		let mut sql = String::from("ARRAY_AGG(");

		if self.distinct {
			sql.push_str("DISTINCT ");
		}

		sql.push_str(&map_field(&self.field));

		if let Some(ordering) = &self.ordering {
			sql.push_str(" ORDER BY ");
			sql.push_str(&mapped_ordering_sql(ordering, &map_field));
		}

		sql.push(')');
		sql
	}
}

/// PostgreSQL JSONB_BUILD_OBJECT function
///
/// Constructs a JSONB object from key-value pairs.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::orm::JsonbBuildObject;
///
/// let builder = JsonbBuildObject::new()
///     .add("id", "user_id")
///     .add("name", "user_name");
/// assert!(builder.to_sql().contains("jsonb_build_object"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonbBuildObject {
	pairs: Vec<(String, String)>,
}

impl JsonbBuildObject {
	/// Create a new JSONB object builder
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::JsonbBuildObject;
	///
	/// let builder = JsonbBuildObject::new();
	/// assert_eq!(builder.to_sql(), "jsonb_build_object()");
	/// ```
	pub fn new() -> Self {
		Self { pairs: Vec::new() }
	}

	/// Add a key-value pair to the JSONB object
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::JsonbBuildObject;
	///
	/// let builder = JsonbBuildObject::new()
	///     .add("user_id", "id")
	///     .add("user_name", "name");
	/// let sql = builder.to_sql();
	/// assert!(sql.contains("'user_id'"));
	/// assert!(sql.contains("id"));
	/// ```
	pub fn add(mut self, key: &str, value_field: &str) -> Self {
		self.pairs.push((key.to_string(), value_field.to_string()));
		self
	}

	/// Generate SQL for this JSONB object construction
	pub fn to_sql(&self) -> String {
		let mut sql = String::from("jsonb_build_object(");

		let parts: Vec<String> = self
			.pairs
			.iter()
			.flat_map(|(k, v)| vec![format!("'{}'", k), v.clone()])
			.collect();

		sql.push_str(&parts.join(", "));
		sql.push(')');
		sql
	}

	pub(crate) fn to_sql_with_field_mapper<F>(&self, map_field: F) -> String
	where
		F: Fn(&str) -> String,
	{
		let parts: Vec<_> = self
			.pairs
			.iter()
			.map(|(key, field)| format!("'{}', {}", key, map_field(field)))
			.collect();
		format!("jsonb_build_object({})", parts.join(", "))
	}
}

impl Default for JsonbBuildObject {
	fn default() -> Self {
		Self::new()
	}
}

/// PostgreSQL Full-Text Search
///
/// Provides full-text search capabilities using PostgreSQL's tsvector and tsquery.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::orm::FullTextSearch;
///
/// let search = FullTextSearch::new("content".to_string(), "rust programming".to_string());
/// assert!(search.to_sql().contains("to_tsvector"));
/// assert!(search.to_sql().contains("to_tsquery"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullTextSearch {
	vector_field: String,
	query: String,
	config: String,
}

impl FullTextSearch {
	/// Create a new full-text search with default English configuration
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::FullTextSearch;
	///
	/// let search = FullTextSearch::new("title".to_string(), "database".to_string());
	/// assert_eq!(search.config(), "english");
	/// ```
	pub fn new(field: String, query: String) -> Self {
		Self {
			vector_field: field,
			query,
			config: "english".to_string(),
		}
	}

	/// Set a custom text search configuration (language)
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::FullTextSearch;
	///
	/// let search = FullTextSearch::new("content".to_string(), "bonjour".to_string())
	///     .with_config("french".to_string());
	/// assert_eq!(search.config(), "french");
	/// ```
	pub fn with_config(mut self, config: String) -> Self {
		self.config = config;
		self
	}

	/// Get the current configuration
	pub fn config(&self) -> &str {
		&self.config
	}

	/// Generate SQL for this full-text search
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::FullTextSearch;
	///
	/// let search = FullTextSearch::new("body".to_string(), "rust".to_string());
	/// let sql = search.to_sql();
	/// assert!(sql.contains("to_tsvector('english', body)"));
	/// assert!(sql.contains("to_tsquery('english', 'rust')"));
	/// ```
	pub fn to_sql(&self) -> String {
		format!(
			"to_tsvector('{}', {}) @@ to_tsquery('{}', '{}')",
			self.config, self.vector_field, self.config, self.query
		)
	}
}

/// PostgreSQL STRING_AGG aggregation function
///
/// Aggregates string values into a single string with a specified separator.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::orm::StringAgg;
///
/// let agg = StringAgg::new("name".to_string(), ", ".to_string());
/// assert_eq!(agg.to_sql(), "STRING_AGG(name, ', ')");
///
/// let distinct_agg = StringAgg::new("category".to_string(), "; ".to_string()).distinct();
/// assert_eq!(distinct_agg.to_sql(), "STRING_AGG(DISTINCT category, '; ')");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringAgg {
	field: String,
	separator: String,
	distinct: bool,
	ordering: Option<Vec<String>>,
}

impl StringAgg {
	/// Create a new StringAgg for the specified field with a separator
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::StringAgg;
	///
	/// let agg = StringAgg::new("name".to_string(), ", ".to_string());
	/// assert_eq!(agg.to_sql(), "STRING_AGG(name, ', ')");
	/// ```
	pub fn new(field: String, separator: String) -> Self {
		Self {
			field,
			separator,
			distinct: false,
			ordering: None,
		}
	}

	/// Apply DISTINCT to the aggregation
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::StringAgg;
	///
	/// let agg = StringAgg::new("name".to_string(), ",".to_string()).distinct();
	/// assert!(agg.to_sql().contains("DISTINCT"));
	/// ```
	pub fn distinct(mut self) -> Self {
		self.distinct = true;
		self
	}

	/// Add ORDER BY clause to the aggregation
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::StringAgg;
	///
	/// let agg = StringAgg::new("name".to_string(), ", ".to_string())
	///     .order_by(vec!["name ASC".to_string()]);
	/// assert!(agg.to_sql().contains("ORDER BY"));
	/// ```
	pub fn order_by(mut self, fields: Vec<String>) -> Self {
		self.ordering = Some(fields);
		self
	}

	/// Generate SQL for this aggregation
	pub fn to_sql(&self) -> String {
		let mut sql = String::from("STRING_AGG(");

		if self.distinct {
			sql.push_str("DISTINCT ");
		}

		sql.push_str(&self.field);
		sql.push_str(", '");
		sql.push_str(&self.separator);
		sql.push('\'');

		if let Some(ref ordering) = self.ordering {
			sql.push_str(" ORDER BY ");
			sql.push_str(&ordering.join(", "));
		}

		sql.push(')');
		sql
	}

	pub(crate) fn to_sql_with_field_mapper<F>(&self, map_field: F) -> String
	where
		F: Fn(&str) -> String,
	{
		let mut sql = String::from("STRING_AGG(");

		if self.distinct {
			sql.push_str("DISTINCT ");
		}

		sql.push_str(&map_field(&self.field));
		sql.push_str(", '");
		sql.push_str(&self.separator);
		sql.push('\'');

		if let Some(ordering) = &self.ordering {
			sql.push_str(" ORDER BY ");
			sql.push_str(&mapped_ordering_sql(ordering, &map_field));
		}

		sql.push(')');
		sql
	}
}

/// PostgreSQL JSONB_AGG aggregation function
///
/// Aggregates values into a JSONB array.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::orm::JsonbAgg;
///
/// let agg = JsonbAgg::new("user_data".to_string());
/// assert_eq!(agg.to_sql(), "JSONB_AGG(user_data)");
///
/// let distinct_agg = JsonbAgg::new("category".to_string()).distinct();
/// assert_eq!(distinct_agg.to_sql(), "JSONB_AGG(DISTINCT category)");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonbAgg {
	expression: String,
	distinct: bool,
	ordering: Option<Vec<String>>,
}

impl JsonbAgg {
	/// Create a new JsonbAgg for the specified expression
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::JsonbAgg;
	///
	/// let agg = JsonbAgg::new("metadata".to_string());
	/// assert_eq!(agg.to_sql(), "JSONB_AGG(metadata)");
	/// ```
	pub fn new(expression: String) -> Self {
		Self {
			expression,
			distinct: false,
			ordering: None,
		}
	}

	/// Apply DISTINCT to the aggregation
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::JsonbAgg;
	///
	/// let agg = JsonbAgg::new("data".to_string()).distinct();
	/// assert!(agg.to_sql().contains("DISTINCT"));
	/// ```
	pub fn distinct(mut self) -> Self {
		self.distinct = true;
		self
	}

	/// Add ORDER BY clause to the aggregation
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::JsonbAgg;
	///
	/// let agg = JsonbAgg::new("items".to_string())
	///     .order_by(vec!["created_at DESC".to_string()]);
	/// assert!(agg.to_sql().contains("ORDER BY"));
	/// ```
	pub fn order_by(mut self, fields: Vec<String>) -> Self {
		self.ordering = Some(fields);
		self
	}

	/// Generate SQL for this aggregation
	pub fn to_sql(&self) -> String {
		let mut sql = String::from("JSONB_AGG(");

		if self.distinct {
			sql.push_str("DISTINCT ");
		}

		sql.push_str(&self.expression);

		if let Some(ref ordering) = self.ordering {
			sql.push_str(" ORDER BY ");
			sql.push_str(&ordering.join(", "));
		}

		sql.push(')');
		sql
	}

	pub(crate) fn to_sql_with_field_mapper<F>(&self, map_field: F) -> String
	where
		F: Fn(&str) -> String,
	{
		let mut sql = String::from("JSONB_AGG(");

		if self.distinct {
			sql.push_str("DISTINCT ");
		}

		sql.push_str(&map_field(&self.expression));

		if let Some(ordering) = &self.ordering {
			sql.push_str(" ORDER BY ");
			sql.push_str(&mapped_ordering_sql(ordering, &map_field));
		}

		sql.push(')');
		sql
	}
}

/// PostgreSQL ts_rank function
///
/// Computes a ranking score for full-text search results based on how well
/// a document matches a tsquery.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::orm::TsRank;
///
/// let rank = TsRank::new("search_vector".to_string(), "rust & programming".to_string());
/// assert!(rank.to_sql().contains("ts_rank"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TsRank {
	vector_field: String,
	query: String,
	config: String,
	normalization: Option<i32>,
}

impl TsRank {
	/// Create a new TsRank for the specified tsvector field and query
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::TsRank;
	///
	/// let rank = TsRank::new("content_vector".to_string(), "database".to_string());
	/// let sql = rank.to_sql();
	/// assert!(sql.contains("ts_rank"));
	/// ```
	pub fn new(vector_field: String, query: String) -> Self {
		Self {
			vector_field,
			query,
			config: "english".to_string(),
			normalization: None,
		}
	}

	/// Set a custom text search configuration (language)
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::TsRank;
	///
	/// let rank = TsRank::new("content".to_string(), "bonjour".to_string())
	///     .with_config("french".to_string());
	/// let sql = rank.to_sql();
	/// assert!(sql.contains("french"));
	/// ```
	pub fn with_config(mut self, config: String) -> Self {
		self.config = config;
		self
	}

	/// Set normalization option
	///
	/// Normalization values:
	/// - 0: ignore document length
	/// - 1: divide the rank by 1 + log(document length)
	/// - 2: divide the rank by the document length
	/// - 4: divide the rank by the mean harmonic distance between extents
	/// - 8: divide the rank by the number of unique words in document
	/// - 16: divide the rank by 1 + log(number of unique words)
	/// - 32: divide the rank by itself + 1
	///
	/// Multiple values can be combined using bitwise OR.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::TsRank;
	///
	/// let rank = TsRank::new("content".to_string(), "rust".to_string())
	///     .with_normalization(2);
	/// let sql = rank.to_sql();
	/// assert!(sql.contains(", 2)"));
	/// ```
	pub fn with_normalization(mut self, norm: i32) -> Self {
		self.normalization = Some(norm);
		self
	}

	/// Get the current configuration
	pub fn config(&self) -> &str {
		&self.config
	}

	/// Generate SQL for this ranking function
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::TsRank;
	///
	/// let rank = TsRank::new("search_vec".to_string(), "rust".to_string());
	/// let sql = rank.to_sql();
	/// assert!(sql.contains("ts_rank(search_vec, to_tsquery('english', 'rust'))"));
	/// ```
	pub fn to_sql(&self) -> String {
		let tsquery = format!("to_tsquery('{}', '{}')", self.config, self.query);

		match self.normalization {
			Some(norm) => format!("ts_rank({}, {}, {})", self.vector_field, tsquery, norm),
			None => format!("ts_rank({}, {})", self.vector_field, tsquery),
		}
	}

	pub(crate) fn to_sql_with_field_mapper<F>(&self, map_field: F) -> String
	where
		F: Fn(&str) -> String,
	{
		let tsquery = format!("to_tsquery('{}', '{}')", self.config, self.query);
		let vector_field = map_field(&self.vector_field);

		match self.normalization {
			Some(norm) => format!("ts_rank({}, {}, {})", vector_field, tsquery, norm),
			None => format!("ts_rank({}, {})", vector_field, tsquery),
		}
	}
}

/// PostgreSQL Array Overlap Operator
///
/// Tests whether two arrays have any elements in common.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::orm::ArrayOverlap;
///
/// let overlap = ArrayOverlap::new("tags".to_string(), vec!["rust".to_string(), "web".to_string()]);
/// assert!(overlap.to_sql().contains("&&"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrayOverlap {
	field: String,
	values: Vec<String>,
}

impl ArrayOverlap {
	/// Create a new array overlap check
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::ArrayOverlap;
	///
	/// let overlap = ArrayOverlap::new(
	///     "categories".to_string(),
	///     vec!["tech".to_string(), "science".to_string()]
	/// );
	/// assert!(overlap.to_sql().contains("ARRAY"));
	/// ```
	pub fn new(field: String, values: Vec<String>) -> Self {
		Self { field, values }
	}

	/// Generate SQL for the array overlap check
	pub fn to_sql(&self) -> String {
		let array_literal = format!(
			"ARRAY[{}]",
			self.values
				.iter()
				.map(|v| format!("'{}'", v))
				.collect::<Vec<_>>()
				.join(", ")
		);
		format!("{} && {}", self.field, array_literal)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_core::exception::Error;

	#[test]
	fn backend_annotation_uses_typed_label_validation() {
		let value = || BackendAnnotationValue::JsonbBuildObject(JsonbBuildObject::new());

		assert!(matches!(
			BackendAnnotation::new("", value()),
			Err(Error::Validation(message))
				if message == "aggregate label must be 1 to 63 ASCII bytes"
		));
		assert!(matches!(
			BackendAnnotation::new("合計", value()),
			Err(Error::Validation(message))
				if message == "aggregate label must be 1 to 63 ASCII bytes"
		));
		assert!(matches!(
			BackendAnnotation::new("a".repeat(64), value()),
			Err(Error::Validation(message))
				if message == "aggregate label must be 1 to 63 ASCII bytes"
		));
		assert!(BackendAnnotation::new("total_1", value()).is_ok());
	}

	#[test]
	fn test_array_agg_basic() {
		let agg = ArrayAgg::<i32>::new("score".to_string());
		assert_eq!(agg.to_sql(), "ARRAY_AGG(score)");
	}

	#[test]
	fn test_array_agg_distinct() {
		let agg = ArrayAgg::<String>::new("category".to_string()).distinct();
		assert_eq!(agg.to_sql(), "ARRAY_AGG(DISTINCT category)");
	}

	#[test]
	fn test_array_agg_with_ordering() {
		let agg =
			ArrayAgg::<i32>::new("id".to_string()).order_by(vec!["created_at DESC".to_string()]);
		assert_eq!(agg.to_sql(), "ARRAY_AGG(id ORDER BY created_at DESC)");
	}

	#[test]
	fn test_array_agg_distinct_with_ordering() {
		let agg = ArrayAgg::<String>::new("name".to_string())
			.distinct()
			.order_by(vec!["name ASC".to_string(), "id DESC".to_string()]);
		assert_eq!(
			agg.to_sql(),
			"ARRAY_AGG(DISTINCT name ORDER BY name ASC, id DESC)"
		);
	}

	#[test]
	fn test_jsonb_build_object_empty() {
		let builder = JsonbBuildObject::new();
		assert_eq!(builder.to_sql(), "jsonb_build_object()");
	}

	#[test]
	fn test_jsonb_build_object_single_pair() {
		let builder = JsonbBuildObject::new().add("id", "user_id");
		assert_eq!(builder.to_sql(), "jsonb_build_object('id', user_id)");
	}

	#[test]
	fn test_jsonb_build_object_multiple_pairs() {
		let builder = JsonbBuildObject::new()
			.add("id", "user_id")
			.add("name", "user_name")
			.add("email", "user_email");
		assert_eq!(
			builder.to_sql(),
			"jsonb_build_object('id', user_id, 'name', user_name, 'email', user_email)"
		);
	}

	#[test]
	fn test_full_text_search_basic() {
		let search = FullTextSearch::new("content".to_string(), "rust".to_string());
		assert_eq!(
			search.to_sql(),
			"to_tsvector('english', content) @@ to_tsquery('english', 'rust')"
		);
	}

	#[test]
	fn test_full_text_search_custom_config() {
		let search = FullTextSearch::new("title".to_string(), "database".to_string())
			.with_config("french".to_string());
		assert_eq!(
			search.to_sql(),
			"to_tsvector('french', title) @@ to_tsquery('french', 'database')"
		);
	}

	#[test]
	fn test_full_text_search_complex_query() {
		let search = FullTextSearch::new("body".to_string(), "rust & programming".to_string());
		let sql = search.to_sql();
		assert!(sql.contains("to_tsvector('english', body)"));
		assert!(sql.contains("to_tsquery('english', 'rust & programming')"));
	}

	#[test]
	fn test_array_overlap_basic() {
		let overlap = ArrayOverlap::new(
			"tags".to_string(),
			vec!["rust".to_string(), "web".to_string()],
		);
		assert_eq!(overlap.to_sql(), "tags && ARRAY['rust', 'web']");
	}

	#[test]
	fn test_array_overlap_single_value() {
		let overlap = ArrayOverlap::new("categories".to_string(), vec!["tech".to_string()]);
		assert_eq!(overlap.to_sql(), "categories && ARRAY['tech']");
	}

	#[test]
	fn test_array_overlap_multiple_values() {
		let overlap = ArrayOverlap::new(
			"labels".to_string(),
			vec![
				"important".to_string(),
				"urgent".to_string(),
				"reviewed".to_string(),
			],
		);
		assert_eq!(
			overlap.to_sql(),
			"labels && ARRAY['important', 'urgent', 'reviewed']"
		);
	}

	#[test]
	fn test_array_agg_type_safety() {
		let int_agg = ArrayAgg::<i32>::new("scores".to_string());
		let string_agg = ArrayAgg::<String>::new("names".to_string());

		assert_eq!(int_agg.to_sql(), "ARRAY_AGG(scores)");
		assert_eq!(string_agg.to_sql(), "ARRAY_AGG(names)");
	}

	#[test]
	fn test_jsonb_build_object_default() {
		let builder = JsonbBuildObject::default();
		assert_eq!(builder.to_sql(), "jsonb_build_object()");
	}

	#[test]
	fn test_full_text_search_config_getter() {
		let search = FullTextSearch::new("text".to_string(), "query".to_string());
		assert_eq!(search.config(), "english");

		let search_fr = search.with_config("french".to_string());
		assert_eq!(search_fr.config(), "french");
	}

	// StringAgg tests
	#[test]
	fn test_string_agg_basic() {
		let agg = StringAgg::new("name".to_string(), ", ".to_string());
		assert_eq!(agg.to_sql(), "STRING_AGG(name, ', ')");
	}

	#[test]
	fn test_string_agg_distinct() {
		let agg = StringAgg::new("category".to_string(), "; ".to_string()).distinct();
		assert_eq!(agg.to_sql(), "STRING_AGG(DISTINCT category, '; ')");
	}

	#[test]
	fn test_string_agg_with_ordering() {
		let agg = StringAgg::new("name".to_string(), ", ".to_string())
			.order_by(vec!["name ASC".to_string()]);
		assert_eq!(agg.to_sql(), "STRING_AGG(name, ', ' ORDER BY name ASC)");
	}

	#[test]
	fn test_string_agg_distinct_with_ordering() {
		let agg = StringAgg::new("name".to_string(), ",".to_string())
			.distinct()
			.order_by(vec!["created_at DESC".to_string()]);
		assert_eq!(
			agg.to_sql(),
			"STRING_AGG(DISTINCT name, ',' ORDER BY created_at DESC)"
		);
	}

	// JsonbAgg tests
	#[test]
	fn test_jsonb_agg_basic() {
		let agg = JsonbAgg::new("user_data".to_string());
		assert_eq!(agg.to_sql(), "JSONB_AGG(user_data)");
	}

	#[test]
	fn test_jsonb_agg_distinct() {
		let agg = JsonbAgg::new("category".to_string()).distinct();
		assert_eq!(agg.to_sql(), "JSONB_AGG(DISTINCT category)");
	}

	#[test]
	fn test_jsonb_agg_with_ordering() {
		let agg = JsonbAgg::new("items".to_string()).order_by(vec!["created_at DESC".to_string()]);
		assert_eq!(agg.to_sql(), "JSONB_AGG(items ORDER BY created_at DESC)");
	}

	#[test]
	fn test_jsonb_agg_distinct_with_ordering() {
		let agg = JsonbAgg::new("data".to_string())
			.distinct()
			.order_by(vec!["id ASC".to_string(), "name DESC".to_string()]);
		assert_eq!(
			agg.to_sql(),
			"JSONB_AGG(DISTINCT data ORDER BY id ASC, name DESC)"
		);
	}

	// TsRank tests
	#[test]
	fn test_ts_rank_basic() {
		let rank = TsRank::new("search_vector".to_string(), "rust".to_string());
		assert_eq!(
			rank.to_sql(),
			"ts_rank(search_vector, to_tsquery('english', 'rust'))"
		);
	}

	#[test]
	fn test_ts_rank_with_config() {
		let rank = TsRank::new("content".to_string(), "bonjour".to_string())
			.with_config("french".to_string());
		assert_eq!(
			rank.to_sql(),
			"ts_rank(content, to_tsquery('french', 'bonjour'))"
		);
	}

	#[test]
	fn test_ts_rank_with_normalization() {
		let rank = TsRank::new("content".to_string(), "rust".to_string()).with_normalization(2);
		assert_eq!(
			rank.to_sql(),
			"ts_rank(content, to_tsquery('english', 'rust'), 2)"
		);
	}

	#[test]
	fn test_ts_rank_with_config_and_normalization() {
		let rank = TsRank::new("text_vector".to_string(), "database".to_string())
			.with_config("simple".to_string())
			.with_normalization(4);
		assert_eq!(
			rank.to_sql(),
			"ts_rank(text_vector, to_tsquery('simple', 'database'), 4)"
		);
	}

	#[test]
	fn test_ts_rank_config_getter() {
		let rank = TsRank::new("content".to_string(), "query".to_string());
		assert_eq!(rank.config(), "english");

		let rank_fr = rank.with_config("french".to_string());
		assert_eq!(rank_fr.config(), "french");
	}
}
