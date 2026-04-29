# reinhardt-graphql

GraphQL integration (facade crate)

## Overview

GraphQL API support with schema generation from models, query and mutation resolvers, and integration with the authentication and permission system. Provides a flexible alternative to REST APIs.

This is a facade crate that re-exports functionality from the following modules:
- **core**: Core GraphQL implementation
- **macros**: Procedural macros for GraphQL (when features enabled)

## Crate Structure

```
reinhardt-graphql/              (facade crate - public API)
└── macros/                     (procedural macros)
```

Users should depend on `` `reinhardt-graphql` `` (this facade crate) for all GraphQL functionality.

## Features

### Implemented ✓

#### Core Type System

- **GraphQL Type Markers**: `GraphQLType` and `GraphQLField` traits for type-safe GraphQL type definitions
- **Error Handling**: Custom `GraphQLError` enum with Schema, Resolver, and NotFound variants
- **Base Resolver Trait**: Async `Resolver` trait with generic output types for flexible resolver implementation

#### Schema & Data Types

- **User Type**: Complete GraphQL object implementation with id, name, email, and active fields
- **User Storage**: Thread-safe in-memory storage using `Arc<RwLock<HashMap>>` for user data
  - `new()`: Create new storage instance
  - `add_user()`: Add or update user in storage
  - `get_user()`: Retrieve user by ID
  - `list_users()`: List all stored users
- **Input Types**: `CreateUserInput` for user creation mutations
- **Schema Builder**: `create_schema()` function to build GraphQL schema with data context

#### Query Operations

- **User Queries**:
  - `user(id: ID)`: Retrieve single user by ID
  - `users()`: List all users
  - `hello(name: Option<String>)`: Simple greeting query for testing
- **Context Integration**: Queries access UserStorage through GraphQL context

#### Mutation Operations

- **User Mutations**:
  - `createUser(input: CreateUserInput)`: Create new user with auto-generated UUID
  - `updateUserStatus(id: ID, active: bool)`: Update user active status
- **State Management**: Mutations persist changes to UserStorage

#### Subscription System

- **Event Types**: `UserEvent` enum supporting Created, Updated, and Deleted events
- **Event Broadcasting**: `EventBroadcaster` with tokio broadcast channel (capacity: 100)
  - `new()`: Create new broadcaster instance
  - `broadcast()`: Send events to all subscribers
  - `subscribe()`: Subscribe to event stream
- **Subscription Root**: `SubscriptionRoot` with filtered subscription streams
  - `userCreated()`: Stream of user creation events
  - `userUpdated()`: Stream of user update events
  - `userDeleted()`: Stream of user deletion events (returns ID only)
- **Async Streams**: Real-time event filtering using async-stream

#### Integration

- **async-graphql Integration**: Built on async-graphql framework for production-ready GraphQL server
- **Type Safety**: Full Rust type system integration with compile-time guarantees
- **Async/Await**: Complete async support with tokio runtime
- **Documentation**: Comprehensive doc comments with examples for all public APIs

#### gRPC Transport (Optional - `graphql-grpc` feature)

- **GraphQL over gRPC Service**: `GraphQLGrpcService` implementing gRPC protocol for GraphQL
  - `execute_query()`: Execute GraphQL queries via unary RPC
  - `execute_mutation()`: Execute GraphQL mutations via unary RPC
  - `execute_subscription()`: Execute GraphQL subscriptions via server streaming RPC
- **Protocol Buffers**: Complete proto definitions in `reinhardt-grpc` crate
  - `GraphQLRequest`: query, variables, operation_name
  - `GraphQLResponse`: data, errors, extensions
  - `SubscriptionEvent`: id, event_type, payload, timestamp
- **Request/Response Conversion**: Automatic conversion between gRPC and async-graphql types
- **Error Handling**: Full error information propagation (message, locations, path, extensions)
- **Performance**: Minimal overhead (5-21%, or 0.2-0.8 µs) compared to direct execution
- **Network Communication**: Full TCP/HTTP2 support via tonic
- **Streaming**: Efficient server-side streaming for real-time subscriptions

#### Dependency Injection (Optional - `di` feature)

- **`#[graphql_handler]` macro**: Attribute macro for resolvers with automatic dependency injection
- **`GraphQLContextExt` trait**: Extension trait for extracting DI context from GraphQL context
- **`SchemaBuilderExt` trait**: Convenience methods for adding DI context to schema
- **Cache Control**: Per-parameter cache control with `#[inject(cache = false)]`
- **Type Safety**: Full compile-time type checking for injected dependencies
- **REST Consistency**: Same DI patterns as REST handlers for unified developer experience

## Installation

Add `reinhardt` to your `Cargo.toml`:

<!-- reinhardt-version-sync:3 -->
```toml
[dependencies]
reinhardt = { version = "0.1.0-rc.24", features = ["graphql"] }

# Or use a preset:
# reinhardt = { version = "0.1.0-rc.24", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.1.0-rc.24", features = ["full"] }      # All features
```

Then import GraphQL features:

```rust
use reinhardt::graphql::{Schema, Query, Mutation};
use reinhardt::graphql::types::{UserStorage, UserEvent};
```

**Note:** GraphQL features are included in the `standard` and `full` feature presets.

### Optional Features

<!-- reinhardt-version-sync:2 -->
```toml
# With dependency injection
reinhardt = { version = "0.1.0-rc.24", features = ["graphql", "di"] }

# With gRPC transport
reinhardt = { version = "0.1.0-rc.24", features = ["graphql", "grpc"] }
```

