+++
title = "Migrating from Axum"
weight = 30
+++

# Migrating from Axum

Guide for developers migrating from Axum to Reinhardt.

## Table of Contents

- [Handler Differences](#handler-differences)
- [State vs DI Context](#state-vs-di-context)
- [Layer vs Middleware](#layer-vs-middleware)
- [Router Differences](#router-differences)
- [Extractors](#extractors)

---

## Handler Differences

### Axum Handlers

```rust
use axum::{
    extract::{Path, Query},
    Json, response::Json,
};

async fn get_user(
    Path(id): Path<u32>,
    Query(params): Query<Params>,
) -> Json<User> {
    let user = fetch_user(id, params).await;
    Json(user)
}
```

### Reinhardt Handlers

```rust
use reinhardt::{Path, Query};
use reinhardt::Response;

async fn get_user(
    Path(id): Path<u32>,
    Query(params): Query<Params>,
) -> Response {
    let user = fetch_user(id, params).await;
    Response::ok().with_json(&user).unwrap()
}

// Or implement Handler trait for stateful handlers
use reinhardt::Handler;
use async_trait::async_trait;

struct UserHandler {
    // State
}

#[async_trait]
impl Handler for UserHandler {
    async fn handle(&self, request: Request) -> Result<Response, Error> {
        Ok(Response::ok())
    }
}
```

---

## State vs DI Context

### Axum State

```rust
use axum::{
    extract::State,
    response::Json,
};
use std::sync::Arc;

struct AppState {
    db: Arc<Database>,
};

async fn get_user(
    State(state): State<Arc<AppState>>,
) -> Json<User> {
    let user = state.db.get_user(123).await;
    Json(user)
}
```

### Reinhardt DI Context

```rust
use reinhardt::Response;

// Get dependencies from request via DI context
async fn get_user(req: Request) -> Response {
    if let Some(db) = req.get_di_context::<Database>() {
        let user = db.get_user(123).await;
        Response::ok().with_json(&user).unwrap()
    } else {
        Response::internal_server_error()
    }
}
```

---

## Layer vs Middleware

### Axum Layers

```rust
use axum::{
    routing::get,
    Router,
    Layer,
    middleware::Logger,
};

let app = Router::new()
    .route("/users", get(handler))
    .layer(Logger::new());
```

### Reinhardt Middleware

```rust
use reinhardt::ServerRouter;
use reinhardt::LoggingMiddleware;

let router = ServerRouter::new()
    .with_middleware(LoggingMiddleware::new())
    .function("/users", Method::GET, handler);
```

---

## Router Differences

### Axum Router

```rust
use axum::{
    routing::get,
    Router,
    Json,
};

let app = Router::new()
    .route("/users", get(get_users))
    .route("/users", post(create_user))
    .route("/users/:id", get(get_user));
```

### Reinhardt Router

```rust
use reinhardt::ServerRouter;
use hyper::Method;

let router = ServerRouter::new()
    .function("/users", Method::GET, get_users)
    .function("/users", Method::POST, create_user)
    .function_named("/users/{id}", Method::GET, "user-detail", get_user);
```

---

## Extractors

### Axum Extractors

```rust
use axum::{
    extract::{Path, Query, Json},
    response::Json,
};

async fn handler(
    Path(id): Path<u32>,
    Query(params): Query<Params>,
    Json(data): Json<Data>,
) -> Json<Response> {
    // ...
}
```

### Reinhardt DI Params

```rust
use reinhardt::{Path, Query, Json};
use reinhardt::Response;

async fn handler(
    Path(id): Path<u32>,
    Query(params): Query<Params>,
    Json(data): Json<Data>,
) -> Response {
    // ...
}
```

Available extractors in Reinhardt:
- `Json<T>` - JSON body
- `Form<T>` - Form data
- `Query<T>` - Query parameters
- `Path<T>` - Path parameters
- `Body` - Raw body bytes
- `Multipart` - Multipart form data
- `Header<T>` - Typed header
- `HeaderNamed` - Named header

---

## Response Building

### Axum

```rust
use axum::{
    extract::Path,
    Json,
    response::{Html, IntoResponse, Json},
};

async fn get_user(Path(id): Path<u32>) -> impl IntoResponse {
    let user = fetch_user(id).await;
    Json(user)
}
```

### Reinhardt

```rust
use reinhardt::Path;
use reinhardt::Response;

async fn get_user(Path(id): Path<u32>) -> Response {
    let user = fetch_user(id).await;
    Response::ok().with_json(&user).unwrap()
}
```

---

## Error Handling

### Axum

```rust
use axum::{
    response::{IntoResponse, Response},
    Json,
    http::StatusCode,
};

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "Not found"),
            AppError::InternalError => (StatusCode::INTERNAL_SERVER_ERROR, "Internal error"),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
```

### Reinhardt

```rust
use reinhardt::{Error, Response};

impl From<AppError> for Response {
    fn from(err: AppError) -> Self {
        match err {
            AppError::NotFound => Response::not_found(),
            AppError::InternalError => Response::internal_server_error(),
        }
    }
}
```

---

## Middleware Creation

### Axum Tower Middleware

```rust
use axum::{
    extract::Request,
    response::Response,
};
use tower::{ServiceBuilder, ServiceExt};
use http::StatusCode;

async fn middleware(request: Request, next: Next) -> Response {
    println!("Request: {:?}", request.uri());
    next.run(request).await
}

let app = Router::new()
    .route("/users", get(handler))
    .layer(middleware);
```

### Reinhardt Middleware

```rust
use async_trait::async_trait;
use reinhardt::{Handler, Middleware, Request, Response};

pub struct LoggingMiddleware;

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
        println!("Request: {} {}", request.method, request.uri.path());
        next.handle(request).await
    }
}
```

---

## Migration Checklist

- [ ] Convert handlers to `Handler` trait or functions
- [ ] Replace State with DI context
- [ ] Replace layers with middleware
- [ ] Convert routing configuration
- [ ] Update extractors to DI params
- [ ] Update error handling
- [ ] Add tests

---

## See Also

- [Request API](https://docs.rs/reinhardt-http/latest/reinhardt_http/struct.Request.html)
- [Response API](https://docs.rs/reinhardt-http/latest/reinhardt_http/struct.Response.html)
- [Router API](https://docs.rs/reinhardt-urls/latest/reinhardt_urls/routers/struct.ServerRouter.html)
- [From Django](../from-django/)
- [From Actix-web](../from-actix/)
