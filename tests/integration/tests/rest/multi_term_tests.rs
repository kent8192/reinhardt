//! Multi-term search tests
//!
//! Tests for `MultiTermSearch` and related types from reinhardt-rest.

use reinhardt_db::orm::{Field, Lookup};
use reinhardt_rest::filters::multi_term::MultiTermSearch;
use reinhardt_rest::filters::{Operator, SearchableModel, TermType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestPost {
	id: Option<i64>,
	title: String,
	content: String,
}

reinhardt_test::impl_test_model!(TestPost, i64, "test_posts");

impl SearchableModel for TestPost {
	fn searchable_fields() -> Vec<Field<Self, String>> {
		vec![
			Field::<TestPost, String>::new(vec!["title"]),
			Field::<TestPost, String>::new(vec!["content"]),
		]
	}
}

#[test]
fn test_search_single_term() {
	let terms = vec!["rust"];
	let lookups = MultiTermSearch::search_terms::<TestPost>(terms);

	assert_eq!(lookups.len(), 1); // One term
	assert_eq!(lookups[0].len(), 2); // Two fields (title, content)
}

#[test]
fn test_search_multiple_terms() {
	let terms = vec!["rust", "programming"];
	let lookups = MultiTermSearch::search_terms::<TestPost>(terms);

	assert_eq!(lookups.len(), 2); // Two terms
	assert_eq!(lookups[0].len(), 2); // Each term searches 2 fields
	assert_eq!(lookups[1].len(), 2);
}

#[test]
fn test_exact_terms() {
	let terms = vec!["rust"];
	let lookups = MultiTermSearch::exact_terms::<TestPost>(terms);

	assert_eq!(lookups.len(), 1);
	assert_eq!(lookups[0].len(), 2);
}

#[test]
fn test_prefix_terms() {
	let terms = vec!["rust"];
	let lookups = MultiTermSearch::prefix_terms::<TestPost>(terms);

	assert_eq!(lookups.len(), 1);
	assert_eq!(lookups[0].len(), 2);
}

#[test]
fn test_compile_single_term_to_sql() {
	let terms = vec!["rust"];
	let lookups = MultiTermSearch::search_terms::<TestPost>(terms);
	let sql = MultiTermSearch::compile_to_sql(lookups).unwrap();

	assert!(sql.contains("title"));
	assert!(sql.contains("content"));
	assert!(sql.contains("OR"));
	// SQLite uses LIKE with LOWER() for case-insensitive
	assert!(sql.contains("LIKE"));
	assert!(sql.contains("LOWER"));
}

#[test]
fn test_compile_multiple_terms_to_sql() {
	let terms = vec!["rust", "web"];
	let lookups = MultiTermSearch::search_terms::<TestPost>(terms);
	let sql = MultiTermSearch::compile_to_sql(lookups).unwrap();

	assert!(sql.contains("title"));
	assert!(sql.contains("content"));
	assert!(sql.contains("OR"));
	assert!(sql.contains("AND"));
	// SQLite uses LIKE with LOWER() for case-insensitive
	assert!(sql.contains("LIKE"));
	assert!(sql.contains("LOWER"));
}

#[test]
fn test_compile_empty_terms() {
	let lookups: Vec<Vec<Lookup<TestPost>>> = vec![];
	let sql = MultiTermSearch::compile_to_sql(lookups);

	assert!(sql.is_none());
}

#[test]
fn test_parse_simple_terms() {
	let input = "rust, programming";
	let terms = MultiTermSearch::parse_search_terms(input);

	assert_eq!(terms.len(), 2);
}

#[test]
fn test_parse_quoted_terms() {
	let input = "\"hello world\", rust";
	let terms = MultiTermSearch::parse_search_terms(input);

	assert_eq!(terms.len(), 2);
	assert_eq!(terms[0], "hello world");
	assert_eq!(terms[1], "rust");
}

#[test]
fn test_parse_empty_string() {
	let input = "";
	let terms = MultiTermSearch::parse_search_terms(input);

	assert_eq!(terms.len(), 0);
}

#[test]
fn test_parse_query_simple_terms() {
	let query = "hello world";
	let terms = MultiTermSearch::parse_query(query);

	assert_eq!(terms.len(), 2);
	assert_eq!(terms[0].value, "hello");
	assert_eq!(terms[0].term_type, TermType::Word);
	assert_eq!(terms[1].value, "world");
	assert_eq!(terms[1].term_type, TermType::Word);
}

#[test]
fn test_parse_query_quoted_phrase() {
	let query = r#""exact phrase""#;
	let terms = MultiTermSearch::parse_query(query);

	assert_eq!(terms.len(), 1);
	assert_eq!(terms[0].value, "exact phrase");
	assert_eq!(terms[0].term_type, TermType::Phrase);
}

#[test]
fn test_parse_query_field_search() {
	let query = "title:rust";
	let terms = MultiTermSearch::parse_query(query);

	assert_eq!(terms.len(), 1);
	assert_eq!(terms[0].value, "rust");
	assert_eq!(terms[0].field, Some("title".to_string()));
	assert_eq!(terms[0].term_type, TermType::FieldValue);
}

#[test]
fn test_parse_query_field_with_quoted_value() {
	let query = r#"title:"machine learning""#;
	let terms = MultiTermSearch::parse_query(query);

	assert_eq!(terms.len(), 1);
	assert_eq!(terms[0].value, "machine learning");
	assert_eq!(terms[0].field, Some("title".to_string()));
	assert_eq!(terms[0].term_type, TermType::FieldValue);
}

#[test]
fn test_parse_query_wildcard() {
	let query = "rust*";
	let terms = MultiTermSearch::parse_query(query);

	assert_eq!(terms.len(), 1);
	assert_eq!(terms[0].value, "rust*");
	assert_eq!(terms[0].term_type, TermType::Wildcard);
}

#[test]
fn test_parse_query_with_and_operator() {
	let query = "rust AND web";
	let terms = MultiTermSearch::parse_query(query);

	assert_eq!(terms.len(), 2);
	assert_eq!(terms[0].value, "rust");
	assert_eq!(terms[0].operator, Operator::And);
	assert_eq!(terms[1].value, "web");
	assert_eq!(terms[1].operator, Operator::And);
}

#[test]
fn test_parse_query_with_or_operator() {
	let query = "rust OR python";
	let terms = MultiTermSearch::parse_query(query);

	assert_eq!(terms.len(), 2);
	assert_eq!(terms[0].value, "rust");
	assert_eq!(terms[1].value, "python");
	assert_eq!(terms[1].operator, Operator::Or);
}

#[test]
fn test_parse_query_complex() {
	let query = r#"title:"machine learning" AND author:Smith OR year:2024"#;
	let terms = MultiTermSearch::parse_query(query);

	assert_eq!(terms.len(), 3);

	// First term: title:"machine learning"
	assert_eq!(terms[0].value, "machine learning");
	assert_eq!(terms[0].field, Some("title".to_string()));
	assert_eq!(terms[0].term_type, TermType::FieldValue);

	// Second term: author:Smith (with AND)
	assert_eq!(terms[1].value, "Smith");
	assert_eq!(terms[1].field, Some("author".to_string()));
	assert_eq!(terms[1].operator, Operator::And);

	// Third term: year:2024 (with OR)
	assert_eq!(terms[2].value, "2024");
	assert_eq!(terms[2].field, Some("year".to_string()));
	assert_eq!(terms[2].operator, Operator::Or);
}

#[test]
fn test_parse_query_multiple_phrases() {
	let query = r#""hello world" AND "rust programming""#;
	let terms = MultiTermSearch::parse_query(query);

	assert_eq!(terms.len(), 2);
	assert_eq!(terms[0].value, "hello world");
	assert_eq!(terms[0].term_type, TermType::Phrase);
	assert_eq!(terms[1].value, "rust programming");
	assert_eq!(terms[1].term_type, TermType::Phrase);
	assert_eq!(terms[1].operator, Operator::And);
}
