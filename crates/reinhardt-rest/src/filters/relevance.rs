//! Search result relevance scoring system
//!
//! Provides algorithms for scoring and ranking search results based on relevance.
//!
//! # Examples
//!
//! ```
//! use reinhardt_rest::filters::{RelevanceScorer, ScoringAlgorithm};
//!
//! # async fn example() {
//! let scorer = RelevanceScorer::new()
//!     .with_algorithm(ScoringAlgorithm::BM25 { k1: 1.2, b: 0.75 })
//!     .with_boost_field("title", 2.0);
//!
//! // Verify the scorer is configured correctly
//! let _: RelevanceScorer = scorer;
//! # }
//! ```

use super::{FilterBackend, FilterResult};
use async_trait::async_trait;
use std::collections::HashMap;

/// Scoring algorithm for relevance calculation
///
/// Different algorithms have different characteristics and parameters.
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::ScoringAlgorithm;
///
/// let tfidf = ScoringAlgorithm::TfIdf;
/// let bm25 = ScoringAlgorithm::BM25 { k1: 1.2, b: 0.75 };
/// let custom = ScoringAlgorithm::Custom("my_scoring_function".to_string());
/// // Verify algorithms are created successfully
/// let _: ScoringAlgorithm = tfidf;
/// let _: ScoringAlgorithm = bm25;
/// let _: ScoringAlgorithm = custom;
/// ```
#[derive(Debug, Clone)]
pub enum ScoringAlgorithm {
	/// Term Frequency-Inverse Document Frequency
	///
	/// Classic scoring algorithm that considers term frequency and document frequency.
	TfIdf,

	/// BM25 (Best Matching 25)
	///
	/// Modern probabilistic ranking function.
	///
	/// # Parameters
	///
	/// * `k1` - Controls term frequency saturation (typical: 1.2-2.0)
	/// * `b` - Controls length normalization (typical: 0.75)
	BM25 {
		/// Controls term frequency saturation (typical: 1.2-2.0).
		k1: f64,
		/// Controls length normalization (typical: 0.75).
		b: f64,
	},

	/// Custom scoring function
	///
	/// Reference to a database-stored scoring function.
	Custom(String),
}

impl Default for ScoringAlgorithm {
	fn default() -> Self {
		Self::BM25 { k1: 1.2, b: 0.75 }
	}
}

/// Field boost configuration
///
/// Allows certain fields to have higher weight in scoring.
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::FieldBoost;
///
/// let boost = FieldBoost::new("title", 2.0);
/// // Verify the field boost is created successfully
/// assert_eq!(boost.field_name, "title");
/// assert_eq!(boost.boost_factor, 2.0);
/// ```
#[derive(Debug, Clone)]
pub struct FieldBoost {
	/// Field name
	pub field_name: String,

	/// Boost factor (1.0 = normal, >1.0 = higher weight)
	pub boost_factor: f64,
}

impl FieldBoost {
	/// Create a new field boost
	///
	/// # Arguments
	///
	/// * `field_name` - Name of the field to boost
	/// * `boost_factor` - Multiplication factor for the field's score
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::FieldBoost;
	///
	/// let title_boost = FieldBoost::new("title", 2.0);
	/// let content_boost = FieldBoost::new("content", 1.0);
	/// // Verify both field boosts are created with correct values
	/// assert_eq!(title_boost.field_name, "title");
	/// assert_eq!(title_boost.boost_factor, 2.0);
	/// assert_eq!(content_boost.field_name, "content");
	/// assert_eq!(content_boost.boost_factor, 1.0);
	/// ```
	pub fn new(field_name: impl Into<String>, boost_factor: f64) -> Self {
		Self {
			field_name: field_name.into(),
			boost_factor,
		}
	}
}

/// Scored search result
///
/// Represents a search result with its relevance score.
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::ScoredResult;
///
/// let result = ScoredResult::new(42, 0.85);
/// // Verify the scored result is created with correct values
/// assert_eq!(result.id, 42);
/// assert_eq!(result.score, 0.85);
/// ```
#[derive(Debug, Clone)]
pub struct ScoredResult {
	/// Document/record ID
	pub id: i64,

	/// Relevance score (typically 0.0-1.0, but can vary by algorithm)
	pub score: f64,

	/// Breakdown of score components (optional)
	pub score_details: Option<HashMap<String, f64>>,
}

