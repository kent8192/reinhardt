# reinhardt-middleware

Request/response processing pipeline for Reinhardt framework

## Overview

Middleware system for processing requests and responses. Provides comprehensive built-in middleware for security, performance optimization, authentication, and request handling.

## Installation

Add `reinhardt` to your `Cargo.toml`:

```toml
[dependencies]
reinhardt = { version = "0.1.0-rc.19", features = ["middleware"] }

# Or use a preset:
# reinhardt = { version = "0.1.0-rc.19", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.1.0-rc.19", features = ["full"] }      # All features
```

Then import middleware features:

```rust
use reinhardt::middleware::{CsrfMiddleware, CorsMiddleware, GZipMiddleware};
use reinhardt::middleware::{LoggingMiddleware, AuthenticationMiddleware};
use reinhardt::middleware::{SecurityMiddleware, HttpsRedirectMiddleware};
```

**Note:** Middleware features are included in the `standard` and `full` feature presets.

## Implemented Features ✓

### Core Middleware System

- **Middleware Pipeline** - Request/response processing chain with handler composition
- **Custom Middleware Support** - Easy integration of user-defined middleware

### Security Middleware

- **CORS (Cross-Origin Resource Sharing)** - Configurable CORS headers with preflight support
  - Custom origin, method, and header configuration
  - Credentials support
  - Max-age caching
  - Permissive mode for development
- **CSRF Protection** - Cross-Site Request Forgery protection via `reinhardt-core` security module
  - Token generation and validation
  - Origin and referer checking
  - Secret management and rotation
- **Content Security Policy (CSP)** - XSS protection with customizable directives
  - Custom CSP directives (default-src, script-src, style-src, etc.)
  - Nonce generation for inline scripts/styles
  - Report-Only mode for testing
  - Strict preset configuration
- **X-Frame-Options** - Clickjacking protection
  - DENY mode (no framing)
  - SAMEORIGIN mode (same-origin framing only)
- **Security Headers** - Comprehensive HTTP security headers
  - HSTS (HTTP Strict Transport Security) with preload support
  - SSL/HTTPS redirects
  - X-Content-Type-Options: nosniff
  - Referrer-Policy configuration
  - Cross-Origin-Opener-Policy (COOP)
- **HTTPS Redirect** - Automatic HTTP to HTTPS redirection
  - Configurable exempt paths
  - Custom status codes (301/302)

### Performance Middleware

- **GZip Compression** - Response compression for bandwidth optimization
  - Configurable compression level (0-9)
  - Minimum size threshold
  - Content-type filtering
  - Automatic Accept-Encoding detection
- **Brotli Compression** - Advanced compression with better ratios
  - Configurable quality levels (Fast, Balanced, Best)
  - Window size configuration (10-24)
  - Content-type filtering
  - Automatic Accept-Encoding: br detection
  - Intelligent compression (only when beneficial)
- **Conditional GET** - HTTP caching with ETags and Last-Modified
  - Automatic ETag generation (SHA-256 based)
  - If-None-Match support
  - If-Modified-Since support
  - If-Match and If-Unmodified-Since validation
  - 304 Not Modified responses

### Authentication & Request Processing

- **Authentication** - JWT-based authentication middleware
  - Bearer token extraction
  - Token validation via `reinhardt-auth`
  - User type support
- **Logging** - Request/response logging
  - Timestamp, method, path, status code
  - Request duration tracking

### Dependency Injection Support

- **DI Middleware** - Integration with `reinhardt-di`
  - Middleware factory pattern
  - Injectable middleware components
  - Automatic dependency resolution

### Request Processing & Utilities

- **Common Middleware** - URL normalization
  - Automatic trailing slash appending (append_slash)
  - WWW subdomain prepending (prepend_www)
  - Smart file extension detection
  - Query parameter preservation
- **Locale Middleware** - Multi-source locale detection
  - Accept-Language header parsing with quality scores
  - Cookie-based locale storage
  - URL path prefix detection
  - Configurable fallback locale
- **Message Framework** - Django-style flash messages
  - Session-based and Cookie-based storage
  - Multiple message levels (Debug, Info, Success, Warning, Error)
  - One-time message delivery
  - Thread-safe storage implementation
- **Redirect Fallback** - Smart 404 error handling
  - Configurable fallback URL
  - Pattern-based path matching (regex)
  - Custom redirect status codes
  - Redirect loop prevention
