# Documentation Standards

## Purpose

This document defines documentation maintenance standards for the Reinhardt project, ensuring documentation stays synchronized with code changes.

---

## Core Principles

### DM-1 (MUST): Documentation Updates with Code Changes

**ALWAYS** update relevant documentation when implementing or modifying features.

**Documentation updates MUST be done in the same workflow as the code changes.**

**DO NOT** leave documentation outdated after code modifications.

**Example Workflow:**
```
1. Implement feature
2. Update relevant docs in the SAME session
3. Verify docs match implementation
4. Submit both code and docs together
```

❌ **BAD:**
```
Day 1: Implement async fetch_data()
Day 2: (Documentation still says sync)
Day 3: User asks: "Why doesn't it match docs?"
Day 4: Fix documentation
```

✅ **GOOD:**
```
Session 1:
1. Implement async fetch_data()
2. Update README.md usage section
3. Update crate documentation
4. Update API reference
5. Submit all changes together
```

---

## Documentation Locations

### DM-2 (MUST): Documentation Locations

When modifying features, check and update the following documentation as applicable:

#### Project-Level Documentation
**File:** `README.md`
**Contents:**
- Project overview
- Installation instructions
- Quick start guide
- Main features (implemented only)
- Basic usage examples
- Links to detailed documentation

**When to Update:**
- Adding new major features
- Changing installation process
- Modifying project structure
- Updating dependencies

#### Crate-Level Documentation
**File:** `crates/<crate>/README.md` (if exists)
**Contents:**
- Crate-specific overview
- Crate features
- Usage examples
- API highlights

**File:** `crates/<crate>/src/lib.rs`
**Contents:**
- Module documentation (`//!`)
- Planned features section
- Architecture overview
- Code examples

**When to Update:**
- Adding new crate features
- Changing public API
- Modifying crate behavior
- Adding planned features

#### Detailed Guides
**Location:** `docs/` directory
**Files:**
- `docs/GETTING_STARTED.md` - Getting started guide
- `docs/FEATURE_FLAGS.md` - Feature flags documentation
- `docs/tutorials/` - Tutorial files
- `docs/MODULE_SYSTEM.md` - Module system standards
- `docs/TESTING_STANDARDS.md` - Testing standards
- `docs/ANTI_PATTERNS.md` - Anti-patterns guide
- Other relevant documentation files

**When to Update:**
- Adding new features requiring detailed explanation
- Changing established patterns
- Adding new standards or conventions
- Updating tutorials

---

## Documentation Consistency

### DM-3 (MUST): Documentation Consistency

Ensure consistency across all documentation levels (project, crate, docs/).

**Consistency Checklist:**
- [ ] Terminology is consistent across all docs
- [ ] Code examples use the same style
- [ ] Version numbers match
- [ ] Links are valid and point to correct locations
- [ ] Examples actually work with current code
- [ ] API signatures match implementation

**Example - Consistent Terminology:**
```markdown
<!-- ✅ GOOD: Consistent across all docs -->
README.md:        "Use QueryBuilder to construct SQL queries"
crate README:     "QueryBuilder provides type-safe query construction"
lib.rs:           "QueryBuilder is the main interface for building queries"
docs/tutorial:    "Create queries with QueryBuilder"

<!-- ❌ BAD: Inconsistent terminology -->
README.md:        "Use QueryBuilder to construct SQL queries"
crate README:     "The QueryConstructor provides query building"
lib.rs:           "Use QueryMaker for database queries"
docs/tutorial:    "Build queries with the SqlBuilder"
```

---

## Documentation Scope

### DM-4 (SHOULD): Documentation Scope

Update documentation for:

#### New Features
- Add feature descriptions
- Add usage examples
- Add API references
- Add to feature list in README

**Example:**
```rust
// New feature implemented
pub fn validate_email(email: &str) -> Result<()> {
    // ...
}
```