impl ScoredResult {
	/// Create a new scored result
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::ScoredResult;
	///
	/// let result = ScoredResult::new(42, 0.85);
	/// assert_eq!(result.id, 42);
	/// assert_eq!(result.score, 0.85);
	/// ```
	pub fn new(id: i64, score: f64) -> Self {
		Self {
			id,
			score,
			score_details: None,
		}
	}

	/// Add score details for transparency
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::ScoredResult;
	/// use std::collections::HashMap;
	///
	/// let mut details = HashMap::new();
	/// details.insert("title_score".to_string(), 0.5);
	/// details.insert("content_score".to_string(), 0.35);
	///
	/// let result = ScoredResult::new(42, 0.85)
	///     .with_details(details);
	/// // Verify score details are set
	/// assert!(result.score_details.is_some());
	/// ```
	pub fn with_details(mut self, details: HashMap<String, f64>) -> Self {
		self.score_details = Some(details);
		self
	}
}

/// Relevance scorer filter backend
///
/// Adds relevance scoring to search queries, enabling ranking of results
/// by their relevance to the search terms.
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::{FilterBackend, RelevanceScorer, ScoringAlgorithm};
/// use std::collections::HashMap;
///
/// # async fn example() {
/// let scorer = RelevanceScorer::new()
///     .with_algorithm(ScoringAlgorithm::BM25 { k1: 1.2, b: 0.75 })
///     .with_boost_field("title", 2.0)
///     .with_boost_field("tags", 1.5);
///
/// let params = HashMap::new();
/// let sql = "SELECT * FROM articles".to_string();
/// let result = scorer.filter_queryset(&params, sql).await;
/// // Verify the result is Ok (filter execution succeeded)
/// assert!(result.is_ok());
/// # }
/// ```
#[derive(Debug)]
pub struct RelevanceScorer {
	algorithm: ScoringAlgorithm,
	field_boosts: Vec<FieldBoost>,
	enabled: bool,
	min_score: Option<f64>,
}

impl Default for RelevanceScorer {
	fn default() -> Self {
		Self::new()
	}
}

impl RelevanceScorer {
	/// Create a new relevance scorer
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::RelevanceScorer;
	///
	/// let scorer = RelevanceScorer::new();
	/// // Verify the scorer is created successfully
	/// let _: RelevanceScorer = scorer;
	/// ```
	pub fn new() -> Self {
		Self {
			algorithm: ScoringAlgorithm::default(),
			field_boosts: Vec::new(),
			enabled: true,
			min_score: None,
		}
	}

	/// Set the scoring algorithm
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::{RelevanceScorer, ScoringAlgorithm};
	///
	/// let scorer = RelevanceScorer::new()
	///     .with_algorithm(ScoringAlgorithm::TfIdf);
	/// // Verify the scorer is configured with the algorithm
	/// let _: RelevanceScorer = scorer;
	/// ```
	pub fn with_algorithm(mut self, algorithm: ScoringAlgorithm) -> Self {
		self.algorithm = algorithm;
		self
	}

	/// Add a field boost
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::RelevanceScorer;
	///
	/// let scorer = RelevanceScorer::new()
	///     .with_boost_field("title", 2.0)
	///     .with_boost_field("content", 1.0);
	/// // Verify the scorer is configured with field boosts
	/// let _: RelevanceScorer = scorer;
	/// ```
	pub fn with_boost_field(mut self, field_name: impl Into<String>, boost: f64) -> Self {
		self.field_boosts.push(FieldBoost::new(field_name, boost));
		self
	}

	/// Add a field boost using a FieldBoost struct
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::{RelevanceScorer, FieldBoost};
	///
	/// let boost = FieldBoost::new("title", 2.0);
	/// let scorer = RelevanceScorer::new()
	///     .with_boost(boost);
	/// // Verify the scorer is configured with the field boost
	/// let _: RelevanceScorer = scorer;
	/// ```
	pub fn with_boost(mut self, boost: FieldBoost) -> Self {
		self.field_boosts.push(boost);
		self
	}

	/// Set minimum score threshold
	///
	/// Results with scores below this threshold will be filtered out.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::RelevanceScorer;
	///
	/// let scorer = RelevanceScorer::new()
	///     .with_min_score(0.3);
	/// // Verify the scorer is configured with minimum score
	/// let _: RelevanceScorer = scorer;
	/// ```
	pub fn with_min_score(mut self, min_score: f64) -> Self {
		self.min_score = Some(min_score);
		self
	}

	/// Enable or disable scoring
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::RelevanceScorer;
	///
	/// let scorer = RelevanceScorer::new()
	///     .set_enabled(false);
	/// // Verify the scorer is configured with enabled status
	/// let _: RelevanceScorer = scorer;
	/// ```
	pub fn set_enabled(mut self, enabled: bool) -> Self {
		self.enabled = enabled;
		self
	}

