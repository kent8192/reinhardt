#![cfg(feature = "graphql")]

use async_graphql::*;
use reinhardt_server::graphql_handler;
use reinhardt_test::APIClient;
use reinhardt_test::server::{shutdown_test_server, spawn_test_server};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// GraphQL Schema for a simple book library

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Book {
	id: ID,
	title: String,
	author: String,
	year: i32,
}

#[Object]
impl Book {
	async fn id(&self) -> &ID {
		&self.id
	}

	async fn title(&self) -> &str {
		&self.title
	}

	async fn author(&self) -> &str {
		&self.author
	}

	async fn year(&self) -> i32 {
		self.year
	}
}

#[derive(Clone)]
struct BookStore {
	books: Arc<Mutex<HashMap<String, Book>>>,
	next_id: Arc<Mutex<u32>>,
}

impl BookStore {
	fn new() -> Self {
		let mut books = HashMap::new();
		books.insert(
			"1".to_string(),
			Book {
				id: ID::from("1"),
				title: "1984".to_string(),
				author: "George Orwell".to_string(),
				year: 1949,
			},
		);
		books.insert(
			"2".to_string(),
			Book {
				id: ID::from("2"),
				title: "To Kill a Mockingbird".to_string(),
				author: "Harper Lee".to_string(),
				year: 1960,
			},
		);

		Self {
			books: Arc::new(Mutex::new(books)),
			next_id: Arc::new(Mutex::new(3)),
		}
	}
}

struct QueryRoot {
	store: BookStore,
}

#[Object]
impl QueryRoot {
	async fn books(&self) -> Vec<Book> {
		let books = self.store.books.lock().unwrap();
		books.values().cloned().collect()
	}

	async fn book(&self, id: ID) -> Option<Book> {
		let books = self.store.books.lock().unwrap();
		books.get(id.as_str()).cloned()
	}

	async fn search_books(&self, title: String) -> Vec<Book> {
		let books = self.store.books.lock().unwrap();
		books
			.values()
			.filter(|book| book.title.to_lowercase().contains(&title.to_lowercase()))
			.cloned()
			.collect()
	}
}

struct MutationRoot {
	store: BookStore,
}

#[Object]
impl MutationRoot {
	async fn add_book(&self, title: String, author: String, year: i32) -> Book {
		let mut next_id = self.store.next_id.lock().unwrap();
		let id = ID::from(next_id.to_string());
		*next_id += 1;

		let book = Book {
			id: id.clone(),
			title,
			author,
			year,
		};

		let mut books = self.store.books.lock().unwrap();
		books.insert(id.to_string(), book.clone());

		book
	}

	async fn update_book(&self, id: ID, title: String, author: String, year: i32) -> Option<Book> {
		let mut books = self.store.books.lock().unwrap();
		if books.contains_key(id.as_str()) {
			let book = Book {
				id: id.clone(),
				title,
				author,
				year,
			};
			books.insert(id.to_string(), book.clone());
			Some(book)
		} else {
			None
		}
	}

	async fn delete_book(&self, id: ID) -> bool {
		let mut books = self.store.books.lock().unwrap();
		books.remove(id.as_str()).is_some()
	}
}

#[tokio::test]
async fn test_e2e_graphql_query_all_books() {
	let store = BookStore::new();
	let query = QueryRoot {
		store: store.clone(),
	};
	let mutation = MutationRoot { store };

	let handler = graphql_handler(query, mutation);
	let (url, handle) = spawn_test_server(handler).await;

	let client = APIClient::with_base_url(&url);
	let graphql_query = r#"{"query": "{ books { id title author year } }"}"#;

	let response = client
		.post_raw("/", graphql_query.as_bytes(), "application/json")
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200);
	let body = response.text();
	let json: Value = serde_json::from_str(&body).unwrap();
	let books = json["data"]["books"].as_array().unwrap();

	// Verify we have 2 books
	assert_eq!(books.len(), 2);

	// Verify book titles
	let titles: Vec<&str> = books.iter().map(|b| b["title"].as_str().unwrap()).collect();
	assert!(titles.contains(&"1984"));
	assert!(titles.contains(&"To Kill a Mockingbird"));

	shutdown_test_server(handle).await;
}

