# reinhardt-apps

Application configuration and registry for Reinhardt framework.

## Overview

`reinhardt-apps` provides the application configuration system inspired by Django's `INSTALLED_APPS`. It enables:

- Application discovery and registration
- App-specific configuration
- Integration with migrations, admin panel, and other framework features

**Important Note:** Unlike Django, `installed_apps!` in Reinhardt is for **user applications only**. Built-in framework features (auth, sessions, admin, etc.) are enabled via Cargo feature flags, not through `installed_apps!`.

## Installation

Add `reinhardt` to your `Cargo.toml`:

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", package = "reinhardt-web", features = ["apps"] }

# Or use a preset:
# reinhardt = { version = "0.1.0-alpha.1", package = "reinhardt-web", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.1.0-alpha.1", package = "reinhardt-web", features = ["full"] }      # All features
```

Then import app features:

```rust
use reinhardt::apps::{AppConfig, installed_apps};
```

**Note:** App features are included in the `standard` and `full` feature presets.

## Usage

Define installed apps using the `installed_apps!` macro:

```rust
use reinhardt::apps::installed_apps;

installed_apps! {
	users: "users",
	posts: "posts",
}
```

**For built-in framework features, use Cargo feature flags:**

```toml
[dependencies]
reinhardt = {
	version = "0.1.0-alpha.1",
	package = "reinhardt-web",
	features = ["auth", "sessions", "admin"]
}
```

Then import them directly in your code:

```rust
use reinhardt::auth::*;
use reinhardt::auth::sessions::*;
use reinhardt::admin::*;
```

## What the Macro Generates

The `installed_apps!` macro automatically generates:

```rust
// Generated enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstalledApp {
	Users,
	Posts,
}

// Display implementation
impl std::fmt::Display for InstalledApp {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Users => write!(f, "users"),
			Self::Posts => write!(f, "posts"),
		}
	}
}

// Helper methods
impl InstalledApp {
	pub fn all_apps() -> Vec<String> {
		vec![
			"users".to_string(),
			"posts".to_string(),
		]
	}
}
```

## Features

### Type-safe App References

Use the generated enum for type-safe app references:

```rust
// Type-safe reference
let app = InstalledApp::Users;
println!("App path: {}", app);  // "users"

// List all apps
let all = InstalledApp::all_apps();
```

### Automatic Discovery

The app registry enables automatic discovery for:

- **Migrations**: Discover migration files for each app
- **Admin Panel**: Auto-register models from each app
- **Static Files**: Collect static files from app directories
- **Templates**: Load templates from app template directories

### Framework Integration

The `installed_apps!` macro integrates with:

```rust
// src/config/apps.rs
use reinhardt::apps::installed_apps;

installed_apps! {
	users: "users",
	posts: "posts",
}

pub fn get_installed_apps() -> Vec<String> {
	InstalledApp::all_apps()
}
```

**Framework features are enabled separately via Cargo.toml:**

```toml
[dependencies]
reinhardt = {
	version = "0.1.0-alpha.1",
	package = "reinhardt-web",
	features = ["auth", "sessions", "admin", "static-files"]
}
```

## App Naming Conventions

### User Apps

User-defined apps use simple names matching their directory:

```rust
users: "users",
blog: "blog",
api: "api",
```

### Framework Features (NOT via installed_apps!)

Framework features are enabled via Cargo feature flags:

| Feature | Cargo.toml | Import |
|---------|------------|--------|
| Authentication | `features = ["auth"]` | `use reinhardt::auth::*;` |
| Admin Panel | `features = ["admin"]` | `use reinhardt::admin::*;` |
| Sessions | `features = ["sessions"]` | `use reinhardt::auth::sessions::*;` |
| Static Files | `features = ["static-files"]` | `use reinhardt::staticfiles::*;` |
| Database | `features = ["database"]` | `use reinhardt::db::*;` |

## Compile-time Validation

The macro performs compile-time validation:

- **Path Format**: Validates module path format
- **Module Existence**: Checks that `reinhardt.*` modules exist (for framework references)
- **Unique Names**: Ensures app names are unique

```rust
// Valid: User app
installed_apps! {
	users: "users",  // ✅ OK
}

// Invalid: Non-existent framework module
installed_apps! {
	nonexistent: "reinhardt.nonexistent",  // ❌ Compile error
}
```

**Note:** If you see compile errors about missing `reinhardt.contrib.*` modules, this is because those modules don't exist. Use Cargo feature flags instead (see above).

## Example Project Structure

```
my-project/
├── src/
│   ├── config/
│   │   ├── apps.rs           # installed_apps! definition
│   │   ├── settings.rs
│   │   └── urls.rs
│   └── apps/
│       ├── users/
│       │   ├── lib.rs
│       │   ├── models.rs
│       │   └── views.rs
│       └── posts/
│           ├── lib.rs
│           ├── models.rs
│           └── views.rs
└── Cargo.toml
```

```rust
// src/config/apps.rs
use reinhardt::apps::installed_apps;

installed_apps! {
	users: "users",
	posts: "posts",
}

pub fn get_installed_apps() -> Vec<String> {
	InstalledApp::all_apps()
}
```

```toml
# Cargo.toml
[dependencies]
reinhardt = {
	version = "0.1.0-alpha.1",
	package = "reinhardt-web",
	features = ["auth", "sessions", "database"]
}
```

## Integration with Other Components

### Migrations

```rust
// Migrations automatically discover apps
use reinhardt::db::migrations::MigrationRunner;

let runner = MigrationRunner::new(db);
let installed = InstalledApp::all_apps();
runner.migrate(&installed).await?;
```

### Admin Panel

```rust
// Admin panel auto-discovers models from apps
use reinhardt::admin::AdminSite;

let admin = AdminSite::new();
admin.autodiscover(&InstalledApp::all_apps()).await?;
```

### Settings Integration

```rust
// src/config/settings.rs
use reinhardt::conf::Settings;

pub fn get_settings() -> Settings {
	Settings::builder()
		.installed_apps(crate::config::apps::get_installed_apps())
		.build()
}
```

## Migrating from Django

If you're familiar with Django's `INSTALLED_APPS`, here are the key differences:

**Django (Python):**
```python
INSTALLED_APPS = [
	'django.contrib.auth',      # Framework feature
	'django.contrib.admin',     # Framework feature
	'users',                    # User app
]
```

**Reinhardt (Rust):**
```toml
# Cargo.toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", package = "reinhardt-web", features = ["auth", "admin"] }
```

```rust
// src/config/apps.rs
installed_apps! {
	users: "users",  // User apps only
}
```

**Why the difference?**
- ✅ **Compile-time optimization**: Unused features are not compiled
- ✅ **Smaller binaries**: Only include what you need
- ✅ **Type safety**: Features are validated at compile time

## License

Licensed under the BSD 3-Clause License.
