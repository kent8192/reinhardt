# Reinhardt API Reference

Welcome to the Reinhardt API reference documentation. This guide provides comprehensive information about Reinhardt's APIs, modules, and components.

> **Note**: Full API documentation will be available at [docs.rs/reinhardt](https://docs.rs/reinhardt) once published to crates.io.
> In the meantime, comprehensive documentation is available in each crate's `lib.rs` file.

## Reinhardt Crate Structure

Reinhardt is organized in a hierarchical structure:

### Facade Crates

Top-level crates that integrate and expose multiple sub-crates:

- **`reinhardt`**: Main crate integrating all features (select features via feature flags)
- **`reinhardt-core`**: Integrates core functionality (types, exception, pagination, etc.)
- **`reinhardt-db`**: Integrates database-related features (orm, migrations, backends, etc.)
- **`reinhardt-rest`**: Integrates REST API features (serializers, filters, throttling, etc.)
- **`reinhardt-views`**: Integrates view features (views-core, viewsets)

### Functional Crates

Independent crates responsible for specific functional areas:

- **`reinhardt-auth`**: Authentication and authorization system
- **`reinhardt-http`**: HTTP primitives
- **`reinhardt-di`**: Dependency injection system (includes params sub-crate)
- Other independent crates

### Sub-crates

Located under facade crates and accessed via parent crate:

- **`reinhardt-db::orm`**: ORM functionality (`crates/reinhardt-db/crates/orm`)
- **`reinhardt-db::migrations`**: Migration functionality
- **`reinhardt-rest::serializers`**: Serializer functionality
- **`reinhardt-di::params`**: Parameter extraction functionality
- Many more

### Access Methods

```rust
// Access via facade crate (recommended)
use reinhardt::prelude::*;

// Or access individual crates directly
use reinhardt::db::orm::Model;
use reinhardt::di::params::{Path, Query};
use reinhardt::rest::serializers::Serializer;
```

---

## Core Modules

### reinhardt-core

Core utilities and fundamental types used throughout the framework.

> **Note**: `reinhardt-core` is a facade crate that integrates multiple sub-modules:
> - `reinhardt-core::types` - Core types
> - `reinhardt-core::exception` - Error handling
> - `reinhardt-core::pagination` - Pagination utilities
> - `reinhardt-core::backends` - Cache and storage backends

**Key Components:**

- Types - Basic type definitions
- Exception handling - Error handling utilities
- Pagination - Pagination functionality
- Backends - Cache and storage backends

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-core) (available after crates.io publish)
- See `crates/reinhardt-core/src/lib.rs` for current documentation

### reinhardt-views

View functions and class-based views for handling HTTP requests.

> **Note**: `reinhardt-views` is a facade crate that integrates views-core and viewsets (feature flag).

**Key Components:**

- Function-based views with HTTP method decorators (`#[get]`, `#[post]`, etc.)
- Class-based views
- Generic views (ListView, DetailView, CreateView, UpdateView, DeleteView)
- ViewSets (enabled via feature flag)

**Example:**

```rust
use reinhardt::views::get;
use reinhardt::http::{Request, Response, ViewResult};

#[get("/", name = "my_view")]
async fn my_view(request: Request) -> ViewResult<Response> {
    // Handle request
    Ok(Response::new(200, "Hello, World!".into()))
}

// HTTP method decorators automatically register this function as a route handler
```

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-views) (available after crates.io publish)
- See `crates/reinhardt-views/crates/viewsets/src/lib.rs` for comprehensive ViewSets documentation

### reinhardt-di::params

FastAPI-style parameter extractors for type-safe request data extraction.

> **Note**: `params` is a sub-module of the `reinhardt-di` crate. Enable the `params` feature of `reinhardt-di` to access it.

**Key Components:**

- `Path<T>` - Extract path parameters
- `Query<T>` - Extract query parameters
- `Header<T>`, `HeaderNamed` - Extract headers
- `Cookie<T>`, `CookieNamed` - Extract cookies
- `Json<T>` - Parse JSON body
- `Form<T>` - Parse form data
- `Body`, `Multipart` - Raw body and multipart form data

**Example:**

