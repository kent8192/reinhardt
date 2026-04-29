# Examples - Project-Specific Instructions

## Purpose

This file defines coding standards and dependency management rules specific to the `examples/` directory. These rules ensure examples demonstrate best practices for using Reinhardt.

---

## Examples Directory Structure

The `examples/` directory contains example projects and shared utilities in a flat structure:

```
examples/
├── examples-tutorial-basis/       # Polls tutorial (MTV/Pages-style)
├── examples-tutorial-rest/        # Snippets tutorial (REST API)
├── examples-twitter/              # End-to-end Twitter-clone demo
├── .cargo/
│   └── config.local.toml          # Template for local development
├── Cargo.toml                     # Workspace configuration
└── README.md
```

### Example Projects

Each `examples-*` directory is an independent Cargo project demonstrating specific Reinhardt features. By default, examples use published crates.io versions of Reinhardt.

---

## Dependency Management

### Default Mode (crates.io)

By default, examples use published versions from crates.io:

<!-- reinhardt-version-sync -->
```toml
[dependencies]
reinhardt = { version = "0.1.0-rc.24", package = "reinhardt-web", features = ["standard"] }
```

### Local Development Mode

For framework contributors testing against local Reinhardt code:

1. Copy `.cargo/config.local.toml` to `.cargo/config.toml`
2. This activates `[patch.crates-io]` overrides pointing to local workspace
3. `.cargo/config.toml` is gitignored

```bash
# Activate local development mode
cp .cargo/config.local.toml .cargo/config.toml

# Deactivate (back to crates.io)
rm -f .cargo/config.toml
```

### DM-1 (MUST): Reinhardt Dependencies Only

**ALL** example projects MUST include ONLY the `reinhardt` crate in their dependencies.

**NEVER** include Reinhardt sub-crates (`reinhardt-*`) directly in:
- `[dependencies]`
- `[dev-dependencies]`
- `[build-dependencies]`

#### ✅ CORRECT Pattern

<!-- reinhardt-version-sync -->
```toml
[dependencies]
# ✅ Main reinhardt crate only
reinhardt = { version = "0.1.0-rc.24", package = "reinhardt-web", features = ["core", "database"] }

# ✅ External crates are fine
tokio = { workspace = true }
serde = { workspace = true }

[dev-dependencies]
# ✅ External test crates are fine
rstest = "0.26.1"
```

#### ❌ INCORRECT Pattern

<!-- reinhardt-version-sync -->
```toml
[dependencies]
reinhardt = { version = "0.1.0-rc.24", package = "reinhardt-web", features = ["core"] }
reinhardt-http = { path = "../../../crates/reinhardt-http" }      # ❌ NEVER
reinhardt-routers = { path = "../../../crates/reinhardt-urls/crates/routers" }  # ❌ NEVER
reinhardt-di = { path = "../../../crates/reinhardt-di" }          # ❌ NEVER
reinhardt-orm = { path = "../../../crates/reinhardt-db/crates/orm" }  # ❌ NEVER

[dev-dependencies]
reinhardt-http = { path = "../../../crates/reinhardt-http" }      # ❌ NEVER (sub-crate dependency)
```

### DM-2 (MUST): Import from reinhardt Crate Only

**ALL** imports MUST use the `reinhardt` crate, NOT sub-crates.

#### ✅ CORRECT Import Patterns

```rust
// Pattern 1: Use prelude for common types
use reinhardt::prelude::*;

// Pattern 2: Explicit imports from reinhardt
use reinhardt::{Request, Response, StatusCode};
use reinhardt::{Method, UnifiedRouter};
use reinhardt::endpoint;

// Pattern 3: Module-qualified imports
use reinhardt::db::orm::Manager;
use reinhardt::db::DatabaseConnection;

// Pattern 4: External dependencies - prefer direct import when in [dependencies]
use serde::{Serialize, Deserialize};  // Preferred (serde is in [dependencies])
use reinhardt::core::serde::json::json;  // Also valid via re-export
use reinhardt::core::async_trait;  // Use re-export (async_trait not in [dependencies])

// Pattern 5: External crate direct imports (when in [dependencies])
use serde::{Serialize, Deserialize};
use serde_json::json;
```

#### ❌ INCORRECT Import Patterns

