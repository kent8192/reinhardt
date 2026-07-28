<div align="center">
  <img src="branding/logo.png" alt="Reinhardt Logo" width="200"/>

  <h1>Reinhardt</h1>

  <h3>🦀 Django's productivity, Rust's performance</h3>

  <p><strong>A composable, batteries-included web framework for Rust</strong></p>
  <p>Build with the integrated experience of Django/DRF,<br/>
  or compose only the pieces you need.</p>

[![Crates.io](https://img.shields.io/crates/v/reinhardt-web.svg)](https://crates.io/crates/reinhardt-web)
[![Documentation](https://docs.rs/reinhardt-web/badge.svg)](https://docs.rs/reinhardt-web)
[![License](https://img.shields.io/badge/license-BSD--3--Clause-blue.svg)](LICENSE)
[![codecov](https://codecov.io/gh/kent8192/reinhardt-web/graph/badge.svg)](https://codecov.io/gh/kent8192/reinhardt-web)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/kent8192/reinhardt-web)

</div>

---

## Quick Navigation

- [Who is Reinhardt For?](#who-is-reinhardt-for)
- [Quick Start](#quick-start)
- [Why Reinhardt?](#why-reinhardt)
- [Key Features](#key-features)
- [Installation](#installation)
- [Getting Started Guide](#getting-started-guide)
- [Available Components](#available-components)
- [Ecosystem](#ecosystem)
- [API Stability](#api-stability)

## Who is Reinhardt For?

Reinhardt is designed for developers who:

- **Know Django/DRF** and want the same productivity in Rust
- **Use Axum/Actix** but miss Django's batteries (ORM, admin, auth, DI)
- **Want an integrated Rust web stack** without assembling everything from scratch
- **Want incremental adoption** -- start with just DI or ORM, grow into a full stack later

If you have written `ModelSerializer` or `Depends()` before, Reinhardt will feel like home.

## Quick Start

<!-- reinhardt-version-sync -->
```bash
# Pin the documented Reinhardt release for reproducibility.
# Omit --version to let Cargo choose the latest stable release.
cargo install reinhardt-admin-cli --version "0.3.3"

reinhardt-admin startproject my-api && cd my-api
cargo run --bin manage runserver  # Visit http://127.0.0.1:8000
```

For a full walkthrough, see the [Getting Started Guide](#getting-started-guide).

New to Reinhardt? Start with the default setup first. You can adopt a smaller custom stack later if needed.

## Why Reinhardt?

Rust web development is powerful, but it often starts with choosing and wiring together many separate libraries.

Reinhardt takes a different approach: integrated batteries when you want them, composable parts when you don't.

We call this **polylithic**: many building blocks that still feel like one coherent framework.

Reinhardt brings together the best of four worlds:

| Inspiration        | What We Borrowed                        | What We Improved                           |
|--------------------|-----------------------------------------|--------------------------------------------|
| 🐍 **Django**      | Batteries-included, ORM, admin          | Composable feature flags, type safety      |
| 🎯 **Django REST** | Serializers, ViewSets, permissions      | Compile-time validation, zero-cost         |
| ⚡ **FastAPI**      | DI system, auto OpenAPI                 | Native performance, no runtime overhead    |
| 🗄️ **SQLAlchemy** | QuerySet patterns, relationships        | Type-safe queries, compile-time checks     |

**Result**: A framework that's familiar to Python developers, but with Rust's performance and safety guarantees.

## ✨ Key Features

- **Type-Safe ORM** with compile-time validation (reinhardt-query)
- **Powerful Serializers** with automatic validation (serde + built-in validation)
- **FastAPI-Style DI** with type-safe dependency injection and caching
- **ViewSets** for rapid CRUD API development
- **Multi-Auth** (JWT, Token, Session, Basic) with BaseUser/FullUser traits
- **Admin Panel** with auto-generated model management interface
- **Management Commands** for migrations, static files, and more
- **GraphQL & WebSocket** support for real-time applications
- **Pagination, Filtering, Rate Limiting** built-in
- **Signals** for event-driven architecture

See [Available Components](#available-components) for complete list and [Getting Started](https://reinhardt-web.dev/quickstart/getting-started/) for examples.

## API Stability

Reinhardt follows a **three-phase lifecycle** for every crate:

| Phase | What to Expect |
|-------|---------------|
| **Alpha** (`0.x.0-alpha.N`) | APIs may change freely. Early adopters welcome. |
| **RC** (`0.x.0-rc.N`) | API frozen. Bug fixes only. Safe to build against. |
| **Stable** (`0.x.0`) | Full SemVer 2.0 guarantees. |

<!-- reinhardt-version-sync -->
**Current release line:** Reinhardt documentation tracks `0.3.3`. From
`0.1.0` onward, all public APIs follow SemVer 2.0; future breaking changes
move through the documented alpha and RC lifecycle before stable publication.

For per-release detail (changelog summary, upgrade notes, known issues),
see the [Release category in GitHub Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).
For 0.2.x applications moving to 0.3.0, use the
[0.3 migration guide](instructions/MIGRATION_0.3.md).
The full lifecycle policy lives in
[Stability Policy](instructions/STABILITY_POLICY.md).

## Installation

Reinhardt is a modular framework. Choose your starting point:

> **New here?** Start with the default standard setup. Use `minimal` plus explicit opt-in features for lightweight APIs. `full` remains available as the exhaustive flagship preset, but it is not the recommended starting point for normal applications.

Feature presets are layered. `full`, `standard`, and `minimal` are top-level presets; each preset enables lower-level feature groups, and those groups enable atom feature flags such as `viewset-routing`, `signals`, `image-validation`, `compressed-parsers`, `commands-autoreload`, `browsable-api`, and `openapi-swagger-ui`. External dependencies are attached at the atom feature boundary wherever possible, so choosing `minimal` or `standard` does not implicitly import every dependency carried by `full`.

### Default: Standard Setup (Balanced) ⚠️ Default Preset

Get a well-balanced feature set with zero configuration:

<!-- reinhardt-version-sync -->
```toml
[dependencies]
# Import as 'reinhardt', published as 'reinhardt-web'
# Default enables the "standard" preset (balanced feature set)
reinhardt = { version = "0.3.3", package = "reinhardt-web" }
```

**Includes:** Core, Database (PostgreSQL), REST API (serializers, parsers, pagination, filters, throttling, versioning, metadata, content negotiation), Auth, Middleware (sessions), Pages (WASM Frontend with SSR), Signals

**Opt-in atoms:** OpenAPI generation/UI, browsable API templates, compressed request parsers, static-file compression, and image validation are intentionally outside `standard`. Add `openapi`, `openapi-swagger-ui`, `browsable-api`, `compressed-parsers`, `middleware-compression`, or `image-validation` when an application needs them.

**Binary**: ~20-30 MB | **Compile**: Medium

Then use in your code:
```rust
use reinhardt::prelude::*;
use reinhardt::{Request, Response, StatusCode};
```

### Option 1: Full-Featured (Exhaustive Flagship)

For compatibility checks, framework development, and projects that intentionally need every available component:

<!-- reinhardt-version-sync -->
```toml
[dependencies]
reinhardt = { version = "0.3.3", package = "reinhardt-web", default-features = false, features = ["full"] }
```

**Includes:** Everything in Standard, plus Admin, GraphQL, WebSockets, Cache, i18n, Mail, Static Files, Storage, and more

**Binary**: ~50+ MB | **Compile**: Slowest, because this preset intentionally pulls the complete dependency graph

### Option 2: Microservices (Minimal Setup)

Lightweight and fast, perfect for simple APIs:

<!-- reinhardt-version-sync -->
```toml
[dependencies]
reinhardt = { version = "0.3.3", package = "reinhardt-web", default-features = false, features = ["minimal"] }
```

**Includes:** HTTP, routing, DI, parameter extraction, server

**Binary**: ~5-10 MB | **Compile**: Very fast

### Option 3: Build Your Custom Stack

Install only the components you need:

<!-- reinhardt-version-sync:8 -->
```toml
[dependencies]
# Core components
reinhardt-http = "0.3.3"
reinhardt-urls = "0.3.3"

# Optional: Database
reinhardt-db = "0.3.3"

# Optional: Authentication
reinhardt-auth = "0.3.3"

# Optional: REST API features
reinhardt-rest = "0.3.3"

# Optional: Admin panel
reinhardt-admin = "0.3.3"

# Optional: Advanced features
reinhardt-graphql = "0.3.3"
reinhardt-websockets = "0.3.3"
```

**Note on Crate Naming:**
The main Reinhardt crate is published on crates.io as `reinhardt-web`, but you import it as `reinhardt` in your code using the `package` attribute.

**📖 For a complete list of available crates and feature flags, see the [Feature Flags Guide](https://reinhardt-web.dev/docs/feature-flags/).**

## Getting Started Guide

### 1. Install Reinhardt Admin Tool

Install the CLI with Cargo. The command below pins this guide to the
documented release for reproducibility; omit `--version` to let Cargo choose
the latest stable release. The literal below is release-managed.

<!-- reinhardt-version-sync -->
```bash
cargo install reinhardt-admin-cli --version "0.3.3"
```

### 2. Create a New Project

```bash
# Create a RESTful API project (default)
reinhardt-admin startproject my-api
cd my-api
```

Interactive terminals can choose the Reinhardt version and feature flags
during project creation. Scripts can pass them explicitly:

<!-- reinhardt-version-sync -->
```bash
reinhardt-admin startproject my-api \
  --reinhardt-version "0.3.3" \
  --features standard,admin \
  --no-interactive
```

For an existing project, update the `reinhardt` dependency in `Cargo.toml`:

```bash
reinhardt-admin configure --features minimal,db-sqlite --no-interactive
```

This generates a complete project structure:

```
my-api/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── config.rs
│   ├── apps.rs
│   ├── config/
│   │   ├── settings.rs
│   │   ├── settings/
│   │   │   ├── base.rs
│   │   │   ├── local.rs
│   │   │   ├── staging.rs
│   │   │   └── production.rs
│   │   ├── urls.rs
│   │   └── apps.rs
│   └── bin/
│       └── manage.rs
└── README.md
```

**Alternative: Create a reinhardt-pages Project (WASM + SSR)**

For a modern WASM-based frontend with SSR:

```bash
# Create a pages project
reinhardt-admin startproject my-app --template pages
cd my-app

# Install WASM build tools (first time only)
cargo make install-wasm-tools

# Build WASM and start development server
cargo make dev
# Visit http://127.0.0.1:8000/
```

### 3. Run the Development Server

```bash
# Using the manage command
cargo run --bin manage runserver

# Server will start at http://127.0.0.1:8000
```

**Auto-Reload Support:**

The development server reloads automatically on file changes:

```bash
cargo run --bin manage -- runserver --with-pages
```

Edit any Rust source file (server-side or wasm-side) and the bundle
plus the server are rebuilt in place. Pass `--noreload` to disable
auto-reload entirely, or `--no-wasm-rebuild` to keep server reload
but manage the wasm build yourself. A successful server restart log is
emitted only after the respawned child accepts connections at the
advertised development address.

For build-loop performance work, use `cargo make bench-builds-dry-run`
to inspect the benchmark commands and `cargo make bench-builds` to write
a reproducible report under `docs/build-perf/`. See
[Build Performance](instructions/BUILD_PERFORMANCE.md) for details.
For cross-framework comparisons against Reinhardt, Axum, Actix Web, and Loco,
use `cargo make benchmark-suite-list` to inspect the matrix and
`cargo make benchmark-suite-check` to validate the committed scenario manifests
under `benchmarks/`. Use `cargo make benchmark-suite-measure` to record the
scenario coverage and manifest validation timing for PR evidence. Use
`cargo make benchmark-runtime-http` to run the concrete loopback HTTP runtime
benchmarks for all seven runtime scenarios across all four targets. Use
`cargo make benchmark-matrix-measure` to run the concrete non-runtime
comparison scenarios and write a dated Markdown report under
`benchmarks/results/`. The list/check/dry-run/measure tasks run through the
std-only `benchmark-suite` checker package, while the concrete comparison
framework dependencies live in the separate benchmark package.

### 4. Create Your First App

```bash
# Create a RESTful API app (default)
reinhardt-admin startapp users

# Or explicitly specify type
reinhardt-admin startapp users --with-rest

# Create a Pages app (WASM + SSR)
reinhardt-admin startapp dashboard --with-pages
```

This creates an app structure — the layout depends on the template:

**RESTful app** (`--with-rest`, default):

```
users/
├── lib.rs
├── models.rs
├── models/
├── views.rs
├── views/
├── serializers.rs
├── serializers/
├── admin.rs
├── admin/
├── urls.rs
├── tests.rs
└── tests/
```

**Pages app** (`--with-pages`, WASM + SSR):

Each app owns its server-side and client-side code under `src/apps/<app>/`.
Per Rust 2024 edition module conventions, the entry point for an app is a
sibling `<app>.rs` file next to the `<app>/` directory — there is no inner
`lib.rs`. The same pattern applies to nested aggregators (`client.rs` is
the sibling of `client/`, `urls.rs` is the sibling of `urls/`).

```
src/
├── apps.rs                       # aggregator: pub mod polls; #[cfg(server)] pub use polls::PollsConfig;
├── apps/
│   ├── polls.rs                  # per-app entry (sibling of polls/)
│   └── polls/
│       ├── client.rs             # #[cfg(client)] aggregator: pub mod components;
│       ├── client/
│       │   └── components.rs     # per-app UI (placeholder() returning Page)
│       ├── pages.rs              # target-neutral page entry points
│       ├── server.rs             # #[cfg(server)] aggregator
│       ├── server/
│       │   ├── admin.rs          # admin registration
│       │   ├── models.rs         # models
│       │   ├── serializers.rs    # serializers
│       │   ├── models/           # (.gitkeep — user adds submodules here)
│       │   └── serializers/      # (.gitkeep)
│       ├── server_fn.rs          # bi-target #[server_fn] handlers (placeholder)
│       ├── tests/                # (.gitkeep)
│       ├── urls.rs               # target-neutral server/client router aggregate
│       └── urls/
│           ├── client_router.rs  # client route table and reverse helper
│           └── server_router.rs  # server-function marker registration
├── bin/
│   └── manage.rs                 # native-only management CLI entry
├── client.rs                     # #[cfg(client)] aggregator: pub mod lib; pub mod components;
├── client/
│   ├── components.rs             # cross-app shell: pub mod nav;
│   ├── components/
│   │   └── nav.rs                # with_nav(body: Page) -> Page helper
│   └── lib.rs                    # #[wasm_bindgen(start)] -> ClientLauncher::router_client(...)
├── config.rs                     # cfg-gated config aggregator
├── config/
│   ├── apps.rs                   # installed_apps! { polls: "polls" }
│   ├── settings.rs               # #[cfg(server)] settings
│   ├── urls.rs                   # #[routes(standalone)] entry
│   └── wasm.rs                   # #[cfg(server)] wasm tooling config
├── lib.rs                        # crate root (`pub mod apps;` is un-gated)
├── shared.rs                     # bi-target shared module
└── shared/
    ├── forms.rs                  # shared form definitions
    └── types.rs                  # DTOs exchanged between WASM and server
```

### 5. Register Routes

Edit your app's `urls.rs`. **`urls.rs` plays two roles**: it **declares the URL
submodules** of the app (via `pub mod ...;`) and **aggregates** them into a
single `url_patterns` (or `server_url_patterns` / `unified_url_patterns`) entry
point that `src/config/urls.rs` mounts:

```rust
// users/urls.rs
//
// 1. Module declarations for sub-URL files (optional, for larger apps):
pub mod api;
pub mod views;

// 2. Aggregator — the single entry point mounted from src/config/urls.rs.
use reinhardt::url_patterns;
use reinhardt::ServerRouter;

use crate::config::apps::InstalledApp;

#[url_patterns(InstalledApp::users, mode = server)]
pub fn server_url_patterns() -> ServerRouter {
	ServerRouter::new()
		.endpoint(views::list_users)
		.endpoint(views::get_user)
		.endpoint(views::create_user)
		.mount("/api/v1/", api::routes())
}
```

The `#[url_patterns]` attribute registers this router with the framework for
automatic discovery. For Pages apps, keep the app-level `urls.rs` as the
target-neutral aggregate and put route implementations in
`urls/client_router.rs` and `urls/server_router.rs`; the project-level
`src/config/urls.rs` mounts the aggregate functions.

Include in `src/config/urls.rs`:

```rust
// src/config/urls.rs
use reinhardt::prelude::*;
use reinhardt::routes;

#[routes]
pub fn routes() -> ServerRouter {
	ServerRouter::new()
		.mount("/api/", users::urls::url_patterns())
}
```

The `#[routes]` attribute macro automatically registers this function with the
framework for discovery via the `inventory` crate.

**Note:** The `reinhardt::prelude` includes commonly used types. Key exports include:

**Always Available:**
- HTTP: `StatusCode`

**Feature-Dependent:**
- **`core` feature**: `Request`, `Response`, `Handler`, `Middleware`, Signals (`post_save`, `pre_save`, etc.)
- **`routing` feature**: `Router`, `DefaultRouter`, `ServerRouter`
- **`api`, `standard`, or `api-only` features**: `View`, `ListView`, `DetailView`, `ViewSet`, `ModelViewSet`, `ReadOnlyModelViewSet`
- **`database` feature**: `Model`, `DatabaseConnection`, `F`, `Q`, `Transaction`, `atomic`, Database functions (`Concat`, `Upper`, `Lower`, `Now`, `CurrentDate`), Window functions (`Window`, `RowNumber`, `Rank`, `DenseRank`), Constraints (`UniqueConstraint`, `CheckConstraint`, `ForeignKeyConstraint`)
- **`auth` feature**: `BaseUser`, `FullUser`, `PermissionsMixin`, `BaseUserManager`, `Argon2Hasher`, `GroupManager`, `CreateGroupData`, `Permission`, `ObjectPermission`, `ObjectPermissionManager`
- **`minimal`, `standard`, or `di` features**: `Body`, `Cookie`, `Header`, `Json`, `Path`, `Query`
- **`rest` feature**: Serializers, Parsers, Pagination, Throttling, Versioning
- **`admin` feature**: Admin panel components
- **`cache` feature**: `Cache`, `InMemoryCache`
- **`sessions` feature**: `Session`, `AuthenticationMiddleware`

For a complete list, see [Feature Flags Guide](https://reinhardt-web.dev/docs/feature-flags/).

For a complete step-by-step guide, see [Getting Started](https://reinhardt-web.dev/quickstart/getting-started/).

## 🎓 Learn by Example

### With Database

Configure the database in `settings/base.toml` under `[core.databases.default]`:

```toml
[core]
debug = true
secret_key = "your-secret-key-for-development"

[core.databases.default]
engine = "postgresql"
host = "localhost"
port = 5432
name = "mydb"
user = "postgres"
password = "postgres"

[contacts]
admins = []
managers = []
```

Settings are automatically composed in `src/config/settings.rs` — this is what
`reinhardt-admin startproject` generates:

```rust
// src/config/settings.rs
use reinhardt::prelude::*;

// `CoreSettings` is registered under the `core` section, so its fields —
// including the `[core.databases.default]` connection that `migrate` /
// `runserver` resolve — live under `[core]` in the TOML above.
// `ContactSettings` is mounted under `[contacts]`; management commands
// require it through the `HasCommonSettings` bound.
#[settings(core: CoreSettings | contacts: ContactSettings)]
pub struct ProjectSettings;
```

`#[settings(...)]` composes settings fragments into `ProjectSettings` using the
`key: Type` syntax. Each fragment is a `#[settings(fragment = true, section = "...")]`
struct mounted under its declared section. `CoreSettings` (section `core`) carries
`debug`, `secret_key`, `allowed_hosts`, the `databases` map, and `security`;
`ContactSettings` (section `contacts`) carries the administrator and manager
contact lists needed by the common management-command settings contract. Add
project-specific fragments the same way:

```rust
#[settings(core: CoreSettings | contacts: ContactSettings | cache: CacheSettings)]
pub struct ProjectSettings;
```

See [Settings Documentation](https://reinhardt-web.dev/docs/settings/) for more details.

**Defining a User Model:**

Define your own user model with `#[user(...)]` + `#[model(...)]`. These two
attribute macros cooperate: `#[user]` implements the auth traits (`BaseUser`,
`PermissionsMixin`, `AuthIdentity`, and optionally `FullUser`) on top of a
normal Reinhardt model:

```rust
// users/models.rs
use reinhardt::prelude::*;
use reinhardt::auth::Argon2Hasher;

#[user(hasher = Argon2Hasher, username_field = "username", full = true)]
#[model(app_label = "users", table_name = "users")]
pub struct User {
	#[field(primary_key = true)]
	pub id: Uuid,

	#[field(max_length = 100)]
	pub username: String,

	#[field(max_length = 255)]
	pub email: String,

	pub password_hash: Option<String>,

	#[field(max_length = 150)]
	pub first_name: String,

	#[field(max_length = 150)]
	pub last_name: String,

	#[field(default = true)]
	pub is_active: bool,

	#[field(default = false)]
	pub is_staff: bool,

	#[field(default = false)]
	pub is_superuser: bool,

	pub last_login: Option<DateTime<Utc>>,

	#[field(auto_now_add = true)]
	pub date_joined: DateTime<Utc>,

	// Add custom fields as needed:
	#[field(max_length = 50, null = true)]
	pub phone_number: Option<String>,
}
```

`#[user]` arguments:
- `hasher` (required) — password hasher type (e.g., `Argon2Hasher`)
- `username_field` (required) — name of the field used as the username
- `full = true` (optional) — also implement `FullUser` (email, first_name, last_name, is_staff, date_joined)

**Model Attribute Macro:**

The `#[model(...)]` attribute automatically generates:
- Implementation of the `Model` trait (includes `#[derive(Model)]` functionality)
- Type-safe field accessors: `User::field_email()`, `User::field_username()`, etc.
- Global model registry registration
- Support for composite primary keys

**Note:** When using `#[model(...)]`, you do NOT need to add `#[derive(Model)]` separately,
as it is automatically applied by the `#[model(...)]` attribute.

**Field Attributes:**
- `#[field(primary_key = true)]` - Mark as primary key
- `#[field(max_length = 255)]` - Set maximum length for string fields
- `#[field(default = value)]` - Set default value
- `#[field(auto_now_add = true)]` - Auto-populate timestamp on creation
- `#[field(auto_now = true)]` - Auto-update timestamp on save
- `#[field(null = true)]` - Allow NULL values
- `#[field(unique = true)]` - Enforce uniqueness constraint

For a complete list of field attributes, see the [Field Attributes Guide](https://reinhardt-web.dev/docs/field-attributes/).

The generated field accessors enable type-safe field references in queries:

```rust
// Generated by #[model(...)] for the User struct above:
impl User {
	pub const fn field_id() -> FieldRef<User, Uuid> { FieldRef::new("id") }
	pub const fn field_username() -> FieldRef<User, String> { FieldRef::new("username") }
	pub const fn field_email() -> FieldRef<User, String> { FieldRef::new("email") }
	pub const fn field_is_active() -> FieldRef<User, bool> { FieldRef::new("is_active") }
	pub const fn field_is_staff() -> FieldRef<User, bool> { FieldRef::new("is_staff") }
	pub const fn field_date_joined() -> FieldRef<User, DateTime<Utc>> { FieldRef::new("date_joined") }
	// ... other fields
}
```

**Advanced Query Examples:**

```rust
use reinhardt::prelude::*;
use crate::models::User;

// Django-style lookup helpers with type-safe field references
async fn complex_user_query() -> Result<Vec<User>, Box<dyn std::error::Error>> {
	// Database functions with type-safe field references
	let email_lower = Lower::new(User::field_email().into());
	let username_upper = Upper::new(User::field_username().into());

	// Aggregations using field accessors
	let user_count = Aggregate::count(User::field_id().into());
	let latest_joined = Aggregate::max(User::field_date_joined().into());

	// Window functions for ranking
	let rank_by_join_date = Window::new()
		.partition_by(vec![User::field_is_active().into()])
		.order_by(vec![(User::field_date_joined().into(), "DESC")])
		.function(RowNumber::new());

	// Build and execute the query using QuerySet
	let users = User::objects()
		.filter(User::field_is_active().exact(true))
		.filter(User::field_email().icontains("example.com"))
		.filter(User::field_id().is_in([1_i64, 2, 3]))
		.filter(User::field_date_joined().year().gte(2026))
		.annotate("email_lower", email_lower)
		.annotate("username_upper", username_upper)
		.annotate("rank", rank_by_join_date)
		.order_by(vec![("-date_joined",)])
		.all()
		.await?;

	Ok(users)
}

// Transaction support
async fn create_user_with_transaction(
	conn: &DatabaseConnection,
	user_data: CreateUserRequest
) -> Result<User, Box<dyn std::error::Error>> {
	// Transaction with automatic rollback on error
	transaction(conn, |_tx| async move {
		let user = User::create(user_data).await?;
		log_user_creation(&user).await?;
		Ok(user)
	}).await
}
```

**Note**: Reinhardt uses reinhardt-query for SQL operations. The `#[model(...)]` attribute automatically generates Model trait implementations, type-safe field accessors, and global model registry registration.

Register in `src/config/apps.rs`:

```rust
// src/config/apps.rs
use reinhardt::installed_apps;

// The installed_apps! macro generates:
// - An enum InstalledApp with variants for each app
// - Implementation of conversion traits (From, Into, Display)
// - A registry for app configuration and discovery
//
// Note: Unlike Django's INSTALLED_APPS, this macro is for user apps only.
// Built-in framework features (auth, sessions, admin, etc.) are enabled via
// Cargo feature flags, not through installed_apps!.
//
// Example:
// [dependencies]
// reinhardt = { version = "0.1", features = ["auth", "sessions", "admin"] }
//
// This enables:
// - Automatic app discovery for migrations, admin panel, etc.
// - Type-safe app references throughout your code
// - Centralized app configuration
installed_apps! {
	users: "users",
}

pub fn get_installed_apps() -> Vec<String> {
	InstalledApp::all_apps()
}
```

### With Authentication

Reinhardt provides Django-style user models and permission primitives. You
bring your own user struct (defined with `#[user(...)]` + `#[model(...)]` as
shown in the previous section); the framework layers auth traits, a
password-management workflow, groups, and object-level permissions on top.

**Two entry points for user data:**

| Need | Use |
|------|-----|
| ORM queries on users (filter/annotate/aggregate) | `User::objects()` (from `#[model]`) |
| User lifecycle: create/password-hashing/superuser | A `BaseUserManager<User>` implementation |
| Groups | `GroupManager` |
| Object-level permissions | `ObjectPermissionManager` |

> `User::objects()` is *not* a shortcut to a manager — it is the `QuerySet`
> entry point from the `Model` trait. Manager types
> (`BaseUserManager<User>` for the user lifecycle, `GroupManager`,
> `ObjectPermissionManager`) are constructed directly via `::new()`.

**User lifecycle example:**

For a custom user, implement `BaseUserManager<User>` (see
`reinhardt::auth::BaseUserManager`). The signature required is:

```rust
use std::collections::HashMap;
use async_trait::async_trait;
use reinhardt::auth::{BaseUserManager, Argon2Hasher, PasswordHasher};
use reinhardt::prelude::*;
use serde_json::Value;
use crate::models::User;

pub struct UserManager {
	hasher: Argon2Hasher,
}

impl UserManager {
	pub fn new() -> Self {
		Self { hasher: Argon2Hasher::new() }
	}
}

#[async_trait]
impl BaseUserManager<User> for UserManager {
	async fn create_user(
		&mut self,
		username: &str,
		password: Option<&str>,
		extra: HashMap<String, Value>,
	) -> Result<User, reinhardt::Error> {
		let mut user = User::build()
			.username(username)
			.email("")
			.password_hash(None)
			.first_name("")
			.last_name("")
			.is_active(true)
			.is_staff(false)
			.is_superuser(false)
			.last_login(None)
			.phone_number(None)
			.finish();
		if let Some(pw) = password {
			user.set_password(pw)?;
		}
		// Apply extra fields (email, first_name, …) as needed …
		user.save().await?;
		Ok(user)
	}
}
```

Then:

```rust
let mut users = UserManager::new();
let alice = users
	.create_user(
		"alice",
		Some("secure_password"),
		HashMap::from([("email".into(), serde_json::json!("alice@example.com"))]),
	)
	.await?;
```

**Groups and object-level permissions:**

`GroupManager` and `ObjectPermissionManager` are always available and are
instantiated directly:

```rust
use reinhardt::auth::{GroupManager, CreateGroupData, ObjectPermissionManager};

let mut groups = GroupManager::new();
let editors = groups
	.create_group(CreateGroupData { name: "editors".to_string() })
	.await?;

let mut perms = ObjectPermissionManager::new();
perms.grant_permission("alice", "article:123", "edit").await;
```

Use authentication in your app's `views/profile.rs`:

```rust
// users/views/profile.rs
use reinhardt::{Response, StatusCode, ViewResult, get};
use reinhardt::auth::CurrentUser;
use crate::models::User;

// SessionMiddleware or JwtAuthMiddleware must be registered in urls.rs to
// populate AuthState in request extensions.
#[get("/profile", name = "get_profile")]
pub async fn get_profile(
	#[inject] CurrentUser(user): CurrentUser<User>,
) -> ViewResult<Response> {
	// CurrentUser<U> loads the full user model from the database using the AuthState
	// set by authentication middleware. Returns an injection error if unauthenticated.
	if !user.is_active() {
		return Err("User account is inactive".into());
	}

	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK).with_body(json))
}
```

### Dependency Injection

Reinhardt ships a FastAPI-inspired, async-first dependency injection (DI)
system in the `reinhardt-di` crate. It is type-safe, scope-aware (`singleton` /
`request` / `transient`), composable (dependencies can depend on other
dependencies), and registered at compile time via the
[`inventory`](https://crates.io/crates/inventory) crate so that there is no
runtime reflection or startup discovery cost.

Three primitives drive everyday use:

1. **`Injectable`** — implemented by framework-owned or application-owned
   types that can be injected directly.
2. **`#[injectable]` provider functions** — register async factories whose
   return type is `FactoryOutput<K, T>`.
3. **`#[inject]` with `Depends<K, T>`** — receive a keyed provider output in a
   handler or another provider.

#### 1. Direct `Injectable` types

Application-owned types can implement `Injectable` directly when they are
safe to identify by their own `TypeId`:

```rust
use reinhardt::di::{DiResult, Injectable, InjectionContext};

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
}

#[async_trait::async_trait]
impl Injectable for Config {
    async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
        Ok(Self {
            database_url: "sqlite://app.db".to_string(),
        })
    }
}
```

Direct injection is appropriate for framework-owned extractors and types with
one obvious meaning in the application.

#### 2. `#[injectable]` provider functions

Use a keyed provider when the same value type can have multiple meanings, or
when a foreign type should not be made globally injectable by its own `TypeId`.
The provider returns `FactoryOutput<K, T>`, and consumers request
`Depends<K, T>`:

```rust
use reinhardt::db::DatabaseConnection;
use reinhardt::di::{Depends, FactoryOutput, injectable, injectable_key};

#[injectable_key]
struct PrimaryDatabase;

#[injectable(scope = "singleton")]
async fn database_connection(
    #[inject] config: Config,
) -> FactoryOutput<PrimaryDatabase, DatabaseConnection> {
    let connection = DatabaseConnection::connect(&config.database_url)
        .await
        .expect("failed to open database connection");

    FactoryOutput::new(connection)
}
```

**Every parameter of an `#[injectable]` provider function must be annotated
with `#[inject]`.** There is no way to pass runtime arguments; providers only
compose over other injectables.

When a provider can fail, put the `Result<T, E>` in the `T` position and keep
the provider key as the unique identity:

```rust
use reinhardt::db::DatabaseConnection;
use reinhardt::{Response, StatusCode, ViewResult, get};
use reinhardt::di::{Depends, FactoryOutput, injectable, injectable_key};

#[derive(Debug)]
struct DatabaseConnectionError;

#[injectable_key]
struct DatabaseHealth;

#[injectable(scope = "singleton")]
async fn database_connection_result(
    #[inject] config: Config,
) -> FactoryOutput<DatabaseHealth, Result<DatabaseConnection, DatabaseConnectionError>> {
    FactoryOutput::new(
        DatabaseConnection::connect(&config.database_url)
            .await
            .map_err(|_| DatabaseConnectionError),
    )
}

#[get("/database/health", name = "database_health")]
async fn database_health(
    #[inject] db: Depends<DatabaseHealth, Result<DatabaseConnection, DatabaseConnectionError>>,
) -> ViewResult<Response> {
    match db.as_ref() {
        Ok(_) => Ok(Response::new(StatusCode::OK)),
        Err(_) => Ok(Response::new(StatusCode::SERVICE_UNAVAILABLE)),
    }
}
```

`#[injectable_factory]` remains as a deprecated compatibility alias for
provider functions. New code should use `#[injectable]`.

**The pseudo orphan rule.** To prevent user factories from silently shadowing
framework-owned types (e.g., `reinhardt_di::InjectionContext`, routers,
middleware bindings), Reinhardt validates every registered factory at startup.
If the return type's fully-qualified name begins with a
framework-reserved crate prefix (`reinhardt::`, `reinhardt_di::`, `reinhardt_http::`,
… 37 prefixes total), registration is rejected unless the factory itself lives
inside that crate. This emulates the orphan rule across the DI boundary: foreign
types are fair game, framework types are not. The validator lives in
[`crates/reinhardt-di/src/validation.rs`](crates/reinhardt-di/src/validation.rs)
(`check_framework_type_override`, lines 51–129).

#### 3. `#[inject]` + `Depends<K, T>` in handlers

Use `#[inject]` on a handler parameter to have the DI container resolve it
before the handler runs. Use direct types for ordinary `Injectable`
dependencies, and `Depends<K, T>` for keyed provider output:

```rust
use reinhardt::{get, Response, StatusCode, ViewResult};
use reinhardt::di::{Depends, injectable_key};
use reinhardt::db::DatabaseConnection;
use reinhardt::extractors::Path;
use crate::models::User;

#[injectable_key]
struct PrimaryDatabase;

#[get("/users/{id}/", name = "get_user")]
pub async fn get_user(
    Path(id): Path<i64>,
    #[inject] db: Depends<PrimaryDatabase, DatabaseConnection>,
) -> ViewResult<Response> {
    let user = User::objects().filter(User::field_id().eq(id)).get().await?;
    let body = serde_json::to_string(&user)?;
    Ok(Response::new(StatusCode::OK).with_body(body))
}
```

**Caching.** Within a scope boundary, resolving the same keyed dependency twice
returns the *same* instance. Opt out per-call with `#[inject(cache = false)]`:

```rust
pub async fn uncached_handler(
    #[inject(cache = false)] db: Depends<PrimaryDatabase, DatabaseConnection>,
) -> ViewResult<Response> { /* always a fresh resolution within the scope */ }
```

`#[inject]` wrapper resolution is trait-based rather than name-based. Renamed
imports and aliases of `Depends<K, T>` work, and custom wrappers can implement
`InjectableType` to resolve a registry key while exposing a domain-specific
parameter type:

```rust
use reinhardt::di::{Depends, FactoryOutput, InjectableKey, InjectableType};

struct Lazy<K, T>(Depends<K, T>)
where
    K: InjectableKey,
    T: Send + Sync + 'static;

impl<K, T> InjectableType for Lazy<K, T>
where
    K: InjectableKey,
    T: Send + Sync + 'static,
{
    type Inner = FactoryOutput<K, T>;

    fn from_resolved(
        inner: std::sync::Arc<Self::Inner>,
        use_cache: bool,
    ) -> Self {
        let depends = Depends::from_output(inner, use_cache);
        Self(depends)
    }
}
```

#### Manual `impl Injectable`

When neither macro fits (generic bounds the macro cannot infer, hand-written
builders, conditional registration), implement `Injectable` directly:

```rust
use async_trait::async_trait;
use reinhardt::di::{Injectable, InjectionContext, DiResult};

#[async_trait]
impl Injectable for MyService {
    async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
        Ok(MyService::new())
    }
}
```

For the full DI reference, see the [`reinhardt-di` crate
documentation](https://docs.rs/reinhardt-di).

### Endpoint Definition

Reinhardt uses HTTP method decorators to define endpoints:

#### HTTP Method Decorators

Use `#[get]`, `#[post]`, `#[put]`, `#[delete]` to define routes:

```rust
use reinhardt::{get, post, Response, ViewResult};
use serde_json::json;

#[get("/")]
pub async fn hello() -> ViewResult<Response> {
	Ok(Response::ok().with_body("Hello, World!"))
}

#[post("/users")]
pub async fn create_user() -> ViewResult<Response> {
	let body = json!({"status": "created"});
	Response::ok().with_json(&body).map_err(Into::into)
}
```

### Deriving HTTP Error Responses

Use `#[derive(HttpError)]` for application error enums that map directly to HTTP
responses. `#[http_error(response)]` generates `From<AppError> for Response`.

```rust
use reinhardt::{HttpError, Response};

#[derive(Debug, HttpError)]
#[http_error(response)]
enum AppError {
	#[http_error(status = NOT_FOUND, message = "User not found")]
	UserNotFound,
	#[http_error(status = INTERNAL_SERVER_ERROR, message = "Database unavailable")]
	Database,
}

fn to_response(error: AppError) -> Response {
	Response::from(error)
}
```

The default body mode exposes `detail` for 4xx responses and suppresses 5xx
details. Use `#[http_error(response, body = "error")]` only when every message is
safe to expose.

**Features:**
- Compile-time path validation
- Concise syntax
- Automatic HTTP method binding
- Support for dependency injection via `#[inject]`

#### Using Dependency Injection

Combine HTTP method decorators with `#[inject]` for automatic dependency injection:

```rust
use reinhardt::{get, Response, StatusCode, ViewResult};
use reinhardt::extractors::Path;
use reinhardt::db::DatabaseConnection;
use crate::models::User;

#[get("/users/{id}/", name = "get_user")]
pub async fn get_user(
	Path(id): Path<i64>,
	#[inject] db: DatabaseConnection,
) -> ViewResult<Response> {
	// Path extractor parses and validates the {id} segment automatically
	let user = User::objects()
		.filter(User::field_id().eq(id))
		.get()
		.await?;

	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK).with_body(json))
}
```

**Dependency Injection Features:**
- Automatic dependency injection via `#[inject]` attribute
- Cache control with `#[inject(cache = false)]`
- FastAPI-inspired dependency injection system
- Works seamlessly with HTTP method decorators

**Result Type:**

All view functions use `ViewResult<T>` as the return type:

```rust
use reinhardt::ViewResult;  // Pre-defined result type
```

### With Parameter Extraction

In your app's `views/user.rs`:

```rust
// users/views/user.rs
use reinhardt::{Response, StatusCode, ViewResult, get};
use reinhardt::extractors::{Path, Query};
use reinhardt::db::DatabaseConnection;
use crate::models::User;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct GetUserParams {
	pub include_inactive: Option<bool>,
}

#[get("/users/{id}/", name = "get_user")]
pub async fn get_user(
	Path(id): Path<i64>,
	Query(params): Query<GetUserParams>,
	#[inject] db: DatabaseConnection,
) -> ViewResult<Response> {
	let user = User::objects()
		.filter(User::field_id().eq(id))
		.get()
		.await?;

	if !params.include_inactive.unwrap_or(false) && !user.is_active {
		return Err("User is inactive".into());
	}

	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK).with_body(json))
}
```

Register route with path parameter in `urls.rs`:

```rust
// users/urls.rs
use reinhardt::ServerRouter;

use crate::apps::users::views;

pub fn url_patterns() -> ServerRouter {
	ServerRouter::new()
		.endpoint(views::get_user)  // Path defined in #[get("/users/{id}/")]
}
```

### With Serializers and Validation

In your app's `serializers/user.rs`:

```rust
// users/serializers/user.rs
use serde::{Serialize, Deserialize};
use reinhardt::Validate;

#[derive(Serialize, Deserialize, Validate)]
pub struct CreateUserRequest {
	#[validate(email)]
	pub email: String,
	#[validate(length(min = 3, max = 50))]
	pub username: String,
	#[validate(length(min = 8))]
	pub password: String,
}

#[derive(Serialize, Deserialize)]
pub struct UserResponse {
	pub id: i64,
	pub username: String,
	pub email: String,
	pub is_active: bool,
}

impl From<User> for UserResponse {
	fn from(user: User) -> Self {
		UserResponse {
			id: user.id,
			username: user.username,
			email: user.email,
			is_active: user.is_active,
		}
	}
}
```

In your app's `views/user.rs`:

```rust
// users/views/user.rs
use reinhardt::{Response, StatusCode, ViewResult, post};
use reinhardt::extractors::Json;
use reinhardt::validation::Validated;
use reinhardt::db::DatabaseConnection;
use crate::models::User;
use crate::serializers::{CreateUserRequest, UserResponse};

#[post("/users", name = "create_user")]
pub async fn create_user(
	Json(body): Json<CreateUserRequest>,
	Validated(create_req): Validated<CreateUserRequest>,
	#[inject] db: DatabaseConnection,
) -> ViewResult<Response> {
	// Json<T> deserializes the body; Validated<T> runs #[validate] rules and yields the validated value

	// Create user using the auto-generated builder from #[user] + #[model]
	let mut user = User::build()
		.username(create_req.username)
		.email(create_req.email)
		.password_hash(None)
		.first_name("")
		.last_name("")
		.is_active(true)
		.is_staff(false)
		.is_superuser(false)
		.last_login(None)
		.phone_number(None)
		.finish();

	// Hash and set password using BaseUser trait
	user.set_password(&create_req.password)?;

	// Save to database
	user.save(&db).await?;

	let response_data = UserResponse::from(user);
	let json = serde_json::to_string(&response_data)?;

	Ok(Response::new(StatusCode::CREATED).with_body(json))
}
```

## Adoption Paths

| Your Goal | Start Here |
|-----------|-----------|
| **Full-stack REST API** | [Getting Started Guide](#getting-started-guide) |
| **Full-stack with Pages (WASM + SSR)** | [Basis Tutorial](examples/examples-tutorial-basis/) |
| **Lightweight DI-focused API** | [Minimal Installation](#option-2-microservices-minimal-setup) |

> **Standalone DI for existing Axum apps** is planned for a future release.
> See [Discussions](https://github.com/kent8192/reinhardt-web/discussions) for updates.

## Available Components

Reinhardt offers modular components you can mix and match:

| Component           | Crate Name                | Features                                    |
|---------------------|---------------------------|---------------------------------------------|
| **Core**            |                           |                                             |
| Core Types          | `reinhardt-core`          | Core traits, types, macros (Model, endpoint)|
| HTTP & Routing      | `reinhardt-http`          | Request/Response, HTTP handling             |
| URL Routing         | `reinhardt-urls`          | Function-based and class-based routes       |
| Server              | `reinhardt-server`        | HTTP server implementation                  |
| Dispatch            | `reinhardt-dispatch`      | HTTP request dispatching, handler composition |
| Configuration       | `reinhardt-conf`          | Settings management, environment loading    |
| Commands            | `reinhardt-commands`      | Management CLI tools (startproject, etc.)   |
| Shortcuts           | `reinhardt-shortcuts`     | Common utility functions                    |
| **Database**        |                           |                                             |
| ORM                 | `reinhardt-db`            | reinhardt-query integration                |
| **Authentication**  |                           |                                             |
| Auth                | `reinhardt-auth`          | JWT, Token, Session, Basic auth, User models|
| **REST API**        |                           |                                             |
| Serializers         | `reinhardt-rest`          | built-in serialization and validation, ViewSets |
| **Forms**           |                           |                                             |
| Forms               | `reinhardt-forms`         | Form handling and validation                |
| **Advanced**        |                           |                                             |
| Admin Panel         | `reinhardt-admin`         | Django-style admin interface                |
| Plugin System       | `reinhardt-dentdelion`    | Static & WASM plugin support, CLI management|
| Background Tasks    | `reinhardt-tasks`         | Task queues (Redis, RabbitMQ, SQLite)       |
| GraphQL             | `reinhardt-graphql`       | Schema generation, subscriptions            |
| WebSockets          | `reinhardt-websockets`    | Real-time communication                     |
| i18n                | `reinhardt-i18n`          | Multi-language support                      |
| Mail                | `reinhardt-mail`          | Email sending and templating                |
| gRPC                | `reinhardt-grpc`          | gRPC services, protobuf types               |
| Deep Link           | `reinhardt-deeplink`      | iOS Universal Links, Android App Links      |
| **Middleware**       |                           |                                             |
| Middleware          | `reinhardt-middleware`    | HTTP middleware components, CORS, security  |
| **Testing**         |                           |                                             |
| Test Utilities      | `reinhardt-test`          | Testing helpers, fixtures, TestContainers   |
| Test Kit            | `reinhardt-testkit`       | Higher-level test abstractions and utilities|

**For detailed feature flags within each crate, see the [Feature Flags Guide](https://reinhardt-web.dev/docs/feature-flags/).**

---

## Ecosystem

| Project | Status | Description |
|---------|--------|-------------|
| [reinhardt-cloud](https://github.com/kent8192/reinhardt-cloud) | WIP | Kubernetes operator & CLI for deploying Reinhardt apps |

> **Dog-fooding in progress:** We are actively developing reinhardt-cloud as the deployment infrastructure for Reinhardt applications, and using it to deploy reinhardt-web itself. As a work-in-progress project, APIs and features may change significantly.

---

## Documentation

- 📚 [Getting Started Guide](https://reinhardt-web.dev/quickstart/getting-started/) - Step-by-step tutorial for beginners
- 🎛️ [Feature Flags Guide](https://reinhardt-web.dev/docs/feature-flags/) - Optimize your build with granular feature control
- 📖 [API Reference](https://docs.rs/reinhardt-web)
- 📝 [Tutorials](https://reinhardt-web.dev/quickstart/tutorials/) - Learn by building real applications

For contributor automation and repository-specific development conventions,
see [CLAUDE.md](CLAUDE.md).

## 💬 Getting Help

Reinhardt is a community-driven project. Here's where you can get help:

- 💭 **GitHub Discussions**: [Ask questions and share ideas](https://github.com/kent8192/reinhardt-web/discussions)
- 🐛 **Issues**: [Report bugs](https://github.com/kent8192/reinhardt-web/issues)
- 📖 **Documentation**: [Read the guides](docs/)

Before asking, please check:

- ✅ [Getting Started Guide](https://reinhardt-web.dev/quickstart/getting-started/)
- ✅ [Examples](examples/)
- ✅ Existing GitHub Issues and Discussions

## 🤝 Contributing

We love contributions! Please read our [Contributing Guide](CONTRIBUTING.md) to get started.

**Quick links**:

- [Development Setup](CONTRIBUTING.md#development-setup)
- [Testing Guidelines](CONTRIBUTING.md#testing-guidelines)
- [Commit Guidelines](CONTRIBUTING.md#commit-guidelines)

## ⭐ Star History

<a href="https://star-history.com/#kent8192/reinhardt-web&Date">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date&theme=dark" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date" />
   <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date" width="600" />
 </picture>
</a>

## Copyright

Copyright © 2026 Tachyon Inc. All rights reserved.

Developed by Tachyon Inc.

## License

This project is licensed under the [BSD 3-Clause License](LICENSE).

### Third-Party Attribution

This project is inspired by:

- [Django](https://www.djangoproject.com/) (BSD 3-Clause License)
- [Django REST Framework](https://www.django-rest-framework.org/) (BSD 3-Clause License)
- [FastAPI](https://fastapi.tiangolo.com/) (MIT License)
- [SQLAlchemy](https://www.sqlalchemy.org/) (MIT License)

See [THIRD-PARTY-NOTICES](THIRD-PARTY-NOTICES) for full attribution.

**Note:** This project is not affiliated with or endorsed by the Django Software Foundation, Encode OSS Ltd., Sebastián Ramírez (FastAPI author), or Michael Bayer (SQLAlchemy author).
