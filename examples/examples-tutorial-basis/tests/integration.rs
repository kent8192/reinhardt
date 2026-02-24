//! Integration tests for polls application
//!
//! This file demonstrates two approaches to database testing:
//!
//! 1. **Manual SQLite Setup** (database_tests module):
//!    - Uses raw SQL with sqlx directly
//!    - Requires manual CREATE TABLE statements
//!    - More control, more boilerplate
//!
//! 2. **reinhardt-test Fixtures** (reinhardt_test_examples module):
//!    - Uses reinhardt-test shared fixtures
//!    - Automatic table creation from models
//!    - Less boilerplate, recommended for new tests
//!
//! For new tests, **prefer the reinhardt-test fixtures approach** (see reinhardt_test_examples module).

#[cfg(with_reinhardt)]
mod database_tests {
	use rstest::*;
	use sqlx::SqlitePool;
	use std::sync::Arc;
	use tempfile::NamedTempFile;

	/// Fixture: SQLite database with tables created
	#[fixture]
	async fn sqlite_with_polls_tables() -> (NamedTempFile, Arc<SqlitePool>) {
		// Create temp file
		let temp_file = NamedTempFile::new().expect("Failed to create temp file");
		let db_path = temp_file.path().to_str().unwrap().to_string();
		let database_url = format!("sqlite://{}?mode=rwc", db_path);

		// Connect to SQLite
		let pool = SqlitePool::connect(&database_url)
			.await
			.expect("Failed to connect to SQLite");
		let pool = Arc::new(pool);

		// polls_question table
		sqlx::query(
			r#"
			CREATE TABLE IF NOT EXISTS polls_question (
				id INTEGER PRIMARY KEY AUTOINCREMENT,
				question_text VARCHAR(200) NOT NULL,
				pub_date DATETIME NOT NULL
			)
			"#,
		)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create polls_question table");

		// polls_choice table
		sqlx::query(
			r#"
			CREATE TABLE IF NOT EXISTS polls_choice (
				id INTEGER PRIMARY KEY AUTOINCREMENT,
				question_id INTEGER NOT NULL,
				choice_text VARCHAR(200) NOT NULL,
				votes INTEGER NOT NULL DEFAULT 0
			)
			"#,
		)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create polls_choice table");

		(temp_file, pool)
	}

	// Database integration tests with manual table creation
	#[rstest]
	#[tokio::test]
	async fn test_question_database_create(
		#[future] sqlite_with_polls_tables: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_polls_tables.await;

		// Create a question
		let question_text = "What's your favorite color?";
		let row = sqlx::query_as::<_, (i64, String, chrono::NaiveDateTime)>(
			"INSERT INTO polls_question (question_text, pub_date) VALUES ($1, CURRENT_TIMESTAMP) RETURNING id, question_text, pub_date"
		)
		.bind(question_text)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to insert question");

		assert_eq!(row.1, question_text);
	}

	#[rstest]
	#[tokio::test]
	async fn test_question_database_read(
		#[future] sqlite_with_polls_tables: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_polls_tables.await;

		// Insert test data
		let question_text = "Test question for reading";
		let id: i64 = sqlx::query_scalar(
			"INSERT INTO polls_question (question_text, pub_date) VALUES ($1, CURRENT_TIMESTAMP) RETURNING id"
		)
		.bind(question_text)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to insert question");

		// Read the question back
		let retrieved_text: String =
			sqlx::query_scalar("SELECT question_text FROM polls_question WHERE id = $1")
				.bind(id)
				.fetch_one(pool.as_ref())
				.await
				.expect("Failed to read question");

		assert_eq!(retrieved_text, question_text);
	}

	#[rstest]
	#[tokio::test]
	async fn test_question_database_update(
		#[future] sqlite_with_polls_tables: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_polls_tables.await;

		// Insert initial data
		let original_text = "Original question text";
		let id: i64 = sqlx::query_scalar(
			"INSERT INTO polls_question (question_text, pub_date) VALUES ($1, CURRENT_TIMESTAMP) RETURNING id",
		)
		.bind(original_text)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to insert question");

		// Update the question
		let updated_text = "Updated question text";
		sqlx::query("UPDATE polls_question SET question_text = $1 WHERE id = $2")
			.bind(updated_text)
			.bind(id)
			.execute(pool.as_ref())
			.await
			.expect("Failed to update question");

		// Verify update
		let retrieved_text: String =
			sqlx::query_scalar("SELECT question_text FROM polls_question WHERE id = $1")
				.bind(id)
				.fetch_one(pool.as_ref())
				.await
				.expect("Failed to verify update");

		assert_eq!(retrieved_text, updated_text);
	}

