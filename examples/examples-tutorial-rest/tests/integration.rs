//! Integration tests for snippets application

#[cfg(with_reinhardt)]
mod tests {
	use rstest::*;
	use sqlx::SqlitePool;
	use std::sync::Arc;
	use tempfile::NamedTempFile;

	#[fixture]
	async fn sqlite_with_migrations() -> (NamedTempFile, Arc<SqlitePool>) {
		// Create temp file
		let temp_file = NamedTempFile::new().expect("Failed to create temp file");
		let db_path = temp_file.path().to_str().unwrap().to_string();
		let database_url = format!("sqlite://{}?mode=rwc", db_path);

		// Connect to SQLite
		let pool = SqlitePool::connect(&database_url)
			.await
			.expect("Failed to connect to SQLite");
		let pool = Arc::new(pool);

		// Manual table creation (SQLite)
		let create_snippets_table = r#"
			CREATE TABLE IF NOT EXISTS snippets (
				id INTEGER PRIMARY KEY AUTOINCREMENT,
				title VARCHAR(255) NOT NULL,
				code TEXT NOT NULL,
				language VARCHAR(50) NOT NULL,
				created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
			)
		"#;

		sqlx::query(create_snippets_table)
			.execute(pool.as_ref())
			.await
			.expect("Failed to create snippets table");

		(temp_file, pool)
	}

	// ============================================================================
	// Unit Tests (2 tests)
	// ============================================================================

	#[rstest]
	fn test_snippet_model() {
		use chrono::Utc;
		use examples_tutorial_rest::apps::snippets::models::Snippet;

		let snippet = Snippet {
			id: 0,
			title: "Hello World".to_string(),
			code: "println!(\"Hello, world!\");".to_string(),
			language: "rust".to_string(),
			created_at: Utc::now(),
		};

		assert_eq!(snippet.title, "Hello World");
		assert_eq!(snippet.code, "println!(\"Hello, world!\");");
		assert_eq!(snippet.language, "rust");
		assert_eq!(snippet.id, 0);
	}

	#[rstest]
	fn test_snippet_serializer_validation() {
		use examples_tutorial_rest::apps::snippets::serializers::SnippetSerializer;
		use validator::Validate;

		// Valid snippet
		let valid = SnippetSerializer {
			title: "Valid".to_string(),
			code: "fn main() {}".to_string(),
			language: "rust".to_string(),
		};
		assert!(valid.validate().is_ok());

		// Invalid: empty title
		let invalid_title = SnippetSerializer {
			title: "".to_string(),
			code: "fn main() {}".to_string(),
			language: "rust".to_string(),
		};
		assert!(invalid_title.validate().is_err());

		// Invalid: empty code
		let invalid_code = SnippetSerializer {
			title: "Valid".to_string(),
			code: "".to_string(),
			language: "rust".to_string(),
		};
		assert!(invalid_code.validate().is_err());
	}

	// ============================================================================
	// Database Integration Tests - CRUD Operations (4 tests)
	// ============================================================================

	#[rstest]
	#[tokio::test]
	async fn test_snippet_create(
		#[future] sqlite_with_migrations: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_migrations.await;

		// Create a snippet
		let result: (i64, String, String, String) = sqlx::query_as(
			r#"
			INSERT INTO snippets (title, code, language)
			VALUES ($1, $2, $3)
			RETURNING id, title, code, language
			"#,
		)
		.bind("Test Snippet")
		.bind("fn main() {}")
		.bind("rust")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to create snippet");

		assert_eq!(result.1, "Test Snippet");
		assert_eq!(result.2, "fn main() {}");
		assert_eq!(result.3, "rust");
	}

	#[rstest]
	#[tokio::test]
	async fn test_snippet_read(#[future] sqlite_with_migrations: (NamedTempFile, Arc<SqlitePool>)) {
		let (_file, pool) = sqlite_with_migrations.await;

		// Create a snippet
		let created: (i64,) = sqlx::query_as(
			r#"
			INSERT INTO snippets (title, code, language)
			VALUES ($1, $2, $3)
			RETURNING id
			"#,
		)
		.bind("Read Test")
		.bind("println!(\"Hello\");")
		.bind("rust")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to create snippet");

		// Read the snippet
		let result: (i64, String, String, String) = sqlx::query_as(
			r#"
			SELECT id, title, code, language
			FROM snippets
			WHERE id = $1
			"#,
		)
		.bind(created.0)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to read snippet");

		assert_eq!(result.0, created.0);
		assert_eq!(result.1, "Read Test");
		assert_eq!(result.2, "println!(\"Hello\");");
		assert_eq!(result.3, "rust");
	}

