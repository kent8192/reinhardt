//! Synonym expansion system for search enhancement
//!
//! Automatically expands search queries with synonyms to improve search recall.
//!
//! # Examples
//!
//! ```
//! use reinhardt_rest::filters::{SynonymExpander, SynonymDictionary};
//!
//! # async fn example() {
//! let mut dict = SynonymDictionary::new();
//! dict.add_synonym("quick", "fast");
//! dict.add_synonym("quick", "rapid");
//!
//! let expander = SynonymExpander::new()
//!     .with_dictionary(dict);
//!
//! // Verify the expander is configured successfully
//! let _: SynonymExpander = expander;
//! # }
//! ```

use super::{FilterBackend, FilterResult};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};

/// Synonym dictionary for managing term relationships
///
/// Stores mappings between terms and their synonyms for query expansion.
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::SynonymDictionary;
///
/// let mut dict = SynonymDictionary::new();
/// dict.add_synonym("car", "automobile");
/// dict.add_synonym("car", "vehicle");
///
/// let synonyms = dict.get_synonyms("car");
/// assert_eq!(synonyms.len(), 2);
/// ```
#[derive(Debug, Clone, Default)]
pub struct SynonymDictionary {
	/// Maps terms to their synonyms
	synonyms: HashMap<String, HashSet<String>>,

	/// Whether to make synonym relationships bidirectional
	bidirectional: bool,
}

impl SynonymDictionary {
	/// Create a new empty synonym dictionary
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::SynonymDictionary;
	///
	/// let dict = SynonymDictionary::new();
	/// // Verify the dictionary is created successfully
	/// let _: SynonymDictionary = dict;
	/// ```
	pub fn new() -> Self {
		Self {
			synonyms: HashMap::new(),
			bidirectional: true,
		}
	}

	/// Create a new dictionary with bidirectional setting
	///
	/// # Arguments
	///
	/// * `bidirectional` - If true, synonyms work both ways (A→B implies B→A)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::SynonymDictionary;
	///
	/// let dict = SynonymDictionary::with_bidirectional(true);
	/// // Verify the dictionary is created with bidirectional setting
	/// let _: SynonymDictionary = dict;
	/// ```
	pub fn with_bidirectional(bidirectional: bool) -> Self {
		Self {
			synonyms: HashMap::new(),
			bidirectional,
		}
	}

	/// Add a synonym relationship
	///
	/// # Arguments
	///
	/// * `term` - The original term
	/// * `synonym` - The synonym for the term
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::SynonymDictionary;
	///
	/// let mut dict = SynonymDictionary::new();
	/// dict.add_synonym("happy", "joyful");
	/// dict.add_synonym("happy", "glad");
	/// // Verify synonyms are added successfully
	/// assert_eq!(dict.get_synonyms("happy").len(), 2);
	/// ```
	pub fn add_synonym(&mut self, term: impl Into<String>, synonym: impl Into<String>) {
		let term = term.into().to_lowercase();
		let synonym = synonym.into().to_lowercase();

		// Add term → synonym
		self.synonyms
			.entry(term.clone())
			.or_default()
			.insert(synonym.clone());

		// Add synonym → term if bidirectional
		if self.bidirectional {
			self.synonyms.entry(synonym).or_default().insert(term);
		}
	}

	/// Add multiple synonyms for a term
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::SynonymDictionary;
	///
	/// let mut dict = SynonymDictionary::new();
	/// dict.add_synonyms("big", vec!["large", "huge", "enormous"]);
	/// // Verify all synonyms are added
	/// assert_eq!(dict.get_synonyms("big").len(), 3);
	/// ```
	pub fn add_synonyms(
		&mut self,
		term: impl Into<String>,
		synonyms: impl IntoIterator<Item = impl Into<String>>,
	) {
		let term = term.into();
		for synonym in synonyms {
			self.add_synonym(term.clone(), synonym);
		}
	}

