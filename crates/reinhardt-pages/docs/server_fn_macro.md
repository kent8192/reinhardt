# server_fn Macro Documentation

## Overview

The `server_fn` macro in `reinhardt-pages` provides a seamless way to define server functions that can be called from both server and client (WASM) environments. It automatically generates:

1. Server-side implementation with dependency injection
2. Client-side HTTP request stub (for WASM)
3. Route registration for automatic endpoint discovery

## Key Features

### Automatic Client Stub Generation

The macro automatically generates client-side code that:
- Serializes function arguments to JSON/URL-encoded/MessagePack
- Sends HTTP POST requests to the server endpoint
- Deserializes responses
- **Excludes `#[inject]` parameters** from the client-side signature

### Dependency Injection Support

Use `#[inject]` attribute to inject dependencies on the server side:

```rust
use reinhardt::pages::server_fn::{ServerFnError, server_fn};
use reinhardt::DatabaseConnection;

#[server_fn(use_inject = true)]
pub async fn get_data(
    id: i64,
    #[inject] db: DatabaseConnection,
) -> Result<String, ServerFnError> {
    // Server-side implementation
    // `db` is automatically resolved from DI context
    Ok(format!("Data for id: {}", id))
}
```

### Client-Side Usage

The generated client stub automatically excludes `#[inject]` parameters:

```rust
// WASM client code
async fn fetch_data() {
    // Notice: no `db` parameter required
    let result = get_data(42).await;
    match result {
        Ok(data) => log!("Got data: {}", data),
        Err(e) => log!("Error: {:?}", e),
    }
}
```

## How It Works

### Code Generation Flow

1. **Parse Function**: The macro analyzes the function signature and identifies `#[inject]` parameters
2. **Generate Server Code**: Creates server-side function with DI resolution
3. **Generate Client Stub**: Creates WASM client stub with filtered parameters (excluding `#[inject]`)
4. **Register Route**: Automatically registers the endpoint for routing

### Example Expansion

**Input:**
```rust
#[server_fn(use_inject = true)]
pub async fn create_user(
    username: String,
    email: String,
    #[inject] db: DatabaseConnection,
) -> Result<User, ServerFnError> {
    // Implementation
}
```

**Generates (conceptual):**

```rust
// Server-side (cfg not wasm32)
pub async fn create_user(
    username: String,
    email: String,
    db: DatabaseConnection,
) -> Result<User, ServerFnError> {
    // Original implementation
}

// Client-side (cfg wasm32)
pub async fn create_user(
    username: String,
    email: String,
) -> Result<User, ServerFnError> {
    #[derive(Serialize)]
    struct CreateUserArgs {
        username: String,
        email: String,
        // Note: `db` is NOT included
    }

    let endpoint = "/api/server_fn/create_user";
    let args = CreateUserArgs { username, email };
    let body = serde_json::to_string(&args)?;

    let response = gloo_net::http::Request::post(endpoint)
        .header("Content-Type", "application/json")
        .body(body)?
        .send()
        .await?;

    response.json().await
}

// Server handler (cfg not wasm32)
pub async fn __server_fn_handler_create_user(
    req: Request,
) -> Result<String, String> {
    // Parse request body
    let args: CreateUserArgs = serde_json::from_str(&body)?;

    // Resolve dependencies from DI context
    let db = resolve_dependency::<DatabaseConnection>(&di_ctx).await?;

    // Call original function
    let result = create_user(args.username, args.email, db).await;

    // Serialize response
    serde_json::to_string(&result)
}
```

## Migration from Manual Double Definition

### Before (v0.1.0-alpha.1 and earlier)

Previously, you had to manually define the function twice:

```rust
// Server-side
#[cfg(not(target_arch = "wasm32"))]
#[server_fn(use_inject = true)]
pub async fn get_questions(
    #[inject] _db: DatabaseConnection,
) -> Result<Vec<QuestionInfo>, ServerFnError> {
    // Server implementation
}

// Client-side (manual stub)
#[cfg(target_arch = "wasm32")]
#[server_fn]
pub async fn get_questions() -> Result<Vec<QuestionInfo>, ServerFnError> {
    unreachable!("Replaced by macro")
}
```

### After (v0.1.0-alpha.2+)

Now you only need a single definition:

```rust
// Single definition works for both environments
#[server_fn(use_inject = true)]
pub async fn get_questions(
    #[inject] _db: DatabaseConnection,
) -> Result<Vec<QuestionInfo>, ServerFnError> {
    // Server implementation
}
```

**The macro automatically:**
- Generates server-side code with `#[inject]` parameters
- Generates client-side stub **without** `#[inject]` parameters
- Handles conditional compilation for both targets

## Macro Attributes

### `#[server_fn]`

Basic usage without options:

```rust
#[server_fn]
pub async fn simple_function() -> Result<String, ServerFnError> {
    Ok("Hello".to_string())
}
```

### `#[server_fn(use_inject = true)]`

Enable dependency injection:

```rust
#[server_fn(use_inject = true)]
pub async fn with_di(
    #[inject] db: DatabaseConnection,
) -> Result<(), ServerFnError> {
    Ok(())
}
```

### `#[server_fn(codec = "json")]`

Specify serialization codec (default: "json"):

```rust
#[server_fn(codec = "json")]  // JSON (default)
#[server_fn(codec = "url")]   // URL-encoded
#[server_fn(codec = "msgpack")] // MessagePack
```

### `#[server_fn(no_csrf = true)]`

Disable CSRF protection:

```rust
#[server_fn(no_csrf = true)]
pub async fn public_endpoint() -> Result<String, ServerFnError> {
    Ok("Public data".to_string())
}
```

## Best Practices