	#[rstest]
	#[tokio::test]
	async fn test_snippet_update(
		#[future] sqlite_with_migrations: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_migrations.await;

		// Create a snippet
		let created: (i64,) = sqlx::query_as(
			r#"
			INSERT INTO snippets (title, code, language)
			VALUES ($1, $2, $3)
			RETURNING id
			"#,
		)
		.bind("Original Title")
		.bind("original code")
		.bind("python")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to create snippet");

		// Update the snippet
		let updated: (i64, String, String, String) = sqlx::query_as(
			r#"
			UPDATE snippets
			SET title = $1, code = $2, language = $3
			WHERE id = $4
			RETURNING id, title, code, language
			"#,
		)
		.bind("Updated Title")
		.bind("updated code")
		.bind("javascript")
		.bind(created.0)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to update snippet");

		assert_eq!(updated.0, created.0);
		assert_eq!(updated.1, "Updated Title");
		assert_eq!(updated.2, "updated code");
		assert_eq!(updated.3, "javascript");
	}

	#[rstest]
	#[tokio::test]
	async fn test_snippet_delete(
		#[future] sqlite_with_migrations: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_migrations.await;

		// Create a snippet
		let created: (i64,) = sqlx::query_as(
			r#"
			INSERT INTO snippets (title, code, language)
			VALUES ($1, $2, $3)
			RETURNING id
			"#,
		)
		.bind("To Delete")
		.bind("delete me")
		.bind("rust")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to create snippet");

		// Delete the snippet
		let deleted_rows = sqlx::query(
			r#"
			DELETE FROM snippets
			WHERE id = $1
			"#,
		)
		.bind(created.0)
		.execute(pool.as_ref())
		.await
		.expect("Failed to delete snippet")
		.rows_affected();

		assert_eq!(deleted_rows, 1);

		// Verify deletion
		let result: Option<(i64,)> = sqlx::query_as(
			r#"
			SELECT id FROM snippets WHERE id = $1
			"#,
		)
		.bind(created.0)
		.fetch_optional(pool.as_ref())
		.await
		.expect("Failed to verify deletion");

		assert!(result.is_none());
	}

	// ============================================================================
	// Database Integration Tests - Query Operations (4 tests)
	// ============================================================================

	#[rstest]
	#[tokio::test]
	async fn test_snippet_list_all(
		#[future] sqlite_with_migrations: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_migrations.await;

		// Create multiple snippets
		sqlx::query(
			r#"
			INSERT INTO snippets (title, code, language)
			VALUES
				($1, $2, $3),
				($4, $5, $6),
				($7, $8, $9)
			"#,
		)
		.bind("Snippet 1")
		.bind("code 1")
		.bind("rust")
		.bind("Snippet 2")
		.bind("code 2")
		.bind("python")
		.bind("Snippet 3")
		.bind("code 3")
		.bind("javascript")
		.execute(pool.as_ref())
		.await
		.expect("Failed to create snippets");

		// List all snippets
		let snippets: Vec<(i64, String, String, String)> = sqlx::query_as(
			r#"
			SELECT id, title, code, language
			FROM snippets
			ORDER BY id
			"#,
		)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to list snippets");

		assert_eq!(snippets.len(), 3);
		assert_eq!(snippets[0].1, "Snippet 1");
		assert_eq!(snippets[1].1, "Snippet 2");
		assert_eq!(snippets[2].1, "Snippet 3");
	}

	#[rstest]
	#[tokio::test]
	async fn test_snippet_filter_by_language(
		#[future] sqlite_with_migrations: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_migrations.await;

		// Create snippets with different languages
		sqlx::query(
			r#"
			INSERT INTO snippets (title, code, language)
			VALUES
				($1, $2, $3),
				($4, $5, $6),
				($7, $8, $9)
			"#,
		)
		.bind("Rust Snippet")
		.bind("fn main() {}")
		.bind("rust")
		.bind("Python Snippet")
		.bind("print('hello')")
		.bind("python")
		.bind("Another Rust")
		.bind("let x = 5;")
		.bind("rust")
		.execute(pool.as_ref())
		.await
		.expect("Failed to create snippets");

		// Filter by language
		let rust_snippets: Vec<(i64, String, String, String)> = sqlx::query_as(
			r#"
			SELECT id, title, code, language
			FROM snippets
			WHERE language = $1
			ORDER BY id
			"#,
		)
		.bind("rust")
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to filter snippets");

		assert_eq!(rust_snippets.len(), 2);
		assert_eq!(rust_snippets[0].1, "Rust Snippet");
		assert_eq!(rust_snippets[1].1, "Another Rust");
	}