## Examples

### Basic GraphQL Usage

```rust
use async_graphql::{EmptySubscription, Schema};
use reinhardt::graphql::schema::{Mutation, Query, UserStorage};

#[tokio::main]
async fn main() {
    let storage = UserStorage::new();
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(storage)
        .finish();

    let query = r#"{ hello(name: "World") }"#;
    let result = schema.execute(query).await;
    println!("{}", result.data);
}
```

### Dependency Injection

Enable the `di` feature to use dependency injection in GraphQL resolvers:

```rust
use async_graphql::{Context, Object, Result, ID, Schema, EmptyMutation, EmptySubscription};
use reinhardt::graphql::{graphql_handler, SchemaBuilderExt};
use reinhardt_di::InjectionContext;
use std::sync::Arc;

// Define your resolvers
pub struct Query;

#[Object]
impl Query {
    async fn user(&self, ctx: &Context<'_>, id: ID) -> Result<User> {
        user_impl(ctx, id).await
    }

    async fn users(&self, ctx: &Context<'_>, limit: Option<i32>) -> Result<Vec<User>> {
        users_impl(ctx, limit).await
    }
}

// Use #[graphql_handler] for DI
#[graphql_handler]
async fn user_impl(
    ctx: &Context<'_>,
    id: ID,
    #[inject] db: DatabaseConnection,  // Auto-injected
    #[inject] cache: RedisCache,       // Auto-injected
) -> Result<User> {
    // Check cache first
    if let Some(user) = cache.get(&id).await {
        return Ok(user);
    }

    // Fetch from database
    let user = db.fetch_user(&id).await?;

    // Update cache
    cache.set(&id, &user).await;

    Ok(user)
}

#[graphql_handler]
async fn users_impl(
    ctx: &Context<'_>,
    limit: Option<i32>,
    #[inject] db: DatabaseConnection,
) -> Result<Vec<User>> {
    let limit = limit.unwrap_or(100);
    let users = db.fetch_users(limit).await?;
    Ok(users)
}

// Build schema with DI context
#[tokio::main]
async fn main() {
    let injection_ctx = Arc::new(InjectionContext::new());

    // Register dependencies
    // injection_ctx.register(DatabaseConnection::new(...));
    // injection_ctx.register(RedisCache::new(...));

    let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
        .with_di_context(injection_ctx)  // Helper method from SchemaBuilderExt
        .finish();

    // Execute query
    let query = r#"{ user(id: "123") { id name email } }"#;
    let result = schema.execute(query).await;
    println!("{}", result.data);
}
```

#### Cache Control

Control dependency caching per parameter:

```rust
#[graphql_handler]
async fn handler(
    ctx: &Context<'_>,
    id: ID,
    #[inject] cached_db: DatabaseConnection,          // Cached (default)
    #[inject(cache = false)] fresh_db: DatabaseConnection,  // Not cached
) -> Result<User> {
    // ...
}
```

### GraphQL over gRPC Server

<!-- reinhardt-version-sync -->
```rust
use async_graphql::{EmptySubscription, Schema};
use reinhardt::graphql::grpc_service::GraphQLGrpcService;
use reinhardt::graphql::schema::{Mutation, Query, UserStorage};
use reinhardt::grpc::proto::graphql::graph_ql_service_server::GraphQlServiceServer;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let storage = UserStorage::new();
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(storage)
        .finish();

    let service = GraphQLGrpcService::new(schema);
    let grpc_service = GraphQlServiceServer::new(service);

    Server::builder()
        .add_service(grpc_service)
        .serve("0.1.0-rc.24:50051".parse()?)
        .await?;

    Ok(())
}
```

### GraphQL over gRPC Client

<!-- reinhardt-version-sync -->
```rust
use reinhardt::grpc::proto::graphql::{
    graph_ql_service_client::GraphQlServiceClient,
    GraphQlRequest,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = GraphQlServiceClient::connect("http://0.1.0-rc.24:50051").await?;

    let request = tonic::Request::new(GraphQlRequest {
        query: r#"{ hello(name: "gRPC") }"#.to_string(),
        variables: None,
        operation_name: None,
    });

    let response = client.execute_query(request).await?;
    println!("{:?}", response.into_inner());

    Ok(())
}
```

### Running Examples

```bash
# Start gRPC server
cargo run --package reinhardt-graphql --features graphql-grpc --example grpc_server

# In another terminal, run client
cargo run --package reinhardt-graphql --features graphql-grpc --example grpc_client
```

## Testing

```bash
# All tests
cargo test --package reinhardt-graphql --features graphql-grpc

# Integration tests
cargo test --package reinhardt-graphql --features graphql-grpc --test grpc_integration_tests

# Subscription streaming tests
cargo test --package reinhardt-graphql --features graphql-grpc --test grpc_subscription_tests

# E2E tests with real network
cargo test --package reinhardt-graphql --features graphql-grpc --test grpc_e2e_tests

# Performance benchmarks
cargo bench --package reinhardt-graphql --features graphql-grpc
```

## Performance

See [PERFORMANCE.md](PERFORMANCE.md) for detailed benchmarks.

**Summary:**

- Direct GraphQL: ~3-4 µs per query
- gRPC GraphQL: ~4-5 µs per query
- Overhead: 5-21% (+0.2-0.8 µs) for gRPC serialization
- Both approaches are highly performant for real-world applications
