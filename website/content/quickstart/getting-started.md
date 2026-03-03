+++
title = "Getting Started"
description = "Step-by-step guide to get up and running with Reinhardt."
weight = 10

[extra]
sidebar_weight = 10
+++

# Getting Started with Reinhardt

Welcome to Reinhardt! This guide will help you set up your first Reinhardt
project and build a simple REST API.

## Prerequisites

Before you begin, make sure you have:

- **Rust** 1.91.1 or later (2024 Edition required)
  ([Install Rust](https://www.rust-lang.org/tools/install))
- **PostgreSQL** (included in `standard` and `full` bundles; optional for custom setups)
- Basic familiarity with Rust and async programming

## Installation

### Step 1: Install Reinhardt Admin

```bash
cargo install reinhardt-admin-cli
```

**Note:** After installation, the command is `reinhardt-admin`, not
`reinhardt-admin-cli`.

### Step 2: Create a New Project

```bash
# Create a RESTful API project (default)
reinhardt-admin startproject my-api
cd my-api
```

This generates a complete project structure:

```
my-api/
├── Cargo.toml
├── settings/
│   ├── base.example.toml
│   ├── local.example.toml
│   ├── staging.example.toml
│   └── production.example.toml
├── src/
│   ├── lib.rs
│   ├── main.rs
│   ├── config.rs
│   ├── apps.rs
│   ├── bin/
│   │   └── manage.rs
│   └── config/
│       ├── settings.rs
│       ├── urls.rs
│       └── apps.rs
└── README.md
```

### Step 2b: Create a reinhardt-pages Project (Alternative)

For a modern WASM-based frontend with SSR:

```bash
# Create a reinhardt-pages project
reinhardt-admin startproject my-app --with-pages
cd my-app

# Install WASM build tools (first time only)
cargo make install-wasm-tools

# Build WASM and start development server
cargo make dev
```

This generates a project with 3-layer architecture:

```
my-app/
├── Cargo.toml
├── Makefile.toml
├── index.html
├── src/
│   ├── client/       # WASM UI (runs in browser)
│   ├── server/       # Server functions (runs on server)
│   ├── shared/       # Shared types (used by both)
│   └── ...
```

Visit `http://127.0.0.1:8000/` in your browser.

**Available commands:**
- `cargo make dev` - Build WASM and start development server
- `cargo make dev-watch` - Watch mode with auto-rebuild
- `cargo make dev-release` - Production build with optimized WASM
- `cargo make wasm-build-dev` - Build WASM only (debug)
- `cargo make wasm-build-release` - Build WASM only (release, with wasm-opt)

See [examples/examples-twitter](https://github.com/kent8192/reinhardt-web/tree/main/examples/examples-twitter) for a complete implementation.

### Step 3: Choose Your Flavor

Reinhardt comes in three flavors. Choose the one that fits your needs:

#### Option A: Full (Everything Included) - Default ⚠️ New Default

All features enabled, best for learning and rapid prototyping:

{% versioned_code(lang="toml") %}
[dependencies]
# Default behavior - all features enabled
reinhardt = { version = "LATEST_VERSION", package = "reinhardt-web" }
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
{% end %}

**Includes:** Database, Auth, REST API, Admin, GraphQL, WebSockets, Cache, i18n,
Mail, Sessions, Static Files, Storage

#### Option B: Standard (Balanced)

Balanced setup for most production projects:

{% versioned_code(lang="toml") %}
[dependencies]
reinhardt = { version = "LATEST_VERSION", package = "reinhardt-web", default-features = false, features = ["standard"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
{% end %}

**Includes:** Core, Database (PostgreSQL), REST API, Auth, Middleware, Templates

#### Option C: Minimal (Lightweight)

For microservices and simple APIs:

{% versioned_code(lang="toml") %}
[dependencies]
reinhardt = { version = "LATEST_VERSION", package = "reinhardt-web", default-features = false, features = ["minimal"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
{% end %}

**Includes:** HTTP, routing, DI, parameter extraction, server

For this guide, we'll use the **Full** flavor (default).

**💡 Want more control?** See the [Feature Flags Guide](/docs/feature-flags/) for
detailed information on 70+ individual feature flags to fine-tune your build.

The project template already includes all necessary dependencies in
`Cargo.toml`.

## Your First API

The generated project already has a working server! Let's customize it.

### Step 4: Run the Development Server

```bash
# Using cargo-make (recommended)
cargo make runserver
```

Visit `http://127.0.0.1:8000/` in your browser. You should see a welcome
message.

### Step 5: Create Your First Endpoint

Create an app and add a simple endpoint:

```bash
reinhardt-admin startapp hello --template-type restful
```

Edit `hello/views.rs`:

```rust
use reinhardt::prelude::*;
use reinhardt::get;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct HelloResponse {
    message: String,
}

#[get("/hello", name = "hello_world")]
pub async fn hello_world() -> Result<Response> {
    let response_data = HelloResponse {
        message: "Hello, Reinhardt!".to_string(),
    };
    Response::ok()
        .with_json(&response_data)
}
```

Register in `hello/urls.rs`, `src/config/urls.rs`, and `src/config/apps.rs` (see
project template for examples).

Test: `curl http://127.0.0.1:8000/hello`

## Building a Real API

Create a CRUD API using ViewSets:

```bash
reinhardt-admin startapp todos --template-type restful
```

Define model (`todos/models.rs`), serializer (`todos/serializers.rs`), and
ViewSet (`todos/views.rs`):

```rust
// todos/views.rs
use reinhardt::viewsets::ModelViewSet;
use crate::models::Todo;
use crate::serializers::TodoSerializer;

pub fn todo_viewset() -> ModelViewSet<Todo, TodoSerializer> {
    ModelViewSet::new("todo")
}
```

Register in `todos/urls.rs`:

```rust
use reinhardt::routers::DefaultRouter;
use std::sync::Arc;
use crate::views::todo_viewset;

pub fn url_patterns() -> DefaultRouter {
    DefaultRouter::new()
        .register_viewset("/todos", Arc::new(todo_viewset()))
}
```

Then wire it up in `config/urls.rs`:

```rust
use reinhardt::prelude::*;
use reinhardt::routes;

#[routes]
pub fn routes() -> DefaultRouter {
    DefaultRouter::new()
        .mount("/api/", todos::urls::url_patterns())
}
```

This automatically creates all CRUD endpoints (GET, POST, PUT, DELETE).

Test:

```bash
# Create
curl -X POST http://127.0.0.1:8000/api/todos/ \
  -H "Content-Type: application/json" \
  -d '{"title":"Learn Reinhardt","completed":false}'

# List
curl http://127.0.0.1:8000/api/todos/
```

## Project Management Commands

The generated project includes `src/bin/manage.rs` for Django-style management
commands.

### Common Commands

```bash
# Create a new app
reinhardt-admin startapp myapp --template-type restful

# Development server
cargo make runserver

# Database migrations (when using database features)
# Auto-detects app label if single app has models
cargo make makemigrations

# Or specify app label explicitly (when multiple apps exist)
cargo make makemigrations-app -- <app_label>

# Apply migrations
cargo make migrate

# Check project for issues
cargo make check

# Collect static files
cargo make collectstatic

# Interactive shell
cargo make shell
```

**Note on makemigrations:** The command now automatically detects the app label
when only one app has registered models. For projects with multiple apps, you
must specify the app label explicitly.

### Global CLI Tool

You already installed `reinhardt-admin` in Step 1. Use it for:

```bash
# Create new projects
reinhardt-admin startproject myproject

# Create new apps (from project root)
reinhardt-admin startapp myapp
```

For more details, see the
reinhardt-commands documentation.

## Next Steps

Congratulations! You've built your first Reinhardt API. Here's what to explore
next:

### 📚 Tutorials

- [Tutorial 1: Serialization](/quickstart/tutorials/rest/1-serialization/)
- [Tutorial 2: Requests and Responses](/quickstart/tutorials/rest/2-requests-and-responses/)
- [Tutorial 3: Class-Based Views](/quickstart/tutorials/rest/3-class-based-views/)
- [Tutorial 4: Authentication & Permissions](/quickstart/tutorials/rest/4-authentication-and-permissions/)

### 🎛️ Advanced Features

- **Dependency Injection** - FastAPI-style DI (Tutorial coming soon)
- [Feature Flags Guide](/docs/feature-flags/) - Optimize your build
- [Database Integration](#database-integration) - Connect to PostgreSQL/MySQL
- Management Commands - Django-style
  CLI

### 🔌 Database Integration

To use a database instead of in-memory storage:

{% versioned_code(lang="toml") %}
[dependencies]
reinhardt = { version = "LATEST_VERSION", package = "reinhardt-web", features = ["standard", "db-postgres"] }
{% end %}

Check out the [ORM documentation](/docs/api/) for more details.

> **Note**: Complete working examples are planned for future releases.

## Getting Help

- 📖 [API Reference](https://docs.rs/reinhardt-web/latest/reinhardt_web/)
- 🗺️ [DeepWiki](https://deepwiki.com/kent8192/reinhardt-web) - AI-generated codebase documentation
- 💬 [GitHub Discussions](https://github.com/kent8192/reinhardt-web/discussions)
- 🐛 [Report Issues](https://github.com/kent8192/reinhardt-web/issues)

## Common Issues

**Port Already in Use**: Change the port in `serve()` function

**Compilation Errors**: Ensure Rust 1.91.1+ (`rustc --version`)

**Async Runtime**: Add `#[tokio::main]` to your main function

---

Happy coding with Reinhardt! 🚀
