# Reinhardt Dispatch

HTTP request dispatching and handler system for the Reinhardt framework.

## Overview

`reinhardt-dispatch` provides the core request handling functionality, equivalent to Django's `django.core.handlers` and `django.dispatch`. It orchestrates the complete request lifecycle including middleware execution, signal emission, and exception handling.

## Installation

Add `reinhardt` to your `Cargo.toml`:

<!-- reinhardt-version-sync:3 -->
```toml
[dependencies]
reinhardt = { version = "0.1.0-rc.25", features = ["dispatch"] }

# Or use a preset:
# reinhardt = { version = "0.1.0-rc.25", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.1.0-rc.25", features = ["full"] }      # All features
```

Then import dispatch features:

```rust
use reinhardt::dispatch::{BaseHandler, MiddlewareChain, Dispatcher};
```

**Note:** Dispatch features are included in the `standard` and `full` feature presets.

## Features

- **Request Lifecycle Management**: Handle HTTP requests from start to finish
- **Middleware Chain**: Composable middleware for request/response processing
- **Signal Integration**: Emit lifecycle signals (`request_started`, `request_finished`)
- **Exception Handling**: Convert errors into appropriate HTTP responses
- **Async Support**: Full async/await support with Tokio

## Architecture

```text
Request → BaseHandler → Middleware Chain → URL Resolver → View → Response
               ↓                                            ↓
          Signals                                      Signals
      (request_started)                          (request_finished)
```

## Usage

### Basic Request Handling

```rust
use reinhardt::dispatch::{BaseHandler, DispatchError};
use reinhardt::http::{Request, Response};

async fn handle_request(request: Request) -> Result<Response, DispatchError> {
    let handler = BaseHandler::new();
    handler.handle_request(request).await
}
```

### With Middleware

```rust
use reinhardt::dispatch::{BaseHandler, MiddlewareChain};
use reinhardt_http::{Handler, Middleware};
use std::sync::Arc;

async fn setup_handler() -> Result<Arc<dyn Handler>, Box<dyn std::error::Error>> {
    let handler = Arc::new(BaseHandler::new());

    let chain = MiddlewareChain::new(handler)
        .add_middleware(Arc::new(LoggingMiddleware))?
        .add_middleware(Arc::new(AuthMiddleware))?
        .build();
    Ok(chain)
}
```

### Exception Handling

The exception handler automatically converts errors into HTTP responses:

- `DispatchError::View` → 500 Internal Server Error
- `DispatchError::UrlResolution` → 404 Not Found
- `DispatchError::Middleware` → 500 Internal Server Error
- `DispatchError::Http` → 400 Bad Request
- `DispatchError::Internal` → 500 Internal Server Error

## Components

### BaseHandler

The core request handler that:

- Emits `request_started` signal
- Processes the request (delegates to URL resolver and views)
- Emits `request_finished` signal
- Handles exceptions

### MiddlewareChain

Composes multiple middleware components into a processing pipeline:

```rust
let chain = MiddlewareChain::new(handler)
    .add_middleware(middleware1)?
    .add_middleware(middleware2)?
    .build();
```

Middleware are executed in reverse order (LIFO), so the last middleware added is the first to process the request.

### Dispatcher

High-level dispatcher that coordinates between the handler and the rest of the framework:

```rust
use reinhardt::dispatch::Dispatcher;

let dispatcher = Dispatcher::new(BaseHandler::new());
let response = dispatcher.dispatch(request).await?;
```

### Exception Handling

The exception module provides:

- `ExceptionHandler` trait for custom exception handling
- `convert_exception_to_response` helper function
- `IntoResponse` trait for converting types to HTTP responses

## Django Equivalents

| Reinhardt                 | Django                                             |
| ------------------------- | -------------------------------------------------- |
| `BaseHandler`             | `django.core.handlers.base.BaseHandler`            |
| `MiddlewareChain`         | `django.core.handlers.base.MiddlewareChain`        |
| `ExceptionHandler`        | `django.core.handlers.exception.exception_handler` |
| `request_started` signal  | `django.core.signals.request_started`              |
| `request_finished` signal | `django.core.signals.request_finished`             |

## Implementation Notes

This crate focuses on HTTP request dispatching, while signal dispatching is handled by `reinhardt-core`'s signals module. This separation provides:

- Clear responsibility boundaries
- Independent signal system that can be used outside HTTP context
- Specialized HTTP request handling with middleware and exception handling

## License

Licensed under the same terms as the Reinhardt project.
