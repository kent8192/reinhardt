# reinhardt-http

HTTP request and response handling for the Reinhardt framework

## Overview

Core HTTP abstractions for the Reinhardt framework. Provides comprehensive request and response types, header handling, cookie management, content negotiation, and streaming support with a Django/DRF-inspired API design.

## Features

### Implemented ✓

#### Request Type

- **Complete HTTP request representation** with all standard components
  - HTTP method, URI, version, headers, body
  - Path parameters (`path_params`) and query string parsing (`query_params`)
  - HTTPS detection (`is_secure`)
  - Remote address tracking (`remote_addr`)
  - Type-safe extensions system (`Extensions`)
- **Builder pattern** for fluent request construction
  - `Request::builder()` - Start building
  - `.method()` - Set HTTP method
  - `.uri()` - Set URI (with automatic query parsing)
  - `.version()` - Set HTTP version (defaults to HTTP/1.1)
  - `.headers()` - Set headers
  - `.header()` - Set single header
  - `.body()` - Set request body
  - `.secure()` - Set HTTPS flag
  - `.remote_addr()` - Set remote address
  - `.build()` - Finalize construction
- **Request parsing** (with `parsers` feature)
  - JSON body parsing
  - Form data parsing
  - Multipart form data
  - Lazy parsing (parse on first access)

#### Response Type

- **Flexible HTTP response creation** with status code helpers
  - `Response::ok()` - 200 OK
  - `Response::created()` - 201 Created
  - `Response::no_content()` - 204 No Content
  - `Response::bad_request()` - 400 Bad Request
  - `Response::unauthorized()` - 401 Unauthorized
  - `Response::forbidden()` - 403 Forbidden
  - `Response::not_found()` - 404 Not Found
  - `Response::gone()` - 410 Gone
  - `Response::internal_server_error()` - 500 Internal Server Error
- **Redirect responses**
  - `Response::permanent_redirect(url)` - 301 Moved Permanently
  - `Response::temporary_redirect(url)` - 302 Found
  - `Response::temporary_redirect_preserve_method(url)` - 307 Temporary Redirect
- **Builder pattern methods**
  - `.with_body(data)` - Set response body (bytes or string)
  - `.with_header(name, value)` - Add single header
  - `.with_typed_header(header)` - Add typed header
  - `.with_json(data)` - Serialize data to JSON and set Content-Type
  - `.with_location(url)` - Set Location header (for redirects)
  - `.with_stop_chain(bool)` - Control middleware chain execution
- **JSON serialization support** with automatic Content-Type
- **Middleware chain control** via `stop_chain` flag

#### StreamingResponse

- **Streaming response support** for large data or real-time content
  - Custom media type configuration
  - Header support
  - Stream-based body (any type implementing `Stream`)

#### Extensions System

- **Type-safe request extensions** for storing arbitrary typed data
  - `request.extensions.insert::<T>(value)` - Store typed data
  - `request.extensions.get::<T>()` - Retrieve typed data
  - Thread-safe with `Arc<Mutex<TypeMap>>`
  - Common use cases: authentication context, request ID, user data

#### Error Integration

- Re-exports `reinhardt_exception::Error` and `Result` for consistent error handling

## Installation

Add `reinhardt` to your `Cargo.toml`:

```toml
[dependencies]
reinhardt = "0.1.0-alpha.1"

# Or use a preset with parsers support:
# reinhardt = { version = "0.1.0-alpha.1", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.1.0-alpha.1", features = ["full"] }      # All features
```

**Note:** HTTP types are available through the main `reinhardt` crate, which provides a unified interface to all framework components.

## Usage Examples

### Basic Request Construction

```rust
use reinhardt::http::Request;
use hyper::Method;
use bytes::Bytes;

// Using builder pattern
let request = Request::builder()
	.method(Method::POST)
	.uri("/api/users?page=1")
	.body(Bytes::from(r#"{"name": "Alice"}"#))
	.build()
	.unwrap();

assert_eq!(request.method, Method::POST);
assert_eq!(request.path(), "/api/users");
assert_eq!(request.query_params.get("page"), Some(&"1".to_string()));
```