```rust
use reinhardt::di::params::{Path, Query, Json};
use serde::Deserialize;

#[derive(Deserialize)]
struct UserQuery {
    page: Option<u32>,
    limit: Option<u32>,
}

async fn get_user(
    Path(user_id): Path<i64>,
    Query(params): Query<UserQuery>,
) -> Result<Response> {
    // user_id and params are type-safe
    Ok(Response::new(format!("User {} page {}", user_id, params.page.unwrap_or(1))))
}
```

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-di) (available after crates.io publish)
- See `crates/reinhardt-di/src/lib.rs` for comprehensive DI documentation
- Feature flag: `reinhardt-di = { version = "...", features = ["params"] }`

### reinhardt-di

Dependency injection system inspired by FastAPI.

**Key Components:**

- `Injectable` trait
- `Depends<T>` - Inject dependencies
- `InjectionContext` - DI container
- Singleton and request scopes

**Example:**

```rust
use reinhardt::di::{Injectable, Depends};

#[derive(Clone)]
struct Database {
    pool: DbPool,
}

#[async_trait]
impl Injectable for Database {
    async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
        Ok(Database { pool: get_pool().await? })
    }
}

async fn handler(db: Depends<Database>) -> Result<Response> {
    // db is automatically injected
}
```

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-di) (available after crates.io publish)
- See `crates/reinhardt-di/src/lib.rs` for comprehensive DI documentation
- [Tutorial: Dependency Injection](../tutorials/en/07-dependency-injection.md)

## Database & ORM

### reinhardt-db::orm

ORM layer for database abstraction with reinhardt-query integration.

> **Note**: `orm` is a sub-module of the `reinhardt-db` crate. Currently provides low-level API based on reinhardt-query.

**Key Components:**

- `Model` trait - Base trait for models
- `QuerySet` / `SQLAlchemyQuery` - Chainable query builders
- `Manager` - Model manager for CRUD operations
- Field types (`fields.rs`, `postgres_fields.rs`) - Database field definitions
- Relationships (`relationship.rs`, `many_to_many.rs`) - Model relationships

**Current Implementation Status:**

- ✅ reinhardt-query-based query builder (implemented)
- ✅ Basic CRUD operations (implemented)
- ✅ Relationship definitions (implemented)
- ✅ `#[model(...)]` attribute macro (implemented - automatically applies Model trait)
- 🚧 Django-style `filter(age__gte=18)` syntax (planned)

**Example (Current API):**

```rust
use reinhardt::db::orm::{Model, Manager};
use reinhardt_query::prelude::{Query, Expr, PostgresQueryBuilder};

// Model definition (currently manual implementation)
struct User {
    id: i64,
    username: String,
    email: String,
    age: i32,
}

// Query using reinhardt-query
let query = Query::select()
    .from(User::table_name())
    .column(User::id)
    .column(User::username)
    .column(User::email)
    .and_where(Expr::col(User::age).gte(18))
    .order_by(User::created, reinhardt_query::prelude::Order::Desc)
    .limit(10)
    .to_owned();

let sql = query.to_string(PostgresQueryBuilder);
// Execute SQL to fetch users
```

**Planned API (Future Implementation):**

```rust
// Planned Django-style API
use reinhardt::db::orm::{Model, QuerySet};

#[derive(Model)]  // Macro is planned
struct User {
    id: i64,
    username: String,
    email: String,
}

// Future planned API
let users = User::objects()
    .filter(age__gte = 18)
    .order_by("-created")
    .limit(10)
    .all()
    .await?;
```

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-db) (available after crates.io publish)
- See `crates/reinhardt-db/crates/orm/src/lib.rs` for ORM documentation
- [reinhardt-query documentation](https://docs.rs/reinhardt-query/)

### reinhardt-db::migrations

Database migration system.

> **Note**: `migrations` is a sub-module of the `reinhardt-db` crate.

**Key Components:**

- Migration files - Management of migration files
- Schema operations - Schema operations (CREATE TABLE, ALTER TABLE, etc.)
- Migration runner - Migration execution engine

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-db) (available after crates.io publish)
- See `crates/reinhardt-db/crates/migrations/src/lib.rs` for migrations documentation

### reinhardt-db

Low-level database operations and connection management.

