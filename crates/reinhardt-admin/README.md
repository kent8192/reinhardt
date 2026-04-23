# reinhardt-admin

Django-style admin panel functionality for Reinhardt framework.

## Overview

This crate provides a web-based admin interface for managing database models,
built as a WASM single-page application served by a Reinhardt server.

## Features

- ✅ **Model Management Interface**: Web-based CRUD operations for database
  models
- ✅ **Automatic Admin Discovery**: Auto-generate admin interfaces from model
  definitions
- ✅ **Customizable Admin Actions**: Bulk operations and custom actions
- ✅ **Search and Filtering**: Advanced search capabilities with multiple filter
  types
- ✅ **Permissions Integration**: Role-based access control for admin operations
- ✅ **Change Logging**: Audit trail for all admin actions
- ✅ **Inline Editing**: Edit related models inline
- ✅ **Responsive Design**: Mobile-friendly admin interface with customizable
  templates

### Command-Line Interface (`reinhardt-admin-cli`)

For project management commands (`startproject`, `startapp`), please use
[`reinhardt-admin-cli`](../reinhardt-admin-cli).

## Installation

Add `reinhardt` to your `Cargo.toml`:

```toml
[dependencies]
reinhardt = { version = "0.1.0-rc.19", features = ["admin"] }

# Or use a preset:
# reinhardt = { version = "0.1.0-rc.19", features = ["full"] }  # All features
```

Then import admin features:

```rust
use reinhardt::admin::{AdminSite, ModelAdmin};
use reinhardt::admin::types::{ListQueryParams, AdminError};
```

## Quick Start

### Using the Admin Panel

```rust
use reinhardt_admin::core::{AdminSite, admin_routes_with_di, admin_static_routes};
use reinhardt_urls::routers::UnifiedRouter;
use std::sync::Arc;

#[tokio::main]
async fn main() {
	let site = Arc::new(AdminSite::new("My Admin"));
	let (admin_router, admin_di) = admin_routes_with_di(site);
	let assets = admin_static_routes();

	let router = UnifiedRouter::new()
		.mount("/admin/", admin_router)
		.mount("/static/admin/", assets)
		.with_di_registrations(admin_di);

	// Attach `router` to your application server
}
```

### Customizing the Admin

```rust
use reinhardt::admin::ModelAdmin;

struct UserAdmin {
	list_display: Vec<String>,
	list_filter: Vec<String>,
	search_fields: Vec<String>,
}

impl Default for UserAdmin {
	fn default() -> Self {
		Self {
			list_display: vec!["username".to_string(), "email".to_string(), "is_active".to_string()],
			list_filter: vec!["is_active".to_string()],
			search_fields: vec!["username".to_string(), "email".to_string()],
		}
	}
}
```

## Architecture

The admin panel is built on several key components:

### Database Layer

Advanced filtering and query building with reinhardt-query integration:

- **FilterOperator**: Eq, Ne, Gt, Gte, Lt, Lte, Contains, StartsWith, EndsWith, In, NotIn, Between, Regex
- **FilterCondition**: AND/OR conditions for complex queries
- **FilterValue**: Type-safe value representation (String, Int, Float, Bool, Array)

For detailed database layer documentation, see the [`core::database`](src/core/database.rs) module.

### Server Functions

All CRUD operations are implemented as reinhardt-pages server functions in
individual modules under `src/server/`:

- `get_dashboard` — admin dashboard data
- `get_list` — model list view with pagination
- `get_detail` — detail view for a single record
- `get_fields` — field metadata for a model
- `create_record` — create a new record
- `update_record` — update an existing record
- `delete_record` — delete a single record
- `bulk_delete_records` — bulk delete operations
- `export_data` — export data (CSV, JSON, XML)
- `import_data` — import data
- `admin_login` — admin authentication
- `admin_logout` — admin session termination

### Routing

Route registration uses two free functions from `core::router`:

```rust
use reinhardt_admin::core::{AdminSite, admin_routes_with_di, admin_static_routes};
use reinhardt_urls::routers::UnifiedRouter;
use std::sync::Arc;

// Default: uses AdminDefaultUser (table "auth_user")
let site = Arc::new(AdminSite::new("My Admin"));
let (admin_router, admin_di) = admin_routes_with_di(site);
let assets = admin_static_routes();

let router = UnifiedRouter::new()
	.mount("/admin/", admin_router)
	.mount("/static/admin/", assets)
	.with_di_registrations(admin_di);

// Routes registered under /admin/:
// POST   /admin/api/server_fn/get_dashboard
// POST   /admin/api/server_fn/get_list
// POST   /admin/api/server_fn/get_detail
// POST   /admin/api/server_fn/get_fields
// POST   /admin/api/server_fn/create_record
// POST   /admin/api/server_fn/update_record
// POST   /admin/api/server_fn/delete_record
// POST   /admin/api/server_fn/bulk_delete_records
// POST   /admin/api/server_fn/export_data
// POST   /admin/api/server_fn/import_data
// POST   /admin/api/server_fn/admin_login
// POST   /admin/api/server_fn/admin_logout
// GET    /admin/              (SPA shell)
// GET    /admin/{*tail}       (SPA client-side routing)

// Static assets registered under /static/admin/:
// GET    /static/admin/{*path}
// HEAD   /static/admin/{*path}
```

For comprehensive routing documentation, see the [`core::router`](src/core/router.rs) module.

## Feature Flags

| Feature | Description |
|---------|-------------|
| `adapters` | Adapter layer utilities |
| `core` | Core admin functionality |
| `pages` | Page rendering support |
| `server` | Server-side request handling |
| `types` | Shared type definitions |
| `all` | All of the above (`adapters`, `core`, `pages`, `server`, `types`) |
| `file-uploads` | File upload support |
| `admin` | Admin feature marker |
| `full` | All features including `file-uploads` |

By default, no features are enabled (`default = []`).

## Documentation

- [API Documentation](https://docs.rs/reinhardt-admin) (coming soon)
- [Core Module Documentation](src/core/)

## License

Licensed under the BSD 3-Clause License.
