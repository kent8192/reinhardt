# Hello World Example

A minimal Reinhardt application demonstrating basic HTTP endpoint handling.

## Overview

This example showcases:

- **Simple HTTP Endpoints**: Two basic endpoints (`/` and `/health`)
- **Response Building**: Using `Response::ok()` builder pattern
- **JSON Responses**: Returning JSON data with proper content-type headers
- **Function-based Views**: Async handler functions
- **URL Routing**: Using `UnifiedRouter` for route registration

## Project Structure

```
examples-hello-world/
├── src/
│   ├── lib.rs                 # Application initialization
│   ├── config.rs              # Configuration module
│   ├── apps.rs                # App registry
│   ├── config/
│   │   ├── settings.rs        # Settings loading
│   │   ├── urls.rs            # Root URL configuration
│   │   └── apps.rs            # Installed apps
│   └── apps/
│       └── hello/
│           ├── views.rs       # View handlers
│           └── urls.rs        # App-specific routes
├── tests/
│   ├── integration.rs         # E2E tests with standard fixtures
│   └── availability.rs        # crates.io availability tests
└── Cargo.toml
```

## Running the Application

### Development Server

```bash
cargo run --bin manage runserver
```

Server starts at `http://127.0.0.1:8000`

**Available Endpoints:**

- `GET /` - Returns "Hello, World!"
- `GET /health` - Returns JSON health status

### Testing the Endpoints

```bash
# Test root endpoint
curl http://localhost:8000/

# Test health endpoint
curl http://localhost:8000/health
```

## Running Tests

This example uses **standard fixtures** from `reinhardt-test` for E2E testing.

### Integration Tests

```bash
# Run all integration tests
cargo nextest run --features with-reinhardt --test integration

# Run specific test
cargo nextest run --features with-reinhardt --test integration test_hello_world_endpoint
```

### Test Coverage

**Normal Cases:**

- ✅ Root endpoint returns "Hello, World!"
- ✅ Health endpoint returns JSON `{"status": "ok"}`

**Error Cases:**

- ✅ Non-existent routes return 404
- ✅ Unsupported methods return 405

### Standard Fixtures Used

- **`test_server_guard`**: Automatically manages test server lifecycle
  - Starts server on random available port
  - Provides `base_url()` for test requests
  - Cleans up resources after test completion

## Code Highlights

### View Handler (views.rs)

```rust
use reinhardt::{Request, Response};
use serde_json::json;

/// Root endpoint - returns "Hello, World!"
pub async fn hello_world(_req: Request) -> reinhardt::Result<Response> {
	Ok(Response::ok().with_body("Hello, World!"))
}

/// Health check endpoint - returns JSON status
pub async fn health_check(_req: Request) -> reinhardt::Result<Response> {
	let body = json!({"status": "ok"});
	Response::ok().with_json(&body).map_err(Into::into)
}
```

### URL Routing (urls.rs)

```rust
use reinhardt::{Method, UnifiedRouter};
use crate::apps::hello::views;

pub fn url_patterns() -> UnifiedRouter {
	UnifiedRouter::new()
		.function("/", Method::GET, views::hello_world)
		.function("/health", Method::GET, views::health_check)
}
```

### Integration Test Example

```rust
use reinhardt::test::fixtures::test_server_guard;
use reinhardt::test::resource::TeardownGuard;
use reinhardt::test::fixtures::TestServerGuard;
use rstest::*;

#[rstest]
async fn test_hello_world_endpoint(
	#[future] test_server_guard: TeardownGuard<TestServerGuard>,
) {
	let server = test_server_guard.await;
	let base_url = server.base_url();

	let client = reqwest::Client::new();
	let response = client
		.get(&format!("{}/", base_url))
		.send()
		.await
		.expect("Failed to send request");

	assert_eq!(response.status(), reqwest::StatusCode::OK);
	let body = response.text().await.expect("Failed to read response body");
	assert_eq!(body, "Hello, World!");
}
```

## Key Concepts

### Response Builder Pattern

Reinhardt uses a builder pattern for HTTP responses:

```rust
// Simple text response
Response::ok().with_body("Hello, World!")

// JSON response with content-type header
Response::ok().with_json(&json!({"key": "value"}))

// Custom status and headers
Response::new(StatusCode::CREATED)
	.with_header("X-Custom", "value")
	.with_body("Created")
```

### Function-based Views

Views are async functions accepting `Request` and returning `Result<Response>`:

```rust
pub async fn my_view(req: Request) -> reinhardt::Result<Response> {
	// Access request data
	let path_param = req.path_params.get("id");
	let query_param = req.query_params.get("filter");

	// Return response
	Ok(Response::ok().with_body("Response body"))
}
```

### Router Chaining

`UnifiedRouter` methods consume `self` and return `Self`, enabling method
chaining:

```rust
UnifiedRouter::new()
	.function("/path1", Method::GET, handler1)
	.function("/path2", Method::POST, handler2)
	.function("/path3", Method::PUT, handler3)
```

## Learning Path

This example demonstrates the foundation for building Reinhardt applications.
Next steps:

1. **database-integration**: Learn database operations with ORM and
   TestContainers
2. **rest-api**: Build RESTful APIs with serializers, viewsets, and
   authentication
3. Explore advanced features (middleware, dependency injection, etc.)

## References

- [Reinhardt Documentation](../../../docs/)
- [Testing Standards](../../../docs/TESTING_STANDARDS.md)
- [Standard Fixtures Guide](../../../crates/reinhardt-test/README.md)
- [Examples Overview](../../README.md)