> **Note**: `reinhardt-db` is a facade crate that integrates multiple sub-modules:
> - `reinhardt-db::orm` - ORM layer
> - `reinhardt-db::migrations` - Migration system
> - `reinhardt-db::backends` - Database backends
> - `reinhardt-db::pool` - Connection pooling
> - `reinhardt-db::hybrid` - Hybrid query support
> - `reinhardt-db::associations` - Model associations

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-db) (available after crates.io publish)
- See `crates/reinhardt-db/crates/backends/src/lib.rs` for backends documentation

## REST API Components

### reinhardt-rest::serializers

Data serialization, deserialization, and validation.

> **Note**: `serializers` is a sub-module of the `reinhardt-rest` crate.

**Key Components:**

- `Serializer` trait - Base serializer
- `ModelSerializer` - Auto-generated from models
- Field validators - Field validation
- Nested serializers - Nested serializers

**Example:**

```rust
use reinhardt::rest::serializers::{Serializer, ModelSerializer};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct UserSerializer {
    id: i64,
    username: String,
    email: String,
}

impl Serializer<User> for UserSerializer {
    fn validate(&self, instance: &User) -> ValidationResult {
        // Custom validation
        Ok(())
    }
}
```

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-rest) (available after crates.io publish)
- See `crates/reinhardt-rest/crates/serializers/src/lib.rs` for comprehensive serializers documentation
- [Tutorial: Serialization](../tutorials/en/rest/1-serialization.md)

### reinhardt-views (viewsets feature)

CRUD views for models with automatic routing.

> **Note**: The `viewsets` functionality is provided as a feature flag of the `reinhardt-views` crate.
> Enable it with `reinhardt-views = { version = "...", features = ["viewsets"] }`.

**Key Components:**

- `ModelViewSet` - Full CRUD operations
- `ReadOnlyModelViewSet` - Read-only views
- Custom actions - Custom action definitions

**Example:**

```rust
use reinhardt::views::viewsets::ModelViewSet;

// Use with viewsets feature enabled
let viewset = ModelViewSet::<User, UserSerializer>::new();
```

**Cargo.toml:**

```toml
[dependencies]
reinhardt-views = { version = "0.1.0-alpha.1", features = ["viewsets"] }
```

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-views) (available after crates.io publish)
- See `crates/reinhardt-views/crates/viewsets/src/lib.rs` for comprehensive ViewSets documentation
- [Tutorial: ViewSets and Routers](../tutorials/en/rest/6-viewsets-and-routers.md)

### reinhardt-rest::routers / reinhardt-urls::routers

Automatic URL routing for ViewSets.

> **Note**: Router functionality is provided by `reinhardt-rest::routers` and `reinhardt-urls::routers`.

**Key Components:**

- `Router` trait - Basic router interface
- `DefaultRouter` - Standard REST routing
- URL pattern generation - Automatic URL pattern generation

**Example:**

```rust
use reinhardt::rest::routers::{DefaultRouter, Router};

let mut router = DefaultRouter::new();
router.register("users", user_viewset);
router.register("posts", post_viewset);
```

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-rest) (available after crates.io publish)
- [Module documentation](https://docs.rs/reinhardt-urls) (available after crates.io publish)
- See `crates/reinhardt-urls/src/lib.rs` for URL routing documentation

### reinhardt-core::pagination

Pagination for large datasets.

> **Note**: Pagination functionality is provided by `reinhardt-core::pagination`.

**Key Components:**

- `PageNumberPagination` - Page number-based pagination
- `LimitOffsetPagination` - Limit/Offset-based pagination
- `CursorPagination` - Cursor-based pagination

**Example:**

```rust
use reinhardt::core::pagination::PageNumberPagination;

let pagination = PageNumberPagination::new(25); // 25 items per page
```

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-core) (available after crates.io publish)

### reinhardt-rest::filters

Query filtering for ViewSets.

> **Note**: Filter functionality is provided by `reinhardt-rest::filters`.

**Key Components:**

- `SearchFilter` - Search filter
- `OrderingFilter` - Ordering filter
- Custom filters - Custom filter definitions

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-rest) (available after crates.io publish)

### reinhardt-rest::throttling

