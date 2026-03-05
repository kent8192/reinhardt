# reinhardt-di

FastAPI-inspired dependency injection system for Reinhardt.

## Overview

Provides a FastAPI-style dependency injection system with support for request-scoped and singleton-scoped dependency caching, automatic resolution of nested dependencies, and integration with authentication and database connections.

Delivers the FastAPI development experience in Rust with type-safe and async-first design.

## Installation

Add `reinhardt` to your `Cargo.toml`:

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", features = ["di"] }

# Or use a preset:
# reinhardt = { version = "0.1.0-alpha.1", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.1.0-alpha.1", features = ["full"] }      # All features
```

Then import DI features:

```rust
use reinhardt::di::{Depends, Injectable, InjectionContext};
use reinhardt::di::{Injected, OptionalInjected, SingletonScope};
```

**Note:** DI features are included in the `standard` and `full` feature presets.

## Core Concepts

### Dependency Scopes

- **Request Scope**: Dependencies cached per request (default)
- **Singleton Scope**: Dependencies shared across the entire application

### Automatic Injection

Types implementing `Default + Clone + Send + Sync + 'static` automatically implement the `Injectable` trait and can be used as dependencies.

## Implemented Features ✓

### Core Dependency Injection

#### Dependency Wrappers

`reinhardt-di` provides two wrapper types for dependency injection:

- ✓ **`Injected<T>` Wrapper**: Low-level dependency wrapper with metadata
  - `Arc<T>` wrapper with injection metadata (scope, cached status)
  - `Deref` trait for transparent access to inner value
  - `resolve(&ctx)` - Resolve with cache (default)
  - `resolve_uncached(&ctx)` - Resolve without cache
  - Metadata access via `.scope()` and `.is_cached()` methods
  - Direct control over dependency resolution

- ✓ **`Depends<T>` Wrapper**: High-level FastAPI-style builder
  - `Depends::<T>::new()` - Cache enabled (default)
  - `Depends::<T>::no_cache()` - Cache disabled
  - `resolve(&ctx)` - Dependency resolution
  - `from_value(value)` - Generate from value for testing
  - More ergonomic API for most use cases

- ✓ **`OptionalInjected<T>` Wrapper**: Optional dependency wrapper
  - `Option<Injected<T>>` for dependencies that may not be available
  - `resolve(&ctx)` - Returns `Ok(None)` if dependency not found
  - Useful for optional features or fallback behavior

**Recommendation**: Use `Depends<T>` for most cases (more ergonomic). Use `Injected<T>` when you need direct control or metadata access.

- ✓ **Injectable Trait**: Define types that can be injected as dependencies
  - Auto-implementation: For types implementing `Default + Clone + Send + Sync + 'static`
  - Custom implementation: When complex initialization logic is needed

- ✓ **InjectionContext**: Context for dependency resolution
  - Builder pattern for context creation: `InjectionContext::builder(singleton).build()`
  - Internal scope management (request and singleton)
  - Generate new context per request

- ✓ **RequestScope**: Caching within requests
  - Type-based cache (using `TypeId` as key)
  - Thread-safe implementation (`Arc<RwLock<HashMap>>`)

- ✓ **SingletonScope**: Application-wide caching
  - Dependencies shared across all requests
  - Thread-safe implementation

### Advanced Features

- ✓ **Dependency Caching**: Automatic caching within request scope
  - Same dependency is generated only once even when requested multiple times
  - Cache is shared between nested dependencies
  - Cache enable/disable control available

- ✓ **Nested Dependencies**: Dependencies can depend on other dependencies
  - Automatic dependency graph resolution
  - Circular dependency detection and error handling

- ✓ **Dependency Overrides**: Dependency overrides for testing
  - Use different implementations for production and testing
  - Application-level override management
  - Support for overrides with sub-dependencies

- ✓ **Provider System**: Async factory pattern
  - `Provider` trait - Generic interface for providing dependencies
  - `ProviderFn` - Function-based provider
  - Any async closure can be used as a provider

### Error Handling