	#[rstest]
	#[tokio::test]
	async fn test_snippet_search_by_title(
		#[future] sqlite_with_migrations: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_migrations.await;

		// Create snippets with searchable titles
		sqlx::query(
			r#"
			INSERT INTO snippets (title, code, language)
			VALUES
				($1, $2, $3),
				($4, $5, $6),
				($7, $8, $9)
			"#,
		)
		.bind("Hello World")
		.bind("println!(\"Hello\");")
		.bind("rust")
		.bind("Goodbye World")
		.bind("println!(\"Goodbye\");")
		.bind("rust")
		.bind("Unrelated")
		.bind("some code")
		.bind("python")
		.execute(pool.as_ref())
		.await
		.expect("Failed to create snippets");

		// Search by title pattern
		let results: Vec<(i64, String, String, String)> = sqlx::query_as(
			r#"
			SELECT id, title, code, language
			FROM snippets
			WHERE title LIKE $1
			ORDER BY id
			"#,
		)
		.bind("%World%")
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to search snippets");

		assert_eq!(results.len(), 2);
		assert_eq!(results[0].1, "Hello World");
		assert_eq!(results[1].1, "Goodbye World");
	}

	#[rstest]
	#[tokio::test]
	async fn test_snippet_pagination(
		#[future] sqlite_with_migrations: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_migrations.await;

		// Create 5 snippets
		for i in 1..=5 {
			sqlx::query(
				r#"
				INSERT INTO snippets (title, code, language)
				VALUES ($1, $2, $3)
				"#,
			)
			.bind(format!("Snippet {}", i))
			.bind(format!("code {}", i))
			.bind("rust")
			.execute(pool.as_ref())
			.await
			.expect("Failed to create snippet");
		}

		// First page (limit 2, offset 0)
		let page1: Vec<(i64, String, String, String)> = sqlx::query_as(
			r#"
			SELECT id, title, code, language
			FROM snippets
			ORDER BY id
			LIMIT $1 OFFSET $2
			"#,
		)
		.bind(2i64)
		.bind(0i64)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to fetch page 1");

		assert_eq!(page1.len(), 2);
		assert_eq!(page1[0].1, "Snippet 1");
		assert_eq!(page1[1].1, "Snippet 2");

		// Second page (limit 2, offset 2)
		let page2: Vec<(i64, String, String, String)> = sqlx::query_as(
			r#"
			SELECT id, title, code, language
			FROM snippets
			ORDER BY id
			LIMIT $1 OFFSET $2
			"#,
		)
		.bind(2i64)
		.bind(2i64)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to fetch page 2");

		assert_eq!(page2.len(), 2);
		assert_eq!(page2[0].1, "Snippet 3");
		assert_eq!(page2[1].1, "Snippet 4");
	}

	// ============================================================================
	// Database Integration Tests - Edge Cases (7 tests)
	// ============================================================================