```rust
// ❌ NEVER import from sub-crates directly
use reinhardt_http::{Request, Response};
use reinhardt_routers::UnifiedRouter;
use reinhardt_macros::endpoint;
use reinhardt_orm::Manager;
use reinhardt_db::DatabaseConnection;
use reinhardt_test::fixtures::postgres_container;

// ❌ NEVER import from hyper directly (use reinhardt re-exports)
use hyper::{Method, StatusCode};

// Note: When serde or serde_json are direct dependencies in Cargo.toml,
// prefer importing from the crate directly: use serde::{Serialize, Deserialize};
// The re-export path (reinhardt::core::serde) is also valid but direct import is preferred.

// ❌ NEVER import external dependencies that are NOT in your Cargo.toml
use async_trait::async_trait;         // Use reinhardt::core::async_trait instead (unless async_trait is in [dependencies])
```

---

## Why These Rules?

### 1. **Demonstrates Correct Usage**
Examples should show users the **recommended way** to use Reinhardt, not internal implementation details.

### 2. **Maintains Facade Pattern**
Reinhardt uses a facade pattern where the main crate re-exports commonly used types. Direct sub-crate dependencies break this abstraction.

### 3. **Prevents Breaking Changes**
Sub-crates are internal implementation details that may change. The `reinhardt` crate provides a stable API surface.

### 4. **Consistency with Documentation**
README.md and other documentation recommend using `reinhardt::prelude::*` and `use reinhardt::{...}`. Examples must match this guidance.

---

## Available Re-Exports

### Always Available (from src/lib.rs)

```rust
use reinhardt::{
	// HTTP & Routing
	StatusCode, Method,
	Request, Response,
	UnifiedRouter, Router, DefaultRouter,

	// Macros
	endpoint,
	installed_apps,

	// Settings
	Settings, SettingsBuilder,

	// View Classes
	View, ListView, DetailView,
	ViewSet, ModelViewSet, ReadOnlyModelViewSet,

	// View Result Type
	ViewResult,
};
```

### Feature-Dependent Re-Exports

```rust
// With feature = "core"
use reinhardt::{Handler, Middleware};

// With feature = "database"
use reinhardt::db::{
	DatabaseConnection,
	orm::Manager,
	orm::Model,
	orm::{F, Q, Transaction},
	migrations::Migration,  // ✅ Correct: module-qualified only
};
// SQL query builder (via reinhardt-query)
use reinhardt::query::prelude::*;

// ❌ NOT available at top level
// use reinhardt::Migration;  // This does NOT compile

// With feature = "di"
use reinhardt::{Body, Cookie, Header, Json, Path, Query};

// With feature = "graphql"
use reinhardt::graphql;

// With feature = "i18n"
use reinhardt::i18n;

// With feature = "mail"
use reinhardt::mail;

// With feature = "grpc"
use reinhardt::grpc;

// With feature = "dispatch"
use reinhardt::dispatch;

// With feature = "deeplink"
use reinhardt::deeplink;

// With feature = "test"
use reinhardt::test::{
	client::APIClient,
	fixtures::test_server_guard,
	resource::TeardownGuard,
};
```

### Prelude Module (use reinhardt::prelude::*)

```rust
use reinhardt::prelude::*;

// This imports:
// - UnifiedRouter, Router, DefaultRouter
// - StatusCode
// - ViewSet, ModelViewSet, ReadOnlyModelViewSet
// - (if core feature) Request, Response, Handler, Middleware, Signals
// - (if database feature) Model, DatabaseConnection, F, Q, Transaction, atomic
// - (if di feature) Body, Cookie, Header, Json, Path, Query
// - (if rest feature) Serializer, Paginator, Throttle, Versioning
// - (if auth feature) User, UserManager, GroupManager, Permission
// - (if cache feature) Cache, InMemoryCache
// - (if sessions feature) Session, AuthenticationMiddleware
```

---

## Code Style

### CS-1 (SHOULD): Prefer Prelude for Common Imports

For most examples, start with:

```rust
use reinhardt::prelude::*;
```

Then add explicit imports only for types not in prelude:

```rust
use reinhardt::prelude::*;
use reinhardt::{endpoint, Method};  // Not in prelude
```

### CS-2 (MUST): Consistent Import Style

Within a single file, be consistent:

**Option A: Prelude + Explicit**
```rust
use reinhardt::prelude::*;
use reinhardt::{endpoint, Method};
use serde_json::json;
```

**Option B: Fully Explicit**
```rust
use reinhardt::{Request, Response, StatusCode, UnifiedRouter, Method, endpoint};
use serde_json::json;
```