Update in same session:
```markdown
<!-- README.md -->
## Features
- Email validation ✅

<!-- crates/reinhardt-validators/README.md -->
### Email Validation
Validate email addresses with RFC 5322 compliance.

<!-- crates/reinhardt-validators/src/lib.rs -->
//! ## Email Validation
//!
//! ```rust
//! use reinhardt_validators::validate_email;
//!
//! assert!(validate_email("user@example.com").is_ok());
//! ```
```

#### Modified Features
- Update affected sections to reflect changes
- Update code examples
- Update API signatures
- Add migration notes if breaking changes

**Example:**
```rust
// Changed from sync to async
pub async fn fetch_data() -> Result<Data> {
    // ...
}
```

Update immediately:
```markdown
<!-- Before -->
## Usage
```rust
let data = fetch_data()?;
```

<!-- After -->
## Usage
```rust
let data = fetch_data().await?;
```

**Migration Note:** `fetch_data()` is now async. Update calls to use `.await`.
```

#### Deprecated Features
- Mark as deprecated
- Provide migration guides
- Set removal timeline

**Example:**
```rust
#[deprecated(since = "0.2.0", note = "Use new_function instead")]
pub fn old_function() {
    // ...
}
```

Update docs:
```markdown
## Deprecated Functions

### `old_function()`
**Deprecated since:** 0.2.0
**Removed in:** 0.3.0
**Migration:** Use `new_function()` instead

