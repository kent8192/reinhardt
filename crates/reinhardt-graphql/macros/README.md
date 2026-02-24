# reinhardt-graphql-macros

Derive macros for GraphQL-gRPC integration in Reinhardt framework

## Overview

`reinhardt-graphql-macros` provides procedural macros to simplify the integration between gRPC and GraphQL in the Reinhardt framework. These macros automatically generate conversion code and subscription implementations to reduce boilerplate.

## Important

**This is an internal subcrate of `reinhardt-graphql`.** Users should depend on `reinhardt` (the parent crate) instead of this crate directly.

```toml
# ✅ Correct - use the reinhardt parent crate
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", features = ["graphql"] }

# Or use a preset:
# reinhardt = { version = "0.1.0-alpha.1", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.1.0-alpha.1", features = ["full"] }      # All features

# ❌ Incorrect - don't depend on this crate directly
[dependencies]
reinhardt-graphql-macros = "0.1.0-alpha.1"
```

The macros are automatically re-exported by `reinhardt::graphql`:

```rust
// ✅ Correct - import from the reinhardt parent crate
use reinhardt::graphql::{GrpcGraphQLConvert, GrpcSubscription};

// Or use the macros module
use reinhardt::graphql::macros::{GrpcGraphQLConvert, GrpcSubscription};
```

## Features

### Implemented ✓

- **GrpcGraphQLConvert** - Automatic type conversion between Protobuf and GraphQL types
  - Derives `From<proto::T> for T` and `From<T> for proto::T`
  - Field renaming with `#[graphql(rename_all = "camelCase")]`
  - Conditional field inclusion with `#[graphql(skip_if = "...")]`
  - Custom protobuf type mapping with `#[proto(...)]` attributes

- **GrpcSubscription** - Automatic GraphQL subscription from gRPC streams
  - Maps gRPC streaming methods to GraphQL subscriptions
  - Service and method specification with `#[grpc(service = "...", method = "...")]`
  - Optional filtering with `#[graphql(filter = "...")]`
  - Rust 2024 lifetime compatibility

## Usage

### Type Conversion

```rust
use reinhardt::graphql::GrpcGraphQLConvert;

#[derive(GrpcGraphQLConvert)]
#[graphql(rename_all = "camelCase")]
struct User {
    id: String,
    name: String,
    #[graphql(skip_if = "Option::is_none")]
    email: Option<String>,
}
```

This generates:

- `From<proto::User> for User`
- `From<User> for proto::User`

### gRPC Subscriptions

```rust
use reinhardt::graphql::GrpcSubscription;

#[derive(GrpcSubscription)]
#[grpc(service = "UserEventsServiceClient", method = "subscribe_user_events")]
#[graphql(filter = "event_type == Created")]
struct UserCreatedSubscription;
```

This automatically generates a GraphQL subscription implementation that:

- Connects to the gRPC service
- Subscribes to the specified method
- Applies the filter to incoming events
- Converts gRPC messages to GraphQL types

## Attributes

### GrpcGraphQLConvert Attributes

- `#[graphql(rename_all = "...")]` - Rename all fields (camelCase, snake_case, PascalCase)
- `#[graphql(skip_if = "...")]` - Skip field if predicate is true
- `#[proto(type = "...")]` - Specify custom protobuf type
- `#[proto(rename = "...")]` - Rename field in protobuf

### GrpcSubscription Attributes

- `#[grpc(service = "...")]` - gRPC service client name
- `#[grpc(method = "...")]` - gRPC method name
- `#[graphql(filter = "...")]` - Filter expression for events

## License

Licensed under the BSD 3-Clause License.