	/// Get all synonyms for a term
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::SynonymDictionary;
	///
	/// let mut dict = SynonymDictionary::new();
	/// dict.add_synonym("car", "automobile");
	/// dict.add_synonym("car", "vehicle");
	///
	/// let synonyms = dict.get_synonyms("car");
	/// assert_eq!(synonyms.len(), 2);
	/// assert!(synonyms.contains(&"automobile".to_string()));
	/// ```
	pub fn get_synonyms(&self, term: &str) -> Vec<String> {
		self.synonyms
			.get(&term.to_lowercase())
			.map(|set| set.iter().cloned().collect())
			.unwrap_or_default()
	}

	/// Load synonyms from a list of synonym groups
	///
	/// Each group is a list of equivalent terms.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::SynonymDictionary;
	///
	/// let groups = vec![
	///     vec!["happy", "joyful", "glad"],
	///     vec!["sad", "unhappy", "sorrowful"],
	/// ];
	///
	/// let dict = SynonymDictionary::from_groups(groups);
	/// // Verify the dictionary is created from groups
	/// assert!(!dict.is_empty());
	/// ```
	pub fn from_groups(groups: Vec<Vec<impl Into<String>>>) -> Self {
		let mut dict = Self::new();

		for group in groups {
			let terms: Vec<String> = group.into_iter().map(|s| s.into()).collect();

			// Add bidirectional relationships within the group
			for i in 0..terms.len() {
				for j in 0..terms.len() {
					if i != j {
						dict.add_synonym(terms[i].clone(), terms[j].clone());
					}
				}
			}
		}

		dict
	}

	/// Get the number of terms in the dictionary
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::SynonymDictionary;
	///
	/// let mut dict = SynonymDictionary::new();
	/// dict.add_synonym("car", "automobile");
	///
	/// assert_eq!(dict.len(), 2); // "car" and "automobile" (bidirectional)
	/// ```
	pub fn len(&self) -> usize {
		self.synonyms.len()
	}

	/// Check if the dictionary is empty
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::SynonymDictionary;
	///
	/// let dict = SynonymDictionary::new();
	/// assert!(dict.is_empty());
	/// ```
	pub fn is_empty(&self) -> bool {
		self.synonyms.is_empty()
	}
}

/// Synonym expansion filter backend
///
/// Automatically expands search queries with synonyms to improve search coverage.
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::{FilterBackend, SynonymExpander, SynonymDictionary};
/// use std::collections::HashMap;
///
/// # async fn example() {
/// let mut dict = SynonymDictionary::new();
/// dict.add_synonym("fast", "quick");
/// dict.add_synonym("fast", "rapid");
///
/// let expander = SynonymExpander::new()
///     .with_dictionary(dict)
///     .with_expansion_limit(5);
///
/// let params = HashMap::new();
/// let sql = "SELECT * FROM articles".to_string();
/// let result = expander.filter_queryset(&params, sql).await;
/// // Verify the filter execution succeeded
/// assert!(result.is_ok());
/// # }
/// ```
#[derive(Debug, Default)]
pub struct SynonymExpander {
	dictionary: SynonymDictionary,
	enabled: bool,
	expansion_limit: Option<usize>,
	min_term_length: usize,
}

impl SynonymExpander {
	/// Create a new synonym expander
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::SynonymExpander;
	///
	/// let expander = SynonymExpander::new();
	/// // Verify the expander is created successfully
	/// let _: SynonymExpander = expander;
	/// ```
	pub fn new() -> Self {
		Self {
			dictionary: SynonymDictionary::new(),
			enabled: true,
			expansion_limit: Some(10),
			min_term_length: 3,
		}
	}

	/// Set the synonym dictionary
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::{SynonymExpander, SynonymDictionary};
	///
	/// let mut dict = SynonymDictionary::new();
	/// dict.add_synonym("car", "automobile");
	///
	/// let expander = SynonymExpander::new()
	///     .with_dictionary(dict);
	/// // Verify the expander is configured with the dictionary
	/// let _: SynonymExpander = expander;
	/// ```
	pub fn with_dictionary(mut self, dictionary: SynonymDictionary) -> Self {
		self.dictionary = dictionary;
		self
	}

