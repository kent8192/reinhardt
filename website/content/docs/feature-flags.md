+++
title = "Feature Flags"
description = "Fine-tune your Reinhardt build with feature flags."
weight = 20

[extra]
sidebar_weight = 20
+++

# Feature Flags Guide

## Table of Contents

- [Overview](#overview)
- [Basic Usage](#basic-usage)
- [Bundle Features](#bundle-features)
- [Feature Categories](#feature-categories)
- [Major Crate Features](#major-crate-features)
- [Usage Scenarios](#usage-scenarios)
- [Best Practices](#best-practices)
- [Troubleshooting](#troubleshooting)
- [Migration Guide](#migration-guide) ⚠️ **Breaking Changes**
- [Summary](#summary)
- [Quick Reference](#quick-reference)

---

## Overview

Reinhardt employs a **highly granular feature flag system** with **70+ features** across **3 levels of granularity**:

1. **Bundle Features**: `minimal`, `standard`, `full`
2. **Feature Groups**: `database`, `auth`, `cache`, `middleware`
3. **Individual Features**: `jwt`, `redis-backend`, `cors`

### Benefits

- **Reduced Compile Time**: Exclude unnecessary features
- **Smaller Binary Size**: Only include used code
- **Minimized Dependencies**: Only required crates included
- **Flexible Configuration**: From microservices to full-stack apps

---

## Basic Usage

### Default (full) ⚠️ Changed in v0.1.0-alpha.2

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.18", package = "reinhardt-web" }  # Enables full bundle (all features)
```

**Note**: The default has changed from `standard` to `full`. See [Migration Guide](#migration-guide) for details.

### Standard Configuration

For a balanced setup without all features:

```toml
[dependencies]
reinhardt = {
	version = "0.1.0-alpha.18",
	package = "reinhardt-web",
	default-features = false,
	features = ["standard"]
}
```

### Custom Configuration

```toml
[dependencies]
reinhardt = {
	version = "0.1.0-alpha.18",
	package = "reinhardt-web",
	default-features = false,
	features = ["minimal", "database", "db-postgres", "auth-jwt"]
}
```

---

## Bundle Features

### minimal ⚠️ Changed in v0.1.0-alpha.2

**Lightweight bundle** with essential features for microservices and simple APIs.

**Includes**:
- Core (types, macros, HTTP)
- Dependency Injection
- HTTP Server
- URL routing (always included)

```toml
reinhardt = { version = "0.1.0-alpha.18", package = "reinhardt-web", default-features = false, features = ["minimal"] }
```

**Binary**: ~5-10 MB | **Compile**: Very fast


---

### standard

Balanced configuration for most projects. ⚠️ PostgreSQL is now included by default.

**Includes**:
- `minimal` features
- Database (ORM, migrations, PostgreSQL backend)
- REST API (Serializers, ViewSets, Parsers, Renderers)
- Auth, Middleware, Sessions
- Pagination, Filtering, Throttling, Versioning
- Templates, Signals

```toml
reinhardt = { version = "0.1.0-alpha.18", package = "reinhardt-web", default-features = false, features = ["standard"] }
```

**Binary**: ~20-30 MB | **Compile**: Medium

**Note**: `db-postgres` is now explicitly included. For other databases, use `db-mysql` or `db-sqlite`.

---

### full (default) ⚠️ Now the default

All features enabled (batteries-included).

**Includes**: `standard` + admin, graphql, websockets, cache, i18n, mail, sessions, static-files, storage

```toml
reinhardt = { version = "0.1.0-alpha.18", package = "reinhardt-web" }  # default enables full
# Or explicitly:
reinhardt = { version = "0.1.0-alpha.18", package = "reinhardt-web", features = ["full"] }
```

**Binary**: ~50+ MB | **Compile**: Slow

**Note**: The `full` feature now recursively enables all `-full` features in sub-crates, ensuring comprehensive activation of all functionality.

**Feature Inheritance Note**: Bundle features use inheritance (`full` includes `standard`, which includes `minimal`). This means features like `database` and `auth` may appear in multiple bundles' definitions in `Cargo.toml`. This is intentional and ensures each bundle works independently without requiring lower-level bundles. Cargo handles duplicate feature activations efficiently.

---

### Recursive `-full` Feature Structure

Reinhardt implements a **recursive feature flag system** where each crate level provides its own `full` or `{module}-full` feature that aggregates all features from that crate and its sub-crates.

**Three-Level Architecture:**

1. **Sub-Crate Level**: Individual `full` features
   - Example: `reinhardt-orm/full`, `reinhardt-sessions/full`
   - Aggregates all features within that specific sub-crate

2. **Parent Crate Level**: Module-specific `{module}-full` features
   - Example: `reinhardt-db/database-full`, `reinhardt-core/core-full`, `reinhardt-rest/rest-full`
   - Aggregates all module features + all sub-crate `full` features

3. **Root Crate Level**: Top-level `full` feature
   - Aggregates all parent `{module}-full` + all standalone crate `full` features
   - Provides complete framework activation

**Example Activation Chain:**

```toml
# Root level
reinhardt = { version = "0.1.0-alpha.18", package = "reinhardt-web", features = ["full"] }
# ↓ enables
# reinhardt-db/database-full
# ↓ enables
# reinhardt-orm/full, reinhardt-migrations/full, etc.
```

**Parent Crate `-full` Features:**

| Parent Crate | Full Feature | Includes |
|--------------|--------------|----------|
| `reinhardt-db` | `database-full` | All 8 database sub-crates with their `full` features |
| `reinhardt-core` | `core-full` | All 11 core sub-crates with their `full` features |
| `reinhardt-rest` | `rest-full` | All 9 REST sub-crates with their `full` features |
| `reinhardt-pages` | `pages-full` | SSR, renderers, and component sub-crates with their `full` features |
| `reinhardt-urls` | `urls-full` | All 3 URL routing sub-crates with their `full` features |
| `reinhardt-views` | `views-full` | ViewSets sub-crate with `full` feature |
| `reinhardt-auth` | `auth-full` | Sessions sub-crate + all auth features |
| `reinhardt-di` | `di-full` | Params sub-crate + DI features |
| `reinhardt-utils` | `utils-full` | All 4 utility sub-crates with their `full` features |
| `reinhardt-admin` | `admin-full` | Panel sub-crate with `full` feature |
| `reinhardt-server` | `server-full` | Server sub-crate with `full` feature |

**Direct Usage of Module-Specific Features:**

You can directly use module-specific `-full` features for fine-grained control:

```toml
# Enable only database functionality with all sub-features
reinhardt-db = { version = "0.1.0-alpha.18", features = ["database-full"] }

# Enable only REST API functionality with all sub-features
reinhardt-rest = { version = "0.1.0-alpha.18", features = ["rest-full"] }

# Combine multiple module-full features
reinhardt = {
    version = "0.1.0-alpha.18",
    package = "reinhardt-web",
    default-features = false,
    features = [
        "minimal",
        "reinhardt-db/database-full",
        "reinhardt-rest/rest-full"
    ]
}
```

**Benefits:**

- **Comprehensive Activation**: One `full` feature activates everything at that level and below
- **Modular Control**: Use `{module}-full` for specific functional domains
- **Predictable Behavior**: Clear hierarchy prevents missing features
- **Easy Testing**: Quickly enable all features for a module during development

---

### Preset Bundles

| Bundle | Purpose | Key Features |
|--------|---------|--------------|
| `api-only` | REST API only | Serializers, ViewSets, Auth, Pagination |
| `graphql-server` | GraphQL API | GraphQL, Auth, Database |
| `websocket-server` | Real-time | WebSockets, Auth, Cache |
| `cli-tools` | CLI/Background jobs | Database, Migrations, Tasks, Mail |
| `test-utils` | Testing | Test utilities, Database |

---

## Feature Categories

### Database

#### database

Enables general database functionality.

```toml
features = ["database"]  # Includes: ORM, migrations, contenttypes
```

#### Database-Specific

| Feature | Database | Notes |
|---------|----------|-------|
| `db-postgres` | PostgreSQL | Default |
| `db-mysql` | MySQL | - |
| `db-sqlite` | SQLite | Lightweight |
| `db-cockroachdb` | CockroachDB | Uses Postgres protocol |

---

### Authentication

| Feature | Method | Auto-enables |
|---------|--------|--------------|
| `auth` | Foundation | - |
| `auth-jwt` | JWT | `auth` |
| `auth-session` | Session | `auth`, `sessions` |
| `auth-oauth` | OAuth | `auth` |
| `auth-token` | Token | `auth` |

---

### Cache

| Feature | Backend | Exposure |
|---------|---------|----------|
| `redis-backend` | Redis | Root-level |
| `redis-cluster` | Redis Cluster | Subcrate only* |
| `redis-sentinel` | Redis Sentinel | Subcrate only* |
| `memcached-backend` | Memcached | Subcrate only* |

**Workaround for subcrate-only features**:
```toml
reinhardt = { version = "0.1.0-alpha.18", package = "reinhardt-web", features = ["cache"] }
reinhardt-cache = { version = "0.1.0-alpha.18", features = ["redis-cluster"] }
```

---

### API

| Feature | Format | Default |
|---------|--------|---------|
| `api` | Basic API | - |
| `serialize-json` | JSON | ✅ (via serializers) |
| `serialize-xml` | XML | - |
| `serialize-yaml` | YAML | - |

---

### Middleware

| Feature | Functionality |
|---------|---------------|
| `middleware` | Foundation (auto-enables `sessions`) |
| `middleware-cors` | CORS |
| `middleware-compression` | gzip/brotli |
| `middleware-security` | Security headers |
| `middleware-rate-limit` | Rate limiting |

---

### Dependency Injection

| Feature | Description | Auto-enables |
|---------|-------------|--------------|
| `di` | Full DI system | `reinhardt-di/params`, `reinhardt-core/di` |

The `di` feature enables FastAPI-style dependency injection with parameter extraction:

```rust
use reinhardt::prelude::*;
use reinhardt::{post, Body, Cookie, Header, Json, Path, Query};

#[post("/handler/{id}/", name = "handler")]
async fn handler(
    Path(id): Path<i64>,
    Query(params): Query<SearchParams>,
    Json(body): Json<CreateRequest>,
) -> ViewResult<Response> {
    // ...
}
```

**Note**: The `minimal` and `standard` bundles automatically include the `di` feature, so parameter types (`Body`, `Cookie`, `Header`, `Json`, `Path`, `Query`) are available without explicit configuration.

---

### Other Features

| Feature | Description | Key Crates |
|---------|-------------|------------|
| `admin` | Admin panel | reinhardt-admin, reinhardt-forms, reinhardt-pages |
| `pages` | WASM-based frontend with SSR | reinhardt-pages |
| `graphql` | GraphQL API | reinhardt-graphql |
| `websockets` | Real-time | reinhardt-websockets |
| `i18n` | Internationalization | reinhardt-i18n |
| `mail` | Email sending | reinhardt-mail |
| `sessions` | Session mgmt | reinhardt-auth (includes sessions subcrate) |
| `static-files` | Static serving | reinhardt-utils/staticfiles |
| `storage` | Storage abstraction | reinhardt-utils/storage |
| `tasks` | Background jobs | reinhardt-tasks |
| `shortcuts` | Django-style helpers | reinhardt-shortcuts |
| `plugin` | Plugin system | reinhardt-dentdelion |

---

### Plugin System

| Feature | Description | Key Features |
|---------|-------------|--------------|
| `plugin` | Plugin system foundation | Static plugin registration, plugin registry |
| `plugin-wasm` | WASM plugin support | Dynamic plugin loading, wasmtime integration |
| `plugin-cli` | CLI integration | crates.io integration, plugin management commands |

The plugin system allows extending Reinhardt applications through static and dynamic plugins:

```toml
# Static plugins only
reinhardt-dentdelion = { version = "0.1", default-features = false }

# With WASM support
reinhardt-dentdelion = { version = "0.1", features = ["wasm"] }

# Full plugin system (static + WASM + CLI)
reinhardt-dentdelion = { version = "0.1", features = ["full"] }
```

See [`reinhardt plugin`](https://github.com/kent8192/reinhardt-web/blob/main/crates/reinhardt-commands/README.md#plugin-command-system) commands for managing plugins.

---

### reinhardt-pages

WASM-based reactive frontend framework with server-side rendering (SSR) support.

**Usage with reinhardt-admin-cli:**

```bash
# Create a pages-based project
reinhardt-admin startproject myapp --with-pages

# Create a pages-based app
reinhardt-admin startapp myfeature --with-pages
```

**Architecture:**
- **3-layer structure**: `client/` (WASM UI), `server/` (server functions), `shared/` (common types)
- **WASM frontend**: Runs in browser using wasm-bindgen
- **Server-side rendering (SSR)**: Pre-render pages on server
- **Client-side hydration**: Interactive after initial load
- **Type-safe server functions**: RPC-style communication with `#[server_fn]` macro

**Key Features:**
- Reactive UI components
- Conditional compilation (`cfg(target_arch = "wasm32")`)
- Single-server architecture (API + static files from same server)
- Bootstrap UI integration
- History API routing
- Global state management
- SPA mode with index.html fallback

**Configuration Example:**

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.18", package = "reinhardt-web", features = ["admin"] }

[lib]
crate-type = ["cdylib", "rlib"]  # cdylib for WASM, rlib for server

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"
web-sys = { version = "0.3", features = ["Window", "Document", "Element"] }

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
tokio = { version = "1", features = ["full"] }
```

**Development Workflow:**

```bash
# Install WASM build tools (first time only)
cargo make install-wasm-tools

# Build WASM and start development server
cargo make dev

# Or watch mode with auto-rebuild
cargo make dev-watch

# Production build
cargo make dev-release
```

**RunServer Options for WASM Projects:**

```bash
# Start server with WASM frontend
cargo run --bin manage runserver --with-pages

# Custom static directory
cargo run --bin manage runserver --with-pages --static-dir build

# Disable SPA mode (no index.html fallback)
cargo run --bin manage runserver --with-pages --no-spa
```

See [examples/examples-twitter](https://github.com/kent8192/reinhardt-web/tree/main/examples/examples-twitter) for a complete implementation.

---

### Task Backends

The `tasks` feature provides background job processing with multiple backend options:

| Feature | Backend | Persistence | Scalability | Use Case |
|---------|---------|-------------|-------------|----------|
| `tasks` | Immediate/Dummy | No | - | Development/Testing |
| `tasks-redis` | Redis | Yes | High | Production (caching) |
| `tasks-rabbitmq` | RabbitMQ | Yes | Very High | Production (messaging) |
| `tasks-sqlite` | SQLite | Yes | Low | Small-scale production |

**Configuration Example**:

```toml
# Redis backend
reinhardt-tasks = { version = "0.1", features = ["redis-backend"] }

# RabbitMQ backend (recommended for production)
reinhardt-tasks = { version = "0.1", features = ["rabbitmq-backend"] }

# SQLite backend
reinhardt-tasks = { version = "0.1", features = ["database-backend"] }
```

See [Task Backends Documentation](https://github.com/kent8192/reinhardt-web/blob/main/crates/reinhardt-tasks/README.md#backend-comparison) for detailed comparison.

---

## Major Crate Features

| Crate | Default Features | Key Features |
|-------|------------------|--------------|
| `reinhardt-di` | None | `params`, `dev-tools`, `generator` |
| `reinhardt-db` | `backends`, `pool`, `postgres`, `orm`, `migrations` | `sqlite`, `mysql`, `contenttypes` |
| `reinhardt-auth` | None | `jwt`, `session`, `oauth`, `token`, `argon2-hasher` |
| `reinhardt-rest` | `serializers`, `parsers`, `renderers` | `pagination`, `filters`, `throttling`, `versioning` |
| `reinhardt-cache` | None | `redis-backend`, `redis-cluster`, `memcached-backend` |
| `reinhardt-middleware` | None | `cors`, `compression`, `security`, `rate-limit` |
| `reinhardt-sessions` | None | `database`, `file`, `cookie`, `jwt` |
| `reinhardt-test` | None | `testcontainers`, `static`, `websockets` |
| `reinhardt-dentdelion` | None | `wasm`, `cli`, `full` |
| `reinhardt-tasks` | None | `redis-backend`, `rabbitmq-backend`, `database-backend` |
| `reinhardt-panel` | None | `templates`, `file-uploads`, `full` |

**Auto-enabled dependencies**:
- `di` feature → `reinhardt-di/params` (parameter extraction types)
- `pool` → `reinhardt-di` (database connection injection)
- `minimal` / `standard` → `di` (DI system included in bundles)

---

## Usage Scenarios

| Use Case | Configuration | Binary |
|----------|---------------|--------|
| Microservice | `default-features = false, features = ["minimal"]` | ~5-10 MB |
| REST API | `features = ["api-only", "db-postgres", "auth-jwt"]` | ~20-25 MB |
| GraphQL/WebSocket | `features = ["graphql", "websockets", "db-postgres"]` | ~30-35 MB |
| Full-Featured | `features = ["full"]` | ~50+ MB |
| CLI/Background | `features = ["cli-tools"]` | ~15-20 MB |

## Best Practices

**Disable default-features**: Use `default-features = false` for explicit control

**Explicit Database**: Specify database backend (e.g., `db-postgres`, `db-sqlite`)

**Environment-Specific**: Use feature profiles (`dev`, `prod`)

**Test Configuration**: Add `test-utils` in `[dev-dependencies]` only

---

## Troubleshooting

### Common Issues

**"feature not found"**: Check [Quick Reference](#quick-reference) for correct feature names

**Linker Errors**: Install database client libraries (e.g., `libpq-dev` for PostgreSQL)

**Runtime "feature not enabled"**: Add required feature to `Cargo.toml`

**Debugging**: Use `cargo tree -e features | grep reinhardt` to check enabled features

---

## Migration Guide

### Breaking Changes in v0.1.0-alpha.2

#### 1. Default Feature Changed: `standard` → `full`

**Before (v0.1.0-alpha.1):**
```toml
reinhardt = { version = "0.1.0-alpha.1", package = "reinhardt-web" }  # Enabled: standard bundle
```

**Now (v0.1.0-alpha.2):**
```toml
reinhardt = { version = "0.1.0-alpha.18", package = "reinhardt-web" }  # Enables: full bundle (all features)
```

**Impact:**
- ⚠️ **Longer compile time**: Full bundle includes all features (admin, graphql, websockets, etc.)
- ⚠️ **Larger binary size**: ~50+ MB (was ~20-30 MB with standard)
- ✅ **More features available**: All Reinhardt features are immediately usable

**To keep previous behavior:**
```toml
reinhardt = { version = "0.1.0-alpha.18", package = "reinhardt-web", default-features = false, features = ["standard"] }
```

#### 2. `minimal` Feature Now Includes Core Functionality

**Before (v0.1.0-alpha.1):**
```toml
# minimal was empty - no features enabled
features = ["minimal"]
```

**Now (v0.1.0-alpha.2):**
```toml
features = ["minimal"]
```

**Impact:**
- ✅ **More useful**: `minimal` now provides a working microservice framework
- ✅ **Backward compatible**: Adding features is non-breaking

**Equivalent to:**
- Routing, DI, params, server, core

#### 3. `standard` Now Includes PostgreSQL by Default

**Before (v0.1.0-alpha.1):**
```toml
features = ["standard"]  # Database support, but no specific backend
```

**Now (v0.1.0-alpha.2):**
```toml
features = ["standard"]  # Includes db-postgres explicitly
```

**Impact:**
- ✅ **Works out of the box**: Database features now work without additional configuration
- ⚠️ **PostgreSQL dependency**: `libpq-dev` (or equivalent) required at build time

**For other databases:**
```toml
# MySQL
reinhardt = { version = "0.1.0-alpha.18", package = "reinhardt-web", default-features = false, features = ["standard", "db-mysql"] }

# SQLite
reinhardt = { version = "0.1.0-alpha.18", package = "reinhardt-web", default-features = false, features = ["standard", "db-sqlite"] }
```

#### 4. Removed Features

The following features have been removed (they had no effect):

- `serialize-json`
- `serialize-xml`
- `serialize-yaml`

**Reason**: All three features enabled the same dependencies (`reinhardt-rest/serializers`), providing no actual format-specific control.

**Impact:**
- ⚠️ **Build may fail** if you explicitly used these features
- ✅ **No functional impact**: Serialization still works via `reinhardt-rest/serializers`

**Migration:**
```toml
# Before
features = ["serialize-json", "serialize-xml"]

# After
features = ["reinhardt-rest", "reinhardt-rest/serializers"]
# Or simply use "rest" or "standard" bundle
```

### Migration Checklist

- [ ] Update `Cargo.toml` if you want to keep `standard` instead of `full`
- [ ] Install PostgreSQL development libraries if using `standard` or `full`
- [ ] Remove `serialize-*` features if explicitly specified
- [ ] Test build with new configuration
- [ ] Update documentation references

---

## Summary

Reinhardt provides **70+ features** with **3 granularity levels** (bundle, group, individual).

**Default**: `full` bundle (all features) ⚠️ Changed from `standard`

**Key bundles**: `minimal` (microservice), `standard` (balanced), `full` (all features, default), `api-only`, `graphql-server`, `cli-tools`

**Auto-enabled dependencies**: `di` → `reinhardt-di/params`, `pool` → `reinhardt-di`, `middleware` → `sessions`, `auth-session` → `sessions`

**Best Practice**: Use `default-features = false` for explicit control

---

## Related Documentation

- **Bundle Features**: See Bundle Features section above
- **Feature Categories**: See Feature Categories section above
- **Usage Scenarios**: See Usage Scenarios section above
- [Project Overview](https://github.com/kent8192/reinhardt-web) - Repository and README
- [Getting Started Guide](/quickstart/getting-started/) - Getting started guide
