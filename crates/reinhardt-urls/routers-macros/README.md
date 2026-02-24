# reinhardt-routers-macros

Procedural macros for URL routing in Reinhardt framework

## Overview

`reinhardt-routers-macros` provides procedural macros to simplify URL routing definition in the Reinhardt framework, enabling compile-time route validation and type-safe URL pattern generation.

## Features

### Implemented ✓

- **Compile-time route validation** - Path syntax validated at compile time
- **Type-safe URL pattern generation** - Validated paths returned as string literals
- **Path parameter validation** - Parameter names must be valid snake_case identifiers
- **Comprehensive error messages** - Detailed compile-time error messages with examples

## Installation

Add `reinhardt` to your `Cargo.toml`:

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", features = ["urls-routers-macros"] }

# Or use a preset:
# reinhardt = { version = "0.1.0-alpha.1", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.1.0-alpha.1", features = ["full"] }      # All features
```

Then import router macro features:

```rust
use reinhardt::urls::routers_macros::path;
```

**Note:** Router macro features are included in the `standard` and `full` feature presets.

## Usage Examples

### Basic Path Validation

The `path!` macro validates URL path syntax at compile time:

```rust
use reinhardt::urls::routers_macros::path;

// Simple static path
let users_path = path!("/users/");
assert_eq!(users_path, "/users/");

// Path with single parameter
let user_detail = path!("/users/{id}/");
assert_eq!(user_detail, "/users/{id}/");

// Path with multiple parameters
let user_post = path!("/users/{user_id}/posts/{post_id}/");
assert_eq!(user_post, "/users/{user_id}/posts/{post_id}/");

// Path with underscores in parameters
let profile_path = path!("/users/{user_id}/profile/");
assert_eq!(profile_path, "/users/{user_id}/profile/");
```

### Valid Path Patterns

The `path!` macro accepts paths with:

**Static segments:**
```rust
path!("/api/v1/users/")         // ✅ Simple static path
path!("/user-profiles/")        // ✅ Hyphens allowed
path!("/files/document.pdf")    // ✅ Dots allowed
path!("/api/v1.0/users/")       // ✅ Version numbers
```

**Dynamic parameters:**
```rust
path!("/users/{id}/")                           // ✅ Single parameter
path!("/users/{user_id}/")                      // ✅ Snake_case parameter
path!("/users/{_id}/")                          // ✅ Leading underscore
path!("/users/{user_id_123}/")                  // ✅ Numbers in parameter
path!("/users/{user_id}/posts/{post_id}/")      // ✅ Multiple parameters
path!("/items/{item_id}/details/")              // ✅ Parameter in middle
```

### Invalid Path Patterns (Compile-Time Errors)

The following patterns will produce compile-time errors:

**Missing leading slash:**
```rust,compile_fail
path!("users/")  // ❌ Error: URL path must start with '/'
```

**Invalid parameter names:**
```rust,compile_fail
path!("/users/{userId}/")   // ❌ Error: Parameter names must be snake_case
path!("/users/{user-id}/")  // ❌ Error: Hyphens not allowed in parameters
path!("/users/{UserId}/")   // ❌ Error: Uppercase not allowed
```

**Empty parameters:**
```rust,compile_fail
path!("/users/{}/")  // ❌ Error: Empty parameter name
```

**Unmatched braces:**
```rust,compile_fail
path!("/users/{id/")    // ❌ Error: Unmatched '{'
path!("/users/id}/")    // ❌ Error: Unmatched '}'
```

**Nested parameters:**
```rust,compile_fail
path!("/users/{{id}}/")  // ❌ Error: Parameters cannot be nested
```

**Double slashes:**
```rust,compile_fail
path!("/users//posts/")  // ❌ Error: Paths should not contain consecutive slashes
```

**Invalid characters:**
```rust,compile_fail
path!("/users/{id}?query=1")  // ❌ Error: Invalid character '?' (query strings not in path)
path!("/users/{id}#section")  // ❌ Error: Invalid character '#' (fragments not in path)
```

## API Reference

### `path!` Macro

```rust
macro_rules! path {
    ($path:literal) => { ... };
}
```

**Parameters:**
- `$path: literal` - String literal containing the URL path pattern

**Returns:**
- `&'static str` - Validated path string literal (same as input if valid)

**Validation Rules:**

1. **Path Structure:**
   - Must start with `/`
   - No double slashes `//` (except after protocol, which is not applicable here)
   - No consecutive slashes

2. **Parameter Format:**
   - Parameters enclosed in `{}`
   - Parameter names must be valid snake_case identifiers
   - Must start with lowercase letter or underscore (`a-z` or `_`)
   - May contain lowercase letters, digits, and underscores (`a-z`, `0-9`, `_`)
   - Cannot be nested (e.g., `{{inner}}` is invalid)

3. **Path Characters (outside parameters):**
   - Alphanumeric: `a-z`, `A-Z`, `0-9`
   - Special: `-`, `_`, `/`, `.`, `*`
   - Other characters are not allowed

**Compile-Time Error Messages:**

The macro provides detailed error messages with examples:

- `MustStartWithSlash` - Path doesn't start with `/`
- `UnmatchedOpenBrace` - Missing closing `}`
- `UnmatchedCloseBrace` - Missing opening `{`
- `EmptyParameterName` - Empty `{}` found
- `InvalidParameterName` - Parameter not in snake_case format
- `DoubleSlash` - Consecutive slashes found
- `InvalidCharacter` - Character not allowed in path
- `NestedParameters` - Nested braces found

## Integration with Routing

The `path!` macro is typically used when defining routes:

```rust
use reinhardt::urls::routers_macros::path;

// Define route patterns
let list_path = path!("/users/");
let detail_path = path!("/users/{id}/");
let nested_path = path!("/users/{user_id}/posts/{post_id}/");

// Use with router (example)
// router.add(list_path, handler);
// router.add(detail_path, detail_handler);
```

## Error Handling

All validation errors are caught at compile time, not runtime. This ensures:

- **No runtime path validation overhead** - All checks done during compilation
- **Immediate feedback** - Errors shown in IDE and during `cargo build`
- **Type safety** - Invalid paths cannot be compiled into the binary

**Example Error Output:**

```
error: URL path must start with '/'

             Example: path!("/users/") instead of path!("users/")
 --> src/main.rs:5:18
  |
5 |     let path = path!("users/");
  |                      ^^^^^^^^^
```

## Limitations

**Current Implementation:**
- Only the `path!` macro is implemented
- Route registration macros are planned but not yet available
- ViewSet routing macros are planned but not yet available

## License

Licensed under the BSD 3-Clause License.
