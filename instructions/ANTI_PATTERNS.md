# Anti-Patterns and What NOT to Do

## Purpose

This document explicitly lists common mistakes, anti-patterns, and practices to
avoid in the Reinhardt project. Use this as a quick reference for code review
and development.

---

## Code Organization Anti-Patterns

### ❌ Using `mod.rs` Files

**DON'T:**

```
src/database/mod.rs  // ❌ Old Rust 2015 style
```

**DO:**

```
src/database.rs      // ✅ Rust 2024 style
```

**Why?** `mod.rs` is deprecated and makes file navigation harder. See
@docs/MODULE_SYSTEM.md

### ❌ Glob Imports

**DON'T:**

```rust
pub use database::*;  // ❌ Pollutes namespace
```

**DO:**

```rust
pub use database::{Pool, Connection, PoolConfig};  // ✅ Explicit
```

**Exception**: Test modules may use `use super::*;` for convenience:

```rust
#[cfg(test)]
mod tests {
    use super::*;  // ✅ Acceptable in test modules

    #[test]
    fn test_functionality() {
        // Test code can access parent module items
    }
}
```

**Why?** Makes it unclear what's exported and causes naming conflicts. However,
in test modules, the scope is limited and readability benefits outweigh the
risks.

### ❌ Circular Module Dependencies

**DON'T:**

```rust
// module_a.rs
use crate::module_b::TypeB;  // ❌ A → B

// module_b.rs
use crate::module_a::TypeA;  // ❌ B → A (circular!)
```

**DO:**

```rust
// types.rs - Extract common types
pub struct TypeA;
pub struct TypeB;

// module_a.rs
use crate::types::{TypeA, TypeB};  // ✅ No cycle

// module_b.rs
use crate::types::{TypeA, TypeB};  // ✅ No cycle
```

**Why?** Causes compilation errors and indicates poor module design.

### ❌ Excessive Flat Structure

**DON'T:**

```
src/
├── user_handler.rs      // ❌ Related files
├── user_service.rs      // scattered across
├── user_repository.rs   // the same level
├── auth_handler.rs
├── auth_service.rs
└── auth_repository.rs
```

**DO:**

```
src/
├── user.rs              // ✅ Grouped by
├── user/                // feature/domain
│   ├── handler.rs
│   ├── service.rs
│   └── repository.rs
├── auth.rs
└── auth/
    ├── handler.rs
    ├── service.rs
    └── repository.rs
```

**Why?** Grouping related files improves maintainability and navigation.

### ❌ Deep Relative Paths

**DON'T:**

```rust
use crate::../../config/settings.toml;  // ❌ Goes up 2+ levels
use super::super::super::utils;         // ❌ Confusing
```

**DO:**

```rust
use crate::config::Settings;  // ✅ Absolute from crate root
use super::sibling_module;    // ✅ One level up is OK
```

**Why?** Deep relative paths are hard to understand and maintain.

---

## Code Style Anti-Patterns

### ❌ Excessive `.to_string()` Calls

**DON'T:**

```rust
fn process_name(name: &str) -> String {
    let greeting = format!("Hello, {}", name.to_string());  // ❌ Unnecessary
    greeting.to_string()  // ❌ Already a String!
}
```

**DO:**

```rust
fn process_name(name: &str) -> String {
    format!("Hello, {}", name)  // ✅ name is already &str
}

// Or use Cow for conditional ownership
use std::borrow::Cow;

fn process_name(name: &str) -> Cow<str> {
    if name.is_empty() {
        Cow::Borrowed("Anonymous")
    } else {
        Cow::Borrowed(name)
    }
}
```

**Why?** Unnecessary allocations hurt performance. Prefer borrowing.

### ❌ Leaving Obsolete Code

**DON'T:**

```rust
// fn old_implementation() {  // ❌ Commented out code
//     // ...
// }

pub fn new_implementation() {
    // ...
}
```

**DO:**

```rust
pub fn new_implementation() {  // ✅ Old code deleted
    // ...
}
```

**Why?** Git history preserves old code. Commented code creates clutter.

### ❌ Deletion Record Comments

**DON'T:**

```rust
// Removed empty test: test_foo - This test was empty  // ❌ Don't document deletions
// Deleted: old_module.rs (deprecated)                  // ❌ Git history has this

pub fn active_function() {
    // ...
}
```

**DO:**

```rust
pub fn active_function() {  // ✅ No deletion comments
    // ...
}

// If important notes are needed, extract to docs/IMPLEMENTATION_NOTES.md
```

**Why?** Git history is the permanent record. Comments clutter the codebase.

### ❌ Using Alternative TODO Notations

**DON'T:**

```rust
// Implementation Note: This needs to be completed    // ❌ Custom notation
// FIXME: Add validation                              // ❌ Use TODO instead
// NOTE: Not implemented yet                          // ❌ NOTE is for info only
```

**DO:**

