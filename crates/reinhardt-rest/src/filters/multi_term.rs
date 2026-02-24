//! Multi-term search with type-safe fields
//!
//! Provides utilities for building queries that search multiple terms across
//! multiple fields, combining them with AND/OR logic.

use super::searchable::SearchableModel;
use reinhardt_db::orm::{Field, Lookup, Model};

/// Combines multiple search terms across multiple fields
///
/// Search for posts containing "rust" AND "programming".
/// This creates: `(title ICONTAINS 'rust' OR content ICONTAINS 'rust') AND (title ICONTAINS 'programming' OR content ICONTAINS 'programming')`
///
/// # Examples
///
/// ```rust
/// # use reinhardt_rest::filters::{MultiTermSearch, SearchableModel};
/// # use reinhardt_db::orm::{Field, FieldSelector, Model};
/// #
/// # #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
/// # struct Post {
/// #     id: i64,
/// #     title: String,
/// #     content: String,
/// # }
/// #
/// # #[derive(Clone)]
/// # struct PostFields;
/// # impl FieldSelector for PostFields {
/// #     fn with_alias(self, _alias: &str) -> Self { self }
/// # }
/// #
/// # impl Model for Post {
/// #     type PrimaryKey = i64;
/// #     type Fields = PostFields;
/// #     fn table_name() -> &'static str { "posts" }
/// #     fn new_fields() -> Self::Fields { PostFields }
/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { Some(self.id) }
/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = value; }
/// # }
/// #
/// # impl SearchableModel for Post {
/// #     fn searchable_fields() -> Vec<Field<Self, String>> {
/// #         vec![
/// #             Field::<Post, String>::new(vec!["title"]),
/// #             Field::<Post, String>::new(vec!["content"]),
/// #         ]
/// #     }
/// # }
/// let terms = vec!["rust", "programming"];
/// let lookups = MultiTermSearch::search_terms::<Post>(terms);
/// assert_eq!(lookups.len(), 2); // Two terms
/// assert_eq!(lookups[0].len(), 2); // Two fields per term
/// ```
pub struct MultiTermSearch;

/// Search term representation with advanced features
#[derive(Debug, Clone, PartialEq)]
pub struct SearchTerm {
	pub value: String,
	pub term_type: TermType,
	pub field: Option<String>,
	pub operator: Operator,
}

/// Term type for different search patterns
#[derive(Debug, Clone, PartialEq)]
pub enum TermType {
	Word,
	Phrase,
	Wildcard,
	FieldValue,
}

/// Boolean operators for combining terms
#[derive(Debug, Clone, PartialEq)]
pub enum Operator {
	And,
	Or,
	Not,
}

impl MultiTermSearch {
	/// Create lookups for searching multiple terms across searchable fields
	///
	/// For each term, creates an OR clause across all searchable fields,
	/// then combines all terms with AND.
	pub fn search_terms<M: SearchableModel>(terms: Vec<&str>) -> Vec<Vec<Lookup<M>>> {
		let fields = M::searchable_fields();

		terms
			.into_iter()
			.map(|term| {
				// For each term, create lookups for all searchable fields
				fields
					.iter()
					.map(|field| {
						// Create a new field with same path and create an icontains lookup
						let new_field = Field::<M, String>::new(field.path().to_vec());
						new_field.icontains(term)
					})
					.collect()
			})
			.collect()
	}

	/// Create lookups for exact match search across multiple terms
	pub fn exact_terms<M: SearchableModel>(terms: Vec<&str>) -> Vec<Vec<Lookup<M>>> {
		let fields = M::searchable_fields();

		terms
			.into_iter()
			.map(|term| {
				fields
					.iter()
					.map(|field| {
						let new_field = Field::<M, String>::new(field.path().to_vec());
						new_field.iexact(term.to_string())
					})
					.collect()
			})
			.collect()
	}

	/// Create lookups for prefix search (startswith) across multiple terms
	pub fn prefix_terms<M: SearchableModel>(terms: Vec<&str>) -> Vec<Vec<Lookup<M>>> {
		let fields = M::searchable_fields();

		terms
			.into_iter()
			.map(|term| {
				fields
					.iter()
					.map(|field| {
						let new_field = Field::<M, String>::new(field.path().to_vec());
						new_field.startswith(term)
					})
					.collect()
			})
			.collect()
	}

	/// Parse a comma-separated search string into individual terms
	///
	/// Handles quoted strings properly, keeping quoted content together.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::multi_term::MultiTermSearch;
	///
	/// let terms = MultiTermSearch::parse_search_terms("rust, programming");
	/// assert_eq!(terms, vec!["rust", "programming"]);
	///
	/// let terms = MultiTermSearch::parse_search_terms("\"hello world\", rust");
	/// assert_eq!(terms, vec!["hello world", "rust"]);
	/// ```
	pub fn parse_search_terms(search: &str) -> Vec<String> {
		let mut terms = Vec::new();
		let mut current_term = String::new();
		let mut in_quotes = false;
		let chars = search.chars().peekable();

		for c in chars {
			match c {
				'"' => {
					in_quotes = !in_quotes;
				}
				',' if !in_quotes => {
					let trimmed = current_term.trim().to_string();
					if !trimmed.is_empty() {
						terms.push(trimmed);
					}
					current_term.clear();
				}
				_ => {
					current_term.push(c);
				}
			}
		}

		// Don't forget the last term
		let trimmed = current_term.trim().to_string();
		if !trimmed.is_empty() {
			terms.push(trimmed);
		}

		terms
	}