	/// Set the maximum number of synonym expansions per term
	///
	/// # Arguments
	///
	/// * `limit` - Maximum number of synonyms to add per term (None = unlimited)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::SynonymExpander;
	///
	/// let expander = SynonymExpander::new()
	///     .with_expansion_limit(5);
	/// // Verify the expander is configured with expansion limit
	/// let _: SynonymExpander = expander;
	/// ```
	pub fn with_expansion_limit(mut self, limit: usize) -> Self {
		self.expansion_limit = Some(limit);
		self
	}

	/// Set minimum term length for synonym expansion
	///
	/// Short terms (e.g., "a", "is") are often not expanded.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::SynonymExpander;
	///
	/// let expander = SynonymExpander::new()
	///     .with_min_term_length(4);
	/// // Verify the expander is configured with minimum term length
	/// let _: SynonymExpander = expander;
	/// ```
	pub fn with_min_term_length(mut self, length: usize) -> Self {
		self.min_term_length = length;
		self
	}

	/// Enable or disable synonym expansion
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::SynonymExpander;
	///
	/// let expander = SynonymExpander::new()
	///     .set_enabled(false);
	/// // Verify the expander is configured with enabled status
	/// let _: SynonymExpander = expander;
	/// ```
	pub fn set_enabled(mut self, enabled: bool) -> Self {
		self.enabled = enabled;
		self
	}

	/// Expand a search query with synonyms
	///
	/// # Arguments
	///
	/// * `query` - The original search query
	///
	/// # Returns
	///
	/// List of expanded terms (including original)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::{SynonymExpander, SynonymDictionary};
	///
	/// let mut dict = SynonymDictionary::new();
	/// dict.add_synonym("fast", "quick");
	/// dict.add_synonym("fast", "rapid");
	///
	/// let expander = SynonymExpander::new()
	///     .with_dictionary(dict);
	///
	/// let expanded = expander.expand_query("fast car");
	/// assert!(expanded.len() > 2);
	/// ```
	pub fn expand_query(&self, query: &str) -> Vec<String> {
		let terms: Vec<&str> = query.split_whitespace().collect();
		let mut expanded_terms = Vec::new();

		for term in terms {
			// Always include original term
			expanded_terms.push(term.to_string());

			// Skip expansion for short terms
			if term.len() < self.min_term_length {
				continue;
			}

			// Get synonyms for this term
			let synonyms = self.dictionary.get_synonyms(term);

			// Apply expansion limit
			let synonyms_to_add = if let Some(limit) = self.expansion_limit {
				synonyms.into_iter().take(limit).collect::<Vec<_>>()
			} else {
				synonyms
			};

			expanded_terms.extend(synonyms_to_add);
		}

		expanded_terms
	}

	/// Generate SQL with synonym expansion
	///
	/// This would modify the WHERE clause to include synonym alternatives.
	///
	/// # Arguments
	///
	/// * `sql` - The original SQL query
	/// * `search_terms` - The search terms to expand with synonyms
	///
	/// # Returns
	///
	/// Modified SQL with synonym expansion in WHERE clause
	fn apply_expansion(&self, sql: String, search_terms: &str) -> FilterResult<String> {
		use reinhardt_query::prelude::{Cond, Expr, MySqlQueryBuilder, QueryStatementBuilder};

		// Expand search terms with synonyms
		let expanded_terms = self.expand_query(search_terms);

		if expanded_terms.is_empty() {
			return Ok(sql);
		}

		// Build WHERE conditions using reinhardt-query with Expr::cust() to avoid identifier quoting
		// This ensures database-agnostic SQL generation
		let mut cond = Cond::any();
		for term in &expanded_terms {
			// Escape single quotes for SQL safety
			let escaped_term = term.replace('\'', "''");
			let like_expr = format!("content LIKE '%{}%'", escaped_term);
			cond = cond.add(Expr::cust(like_expr));
		}

		// Convert the condition to SQL string
		let dummy_query = reinhardt_query::prelude::Query::select()
			.expr(Expr::val(1))
			.cond_where(cond)
			.to_owned();

		let full_sql = dummy_query.to_string(MySqlQueryBuilder);

		// Extract the WHERE clause content from the dummy query
		let where_idx = full_sql.find("WHERE").unwrap_or(0);
		let where_clause = &full_sql[where_idx..];

		// Inject WHERE clause into SQL
		if sql.to_uppercase().contains("WHERE") {
			// SQL already has WHERE clause, append with AND
			Ok(sql.replace("WHERE", &format!("{} AND", where_clause)))
		} else {
			// No WHERE clause, append at the end
			Ok(format!("{} {}", sql, where_clause))
		}
	}
}

