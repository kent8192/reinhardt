+++
title = "Quickstart"
weight = 5

[extra]
sidebar_weight = 10
+++

# Quickstart

Create a simple API for administrators to view and edit users and groups in the system.

## Project Setup

First, install the global tool. During the RC phase, only release-candidate
versions are published to crates.io, so `cargo install` requires an explicit
`--version`. The version below is auto-bumped by release-plz on each release.
Once a stable release ships, the bare `cargo install reinhardt-admin-cli`
will also work.

<!-- reinhardt-version-sync -->
```bash
cargo install reinhardt-admin-cli --version "0.1.0-rc.22"
```

**Note:** After installation, the command is `reinhardt-admin`, not `reinhardt-admin-cli`.

Create a new Reinhardt project called tutorial:

```bash
# Create RESTful API project
reinhardt-admin startproject tutorial --template rest
cd tutorial
```

This generates a complete project structure:

```
tutorial/
├── Cargo.toml
├── README.md
├── Makefile.toml
├── settings/
│   ├── base.toml
│   ├── local.toml
│   ├── staging.toml
│   └── production.toml
└── src/
    ├── lib.rs
    ├── config.rs
    ├── apps.rs
    ├── config/
    │   ├── settings.rs
    │   ├── urls.rs
    │   └── apps.rs
    └── bin/
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
use serde_json::{self as json, json};
use reinhardt::ViewResult;
use reinhardt::{get, post, Json, Response, StatusCode};
use crate::models::User;
use crate::serializers::UserSerializer;

#[get("/users", name = "list_users")]
pub async fn list_users() -> ViewResult<Response> {
    let users = User::objects().all().await?;
    let serialized: Vec<UserSerializer> = users.into_iter()
        .map(|u| UserSerializer::from(u))
        .collect();

    let response_data = json!({ "users": serialized });
    let json = json::to_string(&response_data)?;
    Ok(Response::new(StatusCode::OK)
        .with_header("Content-Type", "application/json")
        .with_body(json))
}
```

**Note**: The `#[inject]` attribute enables automatic dependency injection. For detailed information, see [HTTP Method Decorators Guide - Dependency Injection](0-http-macros.md#dependency-injection-with-inject).

```rust
#[post("/users", name = "create_user")]
pub async fn create_user(
    Json(data): Json<UserSerializer>,
) -> ViewResult<Response> {
    // Create user
    let user = User::objects().create(data.username, data.email).await?;
    let serialized = UserSerializer::from(user);

    let response_data = json!({
        "message": "User created",
        "user": serialized
    });
    let json = json::to_string(&response_data)?;
    Ok(Response::new(StatusCode::CREATED)
        .with_header("Content-Type", "application/json")
        .with_body(json))
}
```

**Note**: ViewSets (like Django REST framework's ViewSets) are now available! For building complex APIs with less code, see [Tutorial 6: ViewSets and Routers](../6-viewsets-and-routers/). This quickstart focuses on function-based endpoints using HTTP method decorators like `#[get]`, `#[post]`, etc.

## Routing

First, create a users app:

```bash
reinhardt-admin startapp users --template rest
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
use serde_json::{self as json, json};
use reinhardt::ViewResult;
use reinhardt::{get, post, Json, Path, Response, StatusCode};
use crate::models::User;
use crate::serializers::UserSerializer;

#[get("/users", name = "list_users")]
pub async fn list_users() -> ViewResult<Response> {
    let users = User::objects().all().await?;
    let serialized: Vec<UserSerializer> = users.into_iter()
        .map(|u| UserSerializer::from(u))
        .collect();

    let response_data = json!({ "users": serialized });
    let json = json::to_string(&response_data)?;
    Ok(Response::new(StatusCode::OK)
        .with_header("Content-Type", "application/json")
        .with_body(json))
}

#[get("/users/{id}/", name = "retrieve_user")]
pub async fn retrieve_user(
    Path(id): Path<i64>,
) -> ViewResult<Response> {
    let user = User::objects().get(id).first().await?
        .ok_or("User not found")?;

    let serialized = UserSerializer::from(user);
    let response_data = json!({ "user": serialized });
    let json = json::to_string(&response_data)?;
    Ok(Response::new(StatusCode::OK)
        .with_header("Content-Type", "application/json")
        .with_body(json))
}

#[post("/users", name = "create_user")]
pub async fn create_user(
    Json(data): Json<UserSerializer>,
) -> ViewResult<Response> {
    let user = User::objects().create(data.username, data.email).await?;
    let serialized = UserSerializer::from(user);

    let response_data = json!({
        "message": "User created",
        "user": serialized
    });
    let json = json::to_string(&response_data)?;
    Ok(Response::new(StatusCode::CREATED)
        .with_header("Content-Type", "application/json")
        .with_body(json))
}
```

### Configure URLs

Edit `users/urls.rs` to register the view functions:

```rust
use reinhardt::ServerRouter;

use super::views;

pub fn url_patterns() -> ServerRouter {
    ServerRouter::new()
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

// Note: UnifiedRouter requires the `client-router` feature flag
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