- ✓ **DiError**: Comprehensive error type
  - `NotFound` - Dependency not found
  - `CircularDependency` - Circular dependency detection
  - `ProviderError` - Provider errors
  - `TypeMismatch` - Type mismatch
  - `ScopeError` - Scope-related errors

### Integration Support

- ✓ **HTTP Integration**: Integration with HTTP requests/responses
  - Dependency injection from requests
  - Support for connection info injection

- ✓ **WebSocket Support**: Dependency injection into WebSocket connections
  - Use `Depends<T>` in WebSocket handlers

### Advanced Dependency Patterns ✓

#### Generator-based Dependencies (yield pattern)

- **Lifecycle Management**: Setup/teardown pattern
- **Context Manager**: Automatic resource cleanup
- **Error Handling**: Cleanup execution even on errors
- **Streaming Support**: Streaming response support
- **WebSocket Support**: Integration with WebSocket handlers

```rust
use reinhardt::di::{Injectable, InjectionContext};

#[derive(Clone)]
struct DatabaseConnection {
    // Setup
}

impl DatabaseConnection {
    async fn setup() -> Self {
        // Initialize connection
        DatabaseConnection { }
    }

    async fn cleanup(self) {
        // Close connection
    }
}
```

#### Dependency Classes (Class-based dependencies)

- **Callable Dependencies**: Struct-based dependencies with call methods
- **Async Callables**: Async dependency method support
- **Stateful Dependencies**: Dependencies with internal state
- **Method-based Injection**: Flexible dependency construction

```rust
#[derive(Clone)]
struct CallableDependency {
    prefix: String,
}

impl CallableDependency {
    fn call(&self, value: String) -> String {
        format!("{}{}", self.prefix, value)
    }
}
```

#### Parametrized Dependencies (Parameterized dependencies)

- **Path Parameter Integration**: Access to path parameters from dependencies
- **Shared Parameters**: Share path parameters between endpoints and dependencies
- **Type-safe Extraction**: Compile-time validated parameter passing

```rust
// Path parameter accessible in dependency
#[async_trait::async_trait]
impl Injectable for UserValidator {
    async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
        let user_id = UserId::inject(ctx).await?;
        Ok(UserValidator { user_id: user_id.0 })
    }
}
```

#### Schema Generation (Schema generation)

- **Dependency Deduplication**: Shared dependencies appear only once in schema
- **Transitive Dependencies**: Automatic caching of nested dependencies
- **Schema Optimization**: Efficient dependency graph representation

#### Security Overrides (Security overrides)

- **Security Dependencies**: OAuth2, JWT, and other authentication schemes
- **Security Scopes**: Scope-based access control
- **Override Support**: Test-friendly replacement of security dependencies

```rust
// Security dependency with scopes
#[async_trait::async_trait]
impl Injectable for UserData {
    async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
        let scopes = ctx.get_request::<SecurityScopes>()?;
        Ok(UserData { scopes: scopes.scopes })
    }
}
```

## Usage Examples

### Basic Usage with `Depends<T>`

```rust
use reinhardt::di::{Depends, Injectable, InjectionContext, SingletonScope};
use std::sync::Arc;

#[derive(Clone, Default)]
struct Config {
    api_key: String,
    database_url: String,
}

#[tokio::main]
async fn main() {
    // Creating a singleton scope
    let singleton = Arc::new(SingletonScope::new());

    // Creating the request context
    let ctx = InjectionContext::builder(singleton).build();

    // Dependency Resolution (Cache Enabled)
    let config = Depends::<Config>::new()
        .resolve(&ctx)
        .await
        .unwrap();

    println!("API Key: {}", config.api_key);
}
```

### Basic Usage with `Injected<T>`