```rust
// Before
old_function();

// After
new_function();
```
```

#### Removed Features
- Remove documentation
- Add migration notes if necessary
- Update examples to remove references

---

## Documentation Quality

### DM-5 (MUST): Documentation Quality

Ensure high-quality documentation:

#### Examples Must Work
All code examples in documentation must be tested and working.

**Use Doc Tests:**
```rust
/// Validates an email address.
///
/// # Examples
///
/// ```
/// use reinhardt_validators::validate_email;
///
/// assert!(validate_email("user@example.com").is_ok());
/// assert!(validate_email("invalid").is_err());
/// ```
pub fn validate_email(email: &str) -> Result<()> {
    // ...
}
```

**Test Documentation:**
```bash
cargo test --doc  # Runs all doc tests
```

#### Update Code Snippets
Keep code snippets current with API changes.

❌ **BAD - Outdated:**
```markdown
## Usage
```rust
let pool = Pool::new("postgres://localhost");  // ❌ Old API
```
```

✅ **GOOD - Current:**
```markdown
## Usage
```rust
let pool = Pool::builder()
    .url("postgres://localhost")
    .max_connections(10)
    .build()?;  // ✅ Current API
```
```

#### Verify Links
Ensure all links and references are valid.

**Check Internal Links:**
```markdown
See [Module System](docs/MODULE_SYSTEM.md)  ✅
See [Module System](docs/MODULES.md)        ❌ Wrong file
```

**Check External Links:**
```markdown
[Rust Book](https://doc.rust-lang.org/book/)  ✅
[Rust Book](https://rustbook.com/)            ❌ Wrong URL
```

#### Maintain Terminology
Use consistent terminology and formatting.

**Standardized Terms:**
- "QueryBuilder" (not "Query Builder", "QueryConstructor", "query builder")
- "ViewSet" (not "View Set", "Viewset", "view set")
- "Serializer" (not "Serialiser" in UK English context)

---

## Planned Features Location

### DM-6 (MUST): Planned Features Location

**Planned Features MUST be documented in the crate's `lib.rs` file header.**

**DO NOT include Planned Features sections in README.md files.**

**Format in `lib.rs`:**
```rust
//! # Crate Name
//!
//! Brief description of what this crate does.
//!
//! ## Features
//!
//! - Feature 1: Description of implemented feature
//! - Feature 2: Description of implemented feature
//!
//! ## Planned Features
//!
//! - Feature 3: Description of planned feature
//! - Feature 4: Description of planned feature
//!
//! ## Examples
//!
//! ```rust
//! // Example code
//! ```
```

**README.md Focus:**
```markdown
# Crate Name

Brief description.

## Features

- Feature 1: Description of implemented feature ✅
- Feature 2: Description of implemented feature ✅

<!-- ❌ NO Planned Features section here -->

## Usage

...
```

**Why?**
- Keeps planned features close to implementation code
- Better visibility during development
- README focuses on what's available NOW
- Reduces user confusion about what's actually implemented

**When Feature is Implemented:**
1. Remove from "Planned Features" in lib.rs
2. Add to "Features" section in lib.rs
3. Update README.md if it's a major feature
4. Update relevant documentation

---

## Rustdoc Formatting Standards

### DM-7 (MUST): Rustdoc Formatting Standards

Doc comments (`///` and `//!`) are processed by rustdoc and must follow specific formatting rules to avoid warnings and ensure proper HTML generation.

#### RD-1: Generic Types Must Be Wrapped in Backticks

Generic types like `<T>` are interpreted as HTML tags by rustdoc. Always wrap them in backticks.

```rust
// ✅ CORRECT
/// Returns `Option<String>` for the result
/// Uses `Result<T, Error>` for fallible operations
/// Stores items in `Vec<T>` or `HashMap<K, V>`

// ❌ INCORRECT (causes "unclosed HTML tag" warnings)
/// Returns Option<String> for the result
/// Uses Result<T, Error> for fallible operations
```

**Common types requiring backticks:**
- `Option<T>`, `Result<T, E>`, `Vec<T>`, `Box<T>`
- `Arc<T>`, `Rc<T>`, `RefCell<T>`, `Mutex<T>`
- `HashMap<K, V>`, `HashSet<T>`, `BTreeMap<K, V>`
- `Pin<T>`, `Future<Output = T>`, `Stream<Item = T>`
- Any custom generic: `MyType<T>`, `Builder<S>`

#### RD-2: Macro Attributes Must Be Wrapped in Backticks

Attributes like `#[derive]` are interpreted as markdown links by rustdoc. Always wrap them in backticks.

```rust
// ✅ CORRECT
/// Apply `#[inject]` to enable dependency injection
/// Use `#[async_trait]` for async trait methods
/// Add `#[derive(Debug, Clone)]` to the struct

// ❌ INCORRECT (causes "unresolved link" warnings)
/// Apply #[inject] to enable dependency injection
/// Use #[async_trait] for async trait methods
```

**Common attributes requiring backticks:**
- `#[derive(...)]`, `#[cfg(...)]`, `#[allow(...)]`
- `#[test]`, `#[tokio::test]`, `#[rstest]`
- `#[inject]`, `#[model(...)]`, `#[api(...)]`
- Any attribute: `#[serde(...)]`, `#[sqlx(...)]`

#### RD-3: URLs Must Be Wrapped in Angle Brackets or Backticks

Bare URLs in doc comments trigger "bare URL" warnings. Wrap them properly.

```rust
// ✅ CORRECT
/// See <https://docs.rs/reinhardt> for API documentation
/// Documentation at `https://github.com/kent8192/reinhardt-web`
/// Visit [our docs](https://docs.rs/reinhardt) for more info

// ❌ INCORRECT (causes "bare URL" warnings)
/// See https://docs.rs/reinhardt for API documentation
/// Documentation at https://github.com/kent8192/reinhardt-web
```

**URL formatting options:**
- Angle brackets: `<https://example.com>` → clickable link
- Backticks: `` `https://example.com` `` → code formatting
- Markdown link: `[text](https://example.com)` → named link

#### RD-4: Code Blocks Must Specify Language

Code blocks without language specification may cause parsing warnings.

````rust
// ✅ CORRECT
/// ```rust
/// let x = 42;
/// ```
///
/// ```sql
/// SELECT * FROM users;
/// ```
///
/// ```ignore
/// // This code won't be tested
/// ```

// ❌ INCORRECT (may cause warnings)
/// ```
/// let x = 42;
/// ```
````

**Common language tags:**
- `rust` - Rust code (default for doc tests)
- `ignore` - Rust code that won't be tested
- `no_run` - Rust code that compiles but won't run
- `compile_fail` - Rust code expected to fail compilation
- `sql`, `json`, `toml`, `bash`, `text` - Other languages

#### RD-5: Bracket Patterns Must Be Wrapped in Backticks

Array/slice indexing patterns are interpreted as markdown links by rustdoc.

```rust
// ✅ CORRECT
/// Access the first element via `array[0]`
/// Use `slice[1..3]` for a range
/// Get nested value with `map["key"]`

// ❌ INCORRECT (causes "unresolved link" warnings)
/// Access the first element via array[0]
/// Use slice[1..3] for a range
```

#### RD-6: Feature-Gated Items Must Use Backticks (Not Intra-Doc Links)

Items behind `#[cfg(feature = "...")]` are not in scope when docs are built without that feature. Use backticks instead of intra-doc links.

```rust
// ✅ CORRECT (works regardless of enabled features)
/// Enable `compression` feature to use `GZipMiddleware`
/// See `CockroachDBBackend` for CockroachDB support (requires `cockroachdb-backend`)

// ❌ INCORRECT (causes "unresolved link" warnings when feature disabled)
/// Enable `compression` feature to use [`GZipMiddleware`]
/// See [`CockroachDBBackend`] for CockroachDB support
```

**Why this happens:**
- Intra-doc links (`` [`TypeName`] ``) are resolved at doc build time
- Feature-gated items don't exist in scope when their feature is disabled
- `cargo doc` without `--all-features` will fail to resolve these links

**Pattern:**
- If an item is behind `#[cfg(feature = "X")]`, use `` `ItemName` `` not `` [`ItemName`] ``
- Mention the required feature in the description: "(requires `X` feature)"

#### Quick Reference Table

| Pattern | Incorrect | Correct |
|---------|-----------|---------|
| Generic types | `Option<T>` | `` `Option<T>` `` |
| Attributes | `#[derive]` | `` `#[derive]` `` |
| URLs | `https://...` | `<https://...>` or `` `https://...` `` |
| Code blocks | ` ``` ` | ` ```rust ` |
| Array access | `arr[0]` | `` `arr[0]` `` |
| Feature-gated items | `` [`TypeName`] `` | `` `TypeName` `` |

#### Verification

Run the following command to check for rustdoc warnings:

```bash
cargo doc --workspace --all-features 2>&1 | grep "warning:"
```

All doc comments should produce zero warnings.

---

## Documentation Workflow

### Standard Documentation Update Process

When implementing or modifying a feature:

```
1. ✅ Implement the code
2. ✅ Update lib.rs documentation
3. ✅ Update README.md if needed
4. ✅ Update crate README if exists
5. ✅ Update docs/ files if relevant
6. ✅ Run doc tests: cargo test --doc
7. ✅ Build docs: cargo doc --no-deps --open
8. ✅ Verify examples work
9. ✅ Check links are valid
10. ✅ Submit code + docs together
```

### Documentation Review Checklist

Before submitting:

- [ ] All relevant documentation files updated
- [ ] Code examples tested and working
- [ ] API signatures match implementation
- [ ] Terminology consistent across all docs
- [ ] Links are valid
- [ ] Formatting is correct
- [ ] No outdated information
- [ ] Planned features in lib.rs, not README
- [ ] Migration guides for breaking changes
- [ ] Doc tests pass
- [ ] Rustdoc warnings: zero (see DM-7)

---

## Examples

### Example 1: Adding a New Feature

**Code Implementation:**
```rust
// crates/reinhardt-validators/src/email.rs
pub fn validate_email(email: &str) -> Result<(), ValidationError> {
    // Implementation
}
```

**Documentation Updates in Same Session:**

**1. lib.rs:**
```rust
//! # reinhardt-validators
//!
//! Validation utilities for Reinhardt.
//!
//! ## Features
//!
//! - Email validation with RFC 5322 compliance ✅ NEW
//! - Phone number validation
//!
//! ## Examples
//!
//! ```rust
//! use reinhardt_validators::validate_email;
//!
//! assert!(validate_email("user@example.com").is_ok());
//! assert!(validate_email("invalid").is_err());
//! ```
```

**2. README.md:**
```markdown
## Features

- Email validation with RFC 5322 compliance ✅ NEW
- Phone number validation
```

**3. docs/tutorials/validation.md:**
```markdown
### Email Validation

Validate email addresses:

```rust
use reinhardt_validators::validate_email;

let result = validate_email("user@example.com");
assert!(result.is_ok());
```
```

### Example 2: Changing API (Breaking Change)

**Code Change:**
```rust
// Before
pub fn connect(url: &str) -> Connection {
    // ...
}

// After (breaking change - returns Result)
pub fn connect(url: &str) -> Result<Connection> {
    // ...
}
```

**Documentation Updates:**

**1. lib.rs:**
```rust
//! ## Breaking Changes in 0.2.0
//!
//! - `connect()` now returns `Result<Connection>` instead of `Connection`
//!
//! ```rust
//! // Before
//! let conn = connect("postgres://localhost");
//!
//! // After
//! let conn = connect("postgres://localhost")?;
//! ```
```

**2. README.md:**
```markdown
## Migration from 0.1.x to 0.2.0

### `connect()` now returns Result

```rust
// Before (0.1.x)
let conn = connect("postgres://localhost");

// After (0.2.0)
let conn = connect("postgres://localhost")?;
```
```

**3. CHANGELOG.md:**

**Language Requirement:**
- **MUST be written in English** (no exceptions)
- All entries, descriptions, and migration guides must use English

```markdown
## [0.2.0] - 2025-01-XX

### Breaking Changes

- `connect()` now returns `Result<Connection>` for better error handling
```

---

## Diagram Standards

### DM-8 (SHOULD): Use Mermaid for Architecture Diagrams

When documenting architecture, data flow, or relationships between components,
**prefer Mermaid diagrams over ASCII art**.

#### Setup

Add `aquamarine` as a dependency in the crate's `Cargo.toml`:

```toml
[dependencies]
aquamarine = { workspace = true }
```

Note: `aquamarine` must be in `[dependencies]` (not `[dev-dependencies]`) because
it needs to be available during documentation generation (`cargo doc`).

#### Usage

Apply the `#[cfg_attr(doc, aquamarine::aquamarine)]` attribute to items with Mermaid diagrams:

```rust
#[cfg_attr(doc, aquamarine::aquamarine)]
/// Component architecture:
///
/// ```mermaid
/// graph TD
///     A[Request] --> B{Router}
///     B --> C[Handler]
/// ```
pub struct Router { ... }
```

#### Diagram Types Reference

| Diagram Type | Mermaid Syntax | Example Use Case |
|--------------|----------------|------------------|
| Flowchart | `graph TD/LR` | Request flow, decision trees |
| Sequence | `sequenceDiagram` | API call sequences |
| Class | `classDiagram` | Trait hierarchies |
| State | `stateDiagram-v2` | State machines |

#### Limitations

- `#[aquamarine]` cannot be applied to module-level documentation (`//!`)
- Move diagrams to the primary public type documentation instead

#### When to Keep ASCII Art

- Simple inline diagrams (1-2 lines)
- Terminal output examples
- Code structure illustrations where text alignment matters

#### Verification

```bash
cargo doc --package <crate-name> --open
```

---

## Related Documentation

- **Main Quick Reference**: @CLAUDE.md (see Quick Reference section)
- **Main standards**: @CLAUDE.md
- **Module system**: @docs/MODULE_SYSTEM.md
- **Testing standards**: @docs/TESTING_STANDARDS.md
- **Anti-patterns**: @docs/ANTI_PATTERNS.md