### 1. Use `#[inject]` for Server-Only Dependencies

```rust
#[server_fn(use_inject = true)]
pub async fn get_user(
    user_id: i64,
    #[inject] db: DatabaseConnection,
    #[inject] session: SessionData,
) -> Result<UserInfo, ServerFnError> {
    // `db` and `session` are injected on server
    // Client calls: get_user(user_id)
}
```

### 2. Keep Function Signatures Simple

```rust
// ✅ Good: Simple types that serialize well
#[server_fn]
pub async fn create_post(
    title: String,
    content: String,
) -> Result<PostInfo, ServerFnError> { }

// ❌ Avoid: Complex types that don't serialize
#[server_fn]
pub async fn bad_function(
    callback: Box<dyn Fn()>, // Won't serialize!
) -> Result<(), ServerFnError> { }
```

### 3. Use Shared Types

Define shared types in a module accessible to both server and client:

```rust
// src/shared/types.rs
#[derive(Serialize, Deserialize)]
pub struct CreatePostRequest {
    pub title: String,
    pub content: String,
}

#[derive(Serialize, Deserialize)]
pub struct PostInfo {
    pub id: i64,
    pub title: String,
}

// src/server_fn/posts.rs
use crate::shared::types::{CreatePostRequest, PostInfo};

#[server_fn(use_inject = true)]
pub async fn create_post(
    request: CreatePostRequest,
    #[inject] db: DatabaseConnection,
) -> Result<PostInfo, ServerFnError> {
    // Implementation
}
```

### 4. Handle Errors Properly

```rust
#[server_fn(use_inject = true)]
pub async fn safe_operation(
    #[inject] db: DatabaseConnection,
) -> Result<Data, ServerFnError> {
    let data = fetch_data(&db)
        .await
        .map_err(|e| ServerFnError::application(e.to_string()))?;

    Ok(data)
}
```

## Troubleshooting

### Issue: "cannot find type `X` in this scope" in WASM build

**Cause**: Server-only types are not available in WASM environment.

**Solution**: Use conditional imports:

```rust
// Common imports
use reinhardt::pages::server_fn::{ServerFnError, server_fn};

// Server-only imports
#[cfg(not(target_arch = "wasm32"))]
use {
    reinhardt::DatabaseConnection,
    crate::models::User,
};

// WASM-only imports (if needed)
#[cfg(target_arch = "wasm32")]
use crate::shared::types::UserInfo;
```

### Issue: "failed to resolve: could not find `gloo_net`"

**Cause**: Missing WASM dependency.

**Solution**: Add to `Cargo.toml`:

```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
gloo-net = "0.6"
```

### Issue: Function signature mismatch between server and client

**Cause**: Manually defined double definitions are no longer needed.

**Solution**: Remove manual `#[cfg(target_arch = "wasm32")]` stubs:

```rust
// ❌ Remove this
#[cfg(target_arch = "wasm32")]
#[server_fn]
pub async fn my_function() -> Result<(), ServerFnError> {
    unreachable!()
}

// ✅ Keep only this
#[server_fn(use_inject = true)]
pub async fn my_function(
    #[inject] db: DatabaseConnection,
) -> Result<(), ServerFnError> {
    // Implementation
}
```

## Implementation Details

### Macro Location

- **Crate**: `reinhardt-pages-macros`
- **File**: `crates/reinhardt-pages/crates/macros/src/server_fn.rs`

### Key Functions

1. **`expand()`** (line 348-359): Main expansion logic
   - Removes `#[inject]` attributes from server function
   - Generates client stub with filtered parameters
   - Generates server handler with DI resolution

2. **`generate_client_stub()`** (line 388-555): Client stub generation
   - **Line 405-417**: Filters out `#[inject]` parameters
   - **Line 430-440**: Creates new signature without `#[inject]` parameters
   - **Line 520**: Uses filtered signature for client stub

3. **`generate_server_handler()`**: Server handler generation
   - Deserializes request body
   - Resolves `#[inject]` dependencies from DI context
   - Calls original function with resolved dependencies

### Conditional Compilation Strategy

The macro uses `#[cfg(target_arch = "wasm32")]` and `#[cfg(not(target_arch = "wasm32"))]` to generate target-specific code:

```rust
quote! {
    // Server-side: Original function (with #[inject] attributes removed)
    #[cfg(not(target_arch = "wasm32"))]
    #clean_func

    // Client-side: HTTP request stub (without #[inject] parameters)
    #client_stub

    // Server-side: Route handler and registration
    #server_handler
}
```

## Version History

### v0.1.0-alpha.2 (2026-01-09)

**Breaking Change**: Automatic `#[inject]` parameter exclusion in client stubs

- **Added**: Client-side signature filtering (removes `#[inject]` parameters)
- **Removed**: Need for manual double definition with conditional compilation
- **Fixed**: Parameter count mismatch between server and client calls

**Migration**: Remove manual `#[cfg(target_arch = "wasm32")]` function stubs.

### v0.1.0-alpha.1 (Initial Release)

- Initial implementation with manual double definition requirement

## See Also

- [reinhardt-pages README](../README.md)
- [Dependency Injection Guide](../../reinhardt-di/README.md)
- [Server Functions Tutorial](../../../docs/tutorials/en/pages/server-functions.md) (coming soon)

## Contributing

If you find issues with the `server_fn` macro or have suggestions for improvements, please:

1. Check existing issues: https://github.com/kent8192/reinhardt-web/issues
2. Open a new issue with:
   - Clear description of the problem
   - Minimal reproducible example
   - Expected vs. actual behavior
3. Submit a pull request if you have a fix

## License

This documentation is part of the Reinhardt project and is licensed under the BSD 3-Clause License.