```rust
use reinhardt::di::{Injected, OptionalInjected, Injectable, InjectionContext, SingletonScope};
use std::sync::Arc;

#[derive(Clone, Default)]
struct Config {
    api_key: String,
    database_url: String,
}

#[derive(Clone, Default)]
struct Cache {
    enabled: bool,
}

#[tokio::main]
async fn main() {
    let singleton = Arc::new(SingletonScope::new());
    let ctx = InjectionContext::builder(singleton).build();

    // Resolve with cache (default)
    let config: Injected<Config> = Injected::resolve(&ctx).await.unwrap();
    println!("API Key: {}", config.api_key);  // Deref trait allows direct access

    // Access metadata
    println!("Scope: {:?}", config.scope());
    println!("Cached: {}", config.is_cached());

    // Resolve without cache
    let fresh_config = Injected::<Config>::resolve_uncached(&ctx).await.unwrap();

    // Optional dependency
    let optional_cache: OptionalInjected<Cache> = OptionalInjected::resolve(&ctx).await.unwrap();
    if let Some(cache) = optional_cache.as_ref() {
        println!("Cache enabled: {}", cache.enabled);
    }
}
```

### Custom Injectable Implementation

```rust
use reinhardt::di::{Injectable, InjectionContext, DiResult};

struct Database {
    pool: DbPool,
}

#[async_trait::async_trait]
impl Injectable for Database {
    async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
        // Custom initialization logic
        let config = Config::inject(ctx).await?;
        let pool = create_pool(&config.database_url).await?;

        Ok(Database { pool })
    }
}
```

### Nested Dependencies

```rust
#[derive(Clone)]
struct ServiceA {
    db: Arc<Database>,
}

#[async_trait::async_trait]
impl Injectable for ServiceA {
    async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
        // Depends on Database
        let db = Database::inject(ctx).await?;
        Ok(ServiceA { db: Arc::new(db) })
    }
}

#[derive(Clone)]
struct ServiceB {
    service_a: Arc<ServiceA>,
    config: Config,
}

#[async_trait::async_trait]
impl Injectable for ServiceB {
    async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
        // Depends on ServiceA and Config (nested dependencies)
        let service_a = ServiceA::inject(ctx).await?;
        let config = Config::inject(ctx).await?;

        Ok(ServiceB {
            service_a: Arc::new(service_a),
            config,
        })
    }
}
```

### Dependency Overrides for Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct MockDatabase {
        // Mock implementation for testing
    }

    #[async_trait::async_trait]
    impl Injectable for MockDatabase {
        async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
            Ok(MockDatabase { /* ... */ })
        }
    }

    #[tokio::test]
    async fn test_with_mock_database() {
        let singleton = Arc::new(SingletonScope::new());
        let ctx = InjectionContext::builder(singleton).build();

        // Inject mock for testing
        let mock_db = MockDatabase::inject(&ctx).await.unwrap();

        // Test code using mock_db
    }
}
```

### Cache Control

```rust
// Cache enabled (default) - Returns the same instance
let config1 = Depends::<Config>::new().resolve(&ctx).await?;
let config2 = Depends::<Config>::new().resolve(&ctx).await?;
// config1 and config2 are the same instance

// Cache disabled - Creates new instance each time
let config3 = Depends::<Config>::no_cache().resolve(&ctx).await?;
let config4 = Depends::<Config>::no_cache().resolve(&ctx).await?;
// config3 and config4 are different instances
```

## Architecture

### Type-Based Caching

Dependency caching is managed using type (`TypeId`) as the key. This allows dependencies of the same type to be automatically cached.

### Scope Hierarchy

```
SingletonScope (Application level)
    ↓ Shared
InjectionContext (Request level)
    ↓ Holds
