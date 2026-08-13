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
- ✅ **Bulk Operations**: Delete multiple records in a single operation
- ⏳ **Customizable Admin Actions** (planned; tracked in
  [#5808](https://github.com/kent8192/reinhardt-web/issues/5808)): Define custom
  `ModelAdmin` actions
- ✅ **Search and Filtering**: Advanced search capabilities with multiple filter
  types
- ✅ **Permissions Integration**: Role-based access control for admin operations
- ✅ **Change Logging**: Audit trail for all admin actions
- ✅ **Inline Editing**: Edit foreign-key related models inline on parent forms
- ✅ **Responsive Design**: Mobile-friendly admin interface with customizable
  templates

### Command-Line Interface (`reinhardt-admin-cli`)

For project management commands (`startproject`, `startapp`), please use
[`reinhardt-admin-cli`](../reinhardt-admin-cli).

## Installation

Add `reinhardt` to your `Cargo.toml`:

<!-- reinhardt-version-sync:2 -->
```toml
[dependencies]
reinhardt = { version = "0.4.0-alpha.6", features = ["admin"] }

# Or use a preset:
# reinhardt = { version = "0.4.0-alpha.6", features = ["full"] }  # All features
```

Then import admin features:

```rust
use reinhardt::admin::{AdminSite, ModelAdmin};
use reinhardt::admin::types::{ListQueryParams, AdminError};
```

## Quick Start

### Configuring Admin Models

Register models with `AdminSite` in a dedicated configuration function:

```rust
use reinhardt::admin::{AdminSite, ModelAdmin};

fn configure_admin() -> AdminSite {
	let mut site = AdminSite::new("My Admin");
	site.register::<User>(UserAdmin::default());
	site
}
```

### Mounting Admin Routes

Admin routes are registered inside the `routes()` function decorated with
`#[routes]`. Use `admin_routes_with_di()` to mount the admin
panel with deferred DI registration:

```rust
use reinhardt::UnifiedRouter;
use reinhardt::admin::{admin_routes_with_di, admin_static_routes};
use reinhardt::routes;
use std::sync::Arc;

#[routes]
pub fn routes() -> UnifiedRouter {
	// Configure admin site (registration only, no DB needed yet)
	#[cfg(native)]
	let admin_site = Arc::new(configure_admin());

	let router = UnifiedRouter::new()
		// Mount your app routes here
		;

	// Mount admin panel routes and static assets (server-only)
	#[cfg(native)]
	let router = {
		let (admin_router, admin_di) = admin_routes_with_di(admin_site);
		router
			.mount("/admin/", admin_router)
			.mount("/static/admin/", admin_static_routes())
			.with_di_registrations(admin_di)
	};
	router
}
```

The `AdminDatabase` is lazily constructed from `DatabaseConnection` at the
first request, so no database connection is needed during route setup.

### Customizing the Admin

Use the `#[admin]` proc macro to register a model with the admin panel. The macro
automatically implements `ModelAdmin` — no manual `impl` block is needed:

```rust
use reinhardt::admin;
use crate::models::User;

#[admin(model,
	for = User,
	name = "User",
	list_display = [username, email, is_active],
	list_filter = [is_active],
	search_fields = [username, email],
	fieldsets = [
		(title = "Identity", fields = [username, email]),
		(title = "Status", fields = [is_active], collapsed = true)
	],
	ordering = [(date_joined, desc)],
	list_per_page = 25,
)]
pub struct UserAdmin;
```

The `#[admin(model, ...)]` attribute expands to a full `ModelAdmin` implementation
at compile time, so you never need to write boilerplate field structs or
`impl Default` blocks.

### Editing Related Models Inline

Manual admin configuration can place foreign-key children on the parent create
and change forms. The child admin must also be registered with `AdminSite`
with the typed child's table name because its view, add, change, and delete
permissions are checked independently. Inline children must use a single-field
integer, text-like, or UUID primary key; the parent key follows the same type
restriction.

```rust
use reinhardt_admin::core::{InlineModelAdmin, InlineStyle, ModelAdminConfig};

let line_items = InlineModelAdmin::new::<Order, LineItem>(
	"LineItem",
	"order_id",
	&["product", "quantity"],
)?
.style(InlineStyle::Tabular)
.extra(1)
.can_delete(true);

let order_admin = ModelAdminConfig::builder()
	.model_name("Order")
	.table_name("orders")
	.fields(vec!["number"])
	.inlines(vec![line_items])
	.build()?;
```

`InlineStyle::Stacked` renders the same rows as labelled field groups. Blank
configured extra rows create new children; no client-side row factory is
needed. The server rejects submitted foreign keys and assigns the trusted
parent key itself. Parent and child creates, updates, and explicit deletes run
in one transaction, so any child failure rolls back the complete edit.

Inline declarations in `#[admin]`, nested inlines, and dynamically adding more
rows in the browser are not supported. Configure the required number of blank
rows with `extra`.

### Grouping Form Fields

Without `fieldsets`, the existing `fields` configuration keeps forms flat. Use
one or the other; configuring both is rejected. Programmatic configurations use
the same ordered `Fieldset` descriptors as the macro:

```rust
use reinhardt::admin::{Fieldset, ModelAdmin, ModelAdminConfig};

let grouped = ModelAdminConfig::builder()
	.model_name("Article")
	.fieldsets(vec![
		Fieldset::new(Some("Content"), &["title", "body"]),
		Fieldset::new(Some("Publishing"), &["published_at"]).collapsed(),
	])
	.build()
	.unwrap();
assert!(grouped.fieldsets().unwrap()[1].collapsed);
```

`collapsed` sets only the initial state of the native `<details>` element; the
open state is not persisted. Fieldsets do not support nesting, custom layout
classes, layout grids, or inline form configuration.

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
- `admin_login` / `admin_login_with_header` — admin authentication
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
// POST   /admin/api/server_fn/admin_login_with_header
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