Rate limiting for API endpoints.

> **Note**: Throttling functionality is provided by `reinhardt-rest::throttling`.

**Key Components:**

- `AnonRateThrottle` - Rate limiting for anonymous users
- `UserRateThrottle` - Rate limiting for authenticated users
- `ScopedRateThrottle` - Scoped rate limiting

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-rest) (available after crates.io publish)

## Authentication & Security

### reinhardt-auth

Authentication backends and permission system.

**Key Components:**

- JWT authentication - `JwtAuth` (note lowercase `t`)
- Token authentication - `TokenAuthentication`
- Session authentication - `SessionAuthentication`
- Basic authentication - `BasicAuthentication`
- Permission classes - `IsAuthenticated`, `IsAdminUser`, etc.

**Example:**

```rust
use reinhardt::auth::{JwtAuth, IsAuthenticated};

// Configure JWT authentication
let secret_key = b"your-secret-key";
let auth = JwtAuth::new(secret_key);

// Use permission classes
// Protect endpoints with IsAuthenticated
```

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-auth) (available after crates.io publish)
- [Tutorial: Authentication & Permissions](../tutorials/en/rest/4-authentication-and-permissions.md)

## Additional Components

### reinhardt-forms

Form handling and validation.

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-forms) (available after crates.io publish)
- See `crates/reinhardt-forms/src/lib.rs` for comprehensive forms documentation

### reinhardt-pages

WASM-based frontend framework with SSR (Server-Side Rendering) support and component-based architecture.

**Overview:**

reinhardt-pages is a modern frontend framework that replaces the Tera template system with a WASM-based reactive architecture. Unlike traditional server-side template rendering, reinhardt-pages provides:

- **Hybrid Rendering**: Server-side rendering (SSR) for initial page load + client-side hydration for interactivity
- **Fine-grained Reactivity**: Leptos/Solid.js-style Signal system with automatic dependency tracking
- **Component-based Architecture**: Reusable components instead of template inheritance
- **Django-like API**: Familiar patterns for Reinhardt developers (forms, auth, CSRF, routing)
- **Type Safety**: Full compile-time checking with Rust
- **Security First**: Built-in CSRF protection, XSS prevention, session management

**Key Differences from Tera Templates:**

| Feature | Tera Templates (Old) | reinhardt-pages (New) |
|---------|---------------------|----------------------|
| Rendering | Server-side only | SSR + Client-side hydration |
| Reactivity | None (full page reload) | Fine-grained reactive updates |
| Reusability | Template inheritance (`{% extends %}`) | Component composition |
| Type Safety | Runtime errors | Compile-time checking |
| Interactivity | Requires JavaScript | Built-in with WASM |

**Key Modules:**

#### reactive
Fine-grained reactivity system with automatic dependency tracking.

- `Signal<T>`: Reactive values that trigger updates when changed
- `Effect`: Side effects that run when dependencies change
- `Memo<T>`: Cached computed values

**Example:**
```rust
use reinhardt::pages::reactive::{Signal, Effect};

let count = Signal::new(0);
let doubled = count.map(|n| n * 2);

Effect::new(move || {
    println!("Count: {}", count.get());
});

count.set(5); // Automatically triggers the effect
```

#### component
Component system for building reusable UI elements.

- `Component` trait: Define custom components
- `IntoView`: Convert any type into a renderable view
- `View` enum: Unified representation of DOM elements

**Example:**
```rust
use reinhardt::pages::component::{Component, IntoView, View};
use reinhardt::pages::builder::html::{div, button};

#[component]
fn Counter() -> impl IntoView {
    let count = Signal::new(0);

    div()
        .child(format!("Count: {}", count.get()))
        .child(
            button()
                .text("Increment")
                .on_click(move |_| count.update(|n| *n += 1))
        )
}
```

#### ssr (Server-Side Rendering)
Render components to HTML strings on the server.

- `SsrRenderer`: Renders component trees to HTML
- `SsrOptions`: Configuration for SSR (hydration markers, state serialization)
- `SsrState`: Server-side state that gets serialized for client hydration