RequestScope (In-request cache)
```

### Thread Safety

- All scopes are thread-safe using `Arc<RwLock<HashMap>>`
- `Injectable` trait requires `Send + Sync`
- Safe to use in async code

## Testing Support

The testing framework includes a comprehensive test suite:

- **Unit Tests**: Unit tests for each component
- **Integration Tests**: Integration tests ported from FastAPI test cases
- **Feature Tests**:
  - Automatic Injectable implementation tests
  - Circular dependency detection tests
  - Cache behavior tests
  - Dependency override tests
  - Nested dependency tests

## Performance Considerations

- **Lazy Initialization**: Dependencies are not generated until needed
- **Cache Efficiency**: Same dependency is generated only once within request scope
- **Zero-Cost Abstractions**: Low-overhead design leveraging Rust's type system
- **Arc-based Sharing**: Efficient instance sharing using `Arc`

## Comparison with FastAPI

| Feature                  | FastAPI (Python) | reinhardt-di (Rust) |
| ------------------------ | ---------------- | ------------------- |
| Basic DI                 | ✓                | ✓                   |
| Request Scope            | ✓                | ✓                   |
| Singleton Scope          | ✓                | ✓                   |
| Dependency Caching       | ✓                | ✓                   |
| Nested Dependencies      | ✓                | ✓                   |
| Dependency Overrides     | ✓                | ✓                   |
| `yield` Pattern          | ✓                | ✓                   |
| Type Safety              | Runtime          | **Compile-time**    |
| Performance              | Dynamic          | **Static & Fast**   |


## macros

The `macros` module provides procedural macros for simplified dependency injection setup.

### Features

### Implemented ✓

#### `#[injectable]` - Struct Injection Registration

Mark a struct as injectable and automatically register it with the global registry.

**Syntax:**
```rust
#[injectable]
#[scope(singleton|request|transient)]
struct YourStruct {
    #[no_inject]
    field: Type,
}
```

**Attributes:**
- `` `#[scope(singleton)]` `` - Singleton scope (default)
- `` `#[scope(request)]` `` - Request scope
- `` `#[scope(transient)]` `` - Transient scope (new instance every time)
- `` `#[no_inject]` `` - Exclude specific fields from automatic injection

**Example:**
```rust
use reinhardt::di::macros::injectable;

#[injectable]
#[scope(singleton)]
struct Config {
    #[no_inject]
    database_url: String,
    api_key: String,
}
```

#### `#[injectable_factory]` - Async Function Factory

Mark an async function as a dependency factory for complex initialization logic.

**Syntax:**
```rust
#[injectable_factory]
#[scope(singleton|request|transient)]
async fn factory_function(#[inject] dep: Arc<Dependency>) -> ReturnType {
    // Initialization logic
}
```

**Attributes:**
- `` `#[scope(singleton)]` `` - Singleton scope (default)
- `` `#[scope(request)]` `` - Request scope
- `` `#[scope(transient)]` `` - Transient scope
- `` `#[inject]` `` - Mark function parameters for automatic injection

**Example:**
```rust
use reinhardt::di::macros::injectable_factory;
use std::sync::Arc;

#[injectable_factory]
#[scope(singleton)]
async fn create_database(#[inject] config: Arc<Config>) -> DatabaseConnection {
    DatabaseConnection::connect(&config.database_url)
        .await
        .expect("Failed to connect to database")
}
```

### Benefits of Using Macros

- **Reduced Boilerplate**: Automatically implements `` `Injectable` `` trait
- **Scope Management**: Declarative scope configuration
- **Type Safety**: Compile-time verification of dependencies
- **Automatic Registration**: Global registry integration without manual setup
- **Async Support**: Native async/await support in factory functions

### Usage Patterns

#### Simple Struct Injection

```rust
use reinhardt::di::{macros::injectable, InjectionContext, SingletonScope};
use std::sync::Arc;

#[injectable]
struct Logger {
    level: String,
}

impl Default for Logger {
    fn default() -> Self {
        Logger {
            level: "info".to_string(),
        }
    }
}

#[tokio::main]
async fn main() {
    let singleton = Arc::new(SingletonScope::new());
    let ctx = InjectionContext::builder(singleton).build();

    let logger = Logger::inject(&ctx).await.unwrap();
    println!("Log level: {}", logger.level);
}
```

#### Factory with Nested Dependencies

```rust
use reinhardt::di::macros::{injectable, injectable_factory};
use std::sync::Arc;

#[injectable]
#[scope(singleton)]
struct AppConfig {
    #[no_inject]
    db_url: String,
    #[no_inject]
    cache_size: usize,
}

#[injectable_factory]
#[scope(request)]
async fn create_service(
    #[inject] config: Arc<AppConfig>
) -> MyService {
    MyService::new(config.db_url.clone(), config.cache_size)
}
```