**❌ Don't mix patterns inconsistently**
```rust
use reinhardt::prelude::*;
use reinhardt::Request;  // ❌ Already in prelude!
use reinhardt_http::Response;  // ❌ Wrong crate!
```

### CS-3 (MUST): Attribute Macro Ordering on Model Structs

`#[model(...)]` MUST come before `#[derive(...)]` on model structs:

```rust
// ✅ CORRECT: #[model] before #[derive]
#[model(app_label = "users", table_name = "users")]
#[derive(Serialize, Deserialize)]
pub struct User { ... }

// ❌ INCORRECT: #[derive] before #[model]
#[derive(Serialize, Deserialize)]
#[model(app_label = "users", table_name = "users")]
pub struct User { ... }
```

**Why:** Rust attribute macros are applied from outside to inside. `#[model]` transforms `ForeignKeyField<T>` into concrete columns and may generate trait implementations (e.g., `Clone`). If `#[derive]` runs first, it sees the untransformed struct and fails.

---

## Testing Standards

### TS-1 (MUST): Test Dependencies

**External test crates** can be used in `[dev-dependencies]`:

```toml
[dev-dependencies]
rstest = "0.26.1"
tokio = { workspace = true, features = ["rt", "macros"] }
```

### TS-2 (MUST): Test Imports

Tests should import from `reinhardt::test` or use shared test utilities:

```rust
#[cfg(test)]
mod tests {
	use reinhardt::prelude::*;
	use reinhardt::test::client::APIClient;
	use reinhardt::test::fixtures::test_server_guard;
}
```

### TS-3 (SHOULD): Direct Invocation for `#[server_fn]` Tests

TS-3 is the examples-project mirror of `instructions/TESTING_STANDARDS.md`
§ TI-7. The underlying convention is identical; TS-3 exists so examples
contributors find the rule here without having to cross-reference the top-level
standards doc.

When testing `#[server_fn]` functions in example projects, prefer **direct
invocation** (call the function as a regular `async fn` and pass injected
dependencies positionally) over routing JSON requests through
`ServerRouter::handle()`. The `#[inject]` attributes are stripped at expansion
time, so server functions are normal Rust functions on the server side.

Reserve HTTP routing for tests whose stated purpose is to verify the
HTTP/DI/middleware pipeline itself (document the rationale in the module
header). See `instructions/TESTING_STANDARDS.md` § TI-7 for the full
convention and #3826 for context.

---

## Quick Reference

### Checklist for New Examples

- [ ] `Cargo.toml` includes ONLY `reinhardt` crate (not `reinhardt-*` sub-crates)
- [ ] All imports use `reinhardt::` prefix (not `reinhardt_*::`)
- [ ] Consider using `use reinhardt::prelude::*;` for common types
- [ ] No direct imports from `hyper` (use `reinhardt::Method`, `reinhardt::StatusCode`)
- [ ] Consistent import style throughout the example

### Common Mistakes to Avoid

Each mistake is paired with its correct alternative:

**Mistake 1: Importing from sub-crates**
- ❌ `use reinhardt_http::{Request, Response};`
- ✅ `use reinhardt::{Request, Response};`

**Mistake 2: Direct router imports from sub-crate**
- ❌ `use reinhardt_routers::UnifiedRouter;`
- ✅ `use reinhardt::UnifiedRouter;`

**Mistake 3: Importing macros from sub-crate**
- ❌ `use reinhardt_macros::endpoint;`
- ✅ `use reinhardt::endpoint;`

**Mistake 4: Importing external dependencies not in your Cargo.toml**
- ❌ `use async_trait::async_trait;` (when async_trait is not in `[dependencies]`)
- ✅ `use reinhardt::core::async_trait;` (use re-export)
- ✅ `use serde::{Serialize, Deserialize};` (direct import is preferred when serde is in `[dependencies]`)

**Mistake 5: Including sub-crates in dependencies**
- ❌ `reinhardt-http = { path = "..." }` in `[dependencies]`
- ✅ `reinhardt = { version = "...", features = [...] }`

### Correct Patterns Summary

- ✅ `use reinhardt::{...}` - Explicit imports from main crate
- ✅ `use reinhardt::prelude::*;` - Common types via prelude
- ✅ `use reinhardt::endpoint;` - Macros from main crate
- ✅ `use reinhardt::Method;` - Re-exported types from main crate

---

## Related Documentation

- **Main Project Standards**: @../CLAUDE.md
- **Project README**: @../README.md
