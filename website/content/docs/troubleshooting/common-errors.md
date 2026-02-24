+++
title = "Common Errors"
weight = 10
+++

# Common Errors

Solutions to common problems encountered during development.

## Table of Contents

- [404 vs 405](#404-vs-405)
- [Path Parameter Extraction Errors](#path-parameter-extraction-errors)
- [Missing DI Context](#missing-di-context)
- [Middleware Order](#middleware-order)
- [CORS Preflight Failures](#cors-preflight-failures)

---

## 404 vs 405

### Problem

Need to determine whether to return 404 (Not Found) or 405 (Method Not Allowed) when a route is not found.

### Causes

- **404 Not Found**: Path itself is not registered in the router
- **405 Method Not Allowed**: Path exists but HTTP method is not allowed

### Solution

Reinhardt's router automatically distinguishes between 404 and 405:

```rust
use reinhardt::ServerRouter;
use hyper::Method;

let router = ServerRouter::new()
    .function("/users", Method::GET, get_users_handler)
    .function("/users", Method::POST, create_user_handler);

// GET /users → 200 OK
// POST /users → 201 Created
// PUT /users → 405 Method Not Allowed (path exists but PUT not registered)
// DELETE /posts → 404 Not Found (path doesn't exist)
```

### Custom Handlers

```rust
use reinhardt::{Error, Response};

async fn handle_route_not_found() -> Response {
    Response::not_found()
        .with_json(&serde_json::json!({
            "error": "Route not found",
            "code": "ROUTE_NOT_FOUND"
        }))
        .unwrap()
}

async fn handle_method_not_allowed(allowed_methods: Vec<String>) -> Response {
    Response::new(StatusCode::METHOD_NOT_ALLOWED)
        .with_header("Allow", &allowed_methods.join(", "))
        .with_json(&serde_json::json!({
            "error": "Method not allowed",
            "code": "METHOD_NOT_ALLOWED",
            "allowed_methods": allowed_methods
        }))
        .unwrap()
}
```

---

## Path Parameter Extraction Errors

### Problem

Path parameters are not being extracted correctly.

### Causes

- Path parameter name doesn't match handler argument name
- Type conversion error

### Solution

```rust
use reinhardt::ServerRouter;
use hyper::Method;
use reinhardt::Path;

// Parameter name in route registration
let router = ServerRouter::new()
    .function("/users/{id}", Method::GET, get_user_handler);

// Handler argument name must match parameter name
async fn get_user(Path(id): Path<u32>) -> Response {
    // id is automatically converted to u32
    Response::ok().with_json(&serde_json::json!({ "user_id": id })).unwrap()
}
```

### Multiple Path Parameters

```rust
async fn get_post(
    Path(user_id): Path<u32>,
    Path(post_id): Path<u32>,
) -> Response {
    Response::ok().with_json(&serde_json::json!({
        "user_id": user_id,
        "post_id": post_id
    })).unwrap()
}
```

---

## Missing DI Context

### Problem

Cannot access dependencies in handler because DI context is not configured.

### Causes

- DI context not set on router
- Handler not requesting DI context with correct type

### Solution

```rust
use reinhardt::ServerRouter;
use reinhardt::di::{InjectionContext, SingletonScope};
use std::sync::Arc;

let singleton_scope = Arc::new(SingletonScope::new());
let di_ctx = Arc::new(InjectionContext::builder(singleton_scope).build());

let router = ServerRouter::new()
    .with_di_context(di_ctx);

// Get DI context in handler
async fn handler_with_di(req: reinhardt::Request) -> Response {
    if let Some(ctx) = req.get_di_context::<InjectionContext>() {
        // Use DI context
        Response::ok()
    } else {
        Response::internal_server_error()
    }
}
```

### Typed DI Parameters

```rust
use reinhardt::Depends;

#[derive(Clone)]
struct Database {
    // ...
}

async fn handler_with_db(Depends(db): Depends<Arc<Database>>) -> Response {
    // Use db for database operations
    Response::ok()
}
```

---

## Middleware Order

### Problem

Middleware not working as expected due to incorrect order.

### Causes

- CORS middleware after authentication middleware
- Logging middleware after request ID generation

### Solution

Recommended middleware order:

```rust
use reinhardt::ServerRouter;
use reinhardt::middleware::*;

let router = ServerRouter::new()
    .with_middleware(RequestIdMiddleware::new())       // 1. Request ID
    .with_middleware(LoggingMiddleware::new())       // 2. Logging
    .with_middleware(TracingMiddleware::new())       // 3. Tracing
    .with_middleware(SecurityMiddleware::new())      // 4. Security headers
    .with_middleware(CorsMiddleware::permissive())    // 5. CORS
    .with_middleware(SessionMiddleware::new(store))   // 6. Session
    .with_middleware(AuthenticationMiddleware)       // 7. Authentication
    .with_middleware(CsrfMiddleware::default())      // 8. CSRF
    .with_middleware(RateLimitMiddleware::new(strat, store)); // 9. Rate limiting
```

---

## CORS Preflight Failures

### Problem

Preflight requests (OPTIONS) are failing.

### Causes

- CORS middleware not registered
- Allowed origins not configured correctly
- `allow_credentials` is `false` when credentials are needed

### Solution

```rust
use reinhardt::{CorsMiddleware, cors::CorsConfig};

let config = CorsConfig {
    allow_origins: vec!["https://app.example.com".to_string()],
    allow_methods: vec!["GET".to_string(), "POST".to_string()],
    allow_headers: vec!["Content-Type".to_string(), "Authorization".to_string()],
    allow_credentials: true,  // true when sending cookies/auth headers
    max_age: Some(3600),
};

let router = ServerRouter::new()
    .with_middleware(CorsMiddleware::new(config));
```

### Verify Preflight Response

```bash
# Test preflight request
curl -X OPTIONS http://localhost:8080/api/users \
  -H "Origin: https://app.example.com" \
  -H "Access-Control-Request-Method: POST" \
  -H "Access-Control-Request-Headers: content-type" \
  -v
```

Expected response:

```
HTTP/1.1 204 No Content
Access-Control-Allow-Origin: https://app.example.com
Access-Control-Allow-Methods: POST
Access-Control-Allow-Headers: content-type
Access-Control-Allow-Credentials: true
Access-Control-Max-Age: 3600
```

---

## See Also

- [Request API](https://docs.rs/reinhardt-http/latest/reinhardt_http/struct.Request.html)
- [Response API](https://docs.rs/reinhardt-http/latest/reinhardt_http/struct.Response.html)
- [Router API](https://docs.rs/reinhardt-urls/latest/reinhardt_urls/routers/struct.ServerRouter.html)
- [CORS Configuration](/docs/cookbook/cors/)