## params

### Features

### Implemented ✓

#### Core Extraction System

- **`FromRequest` trait**: Core abstraction for asynchronous parameter extraction
- **`ParamContext`**: Management of path parameters and header/cookie names
- **Type-safe parameter extraction**: Extraction from requests with compile-time type checking
- **Error handling**: Detailed error messages via `ParamError`

#### Path Parameters (`path.rs`)

- **`Path<T>`**: Extract single value from URL path
  - Support for all primitive types: `i8`, `i16`, `i32`, `i64`, `i128`, `u8`, `u16`, `u32`, `u64`, `u128`, `f32`, `f64`, `bool`, `String`
  - Transparent access via `Deref`: `*path` or `path.0`
  - Value extraction via `into_inner()` method
- **`PathStruct<T>`**: Extract multiple path parameters into struct
  - Supports any struct implementing `DeserializeOwned`
  - Automatic type conversion using URL-encoded format (`"42"` → `42`)

#### Query Parameters (`query.rs`)

- **`Query<T>`**: Extract parameters from URL query string
  - Flexible deserialization using `serde`
  - Support for optional fields (`Option<T>`)
- **Multi-value query parameters** (`multi-value-arrays` feature):
  - `?q=5&q=6` → `Vec<i32>`
  - Automatic type conversion: string → numeric, boolean, etc.
  - JSON value-based deserialization

#### Headers (`header.rs`, `header_named.rs`)

- **`Header<T>`**: Extract value from request headers
  - Support for `String` and `Option<String>`
  - Runtime header name specification via `ParamContext`
- **`HeaderStruct<T>`**: Extract multiple headers into struct
  - Header name lowercase normalization
  - Automatic type conversion using URL-encoded
- **`HeaderNamed<N, T>`**: Compile-time header name specification
  - Type-safe header names via marker types: `Authorization`, `ContentType`
  - Support for `String` and `Option<String>`
  - Custom header name definition via `HeaderName` trait

#### Cookies (`cookie.rs`, `cookie_named.rs`)

- **`Cookie<T>`**: Extract value from cookies
  - Support for `String` and `Option<String>`
  - Runtime cookie name specification via `ParamContext`
- **`CookieStruct<T>`**: Extract multiple cookies into struct
  - RFC 6265-compliant cookie parsing
  - URL-decoding support
- **`CookieNamed<N, T>`**: Compile-time cookie name specification
  - Type-safe cookie names via marker types: `SessionId`, `CsrfToken`
  - Support for `String` and `Option<String>`
  - Custom cookie name definition via `CookieName` trait

#### Body Extraction (`body.rs`, `json.rs`, `form.rs`)

- **`Body`**: Extract raw request body as bytes
- **`Json<T>`**: JSON body deserialization
  - Type-safe deserialization using `serde_json`
  - Access via `Deref` and `into_inner()`
- **`Form<T>`**: Extract application/x-www-form-urlencoded form data
  - Content-Type validation
  - Deserialization using `serde_urlencoded`

#### Multipart Support (`multipart.rs`, requires `multipart` feature)

- **`Multipart`**: Multipart/form-data support
  - Streaming parsing using `multer` crate
  - File upload support
  - Iteration via `next_field()`

#### Validation Support (`validation.rs`, requires `validation` feature)

- **`Validated<T, V>`**: Validated parameter wrapper
- **`WithValidation` trait**: Fluent API for validation constraints
  - **Length constraints**: `min_length()`, `max_length()`
  - **Numeric ranges**: `min_value()`, `max_value()`
  - **Pattern matching**: `regex()`
  - **Format validation**: `email()`, `url()`
- **`ValidationConstraints<T>`**: Chainable validation builder
  - `validate_string()`: String value validation
  - `validate_number()`: Numeric validation
  - Support for combining multiple constraints
- **Type aliases**: `ValidatedPath<T>`, `ValidatedQuery<T>`, `ValidatedForm<T>`
- **Integration with `reinhardt-validators`**

## License

Licensed under the BSD 3-Clause License.