	#[rstest]
	#[tokio::test]
	async fn test_question_database_delete(
		#[future] sqlite_with_polls_tables: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_polls_tables.await;

		// Insert test data
		let question_text = "Question to be deleted";
		let id: i64 = sqlx::query_scalar(
			"INSERT INTO polls_question (question_text, pub_date) VALUES ($1, CURRENT_TIMESTAMP) RETURNING id",
		)
		.bind(question_text)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to insert question");

		// Delete the question
		sqlx::query("DELETE FROM polls_question WHERE id = $1")
			.bind(id)
			.execute(pool.as_ref())
			.await
			.expect("Failed to delete question");

		// Verify deletion
		let deleted_id: Option<i64> =
			sqlx::query_scalar("SELECT id FROM polls_question WHERE id = $1")
				.bind(id)
				.fetch_optional(pool.as_ref())
				.await
				.expect("Failed to verify deletion");

		assert!(deleted_id.is_none());
	}

	// Test that migrations were applied successfully
	#[rstest]
	#[tokio::test]
	async fn test_migrations_applied_successfully(
		#[future] sqlite_with_polls_tables: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_polls_tables.await;

		// Verify table exists (expect true)
		// SQLite system table check
		let exists: bool = sqlx::query_scalar(
			"SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'polls_question')",
		)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to check table existence");

		assert!(exists, "polls_question table should exist after migrations");

		// Try simple insert
		let id: i64 = sqlx::query_scalar(
			"INSERT INTO polls_question (question_text, pub_date) VALUES ($1, CURRENT_TIMESTAMP) RETURNING id",
		)
		.bind("Test")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to insert test record");

		assert!(id > 0, "Inserted ID should be positive");
	}