	#[rstest]
	#[tokio::test]
	async fn test_snippet_empty_database(
		#[future] sqlite_with_migrations: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_migrations.await;

		// Query empty database
		let snippets: Vec<(i64, String, String, String)> = sqlx::query_as(
			r#"
			SELECT id, title, code, language
			FROM snippets
			"#,
		)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to query empty database");

		assert_eq!(snippets.len(), 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_snippet_nonexistent_id(
		#[future] sqlite_with_migrations: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_migrations.await;

		// Query with nonexistent ID
		let result: Option<(i64, String, String, String)> = sqlx::query_as(
			r#"
			SELECT id, title, code, language
			FROM snippets
			WHERE id = $1
			"#,
		)
		.bind(99999i64)
		.fetch_optional(pool.as_ref())
		.await
		.expect("Failed to query nonexistent ID");

		assert!(result.is_none());
	}

	#[rstest]
	#[tokio::test]
	async fn test_snippet_duplicate_title_allowed(
		#[future] sqlite_with_migrations: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_migrations.await;

		// Create first snippet
		sqlx::query(
			r#"
			INSERT INTO snippets (title, code, language)
			VALUES ($1, $2, $3)
			"#,
		)
		.bind("Duplicate Title")
		.bind("code 1")
		.bind("rust")
		.execute(pool.as_ref())
		.await
		.expect("Failed to create first snippet");

		// Create second snippet with same title (should succeed - no unique constraint)
		let result: Result<(i64,), _> = sqlx::query_as(
			r#"
			INSERT INTO snippets (title, code, language)
			VALUES ($1, $2, $3)
			RETURNING id
			"#,
		)
		.bind("Duplicate Title")
		.bind("code 2")
		.bind("python")
		.fetch_one(pool.as_ref())
		.await;

		assert!(result.is_ok());

		// Verify both exist
		let count: (i64,) = sqlx::query_as(
			r#"
			SELECT COUNT(*) as count
			FROM snippets
			WHERE title = $1
			"#,
		)
		.bind("Duplicate Title")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to count snippets");

		assert_eq!(count.0, 2);
	}

	#[rstest]
	#[tokio::test]
	async fn test_snippet_count(
		#[future] sqlite_with_migrations: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_migrations.await;

		// Create 3 snippets
		sqlx::query(
			r#"
			INSERT INTO snippets (title, code, language)
			VALUES
				($1, $2, $3),
				($4, $5, $6),
				($7, $8, $9)
			"#,
		)
		.bind("Snippet 1")
		.bind("code 1")
		.bind("rust")
		.bind("Snippet 2")
		.bind("code 2")
		.bind("python")
		.bind("Snippet 3")
		.bind("code 3")
		.bind("rust")
		.execute(pool.as_ref())
		.await
		.expect("Failed to create snippets");

		// Count all snippets
		let total: (i64,) = sqlx::query_as(
			r#"
			SELECT COUNT(*) as count
			FROM snippets
			"#,
		)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to count all snippets");

		assert_eq!(total.0, 3);

		// Count rust snippets
		let rust_count: (i64,) = sqlx::query_as(
			r#"
			SELECT COUNT(*) as count
			FROM snippets
			WHERE language = $1
			"#,
		)
		.bind("rust")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to count rust snippets");

		assert_eq!(rust_count.0, 2);
	}

	#[rstest]
	#[tokio::test]
	async fn test_snippet_order_by_title(
		#[future] sqlite_with_migrations: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_migrations.await;

		// Create snippets with different titles
		sqlx::query(
			r#"
			INSERT INTO snippets (title, code, language)
			VALUES
				($1, $2, $3),
				($4, $5, $6),
				($7, $8, $9)
			"#,
		)
		.bind("Charlie")
		.bind("code c")
		.bind("rust")
		.bind("Alice")
		.bind("code a")
		.bind("python")
		.bind("Bob")
		.bind("code b")
		.bind("javascript")
		.execute(pool.as_ref())
		.await
		.expect("Failed to create snippets");

		// Order by title ascending
		let results: Vec<(i64, String, String, String)> = sqlx::query_as(
			r#"
			SELECT id, title, code, language
			FROM snippets
			ORDER BY title ASC
			"#,
		)
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to order snippets");

		assert_eq!(results.len(), 3);
		assert_eq!(results[0].1, "Alice");
		assert_eq!(results[1].1, "Bob");
		assert_eq!(results[2].1, "Charlie");
	}

	#[rstest]
	#[tokio::test]
	async fn test_snippet_language_case_sensitivity(
		#[future] sqlite_with_migrations: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_migrations.await;

		// Create snippets with different case languages
		sqlx::query(
			r#"
			INSERT INTO snippets (title, code, language)
			VALUES
				($1, $2, $3),
				($4, $5, $6)
			"#,
		)
		.bind("Lowercase Rust")
		.bind("code 1")
		.bind("rust")
		.bind("Uppercase Rust")
		.bind("code 2")
		.bind("RUST")
		.execute(pool.as_ref())
		.await
		.expect("Failed to create snippets");

		// Exact match (case-sensitive)
		let exact: (i64,) = sqlx::query_as(
			r#"
			SELECT COUNT(*) as count
			FROM snippets
			WHERE language = $1
			"#,
		)
		.bind("rust")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to count exact match");

		assert_eq!(exact.0, 1);

		// Case-insensitive match
		let case_insensitive: (i64,) = sqlx::query_as(
			r#"
			SELECT COUNT(*) as count
			FROM snippets
			WHERE LOWER(language) = LOWER($1)
			"#,
		)
		.bind("rust")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to count case-insensitive match");

		assert_eq!(case_insensitive.0, 2);
	}

	#[rstest]
	#[tokio::test]
	async fn test_snippet_update_nonexistent(
		#[future] sqlite_with_migrations: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_migrations.await;

		// Try to update nonexistent snippet
		let updated_rows = sqlx::query(
			r#"
			UPDATE snippets
			SET title = $1
			WHERE id = $2
			"#,
		)
		.bind("New Title")
		.bind(99999i64)
		.execute(pool.as_ref())
		.await
		.expect("Failed to update nonexistent snippet")
		.rows_affected();

		assert_eq!(updated_rows, 0);
	}
}