#[async_trait]
impl FilterBackend for SynonymExpander {
	async fn filter_queryset(
		&self,
		query_params: &HashMap<String, String>,
		sql: String,
	) -> FilterResult<String> {
		if !self.enabled {
			return Ok(sql);
		}

		// Look for search query parameter
		let search_terms = query_params
			.get("q")
			.or_else(|| query_params.get("search"))
			.or_else(|| query_params.get("query"));

		if let Some(terms) = search_terms {
			self.apply_expansion(sql, terms)
		} else {
			Ok(sql)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_synonym_dictionary_creation() {
		let dict = SynonymDictionary::new();
		assert!(dict.is_empty());
		assert_eq!(dict.len(), 0);
	}

	#[test]
	fn test_synonym_dictionary_add_synonym() {
		let mut dict = SynonymDictionary::new();
		dict.add_synonym("car", "automobile");

		let synonyms = dict.get_synonyms("car");
		assert_eq!(synonyms.len(), 1);
		assert!(synonyms.contains(&"automobile".to_string()));
	}

	#[test]
	fn test_synonym_dictionary_bidirectional() {
		let mut dict = SynonymDictionary::new();
		dict.add_synonym("car", "automobile");

		// Should work both ways
		let car_synonyms = dict.get_synonyms("car");
		assert!(car_synonyms.contains(&"automobile".to_string()));

		let auto_synonyms = dict.get_synonyms("automobile");
		assert!(auto_synonyms.contains(&"car".to_string()));
	}

	#[test]
	fn test_synonym_dictionary_unidirectional() {
		let mut dict = SynonymDictionary::with_bidirectional(false);
		dict.add_synonym("car", "automobile");

		let car_synonyms = dict.get_synonyms("car");
		assert!(car_synonyms.contains(&"automobile".to_string()));

		let auto_synonyms = dict.get_synonyms("automobile");
		assert!(auto_synonyms.is_empty());
	}

	#[test]
	fn test_synonym_dictionary_add_multiple() {
		let mut dict = SynonymDictionary::new();
		dict.add_synonyms("big", vec!["large", "huge", "enormous"]);

		let synonyms = dict.get_synonyms("big");
		assert_eq!(synonyms.len(), 3);
		assert!(synonyms.contains(&"large".to_string()));
		assert!(synonyms.contains(&"huge".to_string()));
		assert!(synonyms.contains(&"enormous".to_string()));
	}

	#[test]
	fn test_synonym_dictionary_from_groups() {
		let groups = vec![
			vec!["happy", "joyful", "glad"],
			vec!["sad", "unhappy", "sorrowful"],
		];

		let dict = SynonymDictionary::from_groups(groups);

		let happy_synonyms = dict.get_synonyms("happy");
		assert_eq!(happy_synonyms.len(), 2);
		assert!(happy_synonyms.contains(&"joyful".to_string()));
		assert!(happy_synonyms.contains(&"glad".to_string()));
	}

	#[test]
	fn test_synonym_dictionary_case_insensitive() {
		let mut dict = SynonymDictionary::new();
		dict.add_synonym("Car", "Automobile");

		let synonyms = dict.get_synonyms("car");
		assert!(synonyms.contains(&"automobile".to_string()));

		let synonyms_upper = dict.get_synonyms("CAR");
		assert!(synonyms_upper.contains(&"automobile".to_string()));
	}

	#[test]
	fn test_synonym_expander_creation() {
		let expander = SynonymExpander::new();
		assert!(expander.enabled);
		assert_eq!(expander.min_term_length, 3);
		assert_eq!(expander.expansion_limit, Some(10));
	}

	#[test]
	fn test_synonym_expander_with_dictionary() {
		let mut dict = SynonymDictionary::new();
		dict.add_synonym("fast", "quick");

		let expander = SynonymExpander::new().with_dictionary(dict);

		let expanded = expander.expand_query("fast");
		assert!(expanded.len() > 1);
	}

	#[test]
	fn test_synonym_expander_expand_query() {
		let mut dict = SynonymDictionary::new();
		dict.add_synonym("fast", "quick");
		dict.add_synonym("fast", "rapid");

		let expander = SynonymExpander::new().with_dictionary(dict);

		let expanded = expander.expand_query("fast car");
		assert!(expanded.contains(&"fast".to_string()));
		assert!(expanded.contains(&"quick".to_string()));
		assert!(expanded.contains(&"rapid".to_string()));
		assert!(expanded.contains(&"car".to_string()));
	}

	#[test]
	fn test_synonym_expander_min_term_length() {
		let mut dict = SynonymDictionary::new();
		dict.add_synonym("is", "exists");

		let expander = SynonymExpander::new()
			.with_dictionary(dict)
			.with_min_term_length(3);

		let expanded = expander.expand_query("is");
		// "is" is too short (< 3 chars), should not be expanded
		assert_eq!(expanded.len(), 1);
		assert_eq!(expanded[0], "is");
	}

	#[test]
	fn test_synonym_expander_expansion_limit() {
		let mut dict = SynonymDictionary::new();
		dict.add_synonyms(
			"big",
			vec!["large", "huge", "enormous", "massive", "gigantic"],
		);

		let expander = SynonymExpander::new()
			.with_dictionary(dict)
			.with_expansion_limit(2);

		let expanded = expander.expand_query("big");
		// Original + max 2 synonyms = 3 terms
		assert!(expanded.len() <= 3);
	}

	#[test]
	fn test_synonym_expander_disabled() {
		let expander = SynonymExpander::new().set_enabled(false);
		assert!(!expander.enabled);
	}

	#[tokio::test]
	async fn test_synonym_expander_no_search_terms() {
		let expander = SynonymExpander::new();

		let params = HashMap::new();
		let sql = "SELECT * FROM articles".to_string();
		let result = expander
			.filter_queryset(&params, sql.clone())
			.await
			.unwrap();

		assert_eq!(result, sql);
	}

	#[tokio::test]
	async fn test_synonym_expander_disabled_passthrough() {
		let expander = SynonymExpander::new().set_enabled(false);

		let mut params = HashMap::new();
		params.insert("q".to_string(), "fast".to_string());

		let sql = "SELECT * FROM articles".to_string();
		let result = expander
			.filter_queryset(&params, sql.clone())
			.await
			.unwrap();

		assert_eq!(result, sql);
	}

	#[tokio::test]
	async fn test_synonym_expander_single_term_expansion() {
		let mut dict = SynonymDictionary::new();
		dict.add_synonym("fast", "quick");
		dict.add_synonym("fast", "rapid");

		let expander = SynonymExpander::new().with_dictionary(dict);

		let mut params = HashMap::new();
		params.insert("q".to_string(), "fast".to_string());

		let sql = "SELECT * FROM articles".to_string();
		let result = expander.filter_queryset(&params, sql).await.unwrap();

		// Should contain WHERE clause with OR conditions
		// Extract WHERE clause for validation
		let where_start = result.find("WHERE").expect("WHERE clause not found");
		let where_clause = &result[where_start..];

		// Validate structure: WHERE clause starts correctly
		// reinhardt-query may or may not wrap conditions in parentheses depending on the number of conditions
		assert!(
			where_clause.starts_with("WHERE "),
			"Expected WHERE clause to start with 'WHERE ', got: {}",
			where_clause
		);

		// Validate all terms are present in LIKE conditions
		assert!(
			where_clause.contains("content LIKE '%fast%'"),
			"Expected 'content LIKE '%fast%' in WHERE clause, got: {}",
			where_clause
		);
		assert!(
			where_clause.contains("content LIKE '%quick%'"),
			"Expected 'content LIKE '%quick%' in WHERE clause, got: {}",
			where_clause
		);
		assert!(
			where_clause.contains("content LIKE '%rapid%'"),
			"Expected 'content LIKE '%rapid%' in WHERE clause, got: {}",
			where_clause
		);

		// Validate OR connectors
		let or_count = where_clause.matches(" OR ").count();
		assert_eq!(
			or_count, 2,
			"Expected 2 OR connectors for 3 terms, got: {}",
			or_count
		);
	}

	#[tokio::test]
	async fn test_synonym_expander_multi_term_expansion() {
		let mut dict = SynonymDictionary::new();
		dict.add_synonym("fast", "quick");
		dict.add_synonym("car", "automobile");

		let expander = SynonymExpander::new().with_dictionary(dict);

		let mut params = HashMap::new();
		params.insert("q".to_string(), "fast car".to_string());

		let sql = "SELECT * FROM articles".to_string();
		let result = expander.filter_queryset(&params, sql).await.unwrap();

		// Should contain WHERE clause with all terms and synonyms
		let where_start = result.find("WHERE").expect("WHERE clause not found");
		let where_clause = &result[where_start..];

		// Validate all expanded terms are present in LIKE conditions
		let expected_terms = vec!["fast", "quick", "car", "automobile"];
		for term in expected_terms {
			assert!(
				where_clause.contains(&format!("content LIKE '%{}%'", term)),
				"Expected 'content LIKE '%{}%' in WHERE clause, got: {}",
				term,
				where_clause
			);
		}

		// Validate OR connectors (4 terms = 3 OR connectors)
		let or_count = where_clause.matches(" OR ").count();
		assert_eq!(
			or_count, 3,
			"Expected 3 OR connectors for 4 terms, got: {}",
			or_count
		);
	}

	#[tokio::test]
	async fn test_synonym_expander_existing_where_clause() {
		let mut dict = SynonymDictionary::new();
		dict.add_synonym("fast", "quick");

		let expander = SynonymExpander::new().with_dictionary(dict);

		let mut params = HashMap::new();
		params.insert("q".to_string(), "fast".to_string());

		let sql = "SELECT * FROM articles WHERE status = 'published'".to_string();
		let result = expander.filter_queryset(&params, sql).await.unwrap();

		// Should preserve existing WHERE clause and add synonym conditions with AND
		// Validate structure: WHERE conditions with synonym expansion
		// The WHERE clause may have parentheses around conditions
		assert!(
			result.contains("WHERE") && result.contains("content LIKE"),
			"Expected result to contain 'WHERE' and 'content LIKE', got: {}",
			result
		);

		// Validate synonym expansion conditions are present
		assert!(
			result.contains("content LIKE '%fast%'"),
			"Expected 'content LIKE '%fast%' in result, got: {}",
			result
		);
		assert!(
			result.contains("content LIKE '%quick%'"),
			"Expected 'content LIKE '%quick%' in result, got: {}",
			result
		);

		// Validate original WHERE condition is preserved
		assert!(
			result.contains("AND") && result.contains("status = 'published'"),
			"Expected 'AND status = 'published'' in result, got: {}",
			result
		);

		// Validate the order: synonym conditions come first, then AND, then original condition
		let synonym_pos = result.find("WHERE").unwrap();
		let and_pos = result.find("AND").unwrap();
		let status_pos = result.find("status = 'published'").unwrap();
		assert!(
			synonym_pos < and_pos && and_pos < status_pos,
			"Expected order: WHERE (synonyms) AND status, got: {}",
			result
		);
	}

	#[tokio::test]
	async fn test_synonym_expander_sql_injection_protection() {
		let mut dict = SynonymDictionary::new();
		dict.add_synonym("test", "test'; DROP TABLE articles; --");

		let expander = SynonymExpander::new().with_dictionary(dict);

		let mut params = HashMap::new();
		params.insert("q".to_string(), "test".to_string());

		let sql = "SELECT * FROM articles".to_string();
		let result = expander.filter_queryset(&params, sql).await.unwrap();

		// Single quotes should be escaped to prevent SQL injection
		// The malicious synonym should be escaped: test'; DROP TABLE -> test''; DROP TABLE
		let expected_escaped_synonym = "test''; drop table articles; --";
		assert!(
			result.contains(expected_escaped_synonym),
			"Expected escaped malicious synonym '{}' in result, got: {}",
			expected_escaped_synonym,
			result
		);

		// Ensure the original test term is present
		assert!(
			result.contains("content LIKE '%test%'"),
			"Expected original term 'content LIKE '%test%' in result, got: {}",
			result
		);

		// Validate the malicious content is in a LIKE clause (treated as literal string)
		assert!(
			result.contains(&format!("content LIKE '%{}%'", expected_escaped_synonym)),
			"Expected malicious content in LIKE clause 'content LIKE '%{}%', got: {}",
			expected_escaped_synonym,
			result
		);

		// Ensure dangerous SQL is treated as a string pattern, not as SQL command
		// The WHERE clause should contain both terms connected with OR
		let where_start = result.find("WHERE").expect("WHERE clause not found");
		let where_clause = &result[where_start..];
		let or_count = where_clause.matches(" OR ").count();
		assert_eq!(
			or_count, 1,
			"Expected 1 OR connector for 2 terms (test + malicious synonym), got: {}",
			or_count
		);
	}

	#[tokio::test]
	async fn test_synonym_expander_with_search_param() {
		let mut dict = SynonymDictionary::new();
		dict.add_synonym("fast", "quick");

		let expander = SynonymExpander::new().with_dictionary(dict);

		let mut params = HashMap::new();
		params.insert("search".to_string(), "fast".to_string());

		let sql = "SELECT * FROM articles".to_string();
		let result = expander.filter_queryset(&params, sql).await.unwrap();

		// Should work with "search" parameter as well
		let where_start = result.find("WHERE").expect("WHERE clause not found");
		let where_clause = &result[where_start..];

		// Validate both terms are present in the WHERE clause
		assert!(
			where_clause.contains("content LIKE '%fast%'"),
			"Expected 'content LIKE '%fast%' in WHERE clause, got: {}",
			where_clause
		);
		assert!(
			where_clause.contains("content LIKE '%quick%'"),
			"Expected 'content LIKE '%quick%' in WHERE clause, got: {}",
			where_clause
		);
	}

	#[tokio::test]
	async fn test_synonym_expander_with_query_param() {
		let mut dict = SynonymDictionary::new();
		dict.add_synonym("fast", "quick");

		let expander = SynonymExpander::new().with_dictionary(dict);

		let mut params = HashMap::new();
		params.insert("query".to_string(), "fast".to_string());

		let sql = "SELECT * FROM articles".to_string();
		let result = expander.filter_queryset(&params, sql).await.unwrap();

		// Should work with "query" parameter as well
		let where_start = result.find("WHERE").expect("WHERE clause not found");
		let where_clause = &result[where_start..];

		// Validate both terms are present in the WHERE clause
		assert!(
			where_clause.contains("content LIKE '%fast%'"),
			"Expected 'content LIKE '%fast%' in WHERE clause, got: {}",
			where_clause
		);
		assert!(
			where_clause.contains("content LIKE '%quick%'"),
			"Expected 'content LIKE '%quick%' in WHERE clause, got: {}",
			where_clause
		);
	}
}
