# GraphQL Facade over gRPC

reinhardt-graphql provides functionality to expose gRPC services as GraphQL APIs.

## Feature Flags

```toml
[dependencies]
reinhardt-graphql = { version = "0.1.0-alpha.1", features = ["full"] }
# Or enable individually
reinhardt-graphql = { version = "0.1.0-alpha.1", features = ["graphql-grpc", "subscription"] }
```

### Available Features

- `graphql-grpc`: Query/Mutation gRPC integration
- `subscription`: Subscription gRPC integration (Rust 2024 compatible)
- `full`: Enable all features

## Usage

### 1. Automatic Conversion between Protobuf and GraphQL Types

Using `#[derive(GrpcGraphQLConvert)]` automatically generates conversions between Protobuf and GraphQL types:

```rust
use reinhardt_graphql::GrpcGraphQLConvert;
use async_graphql::Object;

// Define the struct
#[derive(GrpcGraphQLConvert)]
pub struct User {
    id: String,
    name: String,
    email: Option<String>,
}

// Implement GraphQL Object with async field resolvers
#[Object]
impl User {
    async fn id(&self) -> &str { &self.id }
    async fn name(&self) -> &str { &self.name }
    async fn email(&self) -> Option<&str> { self.email.as_deref() }
}

// Automatically generated:
// - From<proto::User> for User
// - From<User> for proto::User
```

### 2. Query/Mutation gRPC Integration

Create resolvers by implementing the `GrpcServiceAdapter` trait:

```rust
use reinhardt_graphql::GrpcServiceAdapter;
use async_trait::async_trait;

struct UserServiceAdapter {
    grpc_client: proto::UserServiceClient<tonic::transport::Channel>,
}

#[async_trait]
impl GrpcServiceAdapter for UserServiceAdapter {
    type Input = String; // User ID
    type Output = User;  // GraphQL User type
    type Error = anyhow::Error;

    async fn call(&self, user_id: Self::Input) -> Result<Self::Output, Self::Error> {
        let request = proto::GetUserRequest { id: user_id };
        let response = self.grpc_client.get_user(request).await?;
        Ok(response.into_inner().into()) // proto → GraphQL conversion
    }
}

// GraphQL resolver
struct Query;

#[Object]
impl Query {
    async fn user(&self, ctx: &Context<'_>, id: String) -> Result<User> {
        let adapter = ctx.data::<UserServiceAdapter>()?;
        adapter.call(id).await.map_err(|e| e.into())
    }
}
```

### 3. Subscription gRPC Integration

Using `#[derive(GrpcSubscription)]` automatically maps gRPC Server Streaming to GraphQL Subscriptions:

```rust
use reinhardt_graphql::GrpcSubscription;

#[derive(GrpcSubscription)]
#[grpc(
    service = "proto::UserEventsServiceClient",
    method = "subscribe_user_events",
    proto_type = "proto::UserEvent"
)]
#[graphql(type = "User")]
struct UserCreatedSubscription;

// Automatically generated GraphQL Subscription:
// subscription {
//   userCreated {
//     id
//     name
//     email
//   }
// }
```

**Required Attributes:**
- `#[grpc(service = "...")]`: gRPC service client type (e.g., `proto::UserServiceClient`)
- `#[grpc(method = "...")]`: gRPC streaming method name (e.g., `subscribe_user_events`)
- `#[grpc(proto_type = "...")]`: Protobuf event type (e.g., `proto::UserEvent`)
- `#[graphql(type = "...")]`: GraphQL output type (e.g., `User`)

**Optional Attributes:**
- `#[graphql(filter = "...")]`: Filter expression to select specific events (e.g., `|event| event.priority > 5`)

**How it Works:**
1. Retrieves gRPC client from GraphQL context: `ctx.data::<ServiceClient<Channel>>()`
2. Calls the gRPC streaming method: `client.method(request).await?.into_inner()`
3. Converts Protobuf events to GraphQL types using `Into` trait
4. Applies filter expression if specified
5. Returns a `Stream<Item = GraphQLType>`

**Type Conversion:**
The macro expects `From<ProtoType>` or `Into<GraphQLType>` to be implemented for automatic conversion:

```rust
impl From<proto::UserEvent> for User {
    fn from(event: proto::UserEvent) -> Self {
        User {
            id: event.user_id,
            name: event.name,
            email: event.email,
        }
    }
}
```

