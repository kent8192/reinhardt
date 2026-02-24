+++
title = "CORS Configuration"
weight = 20
+++

# CORS Configuration

Guide to configuring Cross-Origin Resource Sharing (CORS).

## Table of Contents

- [Basic Setup](#basic-setup)
- [CORS Configuration Options](#cors-configuration-options)
- [Preflight Requests](#preflight-requests)
- [Development Environment](#development-environment)
- [Production Environment](#production-environment)

---

## Basic Setup

### CorsMiddleware

Use `CorsMiddleware` to configure CORS.

```rust
use reinhardt::{CorsMiddleware, cors::CorsConfig};

let config = CorsConfig {
    allow_origins: vec!["https://example.com".to_string()],
    allow_methods: vec!["GET".to_string(), "POST".to_string()],
    allow_headers: vec!["Content-Type".to_string()],
    allow_credentials: true,
    max_age: Some(3600),
};

let middleware = CorsMiddleware::new(config);
```

---

## CORS Configuration Options

### `CorsConfig`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `allow_origins` | `Vec<String>` | `["*"]` | Allowed origins |
| `allow_methods` | `Vec<String>` | `["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"]` | Allowed HTTP methods |
| `allow_headers` | `Vec<String>` | `["Content-Type", "Authorization"]` | Allowed request headers |
| `allow_credentials` | `bool` | `false` | Whether to allow credentials |
| `max_age` | `Option<u64>` | `Some(3600)` | Preflight cache duration (seconds) |

---

## Preflight Requests

### Automatic OPTIONS Handling

`CorsMiddleware` automatically handles preflight requests (OPTIONS method).

```rust
use reinhardt::CorsMiddleware;

let middleware = CorsMiddleware::permissive();

// Preflight requests are automatically responded to with 204 No Content
// Access-Control-Allow-Origin: *
// Access-Control-Allow-Methods: GET, POST, PUT, PATCH, DELETE, OPTIONS
// Access-Control-Allow-Headers: Content-Type, Authorization
// Access-Control-Max-Age: 3600
```

### Example Preflight Response

```
HTTP/1.1 204 No Content
Access-Control-Allow-Origin: https://example.com
Access-Control-Allow-Methods: GET, POST
Access-Control-Allow-Headers: Content-Type, Authorization
Access-Control-Max-Age: 3600
Access-Control-Allow-Credentials: true
```

---

## Development Environment

### Permissive Mode

Use `permissive()` to allow all origins in development.

```rust
use reinhardt::CorsMiddleware;

// Allow all (development only)
let middleware = CorsMiddleware::permissive();
```

This is equivalent to:

```rust
use reinhardt::cors::CorsConfig;

let config = CorsConfig {
    allow_origins: vec!["*".to_string()],
    allow_methods: vec![
        "GET".to_string(),
        "POST".to_string(),
        "PUT".to_string(),
        "PATCH".to_string(),
        "DELETE".to_string(),
        "OPTIONS".to_string(),
    ],
    allow_headers: vec!["Content-Type".to_string(), "Authorization".to_string()],
    allow_credentials: false,
    max_age: Some(3600),
};
```

---

## Production Environment

### Single Origin

Allow only a specific origin.

```rust
use reinhardt::cors::CorsConfig;

let config = CorsConfig {
    allow_origins: vec!["https://app.example.com".to_string()],
    allow_methods: vec!["GET".to_string(), "POST".to_string()],
    allow_headers: vec!["Content-Type".to_string()],
    allow_credentials: false,
    max_age: Some(3600),
};
```

### Multiple Origins

Allow multiple origins (comma-separated).

```rust
use reinhardt::cors::CorsConfig;

let config = CorsConfig {
    allow_origins: vec![
        "https://app1.example.com".to_string(),
        "https://app2.example.com".to_string(),
    ],
    // ... other config
};
```

### Allow Credentials

Allow requests with cookies and authentication headers.

**Note**: When `allow_credentials: true`, you cannot use `"*"` in `allow_origins`.

```rust
use reinhardt::cors::CorsConfig;

let config = CorsConfig {
    // Cannot use wildcard with credentials
    allow_origins: vec!["https://app.example.com".to_string()],
    allow_methods: vec!["GET".to_string(), "POST".to_string()],
    allow_headers: vec![
        "Content-Type".to_string(),
        "Authorization".to_string(),
        "X-CSRF-Token".to_string(),
    ],
    allow_credentials: true,
    max_age: Some(7200), // 2 hours
};
```

---

## Router Integration

### Apply at Router Level

```rust
use reinhardt::ServerRouter;
use reinhardt::CorsMiddleware;

let router = ServerRouter::new()
    .with_middleware(CorsMiddleware::permissive());
```

### Apply to Specific Routes

```rust
use reinhardt::ServerRouter;
use hyper::Method;

async fn api_handler(_req: reinhardt::Request) -> reinhardt::Result<reinhardt::Response> {
    Ok(reinhardt::Response::ok())
}
let router = ServerRouter::new()
    .function("/api/data", Method::GET, api_handler)
    .with_route_middleware(CorsMiddleware::permissive());
```

---

## Middleware Order

CORS middleware should be applied as early as possible.

**Recommended order**:

1. `RequestIdMiddleware` (request tracking)
2. `LoggingMiddleware` (logging)
3. **CorsMiddleware** (CORS handling)
4. `SecurityMiddleware` (security headers)
5. Other middleware...

```rust
use reinhardt::ServerRouter;
use reinhardt::{
    CorsMiddleware, LoggingMiddleware, SecurityMiddleware
};

let router = ServerRouter::new()
    .with_middleware(LoggingMiddleware::new())
    .with_middleware(CorsMiddleware::permissive())
    .with_middleware(SecurityMiddleware::new());
```

---

## See Also

- [Middleware Creation](../middleware-creation/)
- [Security Headers](https://docs.rs/reinhardt-middleware/latest/reinhardt_middleware/struct.SecurityMiddleware.html)
