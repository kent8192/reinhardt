+++
title = "Part 5: Testing"
weight = 50

[extra]
sidebar_weight = 50
+++

# Part 5: Testing

In this tutorial, we'll write automated tests using modern Rust testing tools: **rstest** for fixtures and **TestContainers** for database isolation.

## Why Testing Matters

Tests help you:

- **Save time**: Automated tests catch bugs faster than manual testing
- **Prevent bugs**: Tests illuminate unexpected behavior before production
- **Build confidence**: Well-tested code is easier to modify and extend
- **Enable collaboration**: Tests protect against accidental breakage by teammates

## Test Dependencies

Add testing dependencies to `Cargo.toml`:

```toml
[dev-dependencies]
rstest = { workspace = true }
reinhardt-test = { workspace = true }
testcontainers = { workspace = true }
tokio = { version = "1", features = ["full", "test-util"] }
```

## Writing Your First Test

Let's identify a bug in our `was_published_recently()` method. It returns `true` for questions whose `pub_date` is in the future, which is incorrect.

Create `polls/tests.rs`:

```rust
use super::models::Question;
use chrono::{Duration, Utc};
use rstest::*;

#[rstest]
fn test_was_published_recently_with_future_question() {
    // Create a question 30 days in the future
    let future_date = Utc::now() + Duration::days(30);
    let question = Question {
        id: Some(1),
        question_text: "Future question".to_string(),
        pub_date: future_date,
    };

    // Should return false for future questions
    assert_eq!(question.was_published_recently(), false);
}

#[rstest]
fn test_was_published_recently_with_old_question() {
    // Create a question 2 days ago
    let old_date = Utc::now() - Duration::days(2);
    let question = Question {
        id: Some(1),
        question_text: "Old question".to_string(),
        pub_date: old_date,
    };

    // Should return false for questions older than 1 day
    assert_eq!(question.was_published_recently(), false);
}

#[rstest]
fn test_was_published_recently_with_recent_question() {
    // Create a question from 23 hours ago
    let recent_date = Utc::now() - Duration::hours(23);
    let question = Question {
        id: Some(1),
        question_text: "Recent question".to_string(),
        pub_date: recent_date,
    };

    // Should return true for recent questions
    assert_eq!(question.was_published_recently(), true);
}
```

Run the tests:

```bash
cargo test --package polls
```

You'll see that the first test fails. The bug is already fixed in our implementation from Part 2:

```rust
impl Question {
    /// Check if this question was published recently (within the last day)
    pub fn was_published_recently(&self) -> bool {
        let now = Utc::now();
        let one_day_ago = now - chrono::Duration::days(1);
        // Fixed: Also check that pub_date is not in the future
        self.pub_date >= one_day_ago && self.pub_date <= now
    }
}
```

## Why TestContainers?

Reinhardt uses **TestContainers** for database testing to ensure test isolation
and reliability. TestContainers automatically manages Docker containers for your
tests.

**Benefits:**

1. **Isolation** - Each test gets a fresh database
   - No shared state between tests
   - Tests can run in parallel safely
   - No cleanup code needed

2. **Real Database** - Tests use actual PostgreSQL/MySQL, not mocks
   - Catches database-specific bugs (SQL syntax, transactions, indexes)
   - Tests behavior matches production exactly
   - No surprises when deploying to production

3. **CI/CD Friendly** - Works anywhere Docker is available
   - GitHub Actions, GitLab CI, local development
   - No manual database setup required
   - Consistent behavior across environments

4. **Automatic Cleanup** - Containers are destroyed after tests
   - No leftover data or processes
   - No manual cleanup scripts needed
   - Tests are self-contained

**How it works:**

```
Test starts → Docker container launches → Test runs → Container auto-destroyed
                     ↓
              Fresh PostgreSQL
              with migrations applied
```

**Prerequisites:**

- **Docker must be running** (Docker Desktop on Mac/Windows, or Docker Engine on Linux)
- No manual database setup needed - TestContainers handles everything

**Alternative (Not Recommended):**

