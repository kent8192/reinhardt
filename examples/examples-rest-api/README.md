# REST API Example

This example demonstrates how to build a RESTful API with the Reinhardt
framework.

## Features

- **Django-style project structure**: Uses config/, settings/, apps.rs
- **Environment-specific configuration**: Separated settings for local, staging,
  production
- **manage CLI**: Django-style management commands (`cargo run --bin manage`)
- **URL routing**: Simple API endpoint definitions

## Project Structure

```
src/
├── config/
│   ├── apps.rs              # Installed apps definition
│   ├── settings.rs          # Environment-based settings loader
│   ├── settings/
│   │   ├── base.rs          # Common settings for all environments
│   │   ├── local.rs         # Local development settings
│   │   ├── staging.rs       # Staging environment settings
│   │   └── production.rs    # Production environment settings
│   └── urls.rs              # URL routing configuration
├── apps.rs                  # App registry
├── config.rs                # config module declaration
├── main.rs                  # Application entry point
└── bin/
    └── manage.rs            # Management CLI tool
```

## Setup

### Prerequisites

- Rust 2024 edition or later
- Cargo

### Build

```bash
# From project root
cargo build --package examples-rest-api
```

**Note**: This example will be buildable after reinhardt is published to
crates.io (version ^0.1).

## Usage

### Starting Development Server

```bash
# Default (127.0.0.1:8000)
cargo run --bin manage runserver

# Custom address
cargo run --bin manage runserver 0.0.0.0:3000
```

### Management Commands

```bash
# Create database migrations (auto-detects app if single app has models)
cargo run --bin manage makemigrations

# Create database migrations for specific app (when multiple apps exist)
cargo run --bin manage makemigrations <app_label>

# Apply migrations
cargo run --bin manage migrate

# Launch interactive shell
cargo run --bin manage shell

# Check project
cargo run --bin manage check

# Collect static files
cargo run --bin manage collectstatic

# Show URL list
cargo run --bin manage showurls
```

**Migration Auto-Detection:**

- Single app project: App label is automatically detected from registered models
- Multiple app project: Explicitly specify the app label (e.g.,
  `makemigrations users`)
- No models found: Error message with usage instructions will be displayed

## API Endpoints

### GET /api/users

Sample endpoint returning user list

**Response example:**

```json
[
    {
        "id": 1,
        "name": "Alice",
        "email": "alice@example.com"
    },
    {
        "id": 2,
        "name": "Bob",
        "email": "bob@example.com"
    }
]
```

## Environment Configuration

Switch settings using the `REINHARDT_ENV` environment variable:

```bash
# Local development (default)
export REINHARDT_ENV=local
cargo run --bin manage runserver

# Staging
export REINHARDT_ENV=staging
export SECRET_KEY=your_secret_key
export ALLOWED_HOSTS=stage.example.com
cargo run --bin manage runserver

# Production
export REINHARDT_ENV=production
export SECRET_KEY=your_secret_key
export ALLOWED_HOSTS=example.com,www.example.com
cargo run --bin manage runserver
```

## Running Tests

This example uses **standard fixtures** from `reinhardt-test` for E2E API
testing with automatic test server management.

### Integration Tests

```bash
# Run all API tests
cargo nextest run --features with-reinhardt --test api_tests

# Run specific test category
cargo nextest run --features with-reinhardt --test api_tests test_create_article
cargo nextest run --features with-reinhardt --test api_tests test_article_crud_workflow
```

### Test Coverage

**Basic Endpoints:**

- ✅ Root endpoint (GET /)
- ✅ Health check endpoint (GET /health)

**Article API - List:**

- ✅ List articles (returns empty array initially)

**Article API - Create:**

- ✅ Create new article with valid data
- ✅ Validation error handling (missing required fields)

**Article API - Read:**

- ✅ Get specific article by ID
- ✅ Get non-existent article returns 404

**Article API - Update:**

- ✅ Update article with partial data
- ✅ Update non-existent article returns 404

**Article API - Delete:**

- ✅ Delete article successfully
- ✅ Delete non-existent article returns 404

**Comprehensive Workflows:**

- ✅ Full CRUD workflow (Create → Read → Update → Delete)

**Error Handling:**

- ✅ Invalid path parameter handling
- ✅ Unsupported HTTP method returns 405
- ✅ Non-existent route returns 404

### Standard Fixtures Used

**`test_server_guard`** - Automatic test server lifecycle management from
`reinhardt-test`

- Starts test server on random available port
- Provides `base_url()` for HTTP requests to API endpoints
- Automatically cleans up resources after test completion
- Ensures isolated test environment for each test

**Usage Example:**

```rust
use reinhardt::test::fixtures::test_server_guard;
use reinhardt::test::resource::TeardownGuard;
use reinhardt::test::fixtures::TestServerGuard;
use rstest::*;

#[rstest]
async fn test_api_endpoint(
    #[future] test_server_guard: TeardownGuard<TestServerGuard>,
) {
    let server = test_server_guard.await;
    let base_url = server.base_url();

    let client = reqwest::Client::new();
    let response = client
        .get(&format!("{}/api/articles", base_url))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), reqwest::StatusCode::OK);

    // Server is automatically cleaned up when test completes
}
```

### Testing Best Practices

**✅ GOOD - Using Standard Fixture for E2E Tests:**

```rust
#[rstest]
async fn test_with_standard_fixture(
    #[future] test_server_guard: TeardownGuard<TestServerGuard>,
) {
    let server = test_server_guard.await;
    let base_url = server.base_url();

    // Test HTTP endpoints with automatic cleanup
    let client = reqwest::Client::new();
    let response = client.get(&format!("{}/api/articles", base_url)).send().await?;
    assert_eq!(response.status(), StatusCode::OK);
}
```

**❌ BAD - Manual Server Management:**

```rust
async fn test_with_manual_setup() {
    let server = start_test_server().await;
    let port = server.port();

    // Test code
    let client = reqwest::Client::new();
    let response = client.get(&format!("http://localhost:{}/api/articles", port)).send().await?;

    server.stop().await; // Manual cleanup required
}
```

**Test Organization:**

- Basic endpoint tests verify server availability and health
- CRUD tests cover all HTTP methods (GET, POST, PUT, DELETE)
- Error handling tests ensure proper status codes (400, 404, 405)
- Comprehensive workflow tests verify end-to-end functionality

See [Testing Standards](../../../docs/TESTING_STANDARDS.md) for comprehensive
guidelines.

## Customization

### Adding New Endpoints

1. Define handler function in your app's `views.rs`
2. Register in project router via `routes()`

```rust
// src/config/urls.rs
use reinhardt::UnifiedRouter;
use reinhardt::routes;

#[routes]
pub fn routes() -> UnifiedRouter {
	UnifiedRouter::new()
		.function("/api/new", Method::GET, new_endpoint)
}
```

### Adding Apps

1. Add to `installed_apps!` macro in `src/config/apps.rs`
2. Add necessary configuration in `src/config/settings/base.rs`

## References

- [Reinhardt Documentation](https://docs.rs/reinhardt)
- [Django Settings Best Practices](https://docs.djangoproject.com/en/stable/topics/settings/)
- [12 Factor App](https://12factor.net/)

## License

This example is provided as part of the Reinhardt project under the BSD 3-Clause License.
