# reinhardt-admin

Django-style admin panel functionality for Reinhardt framework.

## Overview

This crate provides two main components:

- **Panel**: Web-based admin interface for managing database models
- **CLI**: Command-line tool for project management (available as
  `reinhardt-admin-cli`)

## Features

### Admin Panel (`reinhardt-panel`)

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
- ✅ **Drag-and-Drop Reordering**: Reorder model instances with transaction-safe
  operations
- ✅ **Responsive Design**: Mobile-friendly admin interface with customizable
  templates

### Command-Line Interface (`reinhardt-admin-cli`)

For project management commands (`startproject`, `startapp`), please use
[`reinhardt-admin-cli`](../reinhardt-admin-cli).

## Installation

Add `reinhardt` to your `Cargo.toml`:

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", features = ["admin"] }

# Or use a preset:
# reinhardt = { version = "0.1.0-alpha.1", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.1.0-alpha.1", features = ["full"] }      # All features
```

Then import admin features:

```rust
use reinhardt::admin::{AdminSite, ModelAdmin};
use reinhardt::admin::types::{ListQueryParams, AdminError};
```

**Note:** Admin features are included in the `standard` and `full` feature presets.

## Quick Start

### Using the Admin Panel

```rust
use reinhardt::admin::{AdminSite, ModelAdmin};

#[tokio::main]
async fn main() {
    let mut admin = AdminSite::new("My Admin");

    // Register your models
    admin.register::<User>(UserAdmin::default()).await;

    // Start admin server
    admin.serve("127.0.0.1:8001").await.unwrap();
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

## Panel Architecture

The admin panel is built on several key components:

### Database Layer

Advanced filtering and query building with reinhardt-query integration:

- **FilterOperator**: Eq, Ne, Gt, Gte, Lt, Lte, Contains, StartsWith, EndsWith, In, NotIn, Between, Regex
- **FilterCondition**: AND/OR conditions for complex queries
- **FilterValue**: Type-safe value representation (String, Int, Float, Bool, Array)

For detailed database layer documentation, see the [`core::database`](src/core/database.rs) module.

### Handlers

HTTP request handlers for all CRUD operations:

- `AdminHandlers::dashboard()` - Admin dashboard
- `AdminHandlers::list()` - Model list view with pagination
- `AdminHandlers::detail()` - Detail view
- `AdminHandlers::create()` - Create new instance
- `AdminHandlers::update()` - Update instance
- `AdminHandlers::delete()` - Delete instance
- `AdminHandlers::bulk_delete()` - Bulk delete operations
- `AdminHandlers::export()` - Export data (CSV, JSON, XML)
- `AdminHandlers::import()` - Import data

### Routing

Automatic route registration for models:

```rust
use reinhardt::admin::router::AdminRouter;

let router = AdminRouter::new(site, db)
    .with_favicon("static/favicon.ico")
    .build();

// Automatically creates routes:
// GET    /admin/<model>/
// GET    /admin/<model>/{id}/
// POST   /admin/<model>/
// PUT    /admin/<model>/{id}/
// DELETE /admin/<model>/{id}/
// DELETE /admin/<model>/bulk/
// GET    /admin/<model>/export/
// POST   /admin/<model>/import/
router.register_model_routes::<User>("/admin/user/")?;
```

For comprehensive panel documentation, see the [`core`](src/core/) module.

## Advanced Features

### Drag-and-Drop Reordering

Enable drag-and-drop reordering for your models with transaction-safe
operations:

```rust
use reinhardt::admin::{DragDropConfig, ReorderableModel, ReorderHandler};
use async_trait::async_trait;

// 1. Configure drag-and-drop
let config = DragDropConfig {
	order_field: "display_order".to_string(),
	enabled: true,
	custom_js: None,  // Or provide custom JavaScript for client-side handling
};

// 2. Implement ReorderableModel for your model
#[async_trait]
impl ReorderableModel for MenuItem {
	async fn get_order(&self) -> i32 {
		self.display_order
	}

	async fn set_order(&mut self, new_order: i32) {
		self.display_order = new_order;
	}

	fn get_id(&self) -> String {
		self.id.to_string()
	}
}

// 3. Create a reorder handler
let handler = ReorderHandler::new(
	config,
	connection.clone(),
	"menu_items",  // table name
	"id",          // primary key field
);

// 4. Process reorder requests
let reorder_items = vec![
	("item_1".to_string(), 0),
	("item_2".to_string(), 1),
	("item_3".to_string(), 2),
];

match handler.process_reorder(reorder_items).await {
	result if result.is_success() => {
		println!("Reordered {} items successfully", result.items_updated);
	}
	result => {
		eprintln!("Reorder failed: {}", result.message);
	}
}
```

**Key Features:**

- **Validation**: Ensures order values are sequential, non-negative, and unique
- **Transaction Safety**: All updates are executed within a database transaction
- **Error Handling**: Detailed error messages for validation and database
  failures
- **Bulk Updates**: Efficient handling of multiple items in a single transaction

**Implementation Details:**

The `ReorderHandler` validates reorder operations by:

1. Checking for negative order values
2. Detecting duplicate order values
3. Ensuring order values are sequential (0, 1, 2, ...)

All database updates are performed using reinhardt-query within a
transaction, ensuring atomicity.

For a complete implementation example, see the [`core::database`](src/core/database.rs) module.

## Feature Flags

- `panel` (default): Web admin panel
- `cli`: Command-line interface
- `all`: All admin functionality

## Documentation

- [API Documentation](https://docs.rs/reinhardt-admin) (coming soon)
- [Core Module Documentation](src/core/)

## License

Licensed under the BSD 3-Clause License.
