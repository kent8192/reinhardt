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

- **Rust** 1.96.0 or later (2024 Edition required)
  ([Install Rust](https://www.rust-lang.org/tools/install))
- **PostgreSQL** (included in `standard` and `full` bundles; optional for custom setups)
- Basic familiarity with Rust and async programming

## Installation

### Step 1: Install Reinhardt Admin

Install the CLI with Cargo. The command below pins this guide to the
documented release for reproducibility; omit `--version` to let Cargo choose
the latest stable release. The literal below is release-managed.

<!-- reinhardt-version-sync -->
```bash
cargo install reinhardt-admin-cli --version "0.3.3"
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
├── .gitignore
├── Cargo.toml
├── Makefile.toml
├── settings/
│   ├── base.toml
│   ├── local.toml
│   ├── staging.toml
│   └── production.toml
├── src/
│   ├── lib.rs
│   ├── apps.rs
│   ├── config.rs
│   ├── bin/
│   │   └── manage.rs
│   └── config/
│       ├── apps.rs
│       ├── settings.rs
│       └── urls.rs
└── README.md
```

### Step 2b: Create a reinhardt-pages Project (Alternative)

For a modern WASM-based frontend with SSR:

```bash
# Create a reinhardt-pages project
reinhardt-admin startproject my-app --template pages
cd my-app

# Build WASM and start development server
cargo make dev
```

This generates a project with 3-layer architecture:

```
my-app/
├── .gitignore
├── Cargo.toml
├── Makefile.toml
├── build.rs
├── index.html
├── settings/
│   ├── base.example.toml
│   ├── local.example.toml
│   ├── staging.example.toml
│   └── production.example.toml
├── src/
│   ├── lib.rs
│   ├── apps.rs
│   ├── config.rs
│   ├── bin/
│   │   └── manage.rs
│   ├── client.rs
│   ├── client/
│   │   ├── lib.rs
│   │   └── components/
│   │       └── nav.rs
│   ├── config/
│   │   ├── apps.rs
│   │   ├── settings.rs
│   │   ├── wasm.rs
│   │   └── urls.rs
└── README.md
```

Visit `http://127.0.0.1:8000/` in your browser.

**Available commands:**
- `cargo make dev` - Build WASM and start development server
- `cargo make dev-watch` - Watch mode with auto-rebuild
- `cargo make build-release` - Production binary build
- `cargo make wasm-build-dev` - Build WASM only (debug)
- `cargo make wasm-build-release` - Build WASM only (release, with wasm-opt)

See [examples/examples-tutorial-basis](https://github.com/kent8192/reinhardt-web/tree/main/examples/examples-tutorial-basis) for a complete implementation.

### Step 3: Choose Your Flavor

Reinhardt comes in three flavors. Choose the one that fits your needs:

#### Option A: Standard (Balanced) - Default

Balanced setup for most production projects:

{% versioned_code(lang="toml") %}
[dependencies]
# Default behavior - all features enabled
reinhardt = { version = "LATEST_VERSION", package = "reinhardt-web" }
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
{% end %}

**Includes:** Core, Database (PostgreSQL), REST API, Auth, Middleware, Templates

#### Option B: Full (Everything Included)

{% versioned_code(lang="toml") %}
[dependencies]
reinhardt = { version = "LATEST_VERSION", package = "reinhardt-web", features = ["full"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
{% end %}

**Includes:** Database, Auth, REST API, Admin, GraphQL, WebSockets, Cache, i18n,
Mail, Sessions, Static Files, Storage

#### Option C: Minimal (Lightweight)

For microservices and simple APIs:

{% versioned_code(lang="toml") %}
[dependencies]
reinhardt = { version = "LATEST_VERSION", package = "reinhardt-web", default-features = false, features = ["minimal"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
{% end %}

**Includes:** HTTP, routing, DI, parameter extraction, server

For this guide, we'll use the **Standard** flavor (default).

**💡 Want more control?** See the [Feature Flags Guide](/docs/feature-flags/) for
detailed information on 60+ individual feature flags to fine-tune your build.

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
reinhardt-admin startapp hello --template rest
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
pub async fn hello_world() -> ViewResult<Response> {
    let response_data = HelloResponse {
        message: "Hello, Reinhardt!".to_string(),
    };
    Ok(Response::ok()
        .with_json(&response_data)?)
}
```

Register in `hello/urls.rs`, `src/config/urls.rs`, and `src/config/apps.rs` (see
project template for examples).

Test: `curl http://127.0.0.1:8000/hello`

## Building a Real API

Create a CRUD API using ViewSets:

```bash
reinhardt-admin startapp todos --template rest
```

Define model (`todos/models.rs`), serializer (`todos/serializers.rs`), and
ViewSet (`todos/views.rs`):

```rust
// todos/views.rs
use reinhardt::ModelViewSet;
use crate::models::Todo;
use crate::serializers::TodoSerializer;

pub fn todo_viewset() -> ModelViewSet<Todo, TodoSerializer> {
    ModelViewSet::new("todo")
}
```

Register in `todos/urls.rs`:

```rust
use reinhardt::ServerRouter;
use crate::views::todo_viewset;

pub fn url_patterns() -> ServerRouter {
    ServerRouter::new()
        .viewset("/todos", todo_viewset())
}
```

Then wire it up in `config/urls.rs`:

```rust
use reinhardt::prelude::*;
use reinhardt::UnifiedRouter;
use reinhardt::routes;

// Note: UnifiedRouter requires the `client-router` feature flag.
#[routes]
pub fn routes() -> UnifiedRouter {
    UnifiedRouter::new()
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
reinhardt-admin startapp myapp --template rest

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

- [Part 1: Project Setup](/quickstart/tutorials/rest/1-project-setup/)
- [Part 2: Your First Endpoints](/quickstart/tutorials/rest/2-first-endpoints/)
- [Part 3: Models and the Database](/quickstart/tutorials/rest/3-models-and-database/)
- [Part 4: Dependency Injection](/quickstart/tutorials/rest/4-dependency-injection/)
- [Part 5: Serializers and Validation](/quickstart/tutorials/rest/5-serializers-and-validation/)
- [Part 6: Bonus: ViewSets and Routers](/quickstart/tutorials/rest/6-viewsets-and-routers/)

### 🎛️ Advanced Features

- [Dependency Injection](/quickstart/tutorials/rest/4-dependency-injection/) - Wire API handlers to the database
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

<!-- reinhardt-version-sync -->
- 📖 [API Reference](https://docs.rs/reinhardt-web/0.3.3/reinhardt/)
- 🗺️ [DeepWiki](https://deepwiki.com/kent8192/reinhardt-web) - AI-generated codebase documentation
- 💬 [GitHub Discussions](https://github.com/kent8192/reinhardt-web/discussions)
- 🐛 [Report Issues](https://github.com/kent8192/reinhardt-web/issues)

## Common Issues

**Port Already in Use**: Change the port in `serve()` function

**Compilation Errors**: Ensure Rust 1.96.0+ (`rustc --version`)

**Async Runtime**: Add `#[tokio::main]` to your main function

---

Happy coding with Reinhardt! 🚀
