+++
title = "Tutorial 2: Requests and Responses"
weight = 30

[extra]
sidebar_weight = 40
+++

# Tutorial 2: Requests and Responses

Learn how to handle HTTP requests and responses in Reinhardt.

## Request Object

Reinhardt's `Request` object provides access to HTTP request data:

```rust
use reinhardt::prelude::*;
use reinhardt::get;
use hyper::{Method, StatusCode};

#[get("/example", name = "my_view")]
async fn my_view(request: Request) -> Result<Response> {
    // Access HTTP method
    match request.method {
        Method::GET => println!("GET request"),
        Method::POST => println!("POST request"),
        _ => println!("Other method"),
    }

    // Access headers
    if let Some(content_type) = request.headers.get("content-type") {
        println!("Content-Type: {:?}", content_type);
    }

    // Access query parameters
    let query = request.query_string();

    // Access request body
    let body_bytes = request.body();

    Response::ok()
        .with_body("Success")
}
```

## Response Object

Create responses using the builder pattern:

```rust
use reinhardt::prelude::*;
use serde_json::json;

// Simple text response
let response = Response::ok()
    .with_body("Hello, World!");

// JSON response
let data = json!({
    "message": "Success",
    "count": 42
});
let response = Response::ok()
    .with_json(&data)?;

// Custom status code (201 Created)
let response = Response::new(201)
    .with_body("Created");

// Response with custom headers
let response = Response::ok()
    .with_body("Data")
    .with_header("X-Custom-Header", "value");
```

## Status Codes

Reinhardt provides convenience methods for common status codes:

```rust
// 200 OK
Response::ok()
    .with_json(&data)?

// 201 Created
Response::new(201)
    .with_json(&data)?

// 204 No Content
Response::new(204)

// 400 Bad Request
Response::bad_request()
    .with_body("Invalid input")

// 404 Not Found
Response::not_found()
    .with_body("Resource not found")

// 500 Internal Server Error
Response::internal_server_error()
    .with_body("Error occurred")
```

## Parsing Request Data

### Method 1: Using Request Helper Methods (Recommended)

Reinhardt provides convenient helper methods on the `Request` type for parsing request data:

```rust
use reinhardt::prelude::*;
use reinhardt::post;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
struct CreateSnippet {
    title: String,
    code: String,
    language: String,
}

#[post("/snippets", name = "create_snippet")]
async fn create_snippet(request: Request) -> Result<Response> {
    // Recommended: Use request helper method for JSON parsing
    let data: CreateSnippet = request.json()?;

    println!("Title: {}", data.title);
    println!("Code: {}", data.code);

    Response::new(201)
        .with_json(&data)
}
```

**What `request.json()` does:**

1. **Content-Type validation** - Checks that `Content-Type: application/json` header is present
   - Returns error if header is missing or incorrect
   - Prevents attempting to parse non-JSON data

2. **Deserialization** - Parses request body as JSON using serde
   - Validates against type `T`'s schema
   - Returns structured data or detailed error

3. **Error handling** - Returns `Result<T, Box<dyn std::error::Error>>`
   - Invalid JSON syntax → Parse error
   - Missing required fields → Validation error
   - Type mismatches → Deserialization error

**Error handling examples:**

```rust
// Explicit error handling
let data: CreateSnippet = match request.json() {
    Ok(d) => d,
    Err(e) => {
        // Handles: missing Content-Type, invalid JSON, validation errors
        return Response::bad_request()
            .with_body(&format!("Invalid request: {}", e));
    }
};
```

```rust
// Using `?` operator (recommended - cleaner)
let data: CreateSnippet = request.json()?;
// Automatically returns error response (400 Bad Request) on failure
```

**Available Helper Methods:**
- `request.json::<T>().await?` - Parse JSON body into type T
- `request.parse_form().await?` - Parse URL-encoded form data
- `request.body` - Access raw body bytes

**Benefits:**
- Automatic Content-Type header validation
- Built-in error handling for invalid data
- Clean, readable code
- No manual `std::mem::take()` required

