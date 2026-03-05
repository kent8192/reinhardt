+++
title = "Middleware Creation"
weight = 30
+++

# Middleware Creation

Guide to creating custom middleware.

## Table of Contents

- [Middleware trait](#middleware-trait)
- [Basic Middleware](#basic-middleware)
- [Conditional Execution](#conditional-execution)
- [Stateful Middleware](#stateful-middleware)
- [Middleware Ordering](#middleware-ordering)
- [Available Middleware](#available-middleware)

---

## Middleware trait

All middleware must implement the `Middleware` trait.

```rust
use async_trait::async_trait;
use reinhardt::{Handler, Middleware, Request, Response, Result};
use std::sync::Arc;

#[async_trait]
pub trait Middleware: Send + Sync {
    /// Process the request and optionally call the next handler
    async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response>;

    /// Determine if this middleware should process the request
    fn should_continue(&self, _request: &Request) -> bool {
        true
    }
}
```

---

## Basic Middleware

### Request Logging Middleware

```rust
use async_trait::async_trait;
use reinhardt::{Handler, Middleware, Request, Response, Result};
use std::sync::Arc;

pub struct LoggingMiddleware;

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
        println!("Request: {} {}", request.method, request.uri.path());

        // Call next handler or middleware
        let response = next.handle(request).await?;

        println!("Response: {}", response.status);
        Ok(response)
    }
}
```

### Custom Header Middleware

```rust
use async_trait::async_trait;
use reinhardt::{Handler, Middleware, Request, Response, Result};
use std::sync::Arc;

pub struct CustomHeaderMiddleware {
    pub header_name: String,
    pub header_value: String,
}

impl CustomHeaderMiddleware {
    pub fn new(name: &str, value: &str) -> Self {
        Self {
            header_name: name.to_string(),
            header_value: value.to_string(),
        }
    }
}

#[async_trait]
impl Middleware for CustomHeaderMiddleware {
    async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
        let mut response = next.handle(request).await?;

        // Add custom header to response
        response.headers.insert(
            hyper::header::HeaderName::from_bytes(self.header_name.as_bytes()).unwrap(),
            hyper::header::HeaderValue::from_str(&self.header_value).unwrap(),
        );

        Ok(response)
    }
}
```

---

## Conditional Execution

### Implementing `should_continue()`

Execute middleware only under certain conditions.

```rust
use async_trait::async_trait;
use reinhardt::{Handler, Middleware, Request, Response, Result};
use std::sync::Arc;

pub struct AdminOnlyMiddleware;

#[async_trait]
impl Middleware for AdminOnlyMiddleware {
    fn should_continue(&self, request: &Request) -> bool {
        // Only process /admin/ paths
        request.uri.path().starts_with("/admin/")
    }

    async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
        // Admin check logic
        let auth_header = request.get_header("Authorization");

        if auth_header.is_some() && auth_header.unwrap().starts_with("Bearer admin") {
            next.handle(request).await
        } else {
            Ok(Response::forbidden())
        }
    }
}
```

---

## Stateful Middleware

### Rate Limiting Middleware

```rust
use async_trait::async_trait;
use reinhardt::{Handler, Middleware, Request, Response, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct RateLimiter {
    requests: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            requests: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window,
        }
    }

    fn check_rate_limit(&self, key: &str) -> bool {
        let mut requests = self.requests.lock().unwrap();
        let now = Instant::now();
        let entry = requests.entry(key.to_string()).or_insert_with(Vec::new);

        // Remove old requests
        entry.retain(|&t| now.duration_since(t) < self.window);

        if entry.len() >= self.max_requests {
            false
        } else {
            entry.push(now);
            true
        }
    }
}

#[async_trait]
impl Middleware for RateLimiter {
    fn should_continue(&self, _request: &Request) -> bool {
        true
    }

    async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
        let client_ip = request.get_client_ip()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        if self.check_rate_limit(&client_ip) {
            next.handle(request).await
        } else {
            Ok(Response::internal_server_error()
                .with_body("Rate limit exceeded")
                .with_stop_chain(true))
        }
    }
}
```

---

## Middleware Ordering

### Recommended Order

Middleware execution order matters. A typical recommended order:

1. `RequestIdMiddleware` - Generate request ID first
2. `LoggingMiddleware` - Log all requests
3. `TracingMiddleware` - Start tracing span
4. `SecurityMiddleware` - Apply security headers
5. `CorsMiddleware` - Handle CORS preflight
6. `SessionMiddleware` - Load session
7. `AuthenticationMiddleware` - Authenticate user
8. `CsrfMiddleware` - Validate CSRF token
9. `RateLimitMiddleware` - Apply rate limits
10. Application handlers

```rust
use reinhardt::ServerRouter;
use reinhardt::{
    LoggingMiddleware, SecurityMiddleware, CorsMiddleware,
    SessionMiddleware, CsrfMiddleware, RateLimitMiddleware
};

let router = ServerRouter::new()
    .with_middleware(LoggingMiddleware::new())
    .with_middleware(SecurityMiddleware::new())
    .with_middleware(CorsMiddleware::permissive())
    .with_middleware(SessionMiddleware::new(store))
    .with_middleware(CsrfMiddleware::default())
    .with_middleware(RateLimitMiddleware::new(strategy, store));
```

---

## Available Middleware

Reinhardt includes 30+ built-in middleware components.

### Authentication & Authorization

| Middleware | Description |
|------------|-------------|
| `AuthenticationMiddleware` | Session-based user authentication |

### Security

| Middleware | Description |
|------------|-------------|
| `CorsMiddleware` | Cross-Origin Resource Sharing |
| `CsrfMiddleware` | CSRF token protection |
| `CspMiddleware` | Content Security Policy headers |
| `XFrameOptionsMiddleware` | Clickjacking protection |
| `HttpsRedirectMiddleware` | Force HTTPS connections |
| `SecurityMiddleware` | Combined security headers |

### Performance

| Middleware | Description |
|------------|-------------|
| `CacheMiddleware` | HTTP response caching |
| `GZipMiddleware` | Gzip compression |
| `BrotliMiddleware` | Brotli compression |
| `ETagMiddleware` | ETag generation and validation |
| `ConditionalGetMiddleware` | Conditional GET support |

### Observability

| Middleware | Description |
|------------|-------------|
| `LoggingMiddleware` | Request/response logging |
| `TracingMiddleware` | Distributed tracing |
| `MetricsMiddleware` | Performance metrics collection |
| `RequestIdMiddleware` | Unique request ID generation |

### Rate Limiting

| Middleware | Description |
|------------|-------------|
| `RateLimitMiddleware` | API rate limiting |

### Resilience

| Middleware | Description |
|------------|-------------|
| `CircuitBreakerMiddleware` | Circuit breaker pattern |
| `TimeoutMiddleware` | Request timeout handling |

### Session & State

| Middleware | Description |
|------------|-------------|
| `SessionMiddleware` | Session management |
| `SiteMiddleware` | Multi-site support |
| `LocaleMiddleware` | Internationalization and locale detection |

### Utility

| Middleware | Description |
|------------|-------------|
| `CommonMiddleware` | Common HTTP functionality |
| `BrokenLinkEmailsMiddleware` | Broken link notification |
| `FlatpagesMiddleware` | Static page serving from database |

---

## See Also

- [CORS Configuration](../cors/)
- [Serving Static Files](../static-files/)
- [Request API](https://docs.rs/reinhardt-http/latest/reinhardt_http/struct.Request.html)
- [Response API](https://docs.rs/reinhardt-http/latest/reinhardt_http/struct.Response.html)
