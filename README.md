<div align="center">
  <img src="branding/logo.png" alt="Reinhardt Logo" width="200"/>

  <h1>Reinhardt</h1>

  <h3>🦀 Polylithic Batteries Included</h3>

  <p><strong>A composable full-stack API framework for Rust</strong></p>
  <p>Build with <em>all</em> the power of Django's batteries-included philosophy,<br/>
  or compose <em>only</em> what you need—your choice, your way.</p>

[![Crates.io](https://img.shields.io/crates/v/reinhardt-web.svg)](https://crates.io/crates/reinhardt-web)
[![Documentation](https://docs.rs/reinhardt-web/badge.svg)](https://docs.rs/reinhardt-web)
[![License](https://img.shields.io/badge/license-BSD--3--Clause-blue.svg)](LICENSE)
[![codecov](https://codecov.io/gh/kent8192/reinhardt-web/graph/badge.svg)](https://codecov.io/gh/kent8192/reinhardt-web)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/kent8192/reinhardt-web)

🌐 **English** | [日本語](docs/readme-langs/README_JA.md) | [简体中文](docs/readme-langs/README_ZH_CN.md) | [繁體中文](docs/readme-langs/README_ZH_TW.md) | [Русский](docs/readme-langs/README_RU.md) | [Українська](docs/readme-langs/README_UK.md) | [فارسی](docs/readme-langs/README_FA.md) | [العربية](docs/readme-langs/README_AR.md)

</div>

---

## 📍 Quick Navigation

You may be looking for:

- 🌐 [Official Website](https://reinhardt-web.dev) - Documentation, tutorials, and guides
- 🚀 [Quick Start](#quick-start) - Get up and running in 5 minutes
- 📦 [Installation Options](#installation) - Choose your flavor: Micro, Standard, or Full
- 📚 [Getting Started Guide](https://reinhardt-web.dev/quickstart/getting-started/) - Step-by-step tutorial
- 🎛️ [Feature Flags](https://reinhardt-web.dev/docs/feature-flags/) - Fine-tune your build
- 📖 [API Documentation](https://docs.rs/reinhardt-web) - Complete API reference
- 💬 [Community & Support](#getting-help) - Get help from the community

## Why Reinhardt?

**Polylithic = Poly (many) + Lithic (building blocks)**
Unlike monolithic frameworks that force you to use everything, Reinhardt lets you compose your perfect stack from independent, well-tested components.

Reinhardt brings together the best of three worlds:

| Inspiration        | What We Borrowed                                       | What We Improved                                     |
|--------------------|--------------------------------------------------------|------------------------------------------------------|
| 🐍 **Django**      | Batteries-included philosophy, ORM design, admin panel | Feature flags for composable builds, Rust's type safety |
| 🎯 **Django REST** | Serializers, ViewSets, permissions                     | Compile-time validation, zero-cost abstractions      |
| ⚡ **FastAPI**      | DI system, automatic OpenAPI                           | Native Rust performance, no runtime overhead         |
| 🗄️ **SQLAlchemy** | QuerySet patterns, relationship handling               | Type-safe query builder, compile-time validation     |

**Result**: A framework that's familiar to Python developers, but with Rust's performance and safety guarantees.

## ✨ Key Features

- **Type-Safe ORM** with compile-time validation (reinhardt-query)
- **Powerful Serializers** with automatic validation (serde + validator)
- **FastAPI-Style DI** with type-safe dependency injection and caching
- **ViewSets** for rapid CRUD API development
- **Multi-Auth** (JWT, Token, Session, Basic) with BaseUser/FullUser traits
- **Admin Panel** with auto-generated model management interface
- **Management Commands** for migrations, static files, and more
- **GraphQL & WebSocket** support for real-time applications
- **Pagination, Filtering, Rate Limiting** built-in
- **Signals** for event-driven architecture

See [Available Components](#available-components) for complete list and [Getting Started](https://reinhardt-web.dev/quickstart/getting-started/) for examples.

## Installation

Reinhardt is a modular framework. Choose your starting point:

**Note on Crate Naming:**
The main Reinhardt crate is published on crates.io as `reinhardt-web`, but you import it as `reinhardt` in your code using the `package` attribute.

### Default: Full-Featured (Batteries Included) ⚠️ New Default

Get all features with zero configuration:

```toml
[dependencies]
# Import as 'reinhardt', published as 'reinhardt-web'
# Default enables ALL features (full bundle)
reinhardt = { version = "0.1.0-rc.1", package = "reinhardt-web" }
```

**Includes:** Database, Auth, REST API, Admin, GraphQL, WebSockets, Cache, i18n, Mail, Sessions, Static Files, Storage

**Binary**: ~50+ MB | **Compile**: Slower, but everything works out of the box

Then use in your code:
```rust
use reinhardt::prelude::*;
use reinhardt::{Request, Response, StatusCode};
```

### Option 1: Standard Setup (Balanced)

For most projects that don't need all features:

```toml
[dependencies]
reinhardt = { version = "0.1.0-rc.1", package = "reinhardt-web", default-features = false, features = ["standard"] }
```

**Includes:** Core, Database (PostgreSQL), REST API, Auth, Middleware, Pages (WASM Frontend with SSR)

**Binary**: ~20-30 MB | **Compile**: Medium

### Option 2: Microservices (Minimal Setup)

Lightweight and fast, perfect for simple APIs:

```toml
[dependencies]
reinhardt = { version = "0.1.0-rc.1", package = "reinhardt-web", default-features = false, features = ["minimal"] }
```

**Includes:** HTTP, routing, DI, parameter extraction, server

**Binary**: ~5-10 MB | **Compile**: Very fast

### Option 3: Build Your Custom Stack

Install only the components you need:

```toml
[dependencies]
# Core components
reinhardt-http = "0.1.0-rc.1"
reinhardt-urls = "0.1.0-rc.1"

# Optional: Database
reinhardt-db = "0.1.0-rc.1"

# Optional: Authentication
reinhardt-auth = "0.1.0-rc.1"

# Optional: REST API features
reinhardt-rest = "0.1.0-rc.1"

# Optional: Admin panel
reinhardt-admin = "0.1.0-rc.1"

# Optional: Advanced features
reinhardt-graphql = "0.1.0-rc.1"
reinhardt-websockets = "0.1.0-rc.1"
```

**📖 For a complete list of available crates and feature flags, see the [Feature Flags Guide](https://reinhardt-web.dev/docs/feature-flags/).**

## Quick Start

### 1. Install Reinhardt Admin Tool

```bash
cargo install reinhardt-admin-cli
```

### 2. Create a New Project

```bash
# Create a RESTful API project (default)
reinhardt-admin startproject my-api
cd my-api
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
reinhardt-admin startproject my-app --with-pages
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

For automatic reloading on code changes (requires bacon):

```bash
# Install bacon
cargo install --locked bacon

# Run with auto-reload
bacon runserver

# Or use cargo make
cargo make watch

# For tests
bacon test
```

### 4. Create Your First App

```bash
# Create a RESTful API app (default)
cargo run --bin manage startapp users

# Or explicitly specify type
cargo run --bin manage startapp users --restful

# Create a Pages app (WASM + SSR)
cargo run --bin manage startapp dashboard --with-pages
```

This creates an app structure:

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
├── urls.rs
└── tests.rs
```

### 5. Register Routes

Edit your app's `urls.rs`:

```rust
// users/urls.rs
use reinhardt::ServerRouter;

use super::views;

pub fn url_patterns() -> ServerRouter {
	ServerRouter::new()
		.endpoint(views::list_users)
		.endpoint(views::get_user)
		.endpoint(views::create_user)
}
```

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
- Core routing and views: `Router`, `DefaultRouter`, `ServerRouter`, `View`, `ListView`, `DetailView`
- ViewSets: `ViewSet`, `ModelViewSet`, `ReadOnlyModelViewSet`
- HTTP: `StatusCode`

**Feature-Dependent:**
- **`core` feature**: `Request`, `Response`, `Handler`, `Middleware`, Signals (`post_save`, `pre_save`, etc.)
- **`database` feature**: `Model`, `DatabaseConnection`, `F`, `Q`, `Transaction`, `atomic`, Database functions (`Concat`, `Upper`, `Lower`, `Now`, `CurrentDate`), Window functions (`Window`, `RowNumber`, `Rank`, `DenseRank`), Constraints (`UniqueConstraint`, `CheckConstraint`, `ForeignKeyConstraint`)
- **`auth` feature**: `User`, `UserManager`, `GroupManager`, `Permission`, `ObjectPermission`
- **`minimal`, `standard`, or `di` features**: `Body`, `Cookie`, `Header`, `Json`, `Path`, `Query`
- **`rest` feature**: Serializers, Parsers, Pagination, Throttling, Versioning
- **`admin` feature**: Admin panel components
- **`cache` feature**: `Cache`, `InMemoryCache`
- **`sessions` feature**: `Session`, `AuthenticationMiddleware`

For a complete list, see [Feature Flags Guide](https://reinhardt-web.dev/docs/feature-flags/).

For a complete step-by-step guide, see [Getting Started](https://reinhardt-web.dev/quickstart/getting-started/).

## 🎓 Learn by Example

### With Database

Configure database in `settings/base.toml`:

```toml
debug = true
secret_key = "your-secret-key-for-development"

[database]
engine = "postgresql"
host = "localhost"
port = 5432
name = "mydb"
user = "postgres"
password = "postgres"
```

Settings are automatically loaded in `src/config/settings.rs`:

```rust
// src/config/settings.rs
use reinhardt::conf::settings::builder::SettingsBuilder;
use reinhardt::conf::settings::profile::Profile;
use reinhardt::conf::settings::sources::{DefaultSource, LowPriorityEnvSource, TomlFileSource};
use reinhardt::core::Settings;
use std::env;
use std::path::PathBuf;
use std::str::FromStr;

pub fn get_settings() -> Settings {
	let profile_str = env::var("REINHARDT_ENV").unwrap_or_else(|_| "local".to_string());
	let profile = Profile::from_str(&profile_str).unwrap_or(Profile::Development);

	let base_dir = env::current_dir().expect("Failed to get current directory");
	let settings_dir = base_dir.join("settings");

	let merged = SettingsBuilder::new()
		.profile(profile)
		.add_source(
			DefaultSource::new()
				.with_value("debug", serde_json::Value::Bool(false))
				.with_value("language_code", serde_json::Value::String("en-us".to_string()))
				.with_value("time_zone", serde_json::Value::String("UTC".to_string()))
		)
		.add_source(LowPriorityEnvSource::new().with_prefix("REINHARDT_"))
		.add_source(TomlFileSource::new(settings_dir.join("base.toml")))
		.add_source(TomlFileSource::new(settings_dir.join(format!("{}.toml", profile_str))))
		.build()
		.expect("Failed to build settings");

	merged.into_typed().expect("Failed to convert settings to Settings struct")
}
```

**Environment Variable Sources:**

Reinhardt provides two types of environment variable sources with different priorities:

- **`EnvSource`** (priority: 100) - High priority environment variables that override TOML files
  ```rust
  .add_source(EnvSource::new().with_prefix("REINHARDT_"))
  ```

- **`LowPriorityEnvSource`** (priority: 40) - Low priority environment variables that fall back to TOML files
  ```rust
  .add_source(LowPriorityEnvSource::new().with_prefix("REINHARDT_"))
  ```

**Priority Order**:
- Using `EnvSource`: Environment Variables > `{profile}.toml` > `base.toml` > Defaults
- Using `LowPriorityEnvSource` (shown above): `{profile}.toml` > `base.toml` > Environment Variables > Defaults

Choose `EnvSource` when environment variables should always take precedence (e.g., production deployments).
Choose `LowPriorityEnvSource` when TOML files should be the primary configuration source (e.g., development).

See [Settings Documentation](docs/SETTINGS_DOCUMENT.md) for more details.

**Using the Built-in DefaultUser:**

Reinhardt provides a ready-to-use `DefaultUser` implementation (requires `argon2-hasher` feature):

```rust
// users/models.rs
use reinhardt::prelude::*;
use reinhardt::DefaultUser;

// Re-export DefaultUser as User for your app
pub type User = DefaultUser;

// DefaultUser includes:
// - id: Uuid (primary key)
// - username: String
// - email: String
// - password_hash: Option<String>
// - first_name: String
// - last_name: String
// - is_active: bool
// - is_staff: bool
// - is_superuser: bool
// - last_login: Option<DateTime<Utc>>
// - date_joined: DateTime<Utc>

// DefaultUser implements:
// - BaseUser trait (authentication methods)
// - FullUser trait (full user information)
// - PermissionsMixin trait (permission management)
// - Model trait (database operations)
```

**Defining Custom User Models:**

If you need custom fields, define your own model:

```rust
// users/models.rs
use reinhardt::prelude::*;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

#[model(app_label = "users", table_name = "users")]
pub struct CustomUser {
	#[field(primary_key = true)]
	pub id: i64,

	#[field(max_length = 255)]
	pub email: String,

	#[field(max_length = 100)]
	pub username: String,

	#[field(default = true)]
	pub is_active: bool,

	#[field(auto_now_add = true)]
	pub created_at: DateTime<Utc>,

	// Add custom fields
	#[field(max_length = 50, null = true)]
	pub phone_number: Option<String>,
}
```

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

For a complete list of field attributes, see the [Field Attributes Guide](docs/field_attributes.md).

The generated field accessors enable type-safe field references in queries:

```rust
// Generated by #[model(...)] for DefaultUser
impl DefaultUser {
	pub const fn field_id() -> FieldRef<DefaultUser, Uuid> { FieldRef::new("id") }
	pub const fn field_username() -> FieldRef<DefaultUser, String> { FieldRef::new("username") }
	pub const fn field_email() -> FieldRef<DefaultUser, String> { FieldRef::new("email") }
	pub const fn field_is_active() -> FieldRef<DefaultUser, bool> { FieldRef::new("is_active") }
	pub const fn field_is_staff() -> FieldRef<DefaultUser, bool> { FieldRef::new("is_staff") }
	pub const fn field_date_joined() -> FieldRef<DefaultUser, DateTime<Utc>> { FieldRef::new("date_joined") }
	// ... other fields
}
```

**Advanced Query Examples:**

```rust
use reinhardt::prelude::*;
use reinhardt::DefaultUser;

// Django-style F/Q object queries with type-safe field references
async fn complex_user_query() -> Result<Vec<DefaultUser>, Box<dyn std::error::Error>> {
	// Q objects with type-safe field references (using generated field accessors)
	let active_query = Q::new()
		.field("is_active").eq(true)
		.and(Q::new().field("date_joined").gte(Now::new()));

	// Database functions with type-safe field references
	let email_lower = Lower::new(DefaultUser::field_email().into());
	let username_upper = Upper::new(DefaultUser::field_username().into());

	// Aggregations using field accessors
	let user_count = Aggregate::count(DefaultUser::field_id().into());
	let latest_joined = Aggregate::max(DefaultUser::field_date_joined().into());

	// Window functions for ranking
	let rank_by_join_date = Window::new()
		.partition_by(vec![DefaultUser::field_is_active().into()])
		.order_by(vec![(DefaultUser::field_date_joined().into(), "DESC")])
		.function(RowNumber::new());

	todo!("Execute query with these components")
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

**Note**: Reinhardt uses reinhardt-query for SQL operations. The `#[derive(Model)]` macro automatically generates Model trait implementations, type-safe field accessors, and global model registry registration.

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

Reinhardt provides Django-style user models with `BaseUser` and `FullUser` traits, along with comprehensive user management through `UserManager`.

**Note:** Reinhardt includes a built-in `DefaultUser` implementation. You can use it directly or define your own user model as shown below.

**User Management Example:**

```rust
use reinhardt::prelude::*;

// Create and manage users with UserManager
async fn manage_users() -> Result<(), Box<dyn std::error::Error>> {
	let hasher = Argon2Hasher::new();
	let user_manager = UserManager::new(hasher);

	// Create a new user
	let user = user_manager.create_user(CreateUserData {
		username: "alice".to_string(),
		email: "alice@example.com".to_string(),
		password: "secure_password".to_string(),
		first_name: Some("Alice".to_string()),
		last_name: Some("Smith".to_string()),
	}).await?;

	// Update user information
	user_manager.update_user(user.id, UpdateUserData {
		email: Some("alice.smith@example.com".to_string()),
		is_active: Some(true),
		..Default::default()
	}).await?;

	// Manage groups and permissions
	let group_manager = GroupManager::new();
	let editors = group_manager.create_group(CreateGroupData {
		name: "editors".to_string(),
	}).await?;

	// Assign object-level permissions
	let permission = ObjectPermission::new("edit", user.id, article.id);
	let perm_checker = ObjectPermissionChecker::new();
	if perm_checker.has_permission(&user, "edit", &article).await? {
		// User can edit the article
	}

	Ok(())
}
```

Use the built-in `DefaultUser` in `users/models.rs`:

```rust
// users/models.rs
use reinhardt::DefaultUser;

// Re-export DefaultUser as your User type
pub type User = DefaultUser;

// DefaultUser already implements:
// - BaseUser trait (authentication methods)
// - FullUser trait (username, email, first_name, last_name, etc.)
// - PermissionsMixin trait (permission management)
// - Model trait (database operations)
```

**For Custom User Models:**

If you need additional fields beyond DefaultUser, define your own:

```rust
// users/models.rs
use reinhardt::auth::{BaseUser, FullUser, PermissionsMixin};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

#[model(app_label = "users", table_name = "users")]
pub struct CustomUser {
	#[field(primary_key = true)]
	pub id: Uuid,

	#[field(max_length = 150)]
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

	// Custom fields
	#[field(max_length = 20, null = true)]
	pub phone_number: Option<String>,
}

impl BaseUser for CustomUser {
	type PrimaryKey = Uuid;

	fn get_username_field() -> &'static str { "username" }
	fn get_username(&self) -> &str { &self.username }
	fn password_hash(&self) -> Option<&str> { self.password_hash.as_deref() }
	fn set_password_hash(&mut self, hash: String) { self.password_hash = Some(hash); }
	fn last_login(&self) -> Option<DateTime<Utc>> { self.last_login }
	fn set_last_login(&mut self, time: DateTime<Utc>) { self.last_login = Some(time); }
	fn is_active(&self) -> bool { self.is_active }
}

impl FullUser for CustomUser {
	fn username(&self) -> &str { &self.username }
	fn email(&self) -> &str { &self.email }
	fn first_name(&self) -> &str { &self.first_name }
	fn last_name(&self) -> &str { &self.last_name }
	fn is_staff(&self) -> bool { self.is_staff }
	fn is_superuser(&self) -> bool { self.is_superuser }
	fn date_joined(&self) -> DateTime<Utc> { self.date_joined }
}
```

Use JWT authentication in your app's `views/profile.rs`:

```rust
// users/views/profile.rs
use reinhardt::auth::{JwtAuth, BaseUser};
use reinhardt::{Request, Response, StatusCode, ViewResult, get};
use reinhardt::db::DatabaseConnection;
use std::sync::Arc;
use crate::models::User;

#[get("/profile", name = "get_profile")]
pub async fn get_profile(
	req: Request,
	#[inject] db: Arc<DatabaseConnection>,
) -> ViewResult<Response> {
	// Extract JWT token from Authorization header
	let auth_header = req.headers.get("authorization")
		.and_then(|h| h.to_str().ok())
		.ok_or("Missing Authorization header")?;

	let token = auth_header.strip_prefix("Bearer ")
		.ok_or("Invalid Authorization header format")?;

	// Verify token and get user ID
	let jwt_auth = JwtAuth::new(b"your-secret-key");
	let claims = jwt_auth.verify_token(token)?;

	// Load user from database using claims.user_id
	let user = User::find_by_id(&db, &claims.user_id).await?;

	// Check if user is active
	if !user.is_active() {
		return Err("User account is inactive".into());
	}

	// Return user profile as JSON
	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
}
```

### Endpoint Definition

Reinhardt uses HTTP method decorators to define endpoints:

#### HTTP Method Decorators

Use `#[get]`, `#[post]`, `#[put]`, `#[delete]` to define routes:

```rust
use reinhardt::{get, post, Request, Response, ViewResult};
use serde_json::json;

#[get("/")]
pub async fn hello(_req: Request) -> ViewResult<Response> {
	Ok(Response::ok().with_body("Hello, World!"))
}

#[post("/users")]
pub async fn create_user(_req: Request) -> ViewResult<Response> {
	let body = json!({"status": "created"});
	Response::ok().with_json(&body).map_err(Into::into)
}
```

**Features:**
- Compile-time path validation
- Concise syntax
- Automatic HTTP method binding
- Support for dependency injection via `#[inject]`

#### Using Dependency Injection

Combine HTTP method decorators with `#[inject]` for automatic dependency injection:

```rust
use reinhardt::{get, Request, Response, StatusCode, ViewResult};
use reinhardt::db::DatabaseConnection;
use std::sync::Arc;

#[get("/users/{id}/", name = "get_user")]
pub async fn get_user(
	req: Request,
	#[inject] db: Arc<DatabaseConnection>,  // Automatically injected
) -> ViewResult<Response> {
	let id = req.path_params.get("id")
		.ok_or("Missing id")?
		.parse::<i64>()?;

	// Use injected database connection
	let user = db.query("SELECT * FROM users WHERE id = $1")
		.bind(id)
		.fetch_one()
		.await?;

	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
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
use reinhardt::{Request, Response, StatusCode, ViewResult, get};
use reinhardt::db::DatabaseConnection;
use crate::models::User;
use std::sync::Arc;

#[get("/users/{id}/", name = "get_user")]
pub async fn get_user(
	req: Request,
	#[inject] db: Arc<DatabaseConnection>,
) -> ViewResult<Response> {
	// Extract path parameter from request
	let id = req.path_params.get("id")
		.ok_or("Missing id parameter")?
		.parse::<i64>()
		.map_err(|_| "Invalid id format")?;

	// Extract query parameters (e.g., ?include_inactive=true)
	let include_inactive = req.query_params.get("include_inactive")
		.and_then(|v| v.parse::<bool>().ok())
		.unwrap_or(false);

	// Fetch user from database using injected connection
	let user = User::find_by_id(&db, id).await?;

	// Check active status if needed
	if !include_inactive && !user.is_active {
		return Err("User is inactive".into());
	}

	// Return as JSON
	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
}
```

Register route with path parameter in `urls.rs`:

```rust
// users/urls.rs
use reinhardt::ServerRouter;

use super::views;

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
use validator::Validate;

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
use reinhardt::{Request, Response, StatusCode, ViewResult, post};
use reinhardt::db::DatabaseConnection;
use crate::models::User;
use crate::serializers::{CreateUserRequest, UserResponse};
use validator::Validate;
use std::sync::Arc;

#[post("/users", name = "create_user")]
pub async fn create_user(
	mut req: Request,
	#[inject] db: Arc<DatabaseConnection>,
) -> ViewResult<Response> {
	// Parse request body
	let body_bytes = std::mem::take(&mut req.body);
	let create_req: CreateUserRequest = serde_json::from_slice(&body_bytes)?;

	// Validate request
	create_req.validate()?;

	// Create user
	let mut user = User {
		id: 0, // Will be set by database
		username: create_req.username,
		email: create_req.email,
		password_hash: None,
		is_active: true,
		created_at: Utc::now(),
	};

	// Hash password using BaseUser trait
	user.set_password(&create_req.password)?;

	// Save to database using injected connection
	user.save(&db).await?;

	// Convert to response
	let response_data = UserResponse::from(user);
	let json = serde_json::to_string(&response_data)?;

	Ok(Response::new(StatusCode::CREATED)
		.with_body(json))
}
```

## Available Components

Reinhardt offers modular components you can mix and match:

| Component           | Crate Name                | Features                                    |
|---------------------|---------------------------|---------------------------------------------|
| **Core**            |                           |                                             |
| Core Types          | `reinhardt-core`          | Core traits, types, macros (Model, endpoint)|
| HTTP & Routing      | `reinhardt-http`          | Request/Response, HTTP handling             |
| URL Routing         | `reinhardt-urls`          | Function-based and class-based routes       |
| Server              | `reinhardt-server`        | HTTP server implementation                  |
| Middleware          | `reinhardt-dispatch`      | Middleware chain, signal dispatch           |
| Configuration       | `reinhardt-conf`          | Settings management, environment loading    |
| Commands            | `reinhardt-commands`      | Management CLI tools (startproject, etc.)   |
| Shortcuts           | `reinhardt-shortcuts`     | Common utility functions                    |
| **Database**        |                           |                                             |
| ORM                 | `reinhardt-db`            | reinhardt-query integration                |
| **Authentication**  |                           |                                             |
| Auth                | `reinhardt-auth`          | JWT, Token, Session, Basic auth, User models|
| **REST API**        |                           |                                             |
| Serializers         | `reinhardt-rest`          | serde/validator integration, ViewSets       |
| **Forms**           |                           |                                             |
| Forms               | `reinhardt-forms`         | Form handling and validation                |
| **Advanced**        |                           |                                             |
| Admin Panel         | `reinhardt-admin`         | Django-style admin interface                |
| Plugin System       | `reinhardt-dentdelion`    | Static & WASM plugin support, CLI management|
| Background Tasks    | `reinhardt-tasks`         | Task queues (Redis, RabbitMQ, SQLite)       |
| GraphQL             | `reinhardt-graphql`       | Schema generation, subscriptions            |
| WebSockets          | `reinhardt-websockets`    | Real-time communication                     |
| i18n                | `reinhardt-i18n`          | Multi-language support                      |
| **Testing**         |                           |                                             |
| Test Utilities      | `reinhardt-test`          | Testing helpers, fixtures, TestContainers   |

**For detailed feature flags within each crate, see the [Feature Flags Guide](https://reinhardt-web.dev/docs/feature-flags/).**

---

## Documentation

- 📚 [Getting Started Guide](https://reinhardt-web.dev/quickstart/getting-started/) - Step-by-step tutorial for beginners
- 🎛️ [Feature Flags Guide](https://reinhardt-web.dev/docs/feature-flags/) - Optimize your build with granular feature control
- 📖 [API Reference](https://docs.rs/reinhardt) (Coming soon)
- 📝 [Tutorials](https://reinhardt-web.dev/quickstart/tutorials/) - Learn by building real applications

**For AI Assistants**: See [CLAUDE.md](CLAUDE.md) for project-specific coding standards, testing guidelines, and development conventions.

## 💬 Getting Help

Reinhardt is a community-driven project. Here's where you can get help:

- 💬 **Discord**: Join our Discord server for real-time chat (coming soon)
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