### Method 2: Manual Parsing (Advanced Use Cases)

For special parsing requirements, you can manually parse the request body:

```rust
#[post("/snippets", name = "create_snippet_manual")]
async fn create_snippet_manual(request: Request) -> Result<Response> {
    // Manual parsing for advanced use cases
    let body_bytes = request.body();
    let data: CreateSnippet = serde_json::from_slice(body_bytes)?;

    Response::new(201)
        .with_json(&data)
}
```

**When to use manual parsing:**
- Custom validation logic before deserialization
- Streaming large request bodies
- Non-standard content types

## Content Negotiation

Reinhardt supports multiple content types:

```rust
use reinhardt::prelude::*;
use reinhardt::post;
use serde_json::Value;

#[post("/handle", name = "handle_request")]
async fn handle_request(mut request: Request) -> Result<Response> {
    let content_type = request.headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let body_bytes = request.body();

    match content_type {
        "application/json" => {
            let data: Value = serde_json::from_slice(body_bytes)?;
            Response::ok()
                .with_json(&data)
        }
        "application/x-www-form-urlencoded" => {
            let form_data = request.parse_form().await?;
            Response::ok()
                .with_json(&form_data)
        }
        _ => {
            Response::bad_request()
                .with_body("Unsupported content type")
        }
    }
}
```

## Error Handling

Handle errors gracefully:

```rust
use reinhardt::prelude::*;
use reinhardt::post;

#[post("/safe", name = "safe_view")]
async fn safe_view(request: Request) -> Result<Response> {
    // Parse and validate data
    let body_bytes = request.body();
    let data: CreateSnippet = match serde_json::from_slice(body_bytes) {
        Ok(d) => d,
        Err(e) => {
            return Response::bad_request()
                .with_body(&format!("Invalid JSON: {}", e));
        }
    };

    // Validate required fields
    if data.title.is_empty() {
        return Response::bad_request()
            .with_body("Title is required");
    }

    Response::new(201)
        .with_json(&data)
}
```

## Complete Example

Full request/response handling using Reinhardt's helper methods:

```rust
use reinhardt::prelude::*;
use reinhardt::endpoint;
use serde::{Serialize, Deserialize};
use hyper::Method;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Snippet {
    id: Option<i64>,
    title: String,
    code: String,
    language: String,
}

fn validate_snippet(snippet: &Snippet) -> Result<(), String> {
    if snippet.title.is_empty() {
        return Err("Title is required".to_string());
    }

    if snippet.code.is_empty() {
        return Err("Code is required".to_string());
    }

    Ok(())
}

#[get("/snippets", name = "snippet_list")]
async fn snippet_list(mut request: Request) -> Result<Response> {
    match request.method {
        Method::GET => {
            // Return list of snippets
            let snippets = vec![
                Snippet {
                    id: Some(1),
                    title: "Hello".to_string(),
                    code: "print('hello')".to_string(),
                    language: "python".to_string(),
                }
            ];
            Response::ok()
                .with_json(&snippets)
        }
        Method::POST => {
            // Create new snippet using helper method
            let mut snippet: Snippet = request.json()?;

            // Validate
            if let Err(e) = validate_snippet(&snippet) {
                return Response::bad_request()
                    .with_body(&e);
            }

            // Assign ID and save
            snippet.id = Some(1);

            Response::new(201)
                .with_json(&snippet)
        }
        _ => {
            Response::new(405)
                .with_body("Method not allowed")
        }
    }
}
```

**Key improvements in this example:**
- Using `request.json()?` instead of manual parsing
- Clean, readable code with less boilerplate
- Automatic error handling for invalid JSON

## Summary

In this tutorial, you learned:

1. Accessing request data (method, headers, body)
2. Creating responses with different status codes
3. Parsing JSON from request bodies
4. Content negotiation
5. Error handling
6. Complete request/response workflow

Next tutorial: [Tutorial 3: Class-Based Views](../3-class-based-views/)
