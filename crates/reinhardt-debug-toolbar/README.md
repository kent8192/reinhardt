# reinhardt-debug-toolbar

A debug toolbar for the Reinhardt web framework, inspired by Django Debug Toolbar.

## Features

- **SQL Query Panel**: View all SQL queries with execution time, detect duplicates and N+1 queries
- **Request/Response Panel**: Inspect HTTP headers, cookies, and query parameters
- **Template Panel**: Monitor template rendering with context inspection
- **Cache Panel**: Track cache operations with hit/miss statistics
- **Performance Panel**: Profile request processing with timeline visualization

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
reinhardt-debug-toolbar = "0.1.0-alpha.1"
```

## Quick Start

```rust
use reinhardt_debug_toolbar::{DebugToolbarLayer, ToolbarConfig};
use axum::Router;

let config = ToolbarConfig {
    enabled: true,
    internal_ips: vec!["127.0.0.1".parse().unwrap()],
    ..Default::default()
};

let app = Router::new()
    .route("/", get(handler))
    .layer(DebugToolbarLayer::new(config));
```

## Feature Flags

- `sql-panel` - SQL query debugging panel
- `template-panel` - Template rendering panel
- `cache-panel` - Cache statistics panel
- `performance-panel` - Performance profiling panel
- `full` - All panels enabled (default)

## Architecture

The toolbar follows a layered architecture:

1. **Middleware Layer**: Request/response interception using Tower middleware
2. **Collection Layer**: Data collection from framework components
3. **Panel Layer**: Statistics generation and UI rendering
4. **UI Layer**: HTML/CSS/JS rendering and injection

## Security

The toolbar is designed for development use only:

- Only enabled in debug builds by default
- IP whitelist for access control (localhost only by default)
- Sensitive data sanitization (passwords, tokens, etc.)
- Zero-cost abstraction in release builds

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](../../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
