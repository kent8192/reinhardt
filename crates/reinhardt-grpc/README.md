# reinhardt-grpc

gRPC foundation crate for the Reinhardt framework

## Overview

This crate provides the foundation for gRPC functionality in the Reinhardt
framework. It includes only framework-level common types and adapter traits,
with domain-specific implementations left to users.

## Installation

Add `reinhardt` to your `Cargo.toml`:

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", features = ["grpc"] }

# Or use a preset:
# reinhardt = { version = "0.1.0-alpha.1", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.1.0-alpha.1", features = ["full"] }      # All features
```

Then import gRPC features:

```rust
use reinhardt::grpc::{Empty, Timestamp, Error, PageInfo};
use reinhardt::grpc::adapter::{GrpcServiceAdapter, ServiceContext};
```

**Note:** gRPC features are included in the `standard` and `full` feature presets.

## Features

- Common Protobuf types (Empty, Timestamp, Error, PageInfo, BatchResult)
- Adapter traits for gRPC service integration
- Error handling utilities
- **Dependency injection support (optional, with `di` feature)**

### 1. Common Protobuf Types

Generic types provided by the framework:

```protobuf
// Empty - Empty response
message Empty {}

// Timestamp - Timestamp representation
message Timestamp {
  int64 seconds = 1;
  int32 nanos = 2;
}

// Error - Error information
message Error {
  string code = 1;
  string message = 2;
  map<string, string> metadata = 3;
}

// PageInfo - Pagination information
message PageInfo {
  int32 page = 1;
  int32 per_page = 2;
  int32 total = 3;
  bool has_next = 4;
  bool has_prev = 5;
}

// BatchResult - Batch operation result
message BatchResult {
  int32 success_count = 1;
  int32 failure_count = 2;
  repeated Error errors = 3;
}
```

### 2. Adapter Traits

Traits for integrating gRPC services with other framework components (such as
GraphQL):

```rust
use reinhardt::grpc::{GrpcServiceAdapter, GrpcSubscriptionAdapter};

/// Adapter for Query/Mutation
#[async_trait]
pub trait GrpcServiceAdapter: Send + Sync {
    type Input;
    type Output;
    type Error: std::error::Error + Send + Sync + 'static;

    async fn call(&self, input: Self::Input) -> Result<Self::Output, Self::Error>;
}

/// Adapter for Subscription
pub trait GrpcSubscriptionAdapter: Send + Sync {
    type Proto;
    type GraphQL;
    type Error: std::error::Error + Send + Sync + 'static;

    fn map_event(&self, proto: Self::Proto) -> Option<Self::GraphQL>;
}
```

### 3. Error Handling

gRPC error types and conversions:

```rust
use reinhardt::grpc::{GrpcError, GrpcResult};

pub enum GrpcError {
    Connection(String),
    Service(String),
    NotFound(String),
    InvalidArgument(String),
    Internal(String),
}
```

## Usage

### Using Your Own .proto Files

1. Create a `proto/` directory in your project

```
my-app/
├── proto/
│   ├── user.proto
│   └── product.proto
├── src/
│   └── main.rs
└── Cargo.toml
```

2. Compile .proto files in `build.rs`

```rust
// build.rs
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file_descriptors = protox::compile(
        &["proto/user.proto", "proto/product.proto"],
        &["proto"],
    )?;

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_fds(file_descriptors)?;

    Ok(())
}
```

3. Add dependencies to `Cargo.toml`

```toml
[dependencies]
reinhardt-grpc = "0.1.0-alpha.1"
tonic = "0.12"
prost = "0.13"

[build-dependencies]
tonic-build = "0.12"
protox = "0.7"
```

4. Use generated code

```rust
// src/lib.rs
pub mod proto {
    pub mod user {
        tonic::include_proto!("myapp.user");
    }
    pub mod product {
        tonic::include_proto!("myapp.product");
    }
}

// Use common types from reinhardt-grpc
use reinhardt::grpc::proto::common::{Empty, Timestamp, PageInfo};
```

### Dependency Injection

Enable the `di` feature to use dependency injection in gRPC handlers:

```toml
[dependencies]
reinhardt-grpc = { version = "0.1", features = ["di"] }
reinhardt-di = "0.1"
```

#### Basic Usage

```rust
use reinhardt::grpc::{GrpcRequestExt, grpc_handler};
use reinhardt_di::InjectionContext;
use tonic::{Request, Response, Status};
use std::sync::Arc;

pub struct UserServiceImpl {
    injection_context: Arc<InjectionContext>,
}

#[tonic::async_trait]
impl UserService for UserServiceImpl {
    async fn get_user(&self, mut request: Request<GetUserRequest>)
        -> Result<Response<User>, Status>
    {
        // Set DI context in request extensions
        request.extensions_mut().insert(self.injection_context.clone());

        // Call handler with DI support
        self.get_user_impl(request).await
    }
}

impl UserServiceImpl {
    #[grpc_handler]
    async fn get_user_impl(
        &self,
        request: Request<GetUserRequest>,
        #[inject] db: DatabaseConnection,  // Auto-injected
    ) -> Result<Response<User>, Status> {
        let user_id = request.into_inner().id;
        let user = db.fetch_user(user_id).await?;
        Ok(Response::new(user))
    }
}
```

#### Cache Control

Control dependency caching with the `cache` parameter:

```rust
#[grpc_handler]
async fn handler(
    &self,
    request: Request<Req>,
    #[inject] cached_db: DatabaseConnection,          // Cached (default)
    #[inject(cache = false)] fresh_db: DatabaseConnection,  // Not cached
) -> Result<Response<Resp>, Status> {
    // ...
}
```

### Integration with GraphQL

When using with the `reinhardt-graphql` crate, refer to the
[reinhardt-graphql documentation](../reinhardt-graphql/README.md).

## License

Licensed under the BSD 3-Clause License.