**Example:**
```rust
use reinhardt::pages::ssr::{SsrRenderer, SsrOptions};
use reinhardt::pages::component::Component;

let renderer = SsrRenderer::new(SsrOptions {
    include_hydration_markers: true,
    serialize_state: true,
    ..Default::default()
});

let html = renderer.render_page(Counter)?;
// Returns: HTML string with embedded hydration markers and state
```

#### hydration
Client-side hydration to make server-rendered HTML interactive.

- `HydrationContext`: Manages hydration process
- `hydrate()`: Attaches event handlers to SSR HTML

**Example:**
```rust
#[cfg(target_arch = "wasm32")]
use reinhardt::pages::hydration::hydrate;

#[wasm_bindgen(start)]
pub fn main() {
    hydrate(Counter);
}
```

#### server_fn (Server Functions / RPC)
Type-safe RPC calls from client to server.

- `#[server_fn]` macro: Generates client stubs and server handlers
- Automatic CSRF protection
- Session propagation
- Multiple codecs (JSON, URL encoding, MessagePack)

**Example:**
```rust
use reinhardt::pages::server_fn::server_fn;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct User {
    id: i64,
    username: String,
}

#[server_fn]
pub async fn get_user(id: i64) -> Result<User, ServerFnError> {
    // Server-side code (automatic database injection available)
    let user = User::objects().get(id).await?;
    Ok(user)
}

// Client-side usage (WASM):
let user = get_user(42).await?;
```

#### form
Django Form integration for client-side forms.

- `FormBinding`: Two-way binding between forms and Signals
- `FormComponent`: Renders Django `FormMetadata` as interactive HTML
- Automatic CSRF token injection
- Client-side validation

**Example:**
```rust
use reinhardt::pages::form::{FormBinding, FormComponent};
use reinhardt::forms::FormMetadata;

let form_metadata: FormMetadata = get_form_from_server().await?;
let binding = FormBinding::new(form_metadata.clone());

let form_view = FormComponent::new(form_metadata, binding.clone())
    .on_submit(move |data| {
        submit_form(data).await
    });
```

#### csrf
CSRF protection for forms and AJAX requests.

- `CsrfManager`: Reactive CSRF token management
- `get_csrf_token()`: Get current CSRF token
- Automatic injection into forms and AJAX headers

#### auth
Authentication state management.

- `AuthState`: Reactive authentication state (user, permissions)
- `AuthData`: User data and permissions
- Integration with reinhardt-auth

**Example:**
```rust
use reinhardt::pages::auth::auth_state;

let auth = auth_state();
if auth.is_authenticated() {
    println!("User: {}", auth.username());
}
```

#### api
Django QuerySet-like API client for WASM.

- `ApiQuerySet`: Chainable query builder
- `ApiModel` trait: Define models for API access
- Automatic CSRF token injection

**Example:**
```rust
use reinhardt::pages::api::{ApiModel, ApiQuerySet};

let users = User::objects()
    .filter("is_active", true)
    .order_by(&["-created_at"])
    .limit(10)
    .all()
    .await?;
```

#### router
Client-side routing compatible with reinhardt-urls.

- `Router`: Route management
- `Route`: Route definitions with Django-style patterns
- `Link`: Declarative navigation component

**Example:**
```rust
use reinhardt::pages::router::{Router, Route};

let router = Router::new(vec![
    Route::new("/users/{id}/", UserDetail),
    Route::new("/posts/", PostList),
]);
```

**Complete Example: SSR + Hydration + Server Functions:**

```rust
// Shared code (server + client)
use reinhardt::pages::prelude::*;

#[component]
fn App() -> impl IntoView {
    let count = Signal::new(0);

    div()
        .child(h1().text("Counter App"))
        .child(p().text(format!("Count: {}", count.get())))
        .child(
            button()
                .text("Increment")
                .on_click(move |_| count.update(|n| *n += 1))
        )
}

#[server_fn]
async fn increment_on_server(amount: i32) -> Result<i32, ServerFnError> {
    // Server-side logic
    Ok(amount + 1)
}

// Server-side rendering
#[cfg(not(target_arch = "wasm32"))]
fn render_page() -> String {
    let renderer = SsrRenderer::new(SsrOptions::default());
    renderer.render_page(App).unwrap()
}

// Client-side hydration
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn main() {
    hydrate(App);
}
```

