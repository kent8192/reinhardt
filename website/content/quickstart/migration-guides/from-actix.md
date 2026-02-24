+++
title = "Migrating from Actix-web"
weight = 20
+++

# Migrating from Actix-web

Guide for developers migrating from Actix-web to Reinhardt.

## Table of Contents

- [Handler Differences](#handler-differences)
- [Extractors vs DI Params](#extractors-vs-di-params)
- [Middleware](#middleware)
- [State Management](#state-management)
- [Routing Patterns](#routing-patterns)

---

## Handler Differences

### Actix-web Handlers

```rust
use actix_web::{web, HttpResponse};

async fn get_user(path: web::Path<u32>) -> HttpResponse {
    let user_id = path.into_inner();
    HttpResponse::Ok().json(user_id)
}
```

### Reinhardt Handlers

```rust
use reinhardt::{Path, Response};

async fn get_user(Path(id): Path<u32>) -> Response {
    Response::ok().with_json(&id).unwrap()
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

## Extractors vs DI Params

### Actix-web Extractors

```rust
use actix_web::{web, FromRequest};

#[derive(Deserialize)]
struct User {
    username: String,
}

async fn create_user(user: web::Json<User>) -> HttpResponse {
    HttpResponse::Ok().json(user)
}
```

### Reinhardt DI Params

```rust
use reinhardt::{Json, Path};
use serde::Deserialize;

#[derive(Deserialize)]
struct User {
    username: String,
}

async fn create_user(
    Json(user): Json<User>
) -> Response {
    Response::ok().with_json(&user).unwrap()
}
```

---

## Middleware

### Actix-web Middleware

```rust
use actix_web::{dev::Service, Error, HttpMessage};

struct LoggingMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest<B>> for LoggingMiddleware<S>
where
    S: Service<ServiceRequest<B>, B, Response = ServiceResponse<B, Error = Error>,
{
    type Error = Error;
    type Future = Next<S, B>;

    fn call(&self, req: ServiceRequest<B>) -> Self::Future {
        println!("Request: {:?}", req);
        self.service.call(req)
    }
}
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

## State Management

### Actix-web State

```rust
use actix_web::{web, App, HttpServer};

struct AppState {
    db: Database,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let db = Database::new();
    let app_state = web::Data::new(AppState { db });

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/users", web::get().to(get_users))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
```

### Reinhardt DI Context

```rust
use reinhardt::di::{InjectionContext, SingletonScope};
use reinhardt::ServerRouter;

// Set up DI context
let singleton_scope = Arc::new(SingletonScope::new());
let di_ctx = Arc::new(InjectionContext::builder(singleton_scope)
    .add_transient(Database::new())
    .build());

// Register with router
let router = ServerRouter::new()
    .with_di_context(di_ctx);

// Access in handler
async fn handler_with_di(req: Request) -> Response {
    if let Some(db) = req.get_di_context::<Database>() {
        // Use database
        Response::ok()
    } else {
        Response::internal_server_error()
    }
}
```

---

## Routing Patterns

### Actix-web Routing

```rust
use actix_web::{web, App, HttpServer};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .route("/users", web::get().to(get_users))
            .route("/users", web::post().to(create_user))
            .route("/users/{id}", web::get().to(get_user))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
```

### Reinhardt Routing

```rust
use reinhardt::ServerRouter;
use hyper::Method;

let router = ServerRouter::new()
    .function("/users", Method::GET, get_users)
    .function("/users", Method::POST, create_user)
    .function_named("/users/{id}", Method::GET, "user-detail", get_user);

// Or use Handler trait
let router = ServerRouter::new()
    .handler_with_method("/users", Method::GET, GetUsersHandler);
```

---

## Response Building

### Actix-web

```rust
use actix_web::{HttpResponse, http::StatusCode};

HttpResponse::Ok()
    .content_type("application/json")
    .json(data)

HttpResponse::Created()
    .content_type("application/json")
    .json(data)

HttpResponse::NotFound()
    .json(serde_json::json!({
        "error": "Not found"
    }))
```

### Reinhardt

```rust
use reinhardt::Response;

Response::ok()
    .with_json(&data)
    .unwrap()

Response::created()
    .with_json(&data)
    .unwrap()

Response::not_found()
    .with_json(&serde_json::json!({
        "error": "Not found"
    }))
    .unwrap()
```

---

## Shared State

### Actix-web

```rust
// Using web::Data
async fn handler(
    db: web::Data<Arc<Database>>,
) -> HttpResponse {
    let users = db.get_users().await;
    HttpResponse::Ok().json(users)
}
```

### Reinhardt

```rust
// Using DI context via request
async fn handler(req: Request) -> Response {
    if let Some(db) = req.get_di_context::<Database>() {
        let users = db.get_users().await;
        Response::ok().with_json(&users).unwrap()
    } else {
        Response::internal_server_error()
    }
}
```

---

## Migration Checklist

- [ ] Convert handlers to `Handler` trait or functions
- [ ] Replace extractors with DI params (`Json`, `Path`, `Query`, etc.)
- [ ] Convert middleware to `Middleware` trait
- [ ] Replace `web::Data` with DI context
- [ ] Convert routing configuration to `ServerRouter`
- [ ] Update response building
- [ ] Add error handling
- [ ] Test changes

---

## See Also

- [Request API](https://docs.rs/reinhardt-http/latest/reinhardt_http/struct.Request.html)
- [Response API](https://docs.rs/reinhardt-http/latest/reinhardt_http/struct.Response.html)
- [Router API](https://docs.rs/reinhardt-urls/latest/reinhardt_urls/routers/struct.ServerRouter.html)
- [From Django](../from-django/)
- [From Axum](../from-axum/)