#[tokio::test]
async fn test_e2e_graphql_query_single_book() {
	let store = BookStore::new();
	let query = QueryRoot {
		store: store.clone(),
	};
	let mutation = MutationRoot { store };

	let handler = graphql_handler(query, mutation);
	let (url, handle) = spawn_test_server(handler).await;

	let client = APIClient::with_base_url(&url);
	let graphql_query = r#"{"query": "{ book(id: \"1\") { id title author year } }"}"#;

	let response = client
		.post_raw("/", graphql_query.as_bytes(), "application/json")
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200);
	let body = response.text();
	let json: Value = serde_json::from_str(&body).unwrap();
	let book = &json["data"]["book"];

	assert_eq!(book["id"].as_str().unwrap(), "1");
	assert_eq!(book["title"].as_str().unwrap(), "1984");
	assert_eq!(book["author"].as_str().unwrap(), "George Orwell");
	assert_eq!(book["year"].as_i64().unwrap(), 1949);

	shutdown_test_server(handle).await;
}

#[tokio::test]
async fn test_e2e_graphql_search_books() {
	let store = BookStore::new();
	let query = QueryRoot {
		store: store.clone(),
	};
	let mutation = MutationRoot { store };

	let handler = graphql_handler(query, mutation);
	let (url, handle) = spawn_test_server(handler).await;

	let client = APIClient::with_base_url(&url);
	let graphql_query =
		r#"{"query": "{ searchBooks(title: \"mockingbird\") { id title author } }"}"#;

	let response = client
		.post_raw("/", graphql_query.as_bytes(), "application/json")
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200);
	let body = response.text();
	let json: Value = serde_json::from_str(&body).unwrap();
	let books = json["data"]["searchBooks"].as_array().unwrap();

	assert_eq!(books.len(), 1);
	assert_eq!(books[0]["title"].as_str().unwrap(), "To Kill a Mockingbird");

	shutdown_test_server(handle).await;
}

#[tokio::test]
async fn test_e2e_graphql_add_book_mutation() {
	let store = BookStore::new();
	let query = QueryRoot {
		store: store.clone(),
	};
	let mutation = MutationRoot { store };

	let handler = graphql_handler(query, mutation);
	let (url, handle) = spawn_test_server(handler).await;

	let client = APIClient::with_base_url(&url);
	let mutation_query = r#"{"query": "mutation { addBook(title: \"The Great Gatsby\", author: \"F. Scott Fitzgerald\", year: 1925) { id title author year } }"}"#;

	let response = client
		.post_raw("/", mutation_query.as_bytes(), "application/json")
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200);
	let body = response.text();
	let json: Value = serde_json::from_str(&body).unwrap();
	let book = &json["data"]["addBook"];

	assert_eq!(book["title"].as_str().unwrap(), "The Great Gatsby");
	assert_eq!(book["author"].as_str().unwrap(), "F. Scott Fitzgerald");
	assert_eq!(book["year"].as_i64().unwrap(), 1925);

	shutdown_test_server(handle).await;
}

#[tokio::test]
async fn test_e2e_graphql_update_book_mutation() {
	let store = BookStore::new();
	let query = QueryRoot {
		store: store.clone(),
	};
	let mutation = MutationRoot { store };

	let handler = graphql_handler(query, mutation);
	let (url, handle) = spawn_test_server(handler).await;

	let client = APIClient::with_base_url(&url);
	let mutation_query = r#"{"query": "mutation { updateBook(id: \"1\", title: \"Nineteen Eighty-Four\", author: \"George Orwell\", year: 1949) { id title } }"}"#;

	let response = client
		.post_raw("/", mutation_query.as_bytes(), "application/json")
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200);
	let body = response.text();
	let json: Value = serde_json::from_str(&body).unwrap();
	let book = &json["data"]["updateBook"];

	assert_eq!(book["id"].as_str().unwrap(), "1");
	assert_eq!(book["title"].as_str().unwrap(), "Nineteen Eighty-Four");

	shutdown_test_server(handle).await;
}