```rust
// TODO: Implement input validation logic
fn validate_input(data: &str) -> Result<()> {
    todo!("Add validation - planned for next sprint")
}

// Or for intentionally omitted features:
fn legacy_feature() {
    unimplemented!("This feature is intentionally not supported")
}
```

**Why?** Standardized notation (`TODO`, `todo!()`, `unimplemented!()`) is
searchable and clear.

**CI Enforcement:**

The TODO Check CI workflow automatically detects TODO/FIXME comments and `todo!()` macros
in pull requests. PRs introducing new unresolved TODOs will fail the CI check.
Only `unimplemented!()` (for permanently excluded features) is permitted.

Additionally, Clippy enforces the following deny lints:
- `clippy::todo` - prevents `todo!()` macros
- `clippy::unimplemented` - prevents `unimplemented!()` macros (use `#[allow(clippy::unimplemented)]` with comment for intentional exclusions)
- `clippy::dbg_macro` - prevents `dbg!()` macros

### ❌ Unmarked Placeholder Implementations

**DON'T:**

```rust
pub fn get_cache_config() -> CacheConfig {
    CacheConfig::default()  // ❌ Looks like production code!
}

pub fn send_email(to: &str, body: &str) -> Result<()> {
    println!("Would send: {}", body);  // ❌ Mock without marker
    Ok(())
}
```

**DO:**

```rust
pub fn get_cache_config() -> CacheConfig {
    todo!("Implement cache configuration loading from settings")
}

pub fn send_email(to: &str, body: &str) -> Result<()> {
    // TODO: Integrate with actual email service provider
    println!("Would send: {}", body);
    Ok(())
}
```

**Why?** Unmarked placeholders can be mistaken for production code.

### ❌ Undocumented `#[allow(...)]` Attributes

**DON'T:**

```rust
// No explanation why this is allowed
#[allow(dead_code)]
struct ReservedField {
	future_field: Option<String>,  // ❌ Why is this unused?
}

// No explanation for macro-required imports
#[allow(unused_imports)]
use crate::models::User;  // ❌ Why is this "unused"?
```

**DO:**

```rust
// SQLite-specific fields are parsed but intentionally excluded
// from current implementation. Reserved for future constraint generation.
#[allow(dead_code)]
struct ModelConfig {
	strict: Option<bool>,        // Will be used in future
	without_rowid: Option<bool>, // Will be used in future
}

// Used by #[model] macro for type inference in ForeignKeyField<User>.
// The macro requires this type in scope for generating relationship metadata.
#[allow(unused_imports)]
use crate::models::User;
```

**Why?** `#[allow(...)]` attributes suppress important compiler warnings. Every
suppression must be justified with a clear comment explaining:

- **For future implementation**: What will use it and when
- **For macro requirements**: Which macro needs it and why
- **For test code**: What test pattern requires it
- **For Clippy rules**: Why the rule doesn't apply here

**Common Valid Use Cases:**

1. **Macro Type Inference**: `ForeignKeyField<T>`, `ManyToManyField<T, U>`
   require type imports
2. **Test Fixtures**: Test models used only by macro expansion
3. **Future Implementation**: Fields reserved with `todo!()` for planned
   features
4. **Recursive Functions**: Clippy warnings that don't apply to algorithm
   requirements
5. **Intentionally Excluded**: Features marked with `unimplemented!()` for
   architectural reasons

---

## Testing Anti-Patterns

### ❌ Skeleton Tests

Tests without meaningful assertions that always pass.

**Why?** Tests must be capable of failing. See @docs/TESTING_STANDARDS.md TP-1
for detailed examples.

### ❌ Tests Without Reinhardt Components

Tests that only verify standard library or third-party behavior.

**Why?** Every test must verify at least one Reinhardt component. See
@docs/TESTING_STANDARDS.md TP-2.

### ❌ Tests Without Cleanup

Tests that create files/resources without cleaning up.

**Why?** Test artifacts must be cleaned up. See @docs/TESTING_STANDARDS.md TI-3
for cleanup techniques.

### ❌ Global State Tests Without Serialization

Tests modifying global state without `#[serial]` attribute.

**Why?** Global state tests can conflict if run in parallel. See
@docs/TESTING_STANDARDS.md TI-4 for serial test patterns.

### ❌ Loose Assertions

Using `contains()`, range checks, or loose pattern matching instead of exact
value assertions.

**Why?** Loose assertions can pass with incorrect values. See
@docs/TESTING_STANDARDS.md TI-5 for assertion strictness guidelines and
acceptable exceptions.

### ❌ Tests Without Clear AAA Structure

Tests that mix setup, execution, and verification without clear phase separation,
or use non-standard phase labels (`// Setup`, `// Execute`, `// Verify`).

**Why?** Clear Arrange-Act-Assert structure improves test readability and
maintainability. See @docs/TESTING_STANDARDS.md TI-6 for AAA pattern guidelines
and examples.

---

## File Management Anti-Patterns