- **Broken Link Detection** - Internal link monitoring
  - Automatic 404 detection for internal referrers
  - Domain normalization (www. handling)
  - Configurable ignored paths and user agents
  - Email notification support
  - Logging integration
- **Site Middleware** - Multi-site support
  - Domain-based site detection
  - Default site fallback mechanism
  - www subdomain normalization
  - Site ID header injection
  - Thread-safe site registry
- **Flatpages Middleware** - Static page fallback
  - 404 interception and content substitution
  - URL normalization (trailing slash handling)
  - In-memory flatpage storage
  - Template rendering support
  - Registration-based access control

### Observability & Monitoring

- **Request ID Middleware** - Request tracing and correlation
  - UUID generation for unique request identification
  - Automatic propagation through request chain
  - Custom header name support
  - X-Request-ID and X-Correlation-ID compatibility
- **Metrics Middleware** - Prometheus-compatible metrics collection
  - Request count tracking by method and path
  - Response time histograms with percentiles (p50, p95, p99)
  - Status code distribution
  - Custom metrics support
  - /metrics endpoint with Prometheus text format
  - Configurable exclusion paths
- **Tracing Middleware** - Distributed tracing support
  - OpenTelemetry-compatible span tracking
  - Trace ID and Span ID propagation
  - Automatic span lifecycle management
  - Request metadata tagging (method, path, status)
  - Configurable sampling rates
  - Error status tracking

## Related Crates

The following middleware are implemented in separate crates:

- **Session Middleware** - Implemented in `reinhardt-auth`
  - See [reinhardt-auth](../reinhardt-auth/README.md) for session management and persistence
- **Cache Middleware** - Implemented in `reinhardt-utils`
  - See [reinhardt-utils](../reinhardt-utils/README.md) for response caching layer
- **Permissions Middleware** - Implemented in `reinhardt-auth`
  - ✓ Permission-based access control
  - ✓ DRF-style permissions (IsAuthenticated, IsAdminUser, IsAuthenticatedOrReadOnly)
  - ✓ Model-level permissions (object permissions)
  - ✓ Permission operators (AND, OR, NOT)
  - ✓ Advanced permissions (dynamic, conditional, composite)
  - See [reinhardt-auth](../reinhardt-auth/README.md) for details
- **Rate Limiting** - Implemented in `reinhardt-rest`
  - ✓ Request throttling and rate limits
  - ✓ AnonRateThrottle for anonymous users
  - ✓ UserRateThrottle for authenticated users
  - ✓ ScopedRateThrottle for API scopes
  - ✓ BurstRateThrottle for burst protection
  - ✓ TieredRateThrottle for tiered limits
  - ✓ Memory and Redis backends
  - See [reinhardt-rest](../reinhardt-rest/README.md) for details

## CSRF Middleware Usage

### Basic Usage

```rust
use reinhardt::middleware::csrf::{CsrfMiddleware, CsrfMiddlewareConfig};
use reinhardt_http::{Handler, Middleware};
use std::sync::Arc;

// Default configuration
let csrf_middleware = CsrfMiddleware::new();

// Production configuration
let config = CsrfMiddlewareConfig::production(vec![
    "https://example.com".to_string(),
    "https://api.example.com".to_string(),
]);

let csrf_middleware = CsrfMiddleware::with_config(config);
```

### Exempt Paths

```rust
let config = CsrfMiddlewareConfig::default()
    .add_exempt_path("/api/webhooks".to_string())
    .add_exempt_path("/health".to_string());

let csrf_middleware = CsrfMiddleware::with_config(config);
```

### Token Extraction

CSRF tokens can be sent via:

1. **HTTP Header** (recommended): `X-CSRFToken` header
2. **Cookie**: `csrftoken` cookie

```javascript
// Send token via header from JavaScript
fetch("/api/endpoint", {
  method: "POST",
  headers: {
    "X-CSRFToken": getCookie("csrftoken"),
    "Content-Type": "application/json",
  },
  body: JSON.stringify(data),
});
```

### How It Works

1. **GET requests**: Automatically sets a CSRF cookie
2. **POST requests**: Validates the token
   - Extracts token from header or cookie
   - Checks Referer header (if configured)
   - Validates token format and value
3. **Validation failure**: Returns 403 Forbidden