	/// Compile multi-term lookups into SQL WHERE clause
	///
	/// Terms are combined with AND, fields within each term are combined with OR.
	///
	/// # Examples
	///
	/// For terms ["rust", "web"] across fields [title, content]:
	/// ```sql
	/// ((title ILIKE '%rust%' OR content ILIKE '%rust%')
	///  AND
	///  (title ILIKE '%web%' OR content ILIKE '%web%'))
	/// ```
	pub fn compile_to_sql<M: Model>(term_lookups: Vec<Vec<Lookup<M>>>) -> Option<String> {
		if term_lookups.is_empty() {
			return None;
		}

		use reinhardt_db::orm::QueryFieldCompiler;

		let term_clauses: Vec<String> = term_lookups
			.into_iter()
			.filter(|lookups| !lookups.is_empty())
			.map(|lookups| {
				// Each term: OR across all fields
				let field_conditions: Vec<String> = lookups
					.iter()
					.map(|lookup| QueryFieldCompiler::compile(lookup))
					.collect();

				if field_conditions.len() == 1 {
					field_conditions[0].clone()
				} else {
					format!("({})", field_conditions.join(" OR "))
				}
			})
			.collect();

		if term_clauses.is_empty() {
			return None;
		}

		if term_clauses.len() == 1 {
			Some(term_clauses[0].clone())
		} else {
			// All terms: AND together
			Some(format!("({})", term_clauses.join(" AND ")))
		}
	}

	/// Parse search query into structured terms with advanced operators
	///
	/// Supports:
	/// - Quoted phrases (`"exact phrase"`)
	/// - Boolean operators (`AND`, `OR`, `NOT`)
	/// - Field-specific search (`field:value`)
	/// - Wildcard search (`term*`)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::multi_term::MultiTermSearch;
	///
	/// let query = r#"title:"machine learning" AND author:Smith"#;
	/// let terms = MultiTermSearch::parse_query(query);
	/// assert_eq!(terms.len(), 2);
	/// ```
	pub fn parse_query(query: &str) -> Vec<SearchTerm> {
		let mut terms = Vec::new();
		let mut current_term = String::new();
		let mut in_quotes = false;
		let mut chars = query.chars().peekable();
		let mut current_operator = Operator::And;

		while let Some(ch) = chars.next() {
			match ch {
				'"' => {
					in_quotes = !in_quotes;
					if !in_quotes && !current_term.is_empty() {
						// End of quoted phrase
						terms.push(SearchTerm {
							value: current_term.clone(),
							term_type: TermType::Phrase,
							field: None,
							operator: current_operator.clone(),
						});
						current_term.clear();
					}
				}
				' ' if !in_quotes => {
					if !current_term.is_empty() {
						// Check for operators
						match current_term.to_uppercase().as_str() {
							"AND" => {
								current_operator = Operator::And;
							}
							"OR" => {
								current_operator = Operator::Or;
							}
							"NOT" => {
								current_operator = Operator::Not;
							}
							_ => {
								terms.push(Self::parse_single_term(
									&current_term,
									current_operator.clone(),
								));
							}
						}
						current_term.clear();
					}
				}
				':' if !in_quotes => {
					// Field-specific search
					let field = current_term.clone();
					current_term.clear();

					// Check if value starts with quote
					let mut field_in_quotes = false;
					if let Some(&'"') = chars.peek() {
						chars.next(); // Consume opening quote
						field_in_quotes = true;
					}

					// Get the value after ':'
					for next_ch in chars.by_ref() {
						if field_in_quotes {
							if next_ch == '"' {
								// End of quoted value
								break;
							} else {
								current_term.push(next_ch);
							}
						} else if next_ch == ' ' {
							// End of unquoted value
							break;
						} else {
							current_term.push(next_ch);
						}
					}

					terms.push(SearchTerm {
						value: current_term.clone(),
						term_type: TermType::FieldValue,
						field: Some(field),
						operator: current_operator.clone(),
					});
					current_term.clear();
				}
				_ => {
					current_term.push(ch);
				}
			}
		}

		// Handle remaining term
		if !current_term.is_empty() {
			if in_quotes {
				terms.push(SearchTerm {
					value: current_term,
					term_type: TermType::Phrase,
					field: None,
					operator: current_operator,
				});
			} else {
				match current_term.to_uppercase().as_str() {
					"AND" | "OR" | "NOT" => {
						// Skip trailing operators
					}
					_ => {
						terms.push(Self::parse_single_term(&current_term, current_operator));
					}
				}
			}
		}

		terms
	}

	/// Parse single term with wildcard detection
	fn parse_single_term(term: &str, operator: Operator) -> SearchTerm {
		let term_type = if term.ends_with('*') {
			TermType::Wildcard
		} else {
			TermType::Word
		};

		SearchTerm {
			value: term.to_string(),
			term_type,
			field: None,
			operator,
		}
	}
}