### ❌ Saving Files to Project Directory

**DON'T:**

```bash
# Script execution
./analyze.sh > results.md          # ❌ Saved to project root
python3 process.py > output.txt    # ❌ Saved to project root
```

**DO:**

```bash
# Script execution
./analyze.sh > /tmp/results.md     # ✅ Use /tmp
python3 process.py > /tmp/output.txt

# Delete when done
rm /tmp/results.md /tmp/output.txt
```

**Why?** Keeps project directory clean. Prevents accidental commits.

### ❌ Leaving Backup Files

**DON'T:**

```bash
# After editing
ls
file.rs
file.rs.bak        # ❌ Backup file left behind
config.toml.old    # ❌ Old version not deleted
script.sh~         # ❌ Temporary backup
```

**DO:**

```bash
# Clean up immediately
rm file.rs.bak config.toml.old script.sh~  # ✅ Delete backups
```

**Why?** Backup files clutter the codebase and can be accidentally committed.

### ❌ Not Cleaning Up /tmp Files

**DON'T:**

```bash
# Create temp files
echo "data" > /tmp/analysis_results.md
./process.sh > /tmp/output.txt

# ... do work ...

# ❌ Never delete them
```

**DO:**

```bash
# Create temp files
echo "data" > /tmp/analysis_results.md
./process.sh > /tmp/output.txt

# ... do work ...

# ✅ Clean up when done
rm /tmp/analysis_results.md /tmp/output.txt
```

**Why?** Prevents /tmp accumulation and ensures clean environment.

---

## Workflow Anti-Patterns

### ❌ Committing Without User Instruction

**DON'T:**

```bash
# ❌ AI creates commit automatically
git add .
git commit -m "feat: Add feature"
```

**DO:**

```bash
# ✅ Wait for explicit user instruction
# User: "Please commit these changes"
git add <specific files>
git commit -m "..."
```

**Why?** Commits should only be made with explicit user authorization.

### ❌ Batch Operations Without Dry-Run

**DON'T:**

```bash
# ❌ Bulk replace without verification
sed -i 's/old_pattern/new_pattern/g' **/*.rs
```

**DO:**

```bash
# ✅ Create dry-run script first
cat > /tmp/dryrun.sh << 'EOF'
grep -r "old_pattern" --include="*.rs" .
EOF

bash /tmp/dryrun.sh  # Review scope

# Only proceed after confirmation
cat > /tmp/replace.sh << 'EOF'
sed -i 's/old_pattern/new_pattern/g' **/*.rs
EOF

bash /tmp/replace.sh
rm /tmp/dryrun.sh /tmp/replace.sh
```

**Why?** Dry-run prevents unintended bulk changes.

### ❌ Monolithic Commits

**DON'T:**

```bash
# ❌ One huge commit for entire feature
git add .
git commit -m "feat(auth): Implement authentication feature"
# Changes: JWT, sessions, OAuth, password hashing, middleware, tests...
```

**DO:**

```bash
# ✅ Split into specific intents
git add src/auth/password.rs
git commit -m "feat(auth): Implement bcrypt password hashing"

git add src/auth/jwt.rs
git commit -m "feat(auth): Add JWT token generation with RS256"

git add src/auth/session.rs
git commit -m "feat(auth): Create session storage middleware"
```

**Why?** Small, focused commits make history easier to understand and review.

---

## Documentation Anti-Patterns

### ❌ Outdated Documentation After Code Changes

**DON'T:**

```rust
// Code changes from sync to async
pub async fn fetch_data() -> Result<Data> {
    // ...
}
```

```markdown
<!-- README.md still says: -->

## Usage

Use `fetch_data()` synchronously... // ❌ Outdated!
```

**DO:**

```rust
pub async fn fetch_data() -> Result<Data> {
    // ...
}
```

```markdown
<!-- README.md updated: -->

## Usage

Use `fetch_data().await` asynchronously... // ✅ Current!
```

**Why?** Documentation must be updated with code changes in the same workflow.

### ❌ Planned Features in README

**DON'T:**

```markdown
<!-- README.md -->

## Features

- User authentication ✅
- Database migrations ✅

### Planned Features

- GraphQL support
- WebSockets
```

**DO:**

```rust
//! crates/reinhardt-api/src/lib.rs
//!
//! ## Planned Features
//!
//! - GraphQL support
//! - WebSockets
```

```markdown
<!-- README.md - Only implemented features -->

## Features

- User authentication ✅
- Database migrations ✅
```

**Why?** Planned features belong in `lib.rs`, README shows implemented features
only.

---

## Related Documentation

- **Main Quick Reference**: @CLAUDE.md (see Quick Reference section)
- **Main standards**: @CLAUDE.md
- **Module system**: @docs/MODULE_SYSTEM.md
- **Testing standards**: @docs/TESTING_STANDARDS.md
- **Documentation standards**: @docs/DOCUMENTATION_STANDARDS.md