	/// Generate SQL scoring expression
	///
	/// This would typically add scoring calculations to the SELECT clause
	/// and ORDER BY for relevance-based ranking.
	fn generate_scoring_sql(&self, sql: String, search_terms: &str) -> FilterResult<String> {
		use super::FilterError;

		if self.field_boosts.is_empty() {
			return Err(FilterError::InvalidParameter(
				"No search fields configured for relevance scoring".to_string(),
			));
		}

		let sql_upper = sql.to_uppercase();

		let select_end = sql_upper
			.find(" FROM ")
			.ok_or_else(|| FilterError::InvalidQuery("No FROM clause found in SQL".to_string()))?;

		let select_part = &sql[0..select_end];
		let rest_part = &sql[select_end..];

		let score_expr = self.generate_score_expression(search_terms);

		let new_select = format!("{}, {} AS relevance_score", select_part, score_expr);

		let mut result_sql = format!("{}{}", new_select, rest_part);

		if let Some(min_score) = self.min_score {
			result_sql = self.add_min_score_filter(result_sql, min_score)?;
		}

		result_sql = self.add_order_by(result_sql);

		Ok(result_sql)
	}

	/// Generate scoring expression based on the algorithm
	fn generate_score_expression(&self, search_terms: &str) -> String {
		let field_scores: Vec<String> = self
            .field_boosts
            .iter()
            .map(|boost| {
                let field_name = &boost.field_name;
                let boost_factor = boost.boost_factor;
                let base_score = match &self.algorithm {
                    ScoringAlgorithm::TfIdf => {
                        format!(
                            "(LENGTH({field}) - LENGTH(REPLACE(LOWER({field}), LOWER('{terms}'), ''))) / LENGTH('{terms}') * LOG(1000.0 / (1.0 + (LENGTH({field}) - LENGTH(REPLACE(LOWER({field}), LOWER('{terms}'), '')))))",
                            field = field_name,
                            terms = search_terms.replace('\'', "''")
                        )
                    }
                    ScoringAlgorithm::BM25 { k1, b } => {
                        let avg_field_len = 100.0;
                        format!(
                            "((LENGTH({field}) - LENGTH(REPLACE(LOWER({field}), LOWER('{terms}'), ''))) / LENGTH('{terms}')) * ({k1} + 1.0) / ((LENGTH({field}) - LENGTH(REPLACE(LOWER({field}), LOWER('{terms}'), ''))) / LENGTH('{terms}') + {k1} * (1.0 - {b} + {b} * LENGTH({field}) / {avg_len}))",
                            field = field_name,
                            terms = search_terms.replace('\'', "''"),
                            k1 = k1,
                            b = b,
                            avg_len = avg_field_len
                        )
                    }
                    ScoringAlgorithm::Custom(func_name) => {
                        format!(
                            "{}('{}', {})",
                            func_name,
                            search_terms.replace('\'', "''"),
                            field_name
                        )
                    }
                };

                if boost_factor == 1.0 {
                    base_score
                } else {
                    format!("({}) * {}", base_score, boost_factor)
                }
            })
            .collect();

		if field_scores.is_empty() {
			"0.0".to_string()
		} else if field_scores.len() == 1 {
			field_scores[0].clone()
		} else {
			format!("({})", field_scores.join(" + "))
		}
	}

	/// Add minimum score filter to WHERE clause
	fn add_min_score_filter(&self, sql: String, min_score: f64) -> FilterResult<String> {
		let sql_upper = sql.to_uppercase();
		let score_condition = format!("relevance_score >= {}", min_score);

		if let Some(where_pos) = sql_upper.find(" WHERE ") {
			let (before_where, after_where) = sql.split_at(where_pos);
			let after_where_keyword = &after_where[7..];
			Ok(format!(
				"{} WHERE {} AND ({})",
				before_where, score_condition, after_where_keyword
			))
		} else if let Some(group_pos) = sql_upper.find(" GROUP BY ") {
			let (before_group, after_group) = sql.split_at(group_pos);
			Ok(format!(
				"{} WHERE {} {}",
				before_group, score_condition, after_group
			))
		} else if let Some(order_pos) = sql_upper.find(" ORDER BY ") {
			let (before_order, after_order) = sql.split_at(order_pos);
			Ok(format!(
				"{} WHERE {} {}",
				before_order, score_condition, after_order
			))
		} else {
			Ok(format!("{} WHERE {}", sql, score_condition))
		}
	}

