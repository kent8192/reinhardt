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
- ✅ **Change Logging**: Per-object audit history without storing submitted
  values
- ✅ **Inline Editing**: Edit related models inline
- ✅ **Changelist Inline Editing**: Edit selected list columns in one atomic batch
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
reinhardt = { version = "0.4.0-alpha.7", features = ["admin"] }

# Or use a preset:
# reinhardt = { version = "0.4.0-alpha.7", features = ["full"] }  # All features
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

Provision the admin history table before serving requests by calling
`initialize_admin_history_schema()` during application setup, or by applying an
equivalent application migration. Admin request handlers only read and insert
history rows.

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
	list_editable = [email, is_active],
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

### Foreign-key relation fields

Foreign-key form controls are opt-in. Add a relation to
`autocomplete_fields` for a searchable control, or to `raw_id_fields` for a
direct relation-ID input. The two lists are mutually exclusive after field
name normalization:

```rust
use reinhardt_admin::core::{ModelAdmin, ModelAdminConfig};

let post_admin = ModelAdminConfig::builder()
    .model_name("Post")
    .autocomplete_fields(vec!["author"])
    .raw_id_fields(vec!["editor_id"])
    .allow_all(true)
    .build()
    .expect("valid relation configuration");

assert_eq!(post_admin.autocomplete_fields(), vec!["author"]);
assert_eq!(post_admin.raw_id_fields(), vec!["editor_id"]);
```

Each configured name may be either the model's logical relation name (for
example, `author`) or its persisted ID column (`author_id`). Reinhardt uses the
application relationship registry and migration metadata to normalize both
forms to the persisted column used in submissions and to resolve the qualified
target model. An explicit foreign-key `to_field` is honored for lookup,
validation, and saving. Only foreign keys are accepted; a missing target admin, a table
mismatch, an unknown/non-foreign-key field, or a field configured in both lists
is rejected before form metadata or lookup results are returned.

Autocomplete searches use the related `ModelAdmin::search_fields()` values as
OR-combined `Contains` filters. The related admin must configure at least one
search field. A related admin can customize option labels by overriding
`ModelAdmin::object_label()`; returning `None` falls back to the related
object's relation target-field value. Raw-ID controls resolve the exact ID so edit forms
also display a permission-checked label.

Both the source admin and the related admin must grant view permission before a
lookup can return any row or label. Create and update operations perform the
same related view, scalar-ID, target-existence, and nullability checks again at
save time, after the normal field allowlist/readonly validation and before
sanitization or the database write. A relation marked in `readonly_fields`
cannot be changed. Null is accepted only when the foreign-key metadata marks
the relation nullable.

Relation lookups are bounded: the query is at most 200 bytes, the default page
size is 20 and the maximum is 100, page numbers are constrained to 1 through
10,000, and the server fetches at most one extra row to compute `has_next`.
Responses never contain more than the requested page size. Submitted IDs and
labels are always resolved by the server; client-provided labels are not
trusted.
### Registered Model Actions

Manual `ModelAdmin` implementations can expose actions with stable names,
labels, permissions, and an optional confirmation prompt through `actions()`.
The list page applies an action only to the records selected on the current
page.

Override `execute_action()` to perform the mutation with the supplied
`AdminActionTransaction`. The server commits the action only when the hook
returns `AdminActionOutcome`; an error rolls back the transaction. Return the
canonical, duplicate-free IDs that actually succeeded separately from the
total affected row count so audit and history consumers can record the exact
objects. The hook receives no pooled database handle, so every action write
uses the server-owned transaction.

The endpoint validates CSRF, the registered action name, selection size,
primary-key values, and the declared `ModelPermission` before calling the
hook. Confirmation metadata is enforced by the browser UI; server-side callers
must still make an explicit action request.

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

`list_editable` is opt-in; without it, changelists remain read-only. Each entry
must be a real database field in `list_display`, and cannot be the primary key,
the first displayed row-link field, generated, computed, or read-only. The
admin submits only dirty rows when **Save** is selected and commits the current
page as one transaction, so any row failure rolls back the complete batch.
Timezone-aware values are displayed in `datetime-local` controls as UTC;
submitted wall times are also interpreted as UTC.

## Migration notes

List-view struct literals now carry inline-edit metadata. Add `editable`,
`linked`, `required`, and `form_spec` to `Column` and `ColumnInfo`, and add
`pk_field` to `ListViewData` and `ListResponse`. `ListViewData::records` now uses
`HashMap<String, serde_json::Value>` so primary keys and editable values retain
their wire types. Use `false`, `false`, `false`, `None`, and `"id"` respectively
to preserve the previous read-only behavior.
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
- `get_list_action_metadata` — primary-key and registered action metadata
- `get_detail` — detail view for a single record
- `get_history` — newest-first per-object change history, including deleted records
- `get_fields` — field metadata for a model
- `get_relation_options` — search and resolve configured relation field options
- `create_record` — create a new record
- `update_record` — update an existing record
- `update_inline_edits` — atomically update dirty changelist rows
- `delete_record` — delete a single record
- `bulk_delete_records` — bulk delete operations
- `execute_admin_action` — registered model actions
- `update_inline_edits` — atomic changelist inline edits
- `export_data` — export data (CSV, JSON, XML)
- `import_data` — import data
- `admin_login` / `admin_login_with_header` — admin authentication
- `admin_logout` — admin session termination

Successful mutations persist their per-object history metadata in the same
transaction. History records contain changed field names, but not submitted
field values.

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
// POST   /admin/api/server_fn/get_list_action_metadata
// POST   /admin/api/server_fn/get_detail
// POST   /admin/api/server_fn/get_history
// POST   /admin/api/server_fn/get_fields
// POST   /admin/api/server_fn/get_relation_options
// POST   /admin/api/server_fn/create_record
// POST   /admin/api/server_fn/update_record
// POST   /admin/api/server_fn/update_inline_edits
// POST   /admin/api/server_fn/delete_record
// POST   /admin/api/server_fn/bulk_delete_records
// POST   /admin/api/server_fn/execute_admin_action
// POST   /admin/api/server_fn/update_inline_edits
// POST   /admin/api/server_fn/export_data
// POST   /admin/api/server_fn/import_data
// POST   /admin/api/server_fn/admin_login
// POST   /admin/api/server_fn/admin_login_with_header
// POST   /admin/api/server_fn/admin_logout
// GET    /admin/              (SPA shell)
// GET    /admin/{model}/{id}/history/ (per-object history)
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
