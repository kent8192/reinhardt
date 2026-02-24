# Rust 2024 Edition Module System Standards

## Purpose

This document defines the module system standards for the Reinhardt project using Rust 2024 Edition conventions.

## Core Principle

**MUST USE `module.rs` + `module/` directory structure (Rust 2024 Edition)**

**NEVER USE `mod.rs` files** (Rust 2015/2018 deprecated pattern)

---

## Basic Patterns

### Pattern 1: Small Module (Single File)

For modules with no submodules:

```
src/
├── lib.rs          // mod utils;
└── utils.rs        // pub fn helper() {}
```

### Pattern 2: Medium Module (With Submodules)

For modules with submodules:

```
src/
├── lib.rs          // mod database;
├── database.rs     // pub mod pool; pub mod connection;
└── database/
    ├── pool.rs
    └── connection.rs
```

**Key Points:**
- `database.rs` is the entry point
- Declare submodules in `database.rs`: `pub mod pool; pub mod connection;`
- Parent declares with: `mod database;` in `lib.rs`

### Pattern 3: Large Module (Hierarchical Structure)

For complex modules with nested submodules:

```
src/
├── lib.rs             // mod api;
├── api.rs             // pub mod handlers; pub mod middleware;
└── api/
    ├── handlers.rs    // pub mod user; pub mod auth;
    ├── handlers/
    │   ├── user.rs
    │   └── auth.rs
    ├── middleware.rs
    └── middleware/
        └── logging.rs
```

**Key Points:**
- Each level has an entry point file (`api.rs`, `handlers.rs`, `middleware.rs`)
- Submodules declared in their parent's entry point
- Avoid nesting beyond 4 levels

---

## Visibility and Encapsulation

### Controlling Public API with `pub use`

Use `pub use` in module entry points to control what's exposed:

```rust
// database.rs (entry point)
mod pool;           // Private submodule
mod connection;     // Private submodule

// Public API - explicitly re-export
pub use pool::{Pool, PoolConfig};
pub use connection::Connection;

// Internal implementation remains private
// pool::InternalPoolManager is not visible externally
```

**Benefits:**
- Clear separation between public API and implementation details
- Easy to refactor internal structure without breaking external code
- Explicit control over exported items

---

## Anti-Patterns (What NOT to Do)

For detailed anti-patterns and examples, see @docs/ANTI_PATTERNS.md. Key module system anti-patterns:

- **Using `mod.rs`**: Use `module.rs` instead (Rust 2024 Edition)
- **Glob imports**: Use explicit `pub use` (except in test modules)
- **Circular dependencies**: Extract common types to break cycles
- **Excessive flat structure**: Group related files in module directories

---

## Filesystem Structure Principles

### 1. Single Entry Point
Each module has exactly one entry point file (`module.rs`), not `module/mod.rs`

### 2. Logical Hierarchy
File structure mirrors the logical module hierarchy

### 3. Explicit Publicity
Use `pub use` to intentionally expose API, don't default to everything public

### 4. Limited Depth
Avoid excessive nesting (>4 levels makes navigation difficult)

---

## Migration Guide

### Converting from `mod.rs` to `module.rs`

If you have old code using `mod.rs`:

**Before:**
```
src/database/mod.rs
```

**After:**
```
src/database.rs
```

**Steps:**
1. Move `module/mod.rs` → `module.rs`
2. Keep `mod submodule;` declarations in `module.rs`
3. Maintain `pub use` re-exports
4. No changes needed in parent module declaration (`mod module;` stays the same)

---

## Example: Complete Module Structure

Here's a complete example showing best practices:

```
src/
├── lib.rs
│   // mod database;
│   // mod api;
│
├── database.rs
│   // pub mod pool;
│   // pub mod migrations;
│   // pub use pool::{Pool, PoolConfig};
│
├── database/
│   ├── pool.rs
│   │   // pub struct Pool { ... }
│   │   // pub struct PoolConfig { ... }
│   │   // struct InternalPoolManager { ... }  // Not re-exported
│   │
│   └── migrations.rs
│       // pub fn run_migrations() { ... }
│
├── api.rs
│   // pub mod handlers;
│   // pub use handlers::{UserHandler, AuthHandler};
│
└── api/
    ├── handlers.rs
    │   // pub mod user;
    │   // pub mod auth;
    │   // pub use user::UserHandler;
    │   // pub use auth::AuthHandler;
    │
    └── handlers/
        ├── user.rs
        │   // pub struct UserHandler { ... }
        │
        └── auth.rs
            // pub struct AuthHandler { ... }
```

**Usage from external code:**
```rust
use my_crate::database::{Pool, PoolConfig};  // ✅ Works - explicitly re-exported
use my_crate::api::handlers::{UserHandler, AuthHandler};  // ✅ Works
use my_crate::database::pool::InternalPoolManager;  // ❌ Error - not re-exported
```

---

## Related Documentation

- **Main Quick Reference**: @CLAUDE.md (see Quick Reference section)
- **Main standards**: @CLAUDE.md
- **Project structure**: @README.md