```rust
// ❌ Shared database - leads to test failures
let conn = DatabaseConnection::connect("postgres://localhost/test_db").await?;
// Multiple tests compete for same data
// Tests fail randomly due to race conditions
```

**TestContainers Approach (Recommended):**

```rust
// ✅ Isolated database per test
#[rstest]
#[tokio::test]
async fn test_user(
    #[future] postgres_fixture: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>)
) {
    let (_container, conn) = postgres_fixture.await;
    // Each test gets its own PostgreSQL instance!
}
```

For more details on testing infrastructure, see the project's testing standards documentation.

## Testing with Database using rstest + TestContainers

Let's test database operations using rstest fixtures and TestContainers for isolation.

### Understanding reinhardt-test Fixtures

Reinhardt provides shared fixtures in `reinhardt-test/src/fixtures.rs`:

```rust
use rstest::*;
use reinhardt::test::fixtures::postgres_fixture;
use testcontainers::{ContainerAsync, GenericImage};
use reinhardt::db::backends::DatabaseConnection;
use std::sync::Arc;

#[fixture]
async fn postgres_fixture() -> (ContainerAsync<GenericImage>, Arc<DatabaseConnection>) {
    // Automatically starts PostgreSQL container
    // Returns container handle and database connection
}
```

**Available fixtures:**
- `postgres_fixture` - PostgreSQL database in Docker container
- `sqlite_fixture` - SQLite in-memory database
- `mysql_fixture` - MySQL database in Docker container

### Using Fixtures in Tests

Create `polls/tests/database_tests.rs`:

```rust
use rstest::*;
use reinhardt::test::fixtures::postgres_fixture;
use testcontainers::{ContainerAsync, GenericImage};
use reinhardt::db::backends::DatabaseConnection;
use std::sync::Arc;
use chrono::Utc;
use crate::models::{Question, Choice};

#[rstest]
#[tokio::test]
async fn test_create_and_retrieve_question(
    #[future] postgres_fixture: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>)
) {
    let (_container, conn) = postgres_fixture.await;

    // Create a question
    let question = Question::create(
        &conn,
        "What's your favorite language?".to_string(),
        Utc::now(),
    )
    .await
    .unwrap();

    assert!(question.id.is_some());

    // Retrieve it
    let retrieved = Question::get(&conn, question.id.unwrap())
        .await
        .unwrap()
        .expect("Question not found");

    assert_eq!(retrieved.question_text, "What's your favorite language?");

    // Container is automatically cleaned up when test ends
}

#[rstest]
#[tokio::test]
async fn test_question_choices_relationship(
    #[future] postgres_fixture: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>)
) {
    let (_container, conn) = postgres_fixture.await;

    // Create question
    let question = Question::create(
        &conn,
        "Test question".to_string(),
        Utc::now(),
    )
    .await
    .unwrap();

    let question_id = question.id.unwrap();

    // Add choices
    Choice::create(&conn, question_id, "Rust".to_string()).await.unwrap();
    Choice::create(&conn, question_id, "Python".to_string()).await.unwrap();
    Choice::create(&conn, question_id, "Go".to_string()).await.unwrap();

    // Retrieve choices
    let choices = question.choices(&conn).await.unwrap();

    assert_eq!(choices.len(), 3);
    assert_eq!(choices[0].choice_text, "Rust");
}

#[rstest]
#[tokio::test]
async fn test_increment_votes(
    #[future] postgres_fixture: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>)
) {
    let (_container, conn) = postgres_fixture.await;

    let question = Question::create(&conn, "Test".to_string(), Utc::now())
        .await
        .unwrap();

    let mut choice = Choice::create(&conn, question.id.unwrap(), "Option A".to_string())
        .await
        .unwrap();

    assert_eq!(choice.votes, 0);

    // Increment votes
    choice.increment_votes(&conn).await.unwrap();
    assert_eq!(choice.votes, 1);

    choice.increment_votes(&conn).await.unwrap();
    assert_eq!(choice.votes, 2);
}
```

**Key points:**

- `#[rstest]` - Enables fixture injection
- `#[future]` - Required for async fixtures
- `.await` - Don't forget to await the fixture!
- Container cleanup is automatic via RAII