	#[rstest]
	#[tokio::test]
	async fn test_choice_database_create(
		#[future] sqlite_with_polls_tables: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_polls_tables.await;

		// First create a question
		let question_id: i64 = sqlx::query_scalar(
			"INSERT INTO polls_question (question_text, pub_date) VALUES ($1, CURRENT_TIMESTAMP) RETURNING id",
		)
		.bind("Test question for choice")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to insert question");

		// Create a choice
		let choice_text = "Test choice";
		let (retrieved_text, votes): (String, i32) = sqlx::query_as(
			"INSERT INTO polls_choice (question_id, choice_text, votes) VALUES ($1, $2, $3) RETURNING choice_text, votes",
		)
		.bind(question_id)
		.bind(choice_text)
		.bind(0i32)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to insert choice");

		assert_eq!(retrieved_text, choice_text);
		assert_eq!(votes, 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_choice_database_read(
		#[future] sqlite_with_polls_tables: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_polls_tables.await;

		// Create question
		let question_id: i64 = sqlx::query_scalar(
			"INSERT INTO polls_question (question_text, pub_date) VALUES ($1, CURRENT_TIMESTAMP) RETURNING id",
		)
		.bind("Question for choice read test")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to insert question");

		// Insert choice
		let choice_text = "Choice to be read";
		let choice_id: i64 = sqlx::query_scalar(
			"INSERT INTO polls_choice (question_id, choice_text, votes) VALUES ($1, $2, $3) RETURNING id",
		)
		.bind(question_id)
		.bind(choice_text)
		.bind(0i32)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to insert choice");

		// Read the choice back
		let (retrieved_id, retrieved_question_id, retrieved_text, votes): (i64, i64, String, i32) =
			sqlx::query_as(
				"SELECT id, question_id, choice_text, votes FROM polls_choice WHERE id = $1",
			)
			.bind(choice_id)
			.fetch_one(pool.as_ref())
			.await
			.expect("Failed to read choice");

		assert_eq!(retrieved_id, choice_id);
		assert_eq!(retrieved_text, choice_text);
		assert_eq!(retrieved_question_id, question_id);
		assert_eq!(votes, 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_choice_database_update(
		#[future] sqlite_with_polls_tables: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_polls_tables.await;

		// Create question
		let question_id: i64 = sqlx::query_scalar(
			"INSERT INTO polls_question (question_text, pub_date) VALUES ($1, CURRENT_TIMESTAMP) RETURNING id",
		)
		.bind("Question for choice update test")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to insert question");

		// Insert choice
		let original_text = "Original choice text";
		let choice_id: i64 = sqlx::query_scalar(
			"INSERT INTO polls_choice (question_id, choice_text, votes) VALUES ($1, $2, $3) RETURNING id",
		)
		.bind(question_id)
		.bind(original_text)
		.bind(0i32)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to insert choice");

		// Update the choice
		let updated_text = "Updated choice text";
		sqlx::query("UPDATE polls_choice SET choice_text = $1 WHERE id = $2")
			.bind(updated_text)
			.bind(choice_id)
			.execute(pool.as_ref())
			.await
			.expect("Failed to update choice");

		// Verify update
		let retrieved_text: String =
			sqlx::query_scalar("SELECT choice_text FROM polls_choice WHERE id = $1")
				.bind(choice_id)
				.fetch_one(pool.as_ref())
				.await
				.expect("Failed to verify update");

		assert_eq!(retrieved_text, updated_text);
	}

	#[rstest]
	#[tokio::test]
	async fn test_choice_database_delete(
		#[future] sqlite_with_polls_tables: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_polls_tables.await;

		// Create question
		let question_id: i64 = sqlx::query_scalar(
			"INSERT INTO polls_question (question_text, pub_date) VALUES ($1, CURRENT_TIMESTAMP) RETURNING id",
		)
		.bind("Question for choice delete test")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to insert question");

		// Insert choice
		let choice_id: i64 = sqlx::query_scalar(
			"INSERT INTO polls_choice (question_id, choice_text, votes) VALUES ($1, $2, $3) RETURNING id",
		)
		.bind(question_id)
		.bind("Choice to be deleted")
		.bind(0i32)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to insert choice");

		// Delete the choice
		let delete_result = sqlx::query("DELETE FROM polls_choice WHERE id = $1")
			.bind(choice_id)
			.execute(pool.as_ref())
			.await;

		assert!(delete_result.is_ok());

		// Verify deletion
		let verify_result =
			sqlx::query_scalar::<_, i64>("SELECT id FROM polls_choice WHERE id = $1")
				.bind(choice_id)
				.fetch_optional(pool.as_ref())
				.await;

		assert!(verify_result.is_ok());
		assert!(verify_result.unwrap().is_none());
	}

	#[rstest]
	#[tokio::test]
	async fn test_choice_vote_increment(
		#[future] sqlite_with_polls_tables: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_polls_tables.await;

		// Create question
		let question_id: i64 = sqlx::query_scalar(
			"INSERT INTO polls_question (question_text, pub_date) VALUES ($1, CURRENT_TIMESTAMP) RETURNING id",
		)
		.bind("Question for vote test")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to insert question");

		// Insert choice with 0 votes
		let choice_id: i64 = sqlx::query_scalar(
			"INSERT INTO polls_choice (question_id, choice_text, votes) VALUES ($1, $2, $3) RETURNING id",
		)
		.bind(question_id)
		.bind("Choice to vote for")
		.bind(0i32)
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to insert choice");

		// Increment votes
		let update_result = sqlx::query("UPDATE polls_choice SET votes = votes + 1 WHERE id = $1")
			.bind(choice_id)
			.execute(pool.as_ref())
			.await;

		assert!(update_result.is_ok());

		// Verify vote count
		let votes: i32 = sqlx::query_scalar("SELECT votes FROM polls_choice WHERE id = $1")
			.bind(choice_id)
			.fetch_one(pool.as_ref())
			.await
			.expect("Failed to verify votes");

		assert_eq!(votes, 1);
	}

	#[rstest]
	#[tokio::test]
	async fn test_question_recent_pub_date(
		#[future] sqlite_with_polls_tables: (NamedTempFile, Arc<SqlitePool>),
	) {
		let (_file, pool) = sqlite_with_polls_tables.await;

		// Insert a recent question (published now)
		let recent_row = sqlx::query_as::<_, (i64, chrono::NaiveDateTime)>(
			"INSERT INTO polls_question (question_text, pub_date) VALUES ($1, CURRENT_TIMESTAMP) RETURNING id, pub_date",
		)
		.bind("Recent question")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to insert recent question");

		let recent_pub_date = recent_row.1;

		// Verify it's recent (within last minute)
		let now = chrono::Utc::now().naive_utc();
		let diff_seconds =
			(now.and_utc().timestamp() - recent_pub_date.and_utc().timestamp()).abs();
		assert!(diff_seconds < 60);

		// Insert an old question (published 2 days ago)
		let old_row = sqlx::query_as::<_, (i64, chrono::NaiveDateTime)>(
			"INSERT INTO polls_question (question_text, pub_date) VALUES ($1, datetime('now', '-2 days')) RETURNING id, pub_date",
		)
		.bind("Old question")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to insert old question");

		let old_pub_date = old_row.1;

		// Verify it's old (more than 1 day ago)
		let old_diff_seconds =
			(now.and_utc().timestamp() - old_pub_date.and_utc().timestamp()).abs();
		assert!(old_diff_seconds >= 86400); // 1 day in seconds
	}
}

// ============================================================================
// Server Function Tests
// ============================================================================

#[cfg(all(with_reinhardt, not(target_arch = "wasm32")))]
mod server_fn_tests {
	use reinhardt::DatabaseConnection;
	use reinhardt::db::orm::reinitialize_database;
	use rstest::*;
	use serial_test::serial;
	use sqlx::SqlitePool;
	use std::sync::Arc;
	use tempfile::NamedTempFile;

	// Import server functions
	use examples_tutorial_basis::server_fn::polls::{
		get_question_detail, get_question_results, get_questions, vote,
	};
	use examples_tutorial_basis::shared::types::VoteRequest;

	/// Fixture: SQLite database with tables, test data, and DatabaseConnection
	/// Also initializes the global ORM database connection for server functions.
	#[fixture]
	async fn sqlite_with_test_data() -> (NamedTempFile, Arc<SqlitePool>, DatabaseConnection) {
		// Create temp file
		let temp_file = NamedTempFile::new().expect("Failed to create temp file");
		let db_path = temp_file.path().to_str().unwrap().to_string();

		// URL for sqlx direct connection (with mode parameter for create-if-missing)
		let sqlx_url = format!("sqlite://{}?mode=rwc", db_path);

		// URL for reinhardt ORM (use sqlite:/// for absolute path, no query parameters)
		// reinhardt's connect_sqlite automatically sets create_if_missing(true)
		let orm_url = format!("sqlite:///{}", db_path);

		// Connect to SQLite using sqlx directly
		let pool = SqlitePool::connect(&sqlx_url)
			.await
			.expect("Failed to connect to SQLite");
		let pool = Arc::new(pool);

		// Create tables (using model table names: questions, choices)
		sqlx::query(
			r#"
			CREATE TABLE IF NOT EXISTS questions (
				id INTEGER PRIMARY KEY AUTOINCREMENT,
				question_text VARCHAR(200) NOT NULL,
				pub_date DATETIME NOT NULL
			)
			"#,
		)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create questions table");

		sqlx::query(
			r#"
			CREATE TABLE IF NOT EXISTS choices (
				id INTEGER PRIMARY KEY AUTOINCREMENT,
				question_id INTEGER NOT NULL,
				choice_text VARCHAR(200) NOT NULL,
				votes INTEGER NOT NULL DEFAULT 0
			)
			"#,
		)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create choices table");

		// Insert test data (use ISO 8601 format for chrono DateTime<Utc> compatibility)
		let question_id: i64 = sqlx::query_scalar(
			"INSERT INTO questions (question_text, pub_date) VALUES ($1, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')) RETURNING id",
		)
		.bind("What's your favorite color?")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to insert test question");

		sqlx::query("INSERT INTO choices (question_id, choice_text, votes) VALUES ($1, $2, $3)")
			.bind(question_id)
			.bind("Red")
			.bind(0i32)
			.execute(pool.as_ref())
			.await
			.expect("Failed to insert choice 1");

		sqlx::query("INSERT INTO choices (question_id, choice_text, votes) VALUES ($1, $2, $3)")
			.bind(question_id)
			.bind("Blue")
			.bind(0i32)
			.execute(pool.as_ref())
			.await
			.expect("Failed to insert choice 2");

		// Initialize the global ORM database for server functions
		// Server functions use Question::objects() which relies on global database
		reinitialize_database(&orm_url)
			.await
			.expect("Failed to initialize global database");

		// Create DatabaseConnection for server functions
		let db_conn = DatabaseConnection::connect_sqlite(&orm_url)
			.await
			.expect("Failed to create DatabaseConnection");

		(temp_file, pool, db_conn)
	}

	#[rstest]
	#[tokio::test]
	#[serial(server_fn_tests)]
	async fn test_get_questions_server_fn(
		#[future] sqlite_with_test_data: (NamedTempFile, Arc<SqlitePool>, DatabaseConnection),
	) {
		let (_file, _pool, db_conn) = sqlite_with_test_data.await;

		// Test: Get questions via server function (pass DatabaseConnection as argument)
		let result = get_questions(db_conn).await;
		let questions = result.expect("get_questions should succeed");
		assert_eq!(questions.len(), 1, "Should have 1 question");
		assert_eq!(questions[0].question_text, "What's your favorite color?");
	}

	#[rstest]
	#[tokio::test]
	#[serial(server_fn_tests)]
	async fn test_get_question_detail_server_fn(
		#[future] sqlite_with_test_data: (NamedTempFile, Arc<SqlitePool>, DatabaseConnection),
	) {
		let (_file, _pool, db_conn) = sqlite_with_test_data.await;

		// Test: Get question detail via server function
		let result = get_question_detail(1, db_conn).await;
		assert!(result.is_ok(), "get_question_detail should succeed");

		let (question, choices) = result.unwrap();
		assert_eq!(question.question_text, "What's your favorite color?");
		assert_eq!(choices.len(), 2, "Should have 2 choices");
		assert_eq!(choices[0].choice_text, "Red");
		assert_eq!(choices[1].choice_text, "Blue");
	}

	#[rstest]
	#[tokio::test]
	#[serial(server_fn_tests)]
	async fn test_get_question_detail_not_found(
		#[future] sqlite_with_test_data: (NamedTempFile, Arc<SqlitePool>, DatabaseConnection),
	) {
		let (_file, _pool, db_conn) = sqlite_with_test_data.await;

		// Test: Get non-existent question
		let result = get_question_detail(999, db_conn).await;
		assert!(
			result.is_err(),
			"get_question_detail should fail for non-existent question"
		);
	}

	#[rstest]
	#[tokio::test]
	#[serial(server_fn_tests)]
	async fn test_get_question_results_server_fn(
		#[future] sqlite_with_test_data: (NamedTempFile, Arc<SqlitePool>, DatabaseConnection),
	) {
		let (_file, _pool, db_conn) = sqlite_with_test_data.await;

		// Test: Get question results via server function
		let result = get_question_results(1, db_conn).await;
		assert!(result.is_ok(), "get_question_results should succeed");

		let (question, choices, total_votes) = result.unwrap();
		assert_eq!(question.question_text, "What's your favorite color?");
		assert_eq!(choices.len(), 2, "Should have 2 choices");
		assert_eq!(total_votes, 0, "Should have 0 total votes initially");
	}

	#[rstest]
	#[tokio::test]
	#[serial(server_fn_tests)]
	async fn test_vote_server_fn(
		#[future] sqlite_with_test_data: (NamedTempFile, Arc<SqlitePool>, DatabaseConnection),
	) {
		let (_file, _pool, db_conn) = sqlite_with_test_data.await;

		// Test: Vote for a choice
		let vote_request = VoteRequest {
			question_id: 1,
			choice_id: 1, // Vote for "Red"
		};

		let result = vote(vote_request, db_conn).await;
		let choice_info = result.expect("vote should succeed");
		assert_eq!(choice_info.votes, 1, "Choice should have 1 vote");

		// Note: Cannot verify total votes here since db_conn was consumed
		// In a real test, we'd use a fresh connection or clone the connection
	}

	#[rstest]
	#[tokio::test]
	#[serial(server_fn_tests)]
	async fn test_vote_wrong_question(
		#[future] sqlite_with_test_data: (NamedTempFile, Arc<SqlitePool>, DatabaseConnection),
	) {
		let (_file, _pool, db_conn) = sqlite_with_test_data.await;

		// Test: Vote with mismatched question_id and choice_id
		let vote_request = VoteRequest {
			question_id: 999, // Wrong question
			choice_id: 1,
		};

		let result = vote(vote_request, db_conn).await;
		assert!(
			result.is_err(),
			"vote should fail when choice doesn't belong to question"
		);
	}

	#[rstest]
	#[tokio::test]
	#[serial(server_fn_tests)]
	async fn test_vote_multiple_times() {
		// Create temp file
		let temp_file = NamedTempFile::new().expect("Failed to create temp file");
		let db_path = temp_file.path().to_str().unwrap().to_string();

		// URL for sqlx direct connection (with mode parameter for create-if-missing)
		let sqlx_url = format!("sqlite://{}?mode=rwc", db_path);

		// URL for reinhardt ORM (use sqlite:/// for absolute path, no query parameters)
		let orm_url = format!("sqlite:///{}", db_path);

		// Connect to SQLite using sqlx directly
		let pool = SqlitePool::connect(&sqlx_url)
			.await
			.expect("Failed to connect to SQLite");

		// Create tables (using model table names: questions, choices)
		sqlx::query(
			r#"
			CREATE TABLE IF NOT EXISTS questions (
				id INTEGER PRIMARY KEY AUTOINCREMENT,
				question_text VARCHAR(200) NOT NULL,
				pub_date DATETIME NOT NULL
			)
			"#,
		)
		.execute(&pool)
		.await
		.expect("Failed to create questions table");

		sqlx::query(
			r#"
			CREATE TABLE IF NOT EXISTS choices (
				id INTEGER PRIMARY KEY AUTOINCREMENT,
				question_id INTEGER NOT NULL,
				choice_text VARCHAR(200) NOT NULL,
				votes INTEGER NOT NULL DEFAULT 0
			)
			"#,
		)
		.execute(&pool)
		.await
		.expect("Failed to create choices table");

		// Insert test data (use ISO 8601 format for chrono DateTime<Utc> compatibility)
		let question_id: i64 = sqlx::query_scalar(
			"INSERT INTO questions (question_text, pub_date) VALUES ($1, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')) RETURNING id",
		)
		.bind("What's your favorite color?")
		.fetch_one(&pool)
		.await
		.expect("Failed to insert test question");

		sqlx::query("INSERT INTO choices (question_id, choice_text, votes) VALUES ($1, $2, $3)")
			.bind(question_id)
			.bind("Red")
			.bind(0i32)
			.execute(&pool)
			.await
			.expect("Failed to insert choice 1");

		sqlx::query("INSERT INTO choices (question_id, choice_text, votes) VALUES ($1, $2, $3)")
			.bind(question_id)
			.bind("Blue")
			.bind(0i32)
			.execute(&pool)
			.await
			.expect("Failed to insert choice 2");

		// Initialize the global ORM database for server functions
		reinitialize_database(&orm_url)
			.await
			.expect("Failed to initialize global database");

		// Test: Vote multiple times for the same choice (each vote needs fresh connection)
		let vote_request = VoteRequest {
			question_id: 1,
			choice_id: 2, // Vote for "Blue"
		};

		// First vote
		let db_conn1 = DatabaseConnection::connect_sqlite(&orm_url)
			.await
			.expect("Failed to create DatabaseConnection");
		vote(vote_request.clone(), db_conn1).await.unwrap();

		// Second vote
		let db_conn2 = DatabaseConnection::connect_sqlite(&orm_url)
			.await
			.expect("Failed to create DatabaseConnection");
		vote(vote_request.clone(), db_conn2).await.unwrap();

		// Third vote
		let db_conn3 = DatabaseConnection::connect_sqlite(&orm_url)
			.await
			.expect("Failed to create DatabaseConnection");
		vote(vote_request.clone(), db_conn3).await.unwrap();

		// Verify votes counted correctly
		let db_conn_check = DatabaseConnection::connect_sqlite(&orm_url)
			.await
			.expect("Failed to create DatabaseConnection");
		let results = get_question_results(1, db_conn_check).await.unwrap();
		let blue_choice = results.1.iter().find(|c| c.choice_text == "Blue").unwrap();
		assert_eq!(blue_choice.votes, 3, "Blue should have 3 votes");
		assert_eq!(results.2, 3, "Total votes should be 3");

		// Keep temp_file alive
		drop(temp_file);
	}
}