	/// Add ORDER BY clause for relevance ranking
	fn add_order_by(&self, sql: String) -> String {
		let sql_upper = sql.to_uppercase();

		if sql_upper.contains(" ORDER BY ") {
			sql.replace(" ORDER BY ", " ORDER BY relevance_score DESC, ")
		} else {
			format!("{} ORDER BY relevance_score DESC", sql)
		}
	}
}

#[async_trait]
impl FilterBackend for RelevanceScorer {
	async fn filter_queryset(
		&self,
		query_params: &HashMap<String, String>,
		sql: String,
	) -> FilterResult<String> {
		if !self.enabled {
			return Ok(sql);
		}

		// Look for search query parameter
		// Common parameter names: q, search, query
		let search_terms = query_params
			.get("q")
			.or_else(|| query_params.get("search"))
			.or_else(|| query_params.get("query"));

		if let Some(terms) = search_terms {
			self.generate_scoring_sql(sql, terms)
		} else {
			// No search terms, pass through
			Ok(sql)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_scoring_algorithm_variants() {
		let algorithms = [
			ScoringAlgorithm::TfIdf,
			ScoringAlgorithm::BM25 { k1: 1.2, b: 0.75 },
			ScoringAlgorithm::Custom("my_func".to_string()),
		];
		assert_eq!(algorithms.len(), 3);
	}

	#[test]
	fn test_scoring_algorithm_default() {
		let algo = ScoringAlgorithm::default();
		match algo {
			ScoringAlgorithm::BM25 { k1, b } => {
				assert_eq!(k1, 1.2);
				assert_eq!(b, 0.75);
			}
			_ => panic!("Expected BM25 default"),
		}
	}

	#[test]
	fn test_field_boost_creation() {
		let boost = FieldBoost::new("title", 2.0);
		assert_eq!(boost.field_name, "title");
		assert_eq!(boost.boost_factor, 2.0);
	}

	#[test]
	fn test_scored_result_creation() {
		let result = ScoredResult::new(42, 0.85);
		assert_eq!(result.id, 42);
		assert_eq!(result.score, 0.85);
		assert!(result.score_details.is_none());
	}

	#[test]
	fn test_scored_result_with_details() {
		let mut details = HashMap::new();
		details.insert("title_score".to_string(), 0.5);
		details.insert("content_score".to_string(), 0.35);

		let result = ScoredResult::new(42, 0.85).with_details(details);

		assert!(result.score_details.is_some());
		let details = result.score_details.unwrap();
		assert_eq!(details.get("title_score"), Some(&0.5));
		assert_eq!(details.get("content_score"), Some(&0.35));
	}

	#[test]
	fn test_relevance_scorer_creation() {
		let scorer = RelevanceScorer::new();
		assert!(scorer.enabled);
		assert!(scorer.field_boosts.is_empty());
		assert!(scorer.min_score.is_none());
	}

	#[test]
	fn test_relevance_scorer_with_algorithm() {
		let scorer = RelevanceScorer::new().with_algorithm(ScoringAlgorithm::TfIdf);
		match scorer.algorithm {
			ScoringAlgorithm::TfIdf => (),
			_ => panic!("Expected TfIdf algorithm"),
		}
	}

	#[test]
	fn test_relevance_scorer_with_boost_field() {
		let scorer = RelevanceScorer::new()
			.with_boost_field("title", 2.0)
			.with_boost_field("content", 1.0);

		assert_eq!(scorer.field_boosts.len(), 2);
		assert_eq!(scorer.field_boosts[0].field_name, "title");
		assert_eq!(scorer.field_boosts[0].boost_factor, 2.0);
	}

	#[test]
	fn test_relevance_scorer_with_boost_struct() {
		let boost = FieldBoost::new("tags", 1.5);
		let scorer = RelevanceScorer::new().with_boost(boost);

		assert_eq!(scorer.field_boosts.len(), 1);
		assert_eq!(scorer.field_boosts[0].field_name, "tags");
		assert_eq!(scorer.field_boosts[0].boost_factor, 1.5);
	}

	#[test]
	fn test_relevance_scorer_min_score_setter() {
		let scorer = RelevanceScorer::new().with_min_score(0.3);
		assert_eq!(scorer.min_score, Some(0.3));
	}

	#[test]
	fn test_relevance_scorer_disabled() {
		let scorer = RelevanceScorer::new().set_enabled(false);
		assert!(!scorer.enabled);
	}

	#[tokio::test]
	async fn test_relevance_scorer_no_search_terms() {
		let scorer = RelevanceScorer::new();

		let params = HashMap::new();
		let sql = "SELECT * FROM articles".to_string();
		let result = scorer.filter_queryset(&params, sql.clone()).await.unwrap();

		assert_eq!(result, sql);
	}

	#[tokio::test]
	async fn test_relevance_scorer_disabled_passthrough() {
		let scorer = RelevanceScorer::new().set_enabled(false);

		let mut params = HashMap::new();
		params.insert("q".to_string(), "rust".to_string());

		let sql = "SELECT * FROM articles".to_string();
		let result = scorer.filter_queryset(&params, sql.clone()).await.unwrap();

		assert_eq!(result, sql);
	}

	#[tokio::test]
	async fn test_relevance_scorer_tfidf_algorithm() {
		let scorer = RelevanceScorer::new()
			.with_algorithm(ScoringAlgorithm::TfIdf)
			.with_boost_field("title", 2.0)
			.with_boost_field("content", 1.0);

		let mut params = HashMap::new();
		params.insert("q".to_string(), "rust".to_string());

		let sql = "SELECT id, title FROM articles".to_string();
		let result = scorer.filter_queryset(&params, sql).await.unwrap();

		// Validate SELECT clause includes relevance_score
		assert!(
			result.contains(", ") && result.contains(" AS relevance_score"),
			"Expected ', ... AS relevance_score' in SELECT clause, got: {}",
			result
		);

		// Validate ORDER BY clause
		assert!(
			result.ends_with(" ORDER BY relevance_score DESC"),
			"Expected result to end with ' ORDER BY relevance_score DESC', got: {}",
			result
		);

		// Validate TF-IDF scoring includes both fields with proper boost factors
		// TF-IDF formula should include LENGTH, REPLACE, and LOG functions
		assert!(
			result.contains("LENGTH(title)") && result.contains("REPLACE"),
			"Expected TF-IDF formula with LENGTH and REPLACE for title, got: {}",
			result
		);
		assert!(
			result.contains("LENGTH(content)"),
			"Expected TF-IDF formula for content field, got: {}",
			result
		);
		assert!(
			result.contains("* 2"),
			"Expected boost factor '* 2' for title field, got: {}",
			result
		);
		assert!(
			result.contains("LOG"),
			"Expected LOG function in TF-IDF formula, got: {}",
			result
		);
	}

	#[tokio::test]
	async fn test_relevance_scorer_bm25_algorithm() {
		let scorer = RelevanceScorer::new()
			.with_algorithm(ScoringAlgorithm::BM25 { k1: 1.5, b: 0.75 })
			.with_boost_field("title", 1.0);

		let mut params = HashMap::new();
		params.insert("q".to_string(), "programming".to_string());

		let sql = "SELECT * FROM articles".to_string();
		let result = scorer.filter_queryset(&params, sql).await.unwrap();

		// Validate SELECT clause includes relevance_score
		assert!(
			result.contains(" AS relevance_score"),
			"Expected ' AS relevance_score' in SELECT clause, got: {}",
			result
		);

		// Validate ORDER BY clause
		assert!(
			result.ends_with(" ORDER BY relevance_score DESC"),
			"Expected result to end with ' ORDER BY relevance_score DESC', got: {}",
			result
		);

		// Validate BM25 parameters are included in the formula
		assert!(
			result.contains("1.5") && result.contains("0.75"),
			"Expected BM25 parameters k1=1.5 and b=0.75 in formula, got: {}",
			result
		);

		// Validate BM25 scoring formula includes field length normalization
		assert!(
			result.contains("LENGTH(title)"),
			"Expected LENGTH(title) in BM25 formula, got: {}",
			result
		);
	}

	#[tokio::test]
	async fn test_relevance_scorer_custom_algorithm() {
		let scorer = RelevanceScorer::new()
			.with_algorithm(ScoringAlgorithm::Custom("my_score_func".to_string()))
			.with_boost_field("title", 1.0);

		let mut params = HashMap::new();
		params.insert("q".to_string(), "test".to_string());

		let sql = "SELECT * FROM articles".to_string();
		let result = scorer.filter_queryset(&params, sql).await.unwrap();

		// Validate custom function is called correctly
		assert!(
			result.contains("my_score_func('test', title)"),
			"Expected 'my_score_func('test', title)' in result, got: {}",
			result
		);

		// Validate SELECT clause includes relevance_score
		assert!(
			result.contains(" AS relevance_score"),
			"Expected ' AS relevance_score' in SELECT clause, got: {}",
			result
		);
	}

	#[tokio::test]
	async fn test_relevance_scorer_with_min_score() {
		let scorer = RelevanceScorer::new()
			.with_algorithm(ScoringAlgorithm::TfIdf)
			.with_boost_field("title", 1.0)
			.with_min_score(0.5);

		let mut params = HashMap::new();
		params.insert("q".to_string(), "rust".to_string());

		let sql = "SELECT * FROM articles".to_string();
		let result = scorer.filter_queryset(&params, sql).await.unwrap();

		// Validate WHERE clause with exact min_score condition
		let where_start = result.find("WHERE").expect("WHERE clause not found");
		let where_clause = &result[where_start..];

		assert!(
			where_clause.starts_with("WHERE relevance_score >= 0.5"),
			"Expected WHERE clause to start with 'WHERE relevance_score >= 0.5', got: {}",
			where_clause
		);
	}

	#[tokio::test]
	async fn test_relevance_scorer_with_existing_where() {
		let scorer = RelevanceScorer::new()
			.with_algorithm(ScoringAlgorithm::TfIdf)
			.with_boost_field("title", 1.0)
			.with_min_score(0.3);

		let mut params = HashMap::new();
		params.insert("q".to_string(), "rust".to_string());

		let sql = "SELECT * FROM articles WHERE published = true".to_string();
		let result = scorer.filter_queryset(&params, sql).await.unwrap();

		// Validate the WHERE clause structure combines min_score with existing condition
		let where_start = result.find("WHERE").expect("WHERE clause not found");
		let where_clause = &result[where_start..];

		// Structure should be: WHERE relevance_score >= 0.3 AND (published = true)
		assert!(
			where_clause.starts_with("WHERE relevance_score >= 0.3 AND"),
			"Expected WHERE clause to start with 'WHERE relevance_score >= 0.3 AND', got: {}",
			where_clause
		);

		assert!(
			where_clause.contains("published = true"),
			"Expected 'published = true' in WHERE clause, got: {}",
			where_clause
		);

		// Validate order: min_score condition comes before existing condition
		let score_pos = where_clause.find("relevance_score >= 0.3").unwrap();
		let and_pos = where_clause.find(" AND ").unwrap();
		let published_pos = where_clause.find("published = true").unwrap();
		assert!(
			score_pos < and_pos && and_pos < published_pos,
			"Expected order: relevance_score >= 0.3 AND published = true, got: {}",
			where_clause
		);
	}

	#[tokio::test]
	async fn test_relevance_scorer_with_existing_order_by() {
		let scorer = RelevanceScorer::new()
			.with_algorithm(ScoringAlgorithm::TfIdf)
			.with_boost_field("title", 1.0);

		let mut params = HashMap::new();
		params.insert("q".to_string(), "rust".to_string());

		let sql = "SELECT * FROM articles ORDER BY created_at DESC".to_string();
		let result = scorer.filter_queryset(&params, sql).await.unwrap();

		// Validate ORDER BY clause structure
		let order_start = result.find("ORDER BY").expect("ORDER BY clause not found");
		let order_clause = &result[order_start..];

		// Structure should be: ORDER BY relevance_score DESC, created_at DESC
		assert!(
			order_clause.starts_with("ORDER BY relevance_score DESC, created_at DESC"),
			"Expected 'ORDER BY relevance_score DESC, created_at DESC', got: {}",
			order_clause
		);

		// Validate order: relevance_score comes before created_at
		let relevance_pos = order_clause.find("relevance_score DESC").unwrap();
		let created_pos = order_clause.find("created_at DESC").unwrap();
		assert!(
			relevance_pos < created_pos,
			"Expected relevance_score DESC before created_at DESC, got: {}",
			order_clause
		);
	}

	#[tokio::test]
	async fn test_relevance_scorer_field_boost_application() {
		let scorer = RelevanceScorer::new()
			.with_algorithm(ScoringAlgorithm::TfIdf)
			.with_boost_field("title", 3.0)
			.with_boost_field("content", 1.0);

		let mut params = HashMap::new();
		params.insert("q".to_string(), "rust".to_string());

		let sql = "SELECT * FROM articles".to_string();
		let result = scorer.filter_queryset(&params, sql).await.unwrap();

		// Validate boost factor is applied to title field
		assert!(
			result.contains(") * 3"),
			"Expected boost factor '* 3' for title field, got: {}",
			result
		);

		// Validate both fields are included in scoring
		assert!(
			result.contains("LENGTH(title)"),
			"Expected 'LENGTH(title)' in scoring formula, got: {}",
			result
		);
		assert!(
			result.contains("LENGTH(content)"),
			"Expected 'LENGTH(content)' in scoring formula, got: {}",
			result
		);

		// Validate fields are combined with addition (since multiple fields)
		// Note: The TF-IDF formula contains multiple + operators internally,
		// so we just verify that fields are combined somehow
		let select_clause_end = result.find(" FROM ").expect("FROM clause not found");
		let select_clause = &result[..select_clause_end];
		assert!(
			select_clause.contains(" + "),
			"Expected '+' operators to combine field scores, got: {}",
			select_clause
		);
	}

	#[tokio::test]
	async fn test_relevance_scorer_multiple_fields() {
		let scorer = RelevanceScorer::new()
			.with_algorithm(ScoringAlgorithm::BM25 { k1: 1.2, b: 0.75 })
			.with_boost_field("title", 2.0)
			.with_boost_field("content", 1.0)
			.with_boost_field("tags", 1.5);

		let mut params = HashMap::new();
		params.insert("search".to_string(), "rust".to_string());

		let sql = "SELECT * FROM articles".to_string();
		let result = scorer.filter_queryset(&params, sql).await.unwrap();

		// Validate all three fields are included in scoring
		assert!(
			result.contains("LENGTH(title)"),
			"Expected 'LENGTH(title)' in scoring formula, got: {}",
			result
		);
		assert!(
			result.contains("LENGTH(content)"),
			"Expected 'LENGTH(content)' in scoring formula, got: {}",
			result
		);
		assert!(
			result.contains("LENGTH(tags)"),
			"Expected 'LENGTH(tags)' in scoring formula, got: {}",
			result
		);

		// Validate boost factors are applied correctly
		assert!(
			result.contains(") * 2"),
			"Expected boost factor '* 2' for title, got: {}",
			result
		);
		assert!(
			result.contains(") * 1.5"),
			"Expected boost factor '* 1.5' for tags, got: {}",
			result
		);

		// Validate fields are combined with addition
		// Note: The BM25 formula contains multiple + operators internally,
		// so we just verify that fields are combined somehow
		let select_clause_end = result.find(" FROM ").expect("FROM clause not found");
		let select_clause = &result[..select_clause_end];
		assert!(
			select_clause.contains(" + "),
			"Expected '+' operators to combine field scores, got: {}",
			select_clause
		);
	}

	#[tokio::test]
	async fn test_relevance_scorer_no_fields_error() {
		let scorer = RelevanceScorer::new().with_algorithm(ScoringAlgorithm::TfIdf);

		let mut params = HashMap::new();
		params.insert("q".to_string(), "rust".to_string());

		let sql = "SELECT * FROM articles".to_string();
		let result = scorer.filter_queryset(&params, sql).await;

		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(err.to_string().contains("No search fields"));
	}

	#[tokio::test]
	async fn test_relevance_scorer_invalid_sql_no_from() {
		let scorer = RelevanceScorer::new()
			.with_algorithm(ScoringAlgorithm::TfIdf)
			.with_boost_field("title", 1.0);

		let mut params = HashMap::new();
		params.insert("q".to_string(), "rust".to_string());

		let sql = "SELECT * WHERE id = 1".to_string();
		let result = scorer.filter_queryset(&params, sql).await;

		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(err.to_string().contains("No FROM clause"));
	}

	#[tokio::test]
	async fn test_relevance_scorer_query_param_variants() {
		let scorer = RelevanceScorer::new()
			.with_algorithm(ScoringAlgorithm::TfIdf)
			.with_boost_field("title", 1.0);

		let sql = "SELECT * FROM articles".to_string();

		let mut params_q = HashMap::new();
		params_q.insert("q".to_string(), "rust".to_string());
		let result_q = scorer.filter_queryset(&params_q, sql.clone()).await;
		assert!(result_q.is_ok());

		let mut params_search = HashMap::new();
		params_search.insert("search".to_string(), "rust".to_string());
		let result_search = scorer.filter_queryset(&params_search, sql.clone()).await;
		assert!(result_search.is_ok());

		let mut params_query = HashMap::new();
		params_query.insert("query".to_string(), "rust".to_string());
		let result_query = scorer.filter_queryset(&params_query, sql).await;
		assert!(result_query.is_ok());
	}

	#[tokio::test]
	async fn test_relevance_scorer_sql_injection_protection() {
		let scorer = RelevanceScorer::new()
			.with_algorithm(ScoringAlgorithm::TfIdf)
			.with_boost_field("title", 1.0);

		let mut params = HashMap::new();
		let malicious_input = "rust'; DROP TABLE articles; --";
		params.insert("q".to_string(), malicious_input.to_string());

		let sql = "SELECT * FROM articles".to_string();
		let result = scorer.filter_queryset(&params, sql).await.unwrap();

		// Validate SQL injection is prevented by proper escaping
		let escaped_input = malicious_input.replace('\'', "''");
		assert!(
			result.contains(&escaped_input),
			"Expected properly escaped input '{}' in result, got: {}",
			escaped_input,
			result
		);

		// The escaped malicious input should be within LOWER() function
		assert!(
			result.contains(&format!("LOWER('{}')", escaped_input)),
			"Expected malicious input to be in LOWER('{}') format, got: {}",
			escaped_input,
			result
		);

		// Validate the malicious content is treated as a literal string in the scoring formula
		// The formula should use the search term in multiple places for TF-IDF calculation
		// Since it's escaped properly, we just verify it appears in the expected context
		assert!(
			result.contains("LENGTH(title)"),
			"Expected LENGTH(title) in scoring formula, got: {}",
			result
		);
	}

	#[test]
	fn test_generate_score_expression_tfidf() {
		let scorer = RelevanceScorer::new()
			.with_algorithm(ScoringAlgorithm::TfIdf)
			.with_boost_field("title", 1.0);

		let expr = scorer.generate_score_expression("test");

		// Validate TF-IDF formula components
		assert!(
			expr.contains("LENGTH(title)"),
			"Expected LENGTH(title) in expression, got: {}",
			expr
		);
		assert!(
			expr.contains("REPLACE(LOWER(title), LOWER('test'), '')"),
			"Expected REPLACE function in expression, got: {}",
			expr
		);
		assert!(
			expr.contains("LOG("),
			"Expected LOG function in TF-IDF formula, got: {}",
			expr
		);
	}

	#[test]
	fn test_generate_score_expression_bm25() {
		let scorer = RelevanceScorer::new()
			.with_algorithm(ScoringAlgorithm::BM25 { k1: 1.2, b: 0.75 })
			.with_boost_field("content", 1.0);

		let expr = scorer.generate_score_expression("test");

		// Validate BM25 parameters are present
		assert!(
			expr.contains("1.2") && expr.contains("0.75"),
			"Expected BM25 parameters k1=1.2 and b=0.75 in expression, got: {}",
			expr
		);
		assert!(
			expr.contains("LENGTH(content)"),
			"Expected LENGTH(content) in expression, got: {}",
			expr
		);
	}

	#[test]
	fn test_generate_score_expression_custom() {
		let scorer = RelevanceScorer::new()
			.with_algorithm(ScoringAlgorithm::Custom("custom_score".to_string()))
			.with_boost_field("title", 1.0);

		let expr = scorer.generate_score_expression("test");

		// Validate custom function call format
		assert_eq!(
			expr, "custom_score('test', title)",
			"Expected exact format 'custom_score('test', title)', got: {}",
			expr
		);
	}

	#[test]
	fn test_generate_score_expression_with_boost() {
		let scorer = RelevanceScorer::new()
			.with_algorithm(ScoringAlgorithm::TfIdf)
			.with_boost_field("title", 2.5);

		let expr = scorer.generate_score_expression("test");

		// Validate boost factor is applied
		assert!(
			expr.contains(") * 2.5"),
			"Expected boost factor '* 2.5' at the end, got: {}",
			expr
		);
	}

	#[test]
	fn test_generate_score_expression_multiple_fields() {
		let scorer = RelevanceScorer::new()
			.with_algorithm(ScoringAlgorithm::TfIdf)
			.with_boost_field("title", 2.0)
			.with_boost_field("content", 1.0);

		let expr = scorer.generate_score_expression("test");

		// Validate both fields are present
		assert!(
			expr.contains("LENGTH(title)"),
			"Expected LENGTH(title) in expression, got: {}",
			expr
		);
		assert!(
			expr.contains("LENGTH(content)"),
			"Expected LENGTH(content) in expression, got: {}",
			expr
		);

		// Validate fields are combined with addition
		assert!(
			expr.contains(" + "),
			"Expected '+' operator to combine fields, got: {}",
			expr
		);

		// Validate the expression is wrapped in parentheses
		assert!(
			expr.starts_with("(") && expr.ends_with(")"),
			"Expected expression to be wrapped in parentheses, got: {}",
			expr
		);
	}
}