## Using SQLite Fixtures (Alternative)

For projects without migrations (like examples-tutorial-basis), you can use SQLite fixtures with model-based table creation:

```rust
use rstest::*;
use reinhardt::test::fixtures::sqlite_with_models;
use reinhardt::db::backends::DatabaseConnection;
use std::sync::Arc;
use chrono::Utc;
use crate::models::{Question, Choice};

// Create custom fixture for polls app
#[fixture]
async fn polls_sqlite(
    #[future] sqlite_with_models: Arc<DatabaseConnection>
) -> Arc<DatabaseConnection> {
    sqlite_with_models.await
}

#[rstest]
#[tokio::test]
async fn test_create_question_sqlite(
    #[future] polls_sqlite: Arc<DatabaseConnection>
) {
    let conn = polls_sqlite.await;

    // Create a question
    let question = Question::new(
        "What's your favorite language?".to_string(),
    );
    question.save(&conn).await.unwrap();

    assert!(question.id > 0);

    // Retrieve it
    let retrieved = Question::objects()
        .filter(Question::field_id().eq(question.id))
        .get(&conn)
        .await
        .unwrap();

    assert_eq!(retrieved.question_text, "What's your favorite language?");
}

#[rstest]
#[tokio::test]
async fn test_question_choices_relationship_sqlite(
    #[future] polls_sqlite: Arc<DatabaseConnection>
) {
    let conn = polls_sqlite.await;

    // Create question
    let question = Question::new("Test question".to_string());
    question.save(&conn).await.unwrap();

    // Add choices using ForeignKeyField
    let choice1 = Choice::new(
        ForeignKeyField::new(question.id),
        "Rust".to_string(),
        0,
    );
    choice1.save(&conn).await.unwrap();

    let choice2 = Choice::new(
        ForeignKeyField::new(question.id),
        "Python".to_string(),
        0,
    );
    choice2.save(&conn).await.unwrap();

    // Retrieve choices using generated accessor
    let choices_accessor = Choice::question_accessor().reverse(&question, &conn);
    let choices = choices_accessor.all().await.unwrap();

    assert_eq!(choices.len(), 2);
    assert_eq!(choices[0].choice_text, "Rust");
    assert_eq!(choices[1].choice_text, "Python");
}
```

**Key Differences from PostgreSQL:**

- **In-memory database**: SQLite runs entirely in memory (no container)
- **Faster startup**: No Docker container overhead
- **Model-based tables**: Tables created from model definitions, not migrations
- **Simpler teardown**: Database disappears when test ends

**When to use SQLite vs PostgreSQL:**

| Feature | SQLite | PostgreSQL |
|---------|--------|------------|
| Speed | Very fast (in-memory) | Slower (container startup) |
| Isolation | Process-level | Container-level |
| Production parity | Low | High |
| Migrations | Not required | Required |
| Best for | Unit tests, simple integration tests | Full integration tests, pre-production validation |

## Testing Views with rstest

Create `polls/tests/view_tests.rs`:

```rust
use rstest::*;
use reinhardt::test::fixtures::postgres_fixture;
use testcontainers::{ContainerAsync, GenericImage};
use reinhardt::db::backends::DatabaseConnection;
use reinhardt::http::{Request, Response};
use std::sync::Arc;
use chrono::Utc;
use bytes::Bytes;
use hyper::{HeaderMap, Method, Version};
use crate::models::Question;
use crate::views;

#[rstest]
#[tokio::test]
async fn test_index_no_questions(
    #[future] postgres_fixture: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>)
) {
    let (_container, conn) = postgres_fixture.await;

    // Call index view with empty database
    let response = views::index(conn).await.unwrap();

    assert_eq!(response.status(), 200);
    // Verify response body contains "No polls"
}

#[rstest]
#[tokio::test]
async fn test_index_with_questions(
    #[future] postgres_fixture: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>)
) {
    let (_container, conn) = postgres_fixture.await;

    // Create test data
    Question::create(&conn, "Test question 1".to_string(), Utc::now())
        .await
        .unwrap();
    Question::create(&conn, "Test question 2".to_string(), Utc::now())
        .await
        .unwrap();

    // Call index view
    let response = views::index(conn.clone()).await.unwrap();

    assert_eq!(response.status(), 200);
    // Verify both questions appear in response
}

#[rstest]
#[tokio::test]
async fn test_detail_not_found(
    #[future] postgres_fixture: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>)
) {
    let (_container, conn) = postgres_fixture.await;

    let mut request = Request::builder()
        .method(Method::GET)
        .uri("/")
        .version(Version::HTTP_11)
        .headers(HeaderMap::new())
        .body(Bytes::new())
        .build()
        .unwrap();
    request.path_params.insert("question_id".to_string(), "999".to_string());

    let response = views::detail(request, conn).await;

    // Should return 404 for non-existent question
    assert!(response.is_err() || response.unwrap().status() == 404);
}
```