### Path and Query Parameters

```rust
use reinhardt::http::Request;
use hyper::Method;

let mut request = Request::builder()
	.method(Method::GET)
	.uri("/api/users/123?sort=name&order=asc")
	.build()
	.unwrap();

// Access query parameters
assert_eq!(request.query_params.get("sort"), Some(&"sort".to_string()));
assert_eq!(request.query_params.get("order"), Some(&"asc".to_string()));

// Add path parameters (typically done by router)
request.path_params.insert("id".to_string(), "123".to_string());
assert_eq!(request.path_params.get("id"), Some(&"123".to_string()));
```

### Request Extensions

```rust
use reinhardt::http::Request;
use hyper::Method;

#[derive(Clone)]
struct UserId(i64);

let mut request = Request::builder()
	.method(Method::GET)
	.uri("/api/profile")
	.build()
	.unwrap();

// Store typed data in extensions
request.extensions.insert(UserId(42));

// Retrieve typed data
let user_id = request.extensions.get::<UserId>().unwrap();
assert_eq!(user_id.0, 42);
```

### Response Helpers

```rust
use reinhardt::http::Response;

// Success responses
let response = Response::ok()
    .with_body("Success");
assert_eq!(response.status, hyper::StatusCode::OK);

let response = Response::created()
    .with_json(&serde_json::json!({
        "id": 123,
        "name": "Alice"
    }))
    .unwrap();
assert_eq!(response.status, hyper::StatusCode::CREATED);
assert_eq!(
    response.headers.get("content-type").unwrap(),
    "application/json"
);

// Error responses
let response = Response::bad_request()
    .with_body("Invalid input");
assert_eq!(response.status, hyper::StatusCode::BAD_REQUEST);

let response = Response::not_found()
    .with_body("Resource not found");
assert_eq!(response.status, hyper::StatusCode::NOT_FOUND);
```

### Redirect Responses

```rust
use reinhardt::http::Response;

// Permanent redirect (301)
let response = Response::permanent_redirect("/new-location");
assert_eq!(response.status, hyper::StatusCode::MOVED_PERMANENTLY);
assert_eq!(
	response.headers.get("location").unwrap().to_str().unwrap(),
	"/new-location"
);

// Temporary redirect (302)
let response = Response::temporary_redirect("/login");
assert_eq!(response.status, hyper::StatusCode::FOUND);

// Temporary redirect preserving method (307)
let response = Response::temporary_redirect_preserve_method("/users/123");
assert_eq!(response.status, hyper::StatusCode::TEMPORARY_REDIRECT);
```

### JSON Response

```rust
use reinhardt::http::Response;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct User {
    id: i64,
    name: String,
}

let user = User {
    id: 1,
    name: "Alice".to_string(),
};

let response = Response::ok()
    .with_json(&user)
    .unwrap();

// Automatically sets Content-Type: application/json
assert_eq!(
    response.headers.get("content-type").unwrap(),
    "application/json"
);
```

### Middleware Chain Control

```rust
use reinhardt::http::Response;

// Stop middleware chain (useful for authentication, rate limiting)
let response = Response::unauthorized()
    .with_body("Authentication required")
    .with_stop_chain(true);

// This response will stop further middleware execution
assert!(response.should_stop_chain());
```

### Streaming Response

```rust
use reinhardt::http::StreamingResponse;
use futures::stream::{self, StreamExt};
use bytes::Bytes;
use hyper::StatusCode;

let data = vec![
	Bytes::from("chunk1"),
	Bytes::from("chunk2"),
	Bytes::from("chunk3"),
];

let stream = stream::iter(data.into_iter().map(Ok));

// Create streaming response (default status: 200 OK)
let response = StreamingResponse::new(Box::pin(stream))
	.status(StatusCode::OK)
	.media_type("text/plain");

// Or use with_status for custom status code
let response = StreamingResponse::with_status(
	Box::pin(stream),
	StatusCode::OK,
)
.media_type("text/plain");

// Use for large files, server-sent events, etc.
```

