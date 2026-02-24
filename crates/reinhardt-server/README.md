# reinhardt-server

HTTP server implementation for Reinhardt framework

## Overview

`` `reinhardt-server` `` provides a high-performance HTTP server implementation for Reinhardt applications, built on Hyper with support for WebSockets and GraphQL.

## Features

### Implemented ✓

This crate provides the following features:

- **Core HTTP Server**: High-performance HTTP/1.1 server
  - Async request processing with Tokio runtime
  - Custom handler support via Handler trait
  - Middleware pipeline for request/response processing
  - Builder pattern for middleware configuration
  - Efficient TCP connection management
  - Automatic request/response conversion
  - Built-in error handling

- **WebSocket Support** (feature = "websocket"): WebSocket server implementation
  - tokio-tungstenite based WebSocket server
  - Custom message handler support
  - Broadcast support for multiple clients
  - Client connection management with automatic registration/unregistration
  - Connection lifecycle hooks (on_connect, on_disconnect)
  - Text and binary message handling
  - Automatic connection management

- **GraphQL Support** (feature = "graphql"): GraphQL endpoint integration
  - async-graphql integration
  - Schema builder for Query and Mutation roots
  - POST request handling for GraphQL queries
  - JSON response serialization
  - Error handling for GraphQL errors

## Installation

Add `reinhardt` to your `Cargo.toml`:

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", features = ["server"] }

# For WebSocket support:
# reinhardt = { version = "0.1.0-alpha.1", features = ["server", "server-websocket"] }

# For GraphQL support:
# reinhardt = { version = "0.1.0-alpha.1", features = ["server", "server-graphql"] }

# Or use a preset:
# reinhardt = { version = "0.1.0-alpha.1", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.1.0-alpha.1", features = ["full"] }      # All features
```

Then import server features:

```rust
use reinhardt::server::{serve, HttpServer};
use reinhardt::server::{WebSocketServer, WebSocketHandler};  // WebSocket
use reinhardt::server::graphql_handler;  // GraphQL
```

**Note:** Server features are included in the `standard` and `full` feature presets.

## Usage

### Basic HTTP Server

```rust
use reinhardt::server::{serve, HttpServer};
use reinhardt::http::{Request, Response};
use reinhardt::core::exception::Error;
use std::sync::Arc;

async fn my_handler(req: Request) -> Result<Response, Error> {
    Response::ok().with_body("Hello, world!")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let handler = Arc::new(my_handler);
    serve("127.0.0.1:8000", handler).await?;
    Ok(())
}
```

### HTTP Server with Middleware

```rust
use reinhardt::server::HttpServer;
use reinhardt::core::types::{Handler, Middleware};
use reinhardt::http::{Request, Response};
use reinhardt::core::exception::Error;
use std::sync::Arc;

struct MyHandler;

#[async_trait::async_trait]
impl Handler for MyHandler {
    async fn handle(&self, _req: Request) -> Result<Response, Error> {
        Ok(Response::ok().with_body("Hello from handler!"))
    }
}

struct LoggingMiddleware;

#[async_trait::async_trait]
impl Middleware for LoggingMiddleware {
    async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response, Error> {
        println!("Request: {} {}", request.method, request.uri);
        let response = next.handle(request).await?;
        println!("Response: {}", response.status);
        Ok(response)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let handler = Arc::new(MyHandler);
    let middleware = Arc::new(LoggingMiddleware);

    let server = HttpServer::new(handler)
        .with_middleware(middleware);

    server.listen("127.0.0.1:8000".parse()?).await?;
    Ok(())
}
```

### WebSocket Server

```rust
use reinhardt::server::{WebSocketServer, WebSocketHandler};
use std::sync::Arc;

struct EchoHandler;

#[async_trait::async_trait]
impl WebSocketHandler for EchoHandler {
    async fn handle_message(&self, message: String) -> Result<String, String> {
        Ok(format!("Echo: {}", message))
    }

    async fn on_connect(&self) {
        println!("Client connected");
    }

