+++
title = "HTTP Method Decorators"
weight = 10

[extra]
sidebar_weight = 20
+++

# HTTP Method Decorators

This guide covers HTTP method decorators (`#[get]`, `#[post]`, etc.) for building RESTful APIs with Reinhardt.

## Overview

HTTP method decorators provide a FastAPI-inspired approach to defining API endpoints. They automatically handle routing, request parsing, and dependency injection.

**When to use HTTP decorators:**

- Building RESTful APIs consumed by external clients
- Creating JSON/XML endpoints for microservices
- Traditional HTTP request-response patterns
- Server-side only applications (no WASM)

**For full-stack applications**, consider using [reinhardt-pages with server functions](../basis/1-project-setup/) instead.

---

## Basic Usage

### Simple GET Endpoint

```rust
use reinhardt::prelude::*;
use reinhardt::get;

#[get("/users", name = "list_users")]
pub async fn list_users() -> Result<Response> {
    let users = vec!["Alice", "Bob", "Charlie"];
    let json = serde_json::to_string(&users)?;

    Response::ok()
        .with_body(json)
        .with_header("Content-Type", "application/json")
}
```

### Simple POST Endpoint

```rust
use reinhardt::post;
use reinhardt::http::Json;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct CreateUserRequest {
    username: String,
    email: String,
}

#[post("/users", name = "create_user")]
pub async fn create_user(
    Json(data): Json<CreateUserRequest>,
) -> Result<Response> {
    // Process the user creation
    let user_id = 123; // Simulated

    let response = serde_json::json!({
        "id": user_id,
        "username": data.username,
        "email": data.email,
    });

    Response::new(StatusCode::CREATED)
        .with_body(serde_json::to_string(&response)?)
        .with_header("Content-Type", "application/json")
}
```

### All HTTP Method Decorators

```rust
use reinhardt::{get, post, put, patch, delete};

#[get("/resource", name = "read")]
pub async fn read() -> Result<Response> { /* ... */ }

#[post("/resource", name = "create")]
pub async fn create() -> Result<Response> { /* ... */ }

#[put("/resource/{id}/", name = "update")]
pub async fn update() -> Result<Response> { /* ... */ }

#[patch("/resource/{id}/", name = "partial_update")]
pub async fn partial_update() -> Result<Response> { /* ... */ }

#[delete("/resource/{id}/", name = "destroy")]
pub async fn destroy() -> Result<Response> { /* ... */ }
```

---

## Path Parameters

### Extracting Path Parameters

Use the `Path` extractor to capture URL parameters:

```rust
use reinhardt::get;
use reinhardt::http::Path;

#[get("/users/{id}/", name = "get_user")]
pub async fn get_user(
    Path(user_id): Path<i64>,
) -> Result<Response> {
    // user_id is automatically parsed from the URL
    let response = serde_json::json!({
        "id": user_id,
        "username": "Alice",
    });

    Response::ok()
        .with_body(serde_json::to_string(&response)?)
        .with_header("Content-Type", "application/json")
}
```

### Multiple Path Parameters

```rust
#[get("/users/{user_id}/posts/{post_id}/", name = "get_user_post")]
pub async fn get_user_post(
    Path((user_id, post_id)): Path<(i64, i64)>,
) -> Result<Response> {
    let response = serde_json::json!({
        "user_id": user_id,
        "post_id": post_id,
        "title": "My Post",
    });

    Response::ok()
        .with_body(serde_json::to_string(&response)?)
}
```

### Named Path Parameters

```rust
use std::collections::HashMap;

#[get("/articles/{year}/{month}/{slug}/", name = "get_article")]
pub async fn get_article(
    Path((year, month, slug)): Path<(i32, i32, String)>,
) -> Result<Response> {
    // year, month, slug are automatically parsed from the URL

    // ... use year, month, slug
}
```

---

## Dependency Injection with `#[inject]`

### Basic Dependency Injection

The `#[inject]` attribute enables automatic dependency injection:

```rust
use reinhardt::get;
use reinhardt::db::DatabaseConnection;
use std::sync::Arc;

#[get("/data", name = "get_data")]
pub async fn get_data(
    #[inject] db: Arc<DatabaseConnection>,
) -> Result<Response> {
    // db is automatically injected by the framework
    let data = db.query("SELECT * FROM items").fetch_all().await?;
    let json = serde_json::to_string(&data)?;

    Response::ok()
        .with_body(json)
}
```