## Custom Fixtures

You can create your own fixtures for common test data:

```rust
use rstest::*;

#[fixture]
fn sample_question() -> Question {
    Question {
        id: None,
        question_text: "Sample question".to_string(),
        pub_date: Utc::now(),
    }
}

#[fixture]
async fn question_with_choices(
    #[future] postgres_fixture: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>)
) -> (Question, Vec<Choice>) {
    let (_container, conn) = postgres_fixture.await;

    let question = Question::create(&conn, "Test".to_string(), Utc::now())
        .await
        .unwrap();

    let choices = vec![
        Choice::create(&conn, question.id.unwrap(), "A".to_string()).await.unwrap(),
        Choice::create(&conn, question.id.unwrap(), "B".to_string()).await.unwrap(),
        Choice::create(&conn, question.id.unwrap(), "C".to_string()).await.unwrap(),
    ];

    (question, choices)
}

#[rstest]
#[tokio::test]
async fn test_with_custom_fixture(
    #[future] question_with_choices: (Question, Vec<Choice>)
) {
    let (question, choices) = question_with_choices.await;

    assert_eq!(choices.len(), 3);
    assert!(question.id.is_some());
}
```

## Testing Best Practices

### 1. Test Organization

```
polls/
├── lib.rs
├── models.rs
├── views.rs
└── tests.rs           # Unit tests
    ├── database_tests.rs  # Database integration tests
    └── view_tests.rs      # View integration tests
```

### 2. Assertion Strictness

Use exact assertions, not loose matching:

```rust
// ✅ GOOD - Exact assertion
assert_eq!(question.question_text, "Expected text");

// ❌ BAD - Loose assertion
assert!(response.contains("text"));
```

### 3. Test Isolation

Each test should be independent:

```rust
// ✅ GOOD - Each test gets its own database container
#[rstest]
#[tokio::test]
async fn test_a(#[future] postgres_fixture: ...) {
    let (_container, conn) = postgres_fixture.await;
    // Test code
}

#[rstest]
#[tokio::test]
async fn test_b(#[future] postgres_fixture: ...) {
    let (_container, conn) = postgres_fixture.await;
    // Different container, complete isolation
}
```

### 4. Cleanup is Automatic

TestContainers handles cleanup via RAII:

```rust
#[rstest]
#[tokio::test]
async fn test_with_container(
    #[future] postgres_fixture: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>)
) {
    let (_container, conn) = postgres_fixture.await;

    // Use database
    // ...

    // No manual cleanup needed!
    // Container is automatically stopped and removed when _container drops
}
```

## Running Tests

Run all tests:

```bash
cargo test --workspace
```

Run specific test file:

```bash
cargo test --package polls -- database_tests
```

Run with output:

```bash
cargo test -- --nocapture
```

## Summary

In this tutorial, you learned:

- How to use rstest for fixture-based testing
- How to use TestContainers for database isolation
- How to use reinhardt-test shared fixtures (postgres_fixture, etc.)
- How to test models with database operations
- How to test views with dependency injection
- How to create custom fixtures for common test data
- Best practices for test organization and isolation
- The importance of automatic cleanup via RAII

## What's Next?

Now that we have a well-tested application, let's add static files (CSS, JavaScript, images) to improve the user interface.

Continue to [Part 6: Static Files](../6-static-files/).