#[tokio::test]
async fn test_e2e_graphql_delete_book_mutation() {
	let store = BookStore::new();
	let query = QueryRoot {
		store: store.clone(),
	};
	let mutation = MutationRoot { store };

	let handler = graphql_handler(query, mutation);
	let (url, handle) = spawn_test_server(handler).await;

	let client = APIClient::with_base_url(&url);

	// Delete a book
	let mutation_query = r#"{"query": "mutation { deleteBook(id: \"1\") }"}"#;
	let response = client
		.post_raw("/", mutation_query.as_bytes(), "application/json")
		.await
		.unwrap();
	assert_eq!(response.status_code(), 200);

	// Try to fetch the deleted book
	let graphql_query = r#"{"query": "{ book(id: \"1\") { id title } }"}"#;
	let response = client
		.post_raw("/", graphql_query.as_bytes(), "application/json")
		.await
		.unwrap();

	let body = response.text();
	let json: Value = serde_json::from_str(&body).unwrap();

	// Deleted book should return null
	assert!(json["data"]["book"].is_null());

	shutdown_test_server(handle).await;
}

#[tokio::test]
async fn test_e2e_graphql_full_workflow() {
	let store = BookStore::new();
	let query = QueryRoot {
		store: store.clone(),
	};
	let mutation = MutationRoot { store };

	let handler = graphql_handler(query, mutation);
	let (url, handle) = spawn_test_server(handler).await;

	let client = APIClient::with_base_url(&url);

	// 1. Query all books
	let graphql_query = r#"{"query": "{ books { id title } }"}"#;
	let response = client
		.post_raw("/", graphql_query.as_bytes(), "application/json")
		.await
		.unwrap();
	assert_eq!(response.status_code(), 200);

	// 2. Add a new book
	let add_query = r#"{"query": "mutation { addBook(title: \"The Catcher in the Rye\", author: \"J.D. Salinger\", year: 1951) { id title } }"}"#;
	let response = client
		.post_raw("/", add_query.as_bytes(), "application/json")
		.await
		.unwrap();
	assert_eq!(response.status_code(), 200);
	let body = response.text();
	let json: Value = serde_json::from_str(&body).unwrap();
	let book = &json["data"]["addBook"];

	assert_eq!(book["title"].as_str().unwrap(), "The Catcher in the Rye");

	// 3. Search for the new book
	let search_query = r#"{"query": "{ searchBooks(title: \"Catcher\") { id title } }"}"#;
	let response = client
		.post_raw("/", search_query.as_bytes(), "application/json")
		.await
		.unwrap();
	assert_eq!(response.status_code(), 200);
	let body = response.text();
	let json: Value = serde_json::from_str(&body).unwrap();
	let books = json["data"]["searchBooks"].as_array().unwrap();

	assert_eq!(books.len(), 1);
	assert_eq!(
		books[0]["title"].as_str().unwrap(),
		"The Catcher in the Rye"
	);

	shutdown_test_server(handle).await;
}

#[tokio::test]
async fn test_e2e_graphql_invalid_query() {
	let store = BookStore::new();
	let query = QueryRoot {
		store: store.clone(),
	};
	let mutation = MutationRoot { store };

	let handler = graphql_handler(query, mutation);
	let (url, handle) = spawn_test_server(handler).await;

	let client = APIClient::with_base_url(&url);
	let invalid_query = r#"{"query": "{ invalidField { id } }"}"#;

	let response = client
		.post_raw("/", invalid_query.as_bytes(), "application/json")
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200); // GraphQL returns 200 even for query errors
	let body = response.text();
	let json: Value = serde_json::from_str(&body).unwrap();

	// GraphQL errors should be in the "errors" field
	assert!(json.get("errors").is_some());

	shutdown_test_server(handle).await;
}