**Cargo.toml:**

```toml
[dependencies]
reinhardt-pages = { version = "0.1.0-alpha.1", features = ["pages-full"] }
```

**Feature Flags:**

- `pages-full`: All features enabled (SSR, renderers, components)
- `msgpack`: MessagePack codec for server functions
- `static`: Static file integration

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-pages) (available after crates.io publish)
- [Tutorial: Building Interactive UIs](../tutorials/en/pages/1-getting-started.md) (planned)

### reinhardt-core::backends (cache)

Caching backends (Redis, in-memory).

> **Note**: Cache functionality is provided by `reinhardt-core::backends`.

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-core) (available after crates.io publish)

### reinhardt-auth::sessions

Session management.

> **Note**: Session management functionality is provided by `reinhardt-auth::sessions`.

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-auth) (available after crates.io publish)

### reinhardt-mail

Email utilities.

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-mail) (available after crates.io publish)

### reinhardt-utils::static

Static file serving.

> **Note**: Static file serving functionality is provided by `reinhardt-utils::static`.

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-utils) (available after crates.io publish)

### reinhardt-core::backends (storage)

File storage backends (S3, local).

> **Note**: Storage functionality is provided by `reinhardt-core::backends`.

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-core) (available after crates.io publish)

### reinhardt-websockets

WebSocket support.

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-websockets) (available after crates.io publish)

### reinhardt-graphql

GraphQL schema and resolvers.

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-graphql) (available after crates.io publish)

### reinhardt-i18n

Internationalization and localization.

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-i18n) (available after crates.io publish)

## Configuration

### reinhardt-conf

Configuration system and settings management.

> **Note**: `reinhardt-conf` is a crate that integrates configuration management functionality.
> Previously planned as `reinhardt-settings`, it was integrated into `reinhardt-conf`.

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-conf) (available after crates.io publish)

## Testing

### reinhardt-test

Testing utilities and helpers.

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-test) (available after crates.io publish)
- See `crates/reinhardt-test/src/lib.rs` for comprehensive testing documentation

## Meta Packages

### reinhardt

Main package that re-exports all components based on feature flags.

**Documentation:**

- [Main documentation](https://docs.rs/reinhardt) (available after crates.io publish)
- [Feature Flags Guide](../FEATURE_FLAGS.md)

### reinhardt-micro

Lightweight version for microservices.

**Documentation:**

- [Module documentation](https://docs.rs/reinhardt-micro) (available after crates.io publish)

## Common Patterns

### Error Handling

```rust
use reinhardt::prelude::*;
use reinhardt::http::{Request, Response};
use reinhardt::core::exception::Error;

async fn my_handler() -> Result<Response, Error> {
    let data = fetch_data().await?;
    Ok(Response::json(data))
}
```

### Middleware

```rust
use reinhardt::middleware::Middleware;
use reinhardt::http::{Request, Response};
use async_trait::async_trait;

struct LoggingMiddleware;

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn process_request(&self, request: Request) -> Result<Request, Box<dyn std::error::Error>> {
        println!("Request: {} {}", request.method(), request.uri());
        Ok(request)
    }
}
```

### Custom Validators

```rust
use reinhardt::rest::serializers::{ValidationError, ValidationResult};

fn validate_email(email: &str) -> ValidationResult {
    if !email.contains('@') {
        return Err(vec![ValidationError::new("email", "Invalid email format")]);
    }
    Ok(())
}
```

## Quick Links

- [Getting Started Guide](../GETTING_STARTED.md)
- [Tutorials](../tutorials/README.md)
- [Feature Flags](../FEATURE_FLAGS.md)
- [GitHub Repository](https://github.com/kent8192/reinhardt-web)

## Contributing

Found an error in the documentation? Want to improve it?

- [Report documentation issues](https://github.com/kent8192/reinhardt-web/issues)
- [Contribute to docs](https://github.com/kent8192/reinhardt-web/blob/main/CONTRIBUTING.md)

---

**Note**: This is a high-level overview. Full API documentation will be available at [docs.rs/reinhardt](https://docs.rs/reinhardt) once published to crates.io. In the meantime, comprehensive documentation is available in each crate's `lib.rs` file.