    async fn on_disconnect(&self) {
        println!("Client disconnected");
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let handler = Arc::new(EchoHandler);
    let server = WebSocketServer::new(handler);
    server.listen("127.0.0.1:9001".parse()?).await?;
    Ok(())
}
```

### WebSocket Server with Broadcast

```rust
use reinhardt::server::{WebSocketServer, WebSocketHandler};
use std::sync::Arc;

struct ChatHandler;

#[async_trait::async_trait]
impl WebSocketHandler for ChatHandler {
    async fn handle_message(&self, message: String) -> Result<String, String> {
        Ok(format!("Received: {}", message))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let handler = Arc::new(ChatHandler);
    let server = WebSocketServer::new(handler)
        .with_broadcast(100); // Enable broadcast with capacity of 100 messages

    // Clone broadcast manager to send messages from other tasks
    let broadcast_manager = server.broadcast_manager().unwrap().clone();

    // Spawn a task to send periodic broadcasts
    tokio::spawn(async move {
        loop {
// tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            broadcast_manager.broadcast("Server announcement!".to_string()).await;
        }
    });

    server.listen("127.0.0.1:9001".parse()?).await?;
    Ok(())
}
```

### GraphQL Server

```rust
use reinhardt::server::graphql_handler;
use async_graphql::{Object, Schema, EmptyMutation, EmptySubscription};

struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn hello(&self) -> String {
        "Hello, GraphQL!".to_string()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
        .finish();

    let handler = graphql_handler(schema);
    serve("127.0.0.1:8000", handler).await?;
    Ok(())
}
```

## server

### Features

### Implemented ✓

#### Core HTTP Server

- **HTTP/1.1 Server**: High-performance HTTP/1.1 server implementation based on Hyper
- **Async Request Processing**: Full asynchronous processing with Tokio runtime
- **Custom Handler Support**: Add custom logic by implementing the `Handler` trait
- **TCP Connection Management**: Efficient TCP connection management and task spawning
- **Request/Response Conversion**: Automatic conversion between Hyper requests and reinhardt-http Request/Response
- **Error Handling**: Automatically converts handler errors to 500 error responses

#### WebSocket Support (feature = "websocket")

- **WebSocket Server**: WebSocket server implementation based on tokio-tungstenite
- **Custom Message Handlers**: Customize message processing via the `WebSocketHandler` trait
- **Connection Lifecycle Hooks**: Handle connection events with `on_connect` and `on_disconnect`
- **Text/Binary Messages**: Process text messages and echo binary messages
- **Automatic Connection Management**: Automatic handling of WebSocket connection establishment, message loops, and closure
- **Peer Information**: Access to client SocketAddr information

#### GraphQL Support (feature = "graphql")

- **GraphQL Handler**: GraphQL endpoint support with async-graphql integration
- **Schema Builder**: Automatic schema construction from Query and Mutation roots
- **POST Request Processing**: Execute GraphQL queries via POST requests
- **JSON Responses**: Automatic JSON serialization of GraphQL execution results
- **Error Handling**: Proper handling and response of GraphQL errors
- **Empty Subscriptions**: Uses `EmptySubscription` by default

#### Convenience Functions

- **`serve()` function**: Helper function providing easy HTTP server startup
- **`serve_websocket()` function**: Helper function providing easy WebSocket server startup
- **`graphql_handler()` function**: Simplifies Arc wrapping of GraphQL handlers

#### Graceful Shutdown

- **ShutdownCoordinator**: Graceful shutdown coordination mechanism
  - Signal handling (SIGTERM, SIGINT)
  - Wait for existing connections to complete
  - Shutdown with timeout processing
  - Shutdown notification via broadcast channel
- **shutdown_signal()**: Listen for OS shutdown signals
- **listen_with_shutdown()**: Start server with graceful shutdown support
- **serve_with_shutdown()**: Convenience function with graceful shutdown support
- **with_shutdown()**: Add shutdown handling to Futures

#### HTTP/2 Support

- **Http2Server**: HTTP/2 protocol server implementation
  - Uses hyper-util's HTTP/2 builder
  - Full asynchronous request processing
  - Graceful shutdown support
  - Uses same Handler trait as HTTP/1.1
- **serve_http2()**: Easy HTTP/2 server startup
- **serve_http2_with_shutdown()**: HTTP/2 server startup with graceful shutdown support

#### Request Timeouts

- **TimeoutHandler**: Request timeout middleware
  - Configurable timeout duration
  - Returns 408 Request Timeout response on timeout
  - Can wrap any Handler
  - Fully tested

#### Rate Limiting

- **RateLimitHandler**: Rate limiting middleware
  - IP address-based rate limiting
  - Supports Fixed Window and Sliding Window strategies
  - Configurable window period and maximum request count
  - Returns 429 Too Many Requests response when rate limit exceeded
- **RateLimitConfig**: Rate limit configuration
  - `per_minute()`: Per-minute rate limiting
  - `per_hour()`: Per-hour rate limiting
  - Custom configurable

#### Advanced HTTP Features

- **Middleware Pipeline**: Middleware chain for request/response processing
- **Connection Pooling**: Efficient HTTP connection pooling mechanism
- **Request Logging**: Structured request logging

#### WebSocket Advanced Features

- **Broadcast Support**: Message broadcasting to multiple clients
- **Room-Based Management**: Manage clients by rooms
- **Message Compression**: WebSocket message compression support
- **Heartbeat/Ping-Pong**: Connection alive check mechanism
- **Authentication/Authorization**: Authentication and authorization for WebSocket connections

#### GraphQL Advanced Features

- **Subscription Support**: Real-time GraphQL subscriptions
- **DataLoader Integration**: DataLoader for solving N+1 problems
- **GraphQL Playground**: GraphQL IDE integration
- **File Uploads**: File uploads via GraphQL
- **Batch Queries**: Batch execution of multiple queries

#### Testing & Monitoring

- **Metrics**: Server metrics collection and publishing
- **Health Checks**: Server health check endpoints
- **Tracing**: Distributed tracing support

## License

Licensed under the BSD 3-Clause License.
