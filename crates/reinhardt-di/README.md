# reinhardt-di

FastAPI-inspired dependency injection system for Reinhardt.

## Overview

Provides a FastAPI-style dependency injection system with support for request-scoped and singleton-scoped dependency caching, automatic resolution of nested dependencies, and integration with authentication and database connections.

Delivers the FastAPI development experience in Rust with type-safe and async-first design.

## Installation

Add `reinhardt` to your `Cargo.toml`:

<!-- reinhardt-version-sync:3 -->
```toml
[dependencies]
reinhardt = { version = "0.3.3", features = ["di"] }

# Or use a preset:
# reinhardt = { version = "0.3.3", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.3.3", features = ["full"] }      # All features
```

Then import DI features:

```rust
use reinhardt::di::{
    Depends, FactoryOutput, Injectable, InjectableKey, InjectionContext,
    SingletonScope, injectable, injectable_key,
};
```

**Note:** DI features are included in the `standard` and `full` feature presets.

## Core Concepts

### Dependency Scopes

- **Request Scope**: Dependencies cached per request (default)
- **Singleton Scope**: Dependencies shared across the entire application

### Injection Models

Application-owned types can implement `Injectable` directly when their own
type is the dependency identity. Provider functions use `#[injectable]` and
return `FactoryOutput<K, T>` when the dependency should be keyed by an
application-defined `K` instead.

## Implemented Features ✓

### Core Dependency Injection

#### Dependency Wrappers

`reinhardt-di` provides keyed wrappers for provider outputs:

- ✓ **`FactoryOutput<K, T>`**: return type for `#[injectable]` provider functions
  - registered by `TypeId::of::<FactoryOutput<K, T>>()`
  - stores the produced `T`
  - lets multiple providers return the same `T` without colliding

- ✓ **`Depends<K, T>`**: handler/provider parameter wrapper for keyed outputs
  - resolves `FactoryOutput<K, T>` from the registry
  - dereferences to `T` for ergonomic use
  - `Depends::<K, T>::builder()` - cache enabled metadata
  - `Depends::<K, T>::builder_no_cache()` - cache disabled metadata
  - `from_value(value)` - build from a value for tests

Use direct `T` parameters for ordinary `Injectable` values. Use
`Depends<K, T>` when consuming output from a provider function.

- ✓ **Injectable Trait**: Define types that can be injected directly
  - Manual implementation: When the type itself is the dependency identity
  - `Arc<T>` and `Option<T>` blanket implementations for direct injectables

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
  - Use direct `Injectable` values or `Depends<K, T>` in WebSocket handlers

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

### Basic Usage with `Depends<K, T>`

```rust
use reinhardt::di::{
    Depends, FactoryOutput, InjectionContext, InjectableKey, SingletonScope,
    injectable, injectable_key,
};
use std::sync::Arc;

#[derive(Clone, Default)]
struct Config {
    api_key: String,
    database_url: String,
}

#[injectable_key]
struct ConfigKey;

#[injectable(scope = "singleton")]
async fn config_provider() -> FactoryOutput<ConfigKey, Config> {
    FactoryOutput::new(Config {
        api_key: "test-key".to_string(),
        database_url: "sqlite://app.db".to_string(),
    })
}

#[tokio::main]
async fn main() {
    // Creating a singleton scope
    let singleton = Arc::new(SingletonScope::new());

    // Creating the request context
    let ctx = InjectionContext::builder(singleton).build();

    // Keyed dependency resolution (cache enabled metadata)
    let config = Depends::<ConfigKey, Config>::builder()
        .resolve(&ctx)
        .await
        .unwrap();

    println!("API Key: {}", config.api_key);
}
```

### Direct `Injectable` Implementation

```rust
use reinhardt::di::{DiResult, Injectable, InjectionContext};

struct Config {
    api_key: String,
    database_url: String,
}

#[async_trait::async_trait]
impl Injectable for Config {
    async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
        Ok(Self {
            api_key: "test-key".to_string(),
            database_url: "sqlite://app.db".to_string(),
        })
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
let config1 = Depends::<ConfigKey, Config>::builder().resolve(&ctx).await?;
let config2 = Depends::<ConfigKey, Config>::builder().resolve(&ctx).await?;
// config1 and config2 are the same instance

// Cache disabled - Creates new instance each time
let config3 = Depends::<ConfigKey, Config>::builder_no_cache().resolve(&ctx).await?;
let config4 = Depends::<ConfigKey, Config>::builder_no_cache().resolve(&ctx).await?;
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
// `scope` accepts "singleton", "request", or "transient" (see Attributes below).
#[injectable(scope = "singleton")]
struct YourStruct {
    #[no_inject]
    field: Type,
}
```

**Attributes:**

Scope is passed as a macro argument in key-value form. `#[injectable]`
defaults to `request` when no `scope` argument is supplied.

- `` `#[injectable(scope = "request")]` `` - Request scope (default)
- `` `#[injectable(scope = "singleton")]` `` - Singleton scope
- `` `#[injectable(scope = "transient")]` `` - Transient scope (new instance every time)
- `` `#[no_inject]` `` - Exclude specific fields from automatic injection

