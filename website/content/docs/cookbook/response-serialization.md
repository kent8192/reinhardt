+++
title = "Response Serialization"
weight = 60
+++

# Response Serialization

Guide to serializing data as JSON or other formats in responses.

## Table of Contents

- [JSON Responses](#json-responses)
- [Status Code Responses](#status-code-responses)
- [Streaming Responses](#streaming-responses)
- [Error Responses](#error-responses)
- [Paginated Responses](#paginated-responses)

---

## JSON Responses

### Using `.with_json()`

Return JSON responses using `Response::with_json()`.

```rust
use reinhardt::Response;
use serde::Serialize;

#[derive(Serialize)]
struct User {
    id: u32,
    username: String,
    email: String,
}

async fn get_user() -> Response {
    let user = User {
        id: 1,
        username: "alice".to_string(),
        email: "alice@example.com".to_string(),
    };

    Response::ok()
        .with_json(&user)
        .unwrap()
}
```

### Using `` serde_json::json! `` Macro

Use macro for simple JSON responses.

```rust
use reinhardt::Response;
use serde_json::json;

async fn health_check() -> Response {
    Response::ok()
        .with_json(&json!({
            "status": "healthy",
            "version": "1.0.0"
        }))
        .unwrap()
}
```

---

## Status Code Responses

### Created (201 Created)

```rust
use reinhardt::Response;

async fn create_user() -> Response {
    Response::created()
        .with_json(&serde_json::json!({
            "id": 123,
            "username": "alice"
        }))
        .unwrap()
}
```

### Error Response (400 Bad Request)

```rust
use reinhardt::Response;

async fn invalid_request() -> Response {
    Response::bad_request()
        .with_json(&serde_json::json!({
            "error": "Invalid username",
            "code": "INVALID_USERNAME"
        }))
        .unwrap()
}
```

### Not Found (404 Not Found)

```rust
use reinhardt::Response;

async fn not_found() -> Response {
    Response::not_found()
        .with_json(&serde_json::json!({
            "error": "Resource not found",
            "code": "NOT_FOUND"
        }))
        .unwrap()
}
```

---

## Streaming Responses

### `StreamingResponse`

Stream large or chunked data.

```rust
use reinhardt::StreamingResponse;
use futures::stream;
use bytes::Bytes;

async fn stream_data() -> StreamingResponse<impl Stream<Item = Result<Bytes, Box<dyn std::error::Error + Send + Sync>>>> {
    let data = vec![
        Ok(Bytes::from("chunk1")),
        Ok(Bytes::from("chunk2")),
        Ok(Bytes::from("chunk3")),
    ];

    StreamingResponse::new(stream::iter(data))
        .media_type("text/plain")
}
```

### Server-Sent Events

```rust
use reinhardt::StreamingResponse;
use futures::stream;
use std::time::Duration;

async fn sse_events() -> StreamingResponse<impl Stream<Item = Result<Bytes, Box<dyn std::error::Error + Send + Sync>>>> {
    let events = vec![
        Ok(Bytes::from("data: message 1\n\n")),
        Ok(Bytes::from("data: message 2\n\n")),
    ];

    StreamingResponse::new(stream::iter(events))
        .header(
            hyper::header::CONTENT_TYPE,
            "text/event-stream"
        )
}
```

---

## Error Responses

### Conversion from `Error`

Use `From<Error>` implementation.

```rust
use reinhardt::{Error, Response};

async fn handle_result() -> Response {
    let result = fetch_data().await;

    match result {
        Ok(data) => Response::ok().with_json(&data).unwrap(),
        Err(e) => Response::from(e), // Automatically converts to JSON error response
    }
}
```

### Custom Error Responses

```rust
use reinhardt::{Error, Response};

#[derive(Debug)]
enum ApiError {
    UserNotFound,
    InvalidInput(String),
}

impl From<ApiError> for Response {
    fn from(err: ApiError) -> Self {
        match err {
            ApiError::UserNotFound => Response::not_found()
                .with_json(&serde_json::json!({
                    "error": "User not found"
                }))
                .unwrap(),
            ApiError::InvalidInput(msg) => Response::bad_request()
                .with_json(&serde_json::json!({
                    "error": msg
                }))
                .unwrap(),
        }
    }
}
```

---

## Paginated Responses

### `PaginatedResponse`

Return paginated data.

```rust
use reinhardt::pagination::{PaginatedResponse, PaginationMetadata, Page};
use reinhardt::Response;

async fn list_users(page: usize) -> Response {
    let page_data = fetch_users_page(page).await;

    let metadata = PaginationMetadata {
        count: page_data.count,
        next: if page_data.has_next() {
            Some(format!("/api/users?page={}", page + 1))
        } else {
            None
        },
        previous: if page > 1 {
            Some(format!("/api/users?page={}", page - 1))
        } else {
            None
        },
    };

    let response = PaginatedResponse::new(page_data.object_list, metadata);
    Response::ok().with_json(&response).unwrap()
}
```

### `Page` Struct

```rust
use reinhardt::pagination::Page;

let page = Page::new(
    vec!["item1", "item2", "item3"], // results
    2,                                  // current page number
    5,                                  // total pages
    15,                                 // total items
    3,                                  // items per page
);

assert_eq!(page.start_index(), 4);  // (2-1) * 3 + 1
assert_eq!(page.end_index(), 6);    // 4 + 3 - 1
assert!(page.has_next());
```

---

## Header Customization

### Adding Custom Headers

```rust
use reinhardt::Response;

async fn with_custom_headers() -> Response {
    Response::ok()
        .with_header("X-Custom-Header", "custom-value")
        .with_header("X-Request-ID", "12345")
        .with_json(&serde_json::json!({"data": "value"}))
        .unwrap()
}
```

### Explicit Content-Type

```rust
use reinhardt::Response;

async fn custom_content_type() -> Response {
    Response::ok()
        .with_header("Content-Type", "application/vnd.api+json")
        .with_body(r#"{"data": {"type": "users", "id": "1"}}"#)
}
```

---

## Empty Responses

### 204 No Content

```rust
use reinhardt::Response;

async fn delete_resource() -> Response {
    // Delete resource...
    Response::no_content()
}
```

---

## See Also

- [Request API](https://docs.rs/reinhardt-http/latest/reinhardt_http/struct.Request.html)
- [Response API](https://docs.rs/reinhardt-http/latest/reinhardt_http/struct.Response.html)
- [Pagination](../pagination/)
- [Request Body Parsing](../request-body-parsing/)