## API Reference

### Request

**Fields:**
- `method: Method` - HTTP method (GET, POST, etc.)
- `uri: Uri` - Request URI
- `version: Version` - HTTP version
- `headers: HeaderMap` - HTTP headers
- `path_params: HashMap<String, String>` - Path parameters from URL routing
- `query_params: HashMap<String, String>` - Query string parameters
- `is_secure: bool` - Whether request is over HTTPS
- `remote_addr: Option<SocketAddr>` - Client's remote address
- `extensions: Extensions` - Type-safe extension storage

**Methods:**
- `Request::builder()` - Create builder
- `.path()` - Get URI path without query
- `.body()` - Get request body as `Option<&Bytes>`
- `.json::<T>()` - Parse body as JSON (requires `parsers` feature)
- `.post()` - Parse POST data (form/JSON, requires `parsers` feature)
- `.data()` - Get parsed data from body
- `.set_di_context::<T>()` - Set DI context for type T
- `.get_di_context::<T>()` - Get DI context for type T
- `.decoded_query_params()` - Get URL-decoded query parameters
- `.get_accepted_languages()` - Parse Accept-Language header
- `.get_preferred_language()` - Get user's preferred language
- `.is_secure()` - Check if request is over HTTPS
- `.scheme()` - Get URI scheme
- `.build_absolute_uri()` - Build absolute URI from request

### Response

**Fields:**
- `status: StatusCode` - HTTP status code
- `headers: HeaderMap` - HTTP headers
- `body: Bytes` - Response body

**Constructor Methods:**
- `Response::new(status)` - Create with status code
- `Response::ok()` - 200 OK
- `Response::created()` - 201 Created
- `Response::no_content()` - 204 No Content
- `Response::bad_request()` - 400 Bad Request
- `Response::unauthorized()` - 401 Unauthorized
- `Response::forbidden()` - 403 Forbidden
- `Response::not_found()` - 404 Not Found
- `Response::gone()` - 410 Gone
- `Response::internal_server_error()` - 500 Internal Server Error
- `Response::permanent_redirect(url)` - 301 Moved Permanently
- `Response::temporary_redirect(url)` - 302 Found
- `Response::temporary_redirect_preserve_method(url)` - 307 Temporary Redirect

**Builder Methods:**
- `.with_body(data)` - Set body (bytes or string)
- `.with_header(name, value)` - Add header
- `.with_typed_header(header)` - Add typed header
- `.with_json(data)` - Serialize to JSON
- `.with_location(url)` - Set Location header
- `.with_stop_chain(bool)` - Control middleware chain
- `.should_stop_chain()` - Check if chain should stop

### Extensions

**Methods:**
- `.insert::<T>(value)` - Store typed value
- `.get::<T>()` - Retrieve typed value (returns `Option<T>`)
- `.remove::<T>()` - Remove typed value

## Feature Flags

- `parsers` - Enable request body parsing (JSON, form data, multipart)
  - Adds `parse_json()`, `parse_form()` methods to Request
  - Requires `reinhardt-parsers` crate

## Dependencies

- `hyper` - HTTP types (Method, Uri, StatusCode, HeaderMap, Version)
- `bytes` - Efficient byte buffer handling
- `futures` - Stream support for streaming responses
- `serde` - Serialization support (with `serde_json` for JSON)
- `reinhardt-exception` - Error handling
- `reinhardt-parsers` - Request body parsing (optional, with `parsers` feature)

## Testing

The crate includes comprehensive unit tests and doctests covering:
- Request construction and builder pattern
- Response helpers and status codes
- Redirect responses
- JSON serialization
- Extensions system
- Query parameter parsing
- Middleware chain control

Run tests with:
```bash
cargo test
cargo test --features parsers  # With parsers support
```

## License

Licensed under the BSD 3-Clause License.