**Rust 2024 Compatible:** This macro uses `Box::pin` and explicit lifetime annotations to solve Rust 2024's lifetime capture issues.

**Example with Filter:**

```rust
use reinhardt_graphql::GrpcSubscription;

#[derive(GrpcSubscription)]
#[grpc(
    service = "proto::EventServiceClient",
    method = "subscribe_events",
    proto_type = "proto::Event"
)]
#[graphql(
    type = "GraphQLEvent",
    filter = "|event| event.priority > 5"
)]
struct HighPriorityEventsSubscription;
```

This will only emit events where `priority > 5`.

### 4. Manual Implementation (Advanced Use Cases)

For finer control, manually implement `GrpcSubscriptionAdapter`:

```rust
use reinhardt_graphql::GrpcSubscriptionAdapter;

struct UserEventsAdapter;

impl GrpcSubscriptionAdapter for UserEventsAdapter {
    type Proto = proto::UserEvent;
    type GraphQL = User;
    type Error = anyhow::Error;

    fn map_event(&self, proto: Self::Proto) -> Option<Self::GraphQL> {
        // Filter by event type
        if proto.event_type == proto::EventType::Created as i32 {
            proto.user.map(|u| u.into())
        } else {
            None
        }
    }
}

#[Subscription]
impl Subscription {
    async fn user_created<'ctx>(
        &self,
        ctx: &Context<'ctx>,
    ) -> impl Stream<Item = User> + 'ctx {
        use tokio_stream::StreamExt;

        let client = ctx.data::<proto::UserEventsServiceClient<_>>().unwrap();
        let adapter = UserEventsAdapter;

        let stream = client
            .subscribe_user_events(proto::SubscribeRequest::default())
            .await
            .unwrap()
            .into_inner();

        // Rust 2024 compatible: Wrapped with Box::pin
        Box::pin(stream.filter_map(move |result| async move {
            match result {
                Ok(proto_event) => adapter.map_event(proto_event),
                Err(_) => None,
            }
        }))
    }
}
```

## Architecture

```
┌─────────────────┐
│  GraphQL Client │
└────────┬────────┘
         │ GraphQL Query/Mutation/Subscription
         ↓
┌─────────────────────────────────────┐
│  reinhardt-graphql                  │
│  ┌──────────────────────────────┐  │
│  │ GraphQL Schema & Resolvers   │  │
│  └────────┬─────────────────────┘  │
│           │                         │
│  ┌────────↓─────────────────────┐  │
│  │ GrpcServiceAdapter           │  │
│  │ GrpcSubscriptionAdapter      │  │
│  └────────┬─────────────────────┘  │
└───────────┼─────────────────────────┘
            │ gRPC (tonic)
            ↓
┌─────────────────────────────────────┐
│  reinhardt-grpc                     │
│  ┌──────────────────────────────┐  │
│  │ gRPC Services (User Service) │  │
│  └──────────────────────────────┘  │
└─────────────────────────────────────┘
```

## Solving Rust 2024 Subscription Issues

async-graphql 7.0 is not compatible with Rust 2024's new lifetime capture rules. This issue is resolved by using gRPC Server Streaming:

**Traditional async-graphql Subscription (doesn't work):**

```rust
// Compilation error in Rust 2024
async fn user_created<'ctx>(&self, ctx: &Context<'ctx>)
    -> impl Stream<Item = User> + 'ctx
{
    async_stream::stream! {
        // lifetime capture issue
    }
}
```

**gRPC-based Subscription (works):**

```rust
// Rust 2024 compatible
async fn user_created<'ctx>(&self, ctx: &Context<'ctx>)
    -> impl Stream<Item = User> + 'ctx
{
    let stream = grpc_client.subscribe().await?.into_inner();
    Box::pin(stream.filter_map(/* ... */)) // ✅ OK
}
```

## Performance

- **Direct GraphQL**: ~3-4 µs/query
- **GraphQL over gRPC**: ~4-5 µs/query
- **Overhead**: 5-21% (+0.2-0.8 µs)

See [PERFORMANCE.md](PERFORMANCE.md) for details.

## Sample Code

See the `tests/` directory for complete implementation examples:

- `tests/grpc_services/` - gRPC service implementation examples
- `tests/proto/` - Protobuf definition examples

## License

BSD 3-Clause License
