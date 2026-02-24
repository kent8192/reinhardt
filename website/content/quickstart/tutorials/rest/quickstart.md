+++
title = "Quickstart"
weight = 5

[extra]
sidebar_weight = 10
+++

# Quickstart

Create a simple API for administrators to view and edit users and groups in the system.

## Project Setup

First, install the global tool:

```bash
cargo install reinhardt-admin-cli
```

**Note:** After installation, the command is `reinhardt-admin`, not `reinhardt-admin-cli`.

Create a new Reinhardt project called tutorial:

```bash
# Create RESTful API project
reinhardt-admin startproject tutorial --template-type restful
cd tutorial
```

This generates a complete project structure:

```
tutorial/
├── Cargo.toml
├── README.md
└── src/
    ├── main.rs
    ├── config.rs
    ├── apps.rs
    ├── config/
    │   ├── settings.rs
    │   ├── settings/
    │   │   ├── base.rs
    │   │   ├── local.rs
    │   │   ├── staging.rs
    │   │   └── production.rs
    │   ├── urls.rs
    │   └── apps.rs
    └── bin/
        ├── runserver.rs
        └── manage.rs
```

The generated `Cargo.toml` already includes all necessary dependencies for REST API development.

## Models

For this quickstart, we'll use Reinhardt's built-in `User` and `Group` models provided by the auth feature.

## Serializers

Define serializers for data representation. Add to `src/main.rs`:

```rust
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserSerializer {
    pub id: i64,
    pub username: String,
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GroupSerializer {
    pub id: i64,
    pub name: String,
}
```

This example uses simple data structures. In real applications, you can implement the `Serializer` trait to add validation and data transformation logic.

## Views

Implement API endpoints using HTTP method decorators. Add to `users/views.rs`:

```rust
use reinhardt::prelude::*;
use reinhardt::{get, post, Json};
use reinhardt::db::DatabaseConnection;
use std::sync::Arc;
use crate::models::User;
use crate::serializers::UserSerializer;

#[get("/users", name = "list_users")]
pub async fn list_users(
    #[inject] conn: Arc<DatabaseConnection>,
) -> Result<Response> {
    let users = User::all(&conn).await?;
    let serialized: Vec<UserSerializer> = users.into_iter()
        .map(|u| UserSerializer::from(u))
        .collect();

    Response::ok()
        .with_json(&serialized)
}
```

**Note**: The `#[inject]` attribute enables automatic dependency injection. For detailed information, see [HTTP Method Decorators Guide - Dependency Injection](0-http-macros.md#dependency-injection-with-inject).

```rust
#[post("/users", name = "create_user")]
pub async fn create_user(
    Json(data): Json<UserSerializer>,
    #[inject] conn: Arc<DatabaseConnection>,
) -> Result<Response> {
    // Create user
    let user = User::create(&conn, data.username, data.email).await?;
    let serialized = UserSerializer::from(user);

    Response::new(201)
        .with_json(&serialized)
}
```

**Note**: ViewSets (like Django REST framework's ViewSets) are now available! For building complex APIs with less code, see [Tutorial 6: ViewSets and Routers](../6-viewsets-and-routers/). This quickstart focuses on function-based endpoints using HTTP method decorators like `#[get]`, `#[post]`, etc.

## Routing

First, create a users app:

```bash
reinhardt-admin startapp users --template-type restful
```

### Define Models and Serializers

Edit `users/models.rs`:

```rust
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub email: String,
}
```

Edit `users/serializers.rs`:

```rust
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserSerializer {
    pub id: i64,
    pub username: String,
    pub email: String,
}
```

### Create Views

Edit `users/views.rs` to implement full CRUD operations:

```rust
use reinhardt::prelude::*;
use reinhardt::{get, post, Json};
use reinhardt::http::Path;
use reinhardt::db::DatabaseConnection;
use std::sync::Arc;
use crate::models::User;
use crate::serializers::UserSerializer;

#[get("/users", name = "list_users")]
pub async fn list_users(
    #[inject] conn: Arc<DatabaseConnection>,
) -> Result<Response> {
    let users = User::all(&conn).await?;
    let serialized: Vec<UserSerializer> = users.into_iter()
        .map(|u| UserSerializer::from(u))
        .collect();

    Response::ok().with_json(&serialized)
}

#[get("/users/{id}/", name = "retrieve_user")]
pub async fn retrieve_user(
    Path(id): Path<i64>,
    #[inject] conn: Arc<DatabaseConnection>,
) -> Result<Response> {
    let user = User::get(&conn, id).await?
        .ok_or_else(|| Response::not_found().with_body("User not found"))?;

    let serialized = UserSerializer::from(user);
    Response::ok().with_json(&serialized)
}

#[post("/users", name = "create_user")]
pub async fn create_user(
    Json(data): Json<UserSerializer>,
    #[inject] conn: Arc<DatabaseConnection>,
) -> Result<Response> {
    let user = User::create(&conn, data.username, data.email).await?;
    let serialized = UserSerializer::from(user);

    Response::new(201).with_json(&serialized)
}
```

### Configure URLs

Edit `users/urls.rs` to register the view functions:

```rust
use reinhardt::routers::UnifiedRouter;
use hyper::Method;
use crate::views;

pub fn url_patterns() -> UnifiedRouter {
    UnifiedRouter::new()
        .with_namespace("users")
        .endpoint(views::list_users)
        .endpoint(views::retrieve_user)
        .endpoint(views::create_user)
}
```

**URL Patterns Generated:**
- `GET /api/users/` → `views::list_users`
- `GET /api/users/{id}/` → `views::retrieve_user`
- `POST /api/users/` → `views::create_user`

### Register with Project

Edit `src/config/urls.rs`:

```rust
use reinhardt::prelude::*;
use reinhardt::routes;

#[routes]
pub fn routes() -> UnifiedRouter {
    UnifiedRouter::new()
        .mount("/api/", users::urls::url_patterns())
}
```

The `#[routes]` attribute macro automatically registers this function with the
framework for discovery via the `inventory` crate.

Edit `src/config/apps.rs`:

```rust
use reinhardt::installed_apps;

installed_apps! {
    users: "users",
}

pub fn get_installed_apps() -> Vec<String> {
    InstalledApp::all_apps()
}
```

This configures the following URL patterns:

- `GET /api/users/` - List users
- `POST /api/users/` - Create new user
- `GET /api/users/{id}/` - Retrieve specific user

**Note**: To implement full CRUD (UPDATE, DELETE), add additional endpoint functions in `users/views.rs` and register them in `users/urls.rs` following the same pattern.

## Testing the API

First, start the development server:

```bash
cargo make runserver
```

Test the API using curl or httpie:

```bash
# Get list of users
curl http://127.0.0.1:8000/api/users/

# Create new user
curl -X POST http://127.0.0.1:8000/api/users/ \
  -H "Content-Type: application/json" \
  -d '{"username":"alice","email":"alice@example.com"}'

# Get specific user
curl http://127.0.0.1:8000/api/users/1/

# Update user
curl -X PUT http://127.0.0.1:8000/api/users/1/ \
  -H "Content-Type: application/json" \
  -d '{"username":"alice","email":"newemail@example.com"}'

# Delete user
curl -X DELETE http://127.0.0.1:8000/api/users/1/
```

## Summary

In this quickstart, you learned:

1. Setting up a Reinhardt project
2. Defining serializers
3. Creating CRUD APIs with ViewSets
4. Automatic URL generation with routers

For more detailed information, see the [tutorials](../1-serialization/).