### Multiple Dependencies

```rust
use reinhardt::{get, Request};
use reinhardt::db::DatabaseConnection;
use reinhardt::cache::Cache;
use std::sync::Arc;

#[get("/users/{id}/", name = "get_user")]
pub async fn get_user(
    req: Request,
    #[inject] db: Arc<DatabaseConnection>,
    #[inject(cache = true)] cache: Arc<Cache>,
) -> Result<Response> {
    // Extract path parameter
    let id = req.path_params.get("id")
        .ok_or("Missing id")?
        .parse::<i64>()?;

    // Try cache first
    let cache_key = format!("user:{}", id);
    if let Some(cached) = cache.get(&cache_key).await? {
        return Response::ok().with_body(cached);
    }

    // Query database if not cached
    let user = db.query("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_one()
        .await?;

    let json = serde_json::to_string(&user)?;

    // Store in cache (1 hour TTL)
    cache.set(&cache_key, &json, 3600).await?;

    Response::ok().with_body(json)
}
```

### Cache Control

By default, dependencies are resolved per request. Use `cache = true` for singleton-like behavior:

```rust
#[inject(cache = true)] config: Arc<AppConfig>,  // Singleton
#[inject(cache = false)] request_id: RequestId,  // Per-request
#[inject] db: Arc<DatabaseConnection>,           // Default: per-request
```

### How Dependency Injection Works

1. Framework inspects function signatures at compile time
2. For each `#[inject]` parameter, looks up the provider
3. At runtime, calls the provider to get the dependency
4. Dependency is passed to your handler automatically

**Benefits:**

- **Type-safe** - Compiler validates dependency types
- **No boilerplate** - No need to manually thread dependencies
- **Testable** - Easy to mock dependencies for testing
- **Flexible** - Can inject any registered service

---

## UnifiedRouter Integration

### High-Level API (Recommended)

Use `UnifiedRouter::function()` for application routing:

```rust
use reinhardt::routers::UnifiedRouter;
use hyper::Method;
use crate::views;

pub fn url_patterns() -> UnifiedRouter {
    UnifiedRouter::new()
        .endpoint(views::list_users)
        .endpoint(views::get_user)
        .endpoint(views::create_user)
        .endpoint(views::update_user)
        .endpoint(views::delete_user)
}
```

### Mounting Sub-Routers

```rust
use reinhardt::routes;

#[routes]
pub fn routes() -> UnifiedRouter {
    UnifiedRouter::new()
        .mount("/api/v1/users/", users::urls::url_patterns())
        .mount("/api/v1/posts/", posts::urls::url_patterns())
}
```

### Low-Level API: `Route::from_handler()`

For library development or custom routers:

```rust
use reinhardt::routing::Route;

pub fn url_patterns() -> Vec<Route> {
    vec![
        Route::from_handler("/users", list_users),
        Route::from_handler("/users/{id}/", get_user),
    ]
}
```

**Note**: With `Route::from_handler()`, you must handle HTTP method matching manually in your handler.

### Comparison

| Feature | Route::from_handler | UnifiedRouter::function |
|---------|---------------------|------------------------|
| **Level** | Low-level primitive | High-level API |
| **Prefix Support** | Manual concatenation | Automatic via `.mount()` |
| **Namespace** | Manual management | Automatic registration |
| **HTTP Method** | Match in handler logic | Explicit method parameter |
| **URL Parameters** | `{param}` syntax | `{param}` syntax (same) |
| **Recommended For** | Library development | Application routing |

**Recommendation**: Use `UnifiedRouter::function()` for better maintainability and explicit HTTP method handling.

---

## Advanced Topics

### Custom Extractors

Create custom extractors for common patterns:

```rust
use reinhardt::http::FromRequest;

pub struct CurrentUser {
    pub id: i64,
    pub username: String,
}

#[async_trait]
impl FromRequest for CurrentUser {
    type Error = AuthError;

    async fn from_request(req: &Request) -> Result<Self, Self::Error> {
        // Extract from session, JWT, etc.
        let user_id = req.session().get("user_id")?;

        Ok(CurrentUser {
            id: user_id,
            username: "Alice".to_string(),
        })
    }
}

// Use in handlers
#[get("/profile", name = "profile")]
pub async fn profile(user: CurrentUser) -> Result<Response> {
    let response = serde_json::json!({
        "id": user.id,
        "username": user.username,
    });

    Response::ok().with_body(serde_json::to_string(&response)?)
}
```

### Error Handling

Implement custom error handling:

```rust
use reinhardt::prelude::*;

#[derive(Debug)]
pub enum ApiError {
    NotFound,
    Unauthorized,
    BadRequest(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ApiError::NotFound => write!(f, "Not Found"),
            ApiError::Unauthorized => write!(f, "Unauthorized"),
            ApiError::BadRequest(msg) => write!(f, "Bad Request: {}", msg),
        }
    }
}

impl std::error::Error for ApiError {}

// Use in handlers
#[get("/users/{id}/", name = "get_user")]
pub async fn get_user(
    Path(id): Path<i64>,
    #[inject] db: Arc<DatabaseConnection>,
) -> Result<Response, ApiError> {
    let user = User::get(&db, id)
        .await
        .map_err(|_| ApiError::NotFound)?;

    Ok(Response::ok().with_json(&user)?)
}
```

### Middleware Integration

HTTP decorators work seamlessly with Reinhardt middleware:

```rust
use reinhardt::{Middleware, MiddlewareChain, Handler, Request, Response};
use std::sync::Arc;

pub struct AuthMiddleware;

#[async_trait]
impl Middleware for AuthMiddleware {
    async fn process(&self, req: Request, next: Arc<dyn Handler>) -> Result<Response> {
        // Check authentication
        if !req.session().has("user_id") {
            return Ok(Response::new(StatusCode::UNAUTHORIZED)
                .with_body("Unauthorized"));
        }

        next.handle(req).await
    }
}

// Apply to routes
let router = UnifiedRouter::new()
    .function("/protected", Method::GET, protected_handler);

let app = MiddlewareChain::new(Arc::new(router))
    .with_middleware(Arc::new(AuthMiddleware));
```

---

## Best Practices

### 1. Use Named Routes

```rust
#[get("/users/{id}/", name = "get_user")]  // ✅ Good
#[get("/users/{id}/")]                   // ❌ Avoid (no name)
```

Named routes enable URL reversal and better debugging.

### 2. Explicit HTTP Methods

```rust
// ✅ Good: Explicit method
UnifiedRouter::new()
    .function("/users", Method::GET, list_users)
    .function("/users", Method::POST, create_user)

// ❌ Avoid: Ambiguous routing
Route::from_handler("/users", handle_users)  // Must check method manually
```

### 3. Type-Safe Parameter Extraction

```rust
// ✅ Good: Type-safe extraction
Path(user_id): Path<i64>

// ❌ Avoid: Manual parsing
let user_id = req.path_params.get("id")?.parse::<i64>()?;
```

### 4. Dependency Injection Over Manual Threading

```rust
// ✅ Good: Automatic injection
#[get("/data", name = "get_data")]
pub async fn get_data(
    #[inject] db: Arc<DatabaseConnection>,
) -> Result<Response> { /* ... */ }

// ❌ Avoid: Manual threading
pub async fn get_data(req: Request) -> Result<Response> {
    let db = req.app_state.get::<DatabaseConnection>()?;
    // ...
}
```

---

## Summary

HTTP method decorators provide a powerful, type-safe way to build RESTful APIs in Reinhardt:

- **`#[get]`, `#[post]`, etc.** - FastAPI-inspired routing
- **Path extractors** - Type-safe URL parameter parsing
- **`#[inject]`** - Automatic dependency injection
- **`UnifiedRouter`** - High-level routing API
- **Custom extractors** - Extend for your use cases

**When to use:**
- Building RESTful APIs for external clients
- JSON/XML endpoints
- Traditional HTTP request-response patterns

**Alternative:**
- For full-stack applications, see [reinhardt-pages with server functions](../basis/1-project-setup/)

For more examples, see the [REST API Tutorial](../1-serialization/).