**Example:**
```rust
use reinhardt::di::macros::injectable;

#[injectable(scope = "singleton")]
struct Config {
    #[no_inject]
    database_url: String,
    api_key: String,
}
```

#### `#[injectable]` - Async Provider Function

Mark an async function as a keyed dependency provider for complex
initialization logic.

**Syntax:**
```rust
// `scope` accepts "singleton", "request", or "transient" (see Attributes below).
#[injectable(scope = "singleton")]
async fn factory_function(
    #[inject] dep: Dependency,
) -> FactoryOutput<MyKey, ReturnType> {
    // Initialization logic
    FactoryOutput::new(value)
}
```

When initialization can fail, put `Result<T, E>` in the provider value
position. Reinhardt registers `FactoryOutput<K, Result<T, E>>`, so the key `K`
remains the provider identity and callers do not need factory-local wrapper
types only to avoid `TypeId` collisions.

`#[inject]` wrapper detection is trait-based. `Depends<K, T>` resolves
`FactoryOutput<K, T>`, and applications can define their own wrapper types by
implementing `InjectableType` with the registry key in `type Inner`.

```rust
use reinhardt::di::{Depends, FactoryOutput, InjectableKey, InjectableType};

struct Lazy<K, T>(Depends<K, T>)
where
    K: InjectableKey,
    T: Send + Sync + 'static;

impl<K, T> InjectableType for Lazy<K, T>
where
    K: InjectableKey,
    T: Send + Sync + 'static,
{
    type Inner = FactoryOutput<K, T>;

    fn from_resolved(
        inner: std::sync::Arc<Self::Inner>,
        use_cache: bool,
    ) -> Self {
        let depends = Depends::from_output(inner, use_cache);
        Self(depends)
    }
}
```

**Attributes:**

Scope is passed as a macro argument in key-value form.
`#[injectable]` provider functions default to `singleton` when no `scope`
argument is supplied. `#[injectable_factory]` is a deprecated compatibility
alias for provider functions.

Provider registration is native-only. On `wasm32-unknown-unknown`,
`#[injectable]` emits an inert same-name async stub and skips the provider body,
wrapper, registry function, and `inventory` submission. `#[injectable_key]`
keeps the key type available on every target but skips the `InjectableKey` impl
on WASM. This allows shared modules to compile for WASM without wrapping each
provider in call-site `#[cfg(native)]`; DI resolution still runs only on native
targets.

- `` `#[injectable(scope = "singleton")]` `` - Singleton scope (default)
- `` `#[injectable(scope = "request")]` `` - Request scope
- `` `#[injectable(scope = "transient")]` `` - Transient scope
- `` `#[inject]` `` - Mark function parameters for automatic injection

**Example:**
```rust
use reinhardt::di::{Depends, FactoryOutput, injectable, injectable_key};
use reinhardt::{get, Response, StatusCode, ViewResult};

#[derive(Debug)]
struct DatabaseConnectionError;

#[injectable_key]
struct DatabaseKey;

#[injectable(scope = "singleton")]
async fn create_database(
    #[inject] config: Config,
) -> FactoryOutput<DatabaseKey, Result<DatabaseConnection, DatabaseConnectionError>> {
    FactoryOutput::new(
        DatabaseConnection::connect(&config.database_url)
            .await
            .map_err(|_| DatabaseConnectionError),
    )
}

#[get("/database/health", name = "database_health")]
async fn database_health(
    #[inject] db: Depends<DatabaseKey, Result<DatabaseConnection, DatabaseConnectionError>>,
) -> ViewResult<Response> {
    match db.as_ref() {
        Ok(_) => Ok(Response::new(StatusCode::OK)),
        Err(_) => Ok(Response::new(StatusCode::SERVICE_UNAVAILABLE)),
    }
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

#### Provider with Nested Dependencies

```rust
use reinhardt::di::{
    Depends, DiResult, FactoryOutput, Injectable, InjectionContext,
    injectable, injectable_key,
};

struct AppConfig {
    db_url: String,
    cache_size: usize,
}

#[async_trait::async_trait]
impl Injectable for AppConfig {
    async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
        Ok(Self {
            db_url: "sqlite://app.db".to_string(),
            cache_size: 256,
        })
    }
}

#[injectable_key]
struct ServiceKey;

#[injectable(scope = "request")]
async fn create_service(
    #[inject] config: AppConfig,
) -> FactoryOutput<ServiceKey, MyService> {
    FactoryOutput::new(MyService::new(config.db_url, config.cache_size))
}

async fn handler(
    #[inject] service: Depends<ServiceKey, MyService>,
) {
    service.run().await;
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
  - Supported by HTTP route macros and server function macros
- **`CookieNamed<N, T>`**: Compile-time cookie name specification
  - Type-safe cookie names via built-in or custom `CookieName` marker types
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
- **Integration with `reinhardt-core` validators module**

## License

Licensed under the BSD 3-Clause License.
