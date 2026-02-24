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

See [examples/examples-twitter](../examples/examples-twitter) for a complete implementation.

### Step 3: Choose Your Flavor

Reinhardt comes in three flavors. Choose the one that fits your needs:

#### Option A: Full (Everything Included) - Default ⚠️ New Default

All features enabled, best for learning and rapid prototyping:

```toml
[dependencies]
# Default behavior - all features enabled
reinhardt = "0.1.0-alpha.1"
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
```

**Includes:** Database, Auth, REST API, Admin, GraphQL, WebSockets, Cache, i18n,
Mail, Sessions, Static Files, Storage

#### Option B: Standard (Balanced)

Balanced setup for most production projects:

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", default-features = false, features = ["standard"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
```

**Includes:** Core, Database (PostgreSQL), REST API, Auth, Middleware, Templates

#### Option C: Minimal (Lightweight)

For microservices and simple APIs:

```toml
[dependencies]
# Standalone crate
reinhardt-micro = "0.1.0-alpha.1"

# Or via main crate
reinhardt = { version = "0.1.0-alpha.1", default-features = false, features = ["minimal"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
```

**Includes:** HTTP, routing, DI, parameter extraction, server

For this guide, we'll use the **Full** flavor (default).

**💡 Want more control?** See the [Feature Flags Guide](FEATURE_FLAGS.md) for
detailed information on 70+ individual feature flags to fine-tune your build.

The project template already includes all necessary dependencies in
`Cargo.toml`.

## Your First API

The generated project already has a working server! Let's customize it.

### Step 4: Run the Development Server

```bash
# Using the runserver binary (recommended)
cargo run --bin runserver

# Or using manage command
cargo run --bin manage runserver
```

Visit `http://127.0.0.1:8000/` in your browser. You should see a welcome
message.

### Step 5: Create Your First Endpoint

Create an app and add a simple endpoint:

```bash
cargo run --bin manage startapp hello --template-type restful
```

Edit `hello/views.rs`:

```rust
use reinhardt::prelude::*;
use reinhardt_http::ViewResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct HelloResponse {
    message: String,
}

pub async fn hello_world(_req: Request) -> ViewResult<Response> {
    let response_data = HelloResponse {
        message: "Hello, Reinhardt!".to_string(),
    };
    let json = serde_json::to_string(&response_data)?;
    Ok(Response::new(StatusCode::OK).with_body(json))
}
```

Register in `hello/urls.rs`, `src/config/urls.rs`, and `src/config/apps.rs` (see
project template for examples).

Test: `curl http://127.0.0.1:8000/hello`

## Building a Real API

Create a CRUD API using ViewSets:

```bash
cargo run --bin manage startapp todos --template-type restful
```

Define model (`todos/models.rs`), serializer (`todos/serializers.rs`), and
ViewSet (`todos/views.rs`):

```rust
// todos/views.rs
use reinhardt::viewsets::ModelViewSet;
use crate::models::Todo;
use crate::serializers::TodoSerializer;

pub struct TodoViewSet;

impl TodoViewSet {
    pub fn new() -> ModelViewSet<Todo, TodoSerializer> {
        ModelViewSet::new("todo")
    }
}
```

Register in `todos/urls.rs`:

```rust
UnifiedRouter::new().viewset("/todos", Arc::new(TodoViewSet::new()))
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
cargo run --bin manage startapp myapp --template-type restful

# Development server
cargo run --bin manage runserver

# Database migrations (when using database features)
# Auto-detects app label if single app has models
cargo run --bin manage makemigrations

# Or specify app label explicitly (when multiple apps exist)
cargo run --bin manage makemigrations <app_label>

# Apply migrations
cargo run --bin manage migrate

# Check project for issues
cargo run --bin manage check

# Collect static files
cargo run --bin manage collectstatic

# Interactive shell
cargo run --bin manage shell
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
[reinhardt-commands documentation](../crates/reinhardt-commands/README.md).

## Next Steps

Congratulations! You've built your first Reinhardt API. Here's what to explore
next:

### 📚 Tutorials

- [Tutorial 1: Serialization](tutorials/en/rest/1-serialization.md)
- [Tutorial 2: Requests and Responses](tutorials/en/rest/2-requests-and-responses.md)
- [Tutorial 3: Class-Based Views](tutorials/en/rest/3-class-based-views.md)
- [Tutorial 4: Authentication & Permissions](tutorials/en/rest/4-authentication-and-permissions.md)

### 🎛️ Advanced Features

- **Dependency Injection** - FastAPI-style DI (Tutorial coming soon)
- [Feature Flags Guide](FEATURE_FLAGS.md) - Optimize your build
- [Database Integration](#database-integration) - Connect to PostgreSQL/MySQL
- [Management Commands](../crates/reinhardt-commands/README.md) - Django-style
  CLI

### 🔌 Database Integration

To use a database instead of in-memory storage:

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", features = ["standard", "database"] }
sqlx = { version = "0.8", features = ["postgres", "runtime-tokio-native-tls"] }
```

Check out the [ORM documentation](api/README.md) for more details.

> **Note**: Complete working examples are planned for future releases.

## Getting Help

- 📖 [API Reference](https://docs.rs/reinhardt)
- 💬 [GitHub Discussions](https://github.com/kent8192/reinhardt-web/discussions)
- 🐛 [Report Issues](https://github.com/kent8192/reinhardt-web/issues)

## Common Issues

**Port Already in Use**: Change the port in `serve()` function

**Compilation Errors**: Ensure Rust 1.91.1+ (`rustc --version`)

**Async Runtime**: Add `#[tokio::main]` to your main function

---

Happy coding with Reinhardt! 🚀
