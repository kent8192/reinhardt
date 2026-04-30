# reinhardt-utils

Common utilities and helper functions

## Overview

Collection of utility functions and helpers used throughout the framework.

Includes date/time utilities, string manipulation, encoding/decoding, and other common operations.

## Installation

Add `reinhardt` to your `Cargo.toml`:

<!-- reinhardt-version-sync:3 -->
```toml
[dependencies]
reinhardt = { version = "0.1.0-rc.25", features = ["utils"] }

# Or use a preset:
# reinhardt = { version = "0.1.0-rc.25", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.1.0-rc.25", features = ["full"] }      # All features
```

Then import utility features:

```rust
use reinhardt::utils::cache::{Cache, InMemoryCache};
use reinhardt::utils::logging::{Logger, LogLevel};
use reinhardt::utils::storage::{Storage, LocalStorage};
use reinhardt::utils::core::html::{escape, unescape};
```

**Note:** Utility features are included in the `standard` and `full` feature presets.

## Features

### Implemented ✓

#### HTML Utilities (`html` module)

- **HTML Escaping/Unescaping**
  - `escape()`: Escapes HTML special characters (`<`, `>`, `&`, `"`, `'`)
  - `unescape()`: Unescapes HTML entities
  - `conditional_escape()`: Conditional escaping with autoescape flag support
  - `escape_attr()`: HTML attribute value escaping (handles newlines and tabs)
- **HTML Manipulation**
  - `strip_tags()`: Removes HTML tags
  - `strip_spaces_between_tags()`: Removes whitespace between tags
  - `truncate_html_words()`: Truncates by word count while preserving HTML tags
  - `format_html()`: HTML generation via placeholder replacement
- **Safe Strings**
  - `SafeString`: Safe string type for bypassing automatic escaping

#### Encoding Utilities (`encoding` module)

- **URL Encoding**
  - `urlencode()`: URL encoding (spaces converted to `+`)
  - `urldecode()`: URL decoding
- **JavaScript Escaping**
  - `escapejs()`: JavaScript string escaping (handles quotes, control characters, special characters)
- **Slugification**
  - `slugify()`: URL slug generation (lowercase, special character removal, hyphen-separated)
- **Text Processing**
  - `truncate_chars()`: Truncate by character count (appends `...`)
  - `truncate_words()`: Truncate by word count (appends `...`)
  - `wrap_text()`: Wrap text at specified width
  - `force_str()`: Safely convert byte sequences to UTF-8 strings
  - `force_bytes()`: Convert strings to byte sequences
- **Line Break Processing**
  - `linebreaks()`: Convert line breaks to `<br>` tags (with paragraph support)
  - `linebreaksbr()`: Convert line breaks to `<br>` tags (simple version)

#### Date/Time Formatting (`dateformat` module)

- **Django/PHP-style Formatting**
  - `format()`: Date/time formatting with format strings
  - Supported format codes:
    - Year: `Y` (4-digit), `y` (2-digit)
    - Month: `m` (zero-padded), `n` (no padding), `F` (full name), `M` (abbreviated)
    - Day: `d` (zero-padded), `j` (no padding), `l` (day name), `D` (day abbreviated)
    - Hour: `H` (24-hour), `h` (12-hour), `G`/`g` (unpadded versions)
    - Minute: `i`, Second: `s`
    - AM/PM: `A` (uppercase), `a` (lowercase)
- **Shortcut Functions** (`shortcuts` submodule)
  - `iso_date()`: YYYY-MM-DD format
  - `iso_datetime()`: YYYY-MM-DD HH:MM:SS format
  - `us_date()`: MM/DD/YYYY format
  - `eu_date()`: DD/MM/YYYY format
  - `full_date()`: "Monday, January 1, 2025" format
  - `short_date()`: "Jan 1, 2025" format
  - `time_24()`: 24-hour format time
  - `time_12()`: 12-hour format time (with AM/PM)

#### Text Manipulation (`text` module)

- **Case Conversion**
  - `capfirst()`: Capitalize first letter of each word
  - `title()`: Title case conversion (first letter uppercase, rest lowercase)
- **Number Formatting**
  - `intcomma()`: Add comma separators to integers (every 3 digits)
  - `floatcomma()`: Add comma separators to floating-point numbers
  - `ordinal()`: Add ordinal suffixes (1st, 2nd, 3rd, 4th, etc.)
- **Singular/Plural**
  - `pluralize()`: Toggle singular/plural based on count
- **Padding**
  - `ljust()`: Left-justify (right padding)
  - `rjust()`: Right-justify (left padding)
  - `center()`: Center-align (both-side padding)
- **Phone Number Formatting**
  - `phone_format()`: Convert 10/11-digit phone numbers to `(XXX) XXX-XXXX` format

#### Timezone Utilities (`timezone` module)

- **Basic DateTime Retrieval**
  - `now()`: Current UTC time
  - `localtime()`: Current local time
- **Timezone Conversion**
  - `to_local()`: UTC to local timezone conversion
  - `to_utc()`: Local to UTC conversion
  - `to_timezone()`: Timezone conversion by full IANA name (e.g., `America/New_York`, `Asia/Tokyo`) via `chrono-tz`
- **Naive/Aware Conversion**
  - `make_aware_utc()`: Convert naive datetime to UTC timezone-aware
  - `make_aware_local()`: Convert naive datetime to local timezone-aware
  - `is_aware()`: Check for timezone information presence (always `true` in Rust)
- **Parse/Format**
  - `parse_datetime()`: Parse ISO 8601 datetime strings
  - `format_datetime()`: Output datetime in ISO 8601 format (RFC 3339)
- **Timezone Name Retrieval**
  - `get_timezone_name_utc()`: Get timezone name for UTC datetime
  - `get_timezone_name_local()`: Get timezone name for local datetime


## cache

### Features

### Core Cache API - Implemented ✓

- **`Cache` trait**: Async-first trait for cache operations with generic type support
  - `get<T>()`: Retrieve values from cache with automatic deserialization
  - `set<T>()`: Store values with optional TTL (Time-To-Live)
  - `delete()`: Remove individual cache entries
  - `has_key()`: Check cache key existence
  - `clear()`: Remove all entries from cache
  - `get_many()`: Batch retrieval of multiple cache keys
  - `set_many()`: Batch storage of multiple values
  - `delete_many()`: Batch deletion of multiple keys
  - `incr()`: Atomic increment for numeric values
  - `decr()`: Atomic decrement for numeric values

### Cache Backends - Implemented ✓

- **InMemoryCache**: Thread-safe in-memory cache backend
  - Built on `Arc<RwLock<HashMap>>` for concurrent access
  - Automatic expiration with TTL support
  - `with_default_ttl()`: Configure default expiration time
  - `cleanup_expired()`: Manual cleanup of expired entries
  - JSON serialization via serde for type safety

- **RedisCache**: Redis-backed distributed cache (requires `redis-backend` feature)
  - Connection pooling with `ConnectionManager` for efficient connection reuse
  - `with_default_ttl()`: Default TTL configuration
  - `with_key_prefix()`: Namespace support for multi-tenant scenarios
  - Automatic key prefixing for organized cache entries
  - Full Redis integration with all core operations implemented
  - Batch operations (`get_many`, `set_many`, `delete_many`) for improved performance
  - Atomic operations (`incr`, `decr`) using Redis native commands

### Cache Key Management - Implemented ✓

- **CacheKeyBuilder**: Utility for generating versioned cache keys
  - `new()`: Create builder with custom prefix
  - `with_version()`: Version-based cache invalidation
  - `build()`: Generate prefixed and versioned keys
  - `build_many()`: Batch key generation
  - Format: `{prefix}:{version}:{key}`

### HTTP Middleware - Implemented ✓

- **CacheMiddleware**: Automatic HTTP response caching
  - Request method filtering (GET-only by default via `cache_get_only`)
  - Response status code filtering (2xx-only by default via `cache_success_only`)
  - Cache-Control header parsing (max-age, no-cache, no-store directives)
  - Configurable cache timeout with `CacheMiddlewareConfig`
  - Query parameter-aware cache key generation
  - Full response caching (status, headers, body)

- **CacheMiddlewareConfig**: Middleware configuration
  - `with_default_timeout()`: Set default cache duration
  - `with_key_prefix()`: Configure cache namespace
  - `cache_all_methods()`: Enable caching for non-GET requests
  - `cache_all_responses()`: Cache non-2xx responses
  - Custom Cache-Control header name support

### Dependency Injection Support - Implemented ✓

- **CacheService**: High-level service with DI integration
  - Automatic injection via `reinhardt-di`
  - Integrated `CacheKeyBuilder` for automatic key prefixing
  - Methods: `get()`, `set()`, `delete()` with automatic key building
  - Access to underlying cache via `cache()` method
  - Access to key builder via `key_builder()` method

- **RedisConfig**: Redis configuration for DI (requires `redis-backend` feature)
  - `new()`: Custom Redis URL configuration
  - `localhost()`: Quick localhost setup
  - Automatic injection from singleton scope
  - Fallback to localhost if not configured

- **Injectable trait implementations**:
  - `InMemoryCache`: Uses default singleton-based injection
  - `CacheKeyBuilder`: Custom default ("app" prefix, version 1)
  - `RedisCache`: Injects with `RedisConfig` dependency
  - `CacheService`: Composes cache and key builder via DI

### Feature Flags - Implemented ✓

- `redis-backend`: Enable Redis support (optional dependency)
- `memcached-backend`: Enable Memcached support (optional dependency)
- `all-backends`: Enable all backend implementations


## logging

### Features

### Implemented ✓

#### Core Logging Infrastructure

- **Logger System**: Core logger implementation with handler attachment support
  - `Logger` struct with warning level logging
  - `LoggerHandle`: Wrapper around `Arc<Logger>` for thread-safe logger access
  - LogHandler trait for extensible log processing
  - Thread-safe handler management
- **Log Levels**: Support for Debug, Info, Warn, and Error severity levels
- **Log Records**: Structured log record representation

#### Logging Configuration

- **Global Logging Manager**: Singleton-based global logging initialization and management
  - `init_global_logging()` for one-time setup
  - `get_logger(name)` returns `LoggerHandle` for retrieving named loggers
  - Thread-safe access via `once_cell`
- **Configuration Structures**:
  - `LoggingConfig`: Main configuration container
  - `HandlerConfig`: Handler-specific settings
  - `LoggerConfig`: Logger-specific settings

#### Log Handlers

- **Console Handler**: Output logs to standard output/error
- **File Handler**: Write logs to file system
- **JSON Handler**: Structured JSON log output
- **Memory Handler**: In-memory log storage for testing and debugging
  - Level-based filtering
  - Cloneable for test assertions

#### Formatters

- **Formatter Trait**: Extensible log formatting interface
- **Standard Formatter**: Default human-readable format
- **Server Formatter**: Server-optimized log format
- **Control Character Escaping**: `escape_control_chars()` utility for safe log output

#### Filters

- **Filter Trait**: Generic log filtering interface
- **Callback Filter**: Custom filter implementation support
- **Debug-based Filters**:
  - `RequireDebugTrue`: Only log when debug mode is enabled
  - `RequireDebugFalse`: Only log when debug mode is disabled

#### Parameter Representation Utilities

- **Smart Parameter Truncation**: Prevent log overflow with large data structures
  - `repr_params()`: Truncate arrays, objects, and nested structures
  - `truncate_param()`: Truncate individual string values
  - `ReprParamsConfig`: Configurable truncation behavior
- **Multi-batch Parameter Display**: Show first/last items for large parameter sets
- **Character-level Truncation**: Middle truncation preserving start/end context
- **SQLAlchemy-inspired Design**: Based on proven parameter representation patterns

#### Convenience APIs

- **Global Logging Functions**:
  - `emit_warning()`: Quick warning emission
  - `attach_memory_handler()`: Easy test handler attachment
- **Type Safety**: Strongly-typed configuration and log levels
- **Async Support**: Built on Tokio for async runtime compatibility

#### Security Logging

- **SecurityLogger**: Dedicated logger for security-related events
  - Authentication events (success/failure)
  - Authorization violations
  - CSRF violations
  - Rate limit exceeded events
  - Suspicious file operations
  - Disallowed host access
- **SecurityError**: Enum for categorizing security events
  - `AuthenticationFailed`, `AuthorizationFailed`, `InvalidToken`
  - `RateLimitExceeded`, `SuspiciousActivity`, `CsrfViolation`
  - `InvalidInput`, `AccessDenied`, `DisallowedHost`

**Usage Example**:

<!-- reinhardt-version-sync -->
```rust
use reinhardt::utils::logging::security::{SecurityLogger, SecurityError};

let logger = SecurityLogger::new();

// Log authentication events
logger.log_auth_event(true, "user@example.com");  // INFO level
logger.log_auth_event(false, "attacker@evil.com"); // WARNING level

// Log security errors
logger.log_security_error(&SecurityError::CsrfViolation);  // ERROR level

// Log CSRF violation with details
logger.log_csrf_violation("http://evil.com");

// Log rate limit exceeded
logger.log_rate_limit_exceeded("0.1.0-rc.25", 100);

// Log suspicious file operations
logger.log_suspicious_file_operation("delete", Path::new("/etc/passwd"));

// Log disallowed host access
logger.log_disallowed_host("malicious.com");
```

**Log Level Mapping**:

| Event | Log Level |
|-------|-----------|
| Authentication success | INFO |
| Authentication failure | WARNING |
| CSRF violation | ERROR |
| Rate limit exceeded | WARNING |
| Authorization failure | WARNING |
| Suspicious activity | ERROR |


## static

### Features

### Core Functionality

#### ✓ Implemented

- **Static File Configuration** (`StaticFilesConfig`)
  - Configurable static root directory for collected files
  - Static URL path configuration with validation
  - Multiple source directories support via `STATICFILES_DIRS`
  - Media URL configuration and conflict detection

- **Storage Backends** (`Storage` trait)
  - `FileSystemStorage` - Local file system storage
  - `MemoryStorage` - In-memory storage for testing
  - Extensible storage backend system

- **Static File Finder** (`StaticFilesFinder`)
  - Locate files across multiple static directories
  - Support for collecting files from various sources
  - `find_all()` - Recursively discover all static files across configured directories
  - Efficient directory tree traversal with proper error handling

- **Hashed File Storage** (`HashedFileStorage`)
  - File hashing for cache busting
  - Configurable hashing algorithms (MD5, SHA-256)
  - Automatic hash calculation and filename generation
  - Integration with manifest system

- **Manifest System** (`ManifestStaticFilesStorage`)
  - JSON manifest for mapping original filenames to hashed versions
  - Versioned manifest format (currently V1)
  - Enables efficient static file lookup in production
  - Supports deployment workflows with pre-collected assets

- **Media Asset Management** (`Media`, `HasMedia`)
  - CSS and JavaScript dependency declaration for forms and widgets
  - Media type organization (e.g., "all", "screen", "print")
  - HTML rendering for `<link>` and `<script>` tags
  - Dependency merging with duplicate prevention
  - Trait-based system for components to declare their assets

- **Static File Handler** (`StaticFileHandler`)
  - HTTP request handling for static files
  - MIME type detection via `mime_guess`
  - Error handling with `StaticError` and `StaticResult` types
  - File serving with proper content types
  - Directory serving with automatic index file detection
  - Configurable index files (default: `["index.html"]`) via `with_index_files()`
  - Serves index.html when accessing directories directly
  - **ETag Support**: Content-based ETag generation for conditional requests
    - Automatic ETag generation using hash of file content
    - Support for `If-None-Match` headers
    - 304 Not Modified responses for cached resources
    - Implemented in `handler.rs` (`StaticFile::etag()` method)

- **Configuration Validation** (`checks` module)
  - Django-style system checks for static files configuration
  - Multiple check levels: Debug, Info, Warning, Error, Critical
  - Comprehensive validation rules:
    - `static.E001` - STATIC_ROOT not set
    - `static.E002` - STATIC_ROOT in STATICFILES_DIRS
    - `static.E003` - STATIC_URL is empty
    - `static.E004` - STATICFILES_DIRS entry is not a directory
    - `static.W001` - STATIC_ROOT is subdirectory of STATICFILES_DIRS
    - `static.W002` - STATIC_URL doesn't start with '/'
    - `static.W003` - STATIC_URL doesn't end with '/'
    - `static.W004` - STATICFILES_DIRS is empty
    - `static.W005` - Directory doesn't exist
    - `static.W006` - Duplicate STATICFILES_DIRS entries
    - `static.W007` - MEDIA_URL doesn't start with '/'
    - `static.W008` - MEDIA_URL doesn't end with '/'
    - `static.W009` - MEDIA_URL prefix conflict with STATIC_URL
  - Helpful hints for fixing configuration issues

- **Health Check System** (`health` module)
  - Health status monitoring (Healthy, Degraded, Unhealthy)
  - Async health check trait with `async_trait`
  - Health check manager for centralized monitoring
  - Detailed health reports with metadata support
  - Marker traits for specialized checks:
    - `CacheHealthCheck` - Cache-related health checks
    - `DatabaseHealthCheck` - Database-related health checks
  - Component-level health status tracking
  - Production-ready monitoring integration

- **Metrics Collection** (`metrics` module)
  - Performance metrics tracking
  - Request timing and profiling (`RequestTimer`)
  - Request-specific metrics (`RequestMetrics`)
  - Centralized metrics collection (`MetricsCollector`)
  - Generic metric types for custom measurements

- **Middleware** (`StaticFilesMiddleware`)
  - Request/response processing for static files
  - Integration with HTTP pipeline
  - Automatic static file serving in development

- **Dependency Resolution** (`DependencyGraph`)
  - Track dependencies between static assets
  - Resolve asset loading order
  - Support for complex asset dependency chains

#### Implemented in Related Crates

- **collectstatic Command** (implemented in `reinhardt-commands`)
  - ✓ CLI command for collecting static files from all sources
  - ✓ Copy files to STATIC_ROOT with optional processing
  - ✓ Integration with deployment workflows
  - ✓ Progress reporting and verbose output
  - See [reinhardt-commands](../reinhardt-commands/README.md) for details

- **GZip Compression** (implemented in `reinhardt-middleware`)
  - ✓ Response compression for bandwidth optimization
  - ✓ Configurable compression level (0-9)
  - ✓ Minimum size threshold configuration
  - ✓ Content-type filtering (text/\*, application/json, etc.)
  - ✓ Automatic Accept-Encoding detection
  - ✓ Compression only when beneficial (size check)
  - See [reinhardt-middleware](../reinhardt-middleware/README.md) for details

- **Brotli Compression** (implemented in `reinhardt-middleware`)
  - ✓ Advanced compression with better ratios than gzip
  - ✓ Configurable quality levels (Fast, Balanced, Best)
  - ✓ Window size configuration (10-24)
  - ✓ Content-type filtering
  - ✓ Automatic Accept-Encoding: br detection
  - ✓ Intelligent compression (only when beneficial)
  - See [reinhardt-middleware](../reinhardt-middleware/README.md) for details

- **Cache-Control Header Management**
  - ✓ Configurable cache policies per file type
  - ✓ Long-term caching for static assets (CSS, JS, fonts, images)
  - ✓ Short-term caching for HTML files
  - ✓ Flexible cache directives (public, private, no-cache, immutable, etc.)
  - ✓ max-age and s-maxage configuration
  - ✓ Vary header support
  - ✓ Pattern-based cache policies

- **CDN Integration**
  - ✓ Multi-provider support (CloudFront, Fastly, Cloudflare, Custom)
  - ✓ CDN URL generation with path prefixes
  - ✓ Versioned URL generation
  - ✓ HTTPS/HTTP configuration
  - ✓ Custom header support
  - ✓ Cache invalidation request helpers
  - ✓ Wildcard purge support

- **Advanced Storage Backends** (`storage` module)
  - `S3Storage` - S3-compatible storage backend (AWS S3, MinIO, LocalStack)
    - Configurable credentials (access key, secret key)
    - Custom endpoint support for S3-compatible services
    - Path-style addressing configuration
    - Path prefix support within buckets
  - `AzureBlobStorage` - Azure Blob Storage backend
    - Shared key and SAS token authentication
    - Custom endpoint support for Azure emulator
    - Container and path prefix configuration
  - `GcsStorage` - Google Cloud Storage backend
    - Service account credentials (JSON or file path)
    - Custom endpoint support for GCS emulator
    - Project ID and bucket configuration
  - `StorageRegistry` - Custom storage backend registration system
    - Dynamic registration of storage backends
    - Factory pattern for creating storage instances
    - Backend lifecycle management (register, unregister, clear)

- **Pages/SSR Integration** (`template_integration` module)
  - `TemplateStaticConfig` - Configuration for static file generation in SSR contexts
  - Automatic hashed filename resolution via manifest
  - Supports custom static URLs (CDN, etc.)
  - Can be integrated with reinhardt-pages SSR and other rendering systems

- **File Processing Pipeline** (`processing` module)
  - CSS/JavaScript minification (basic whitespace and comment removal)
  - Asset bundling with dependency resolution
  - Processing pipeline manager
  - Configurable optimization levels
  - Feature flag: `processing` (default: disabled)

- **Development Server Features** (`dev_server` module)
  - File system watching with `notify` crate
  - Auto-reload notification system using broadcast channels
  - Development error pages with detailed debugging information
  - WebSocket-based reload notifications (port 35729 by default)
  - Smart reload strategies:
    - CSS files: Reload without full page refresh
    - Other files: Full page reload
  - Multiple path watching support
  - Client connection tracking
  - Feature flag: `dev-server` (default: disabled)

- **Advanced File Processing**
  - Image optimization (PNG, JPEG, WebP) - Feature flag: `image-optimization`
  - Source map generation - Feature flag: `source-maps`
  - Asset compression (gzip, brotli) - Feature flag: `compression`
  - Minification for CSS and JavaScript
  - Asset bundling with dependency resolution

- **Advanced Minification** (OXC-powered)
  - Variable renaming (mangling) - Feature flag: `advanced-minification`
  - Dead code elimination
  - Production-grade compression
  - Console.log removal option
  - Debugger statement removal


## storage

### Features

### Implemented ✓

#### Core Storage Abstraction

- **Storage Trait**: Async trait defining standard storage operations
  - File save, read, delete operations
  - File existence checking and metadata retrieval
  - Directory listing functionality
  - URL generation for file access
  - File timestamp operations (accessed, created, modified times)

#### File and Metadata Handling

- **FileMetadata**: Comprehensive file metadata structure
  - Path, size, content type tracking
  - Creation and modification timestamps
  - Optional checksum support (SHA-256)
  - Builder pattern methods (`with_content_type`, `with_checksum`)
- **StoredFile**: File representation with metadata and content

#### Error Handling

- **StorageError**: Comprehensive error types
  - File not found errors
  - I/O error propagation
  - Invalid path detection
  - Storage full conditions
  - Permission denied errors
  - File already exists errors

#### Local Filesystem Storage

- **LocalStorage**: Production-ready local filesystem backend
  - Automatic directory creation
  - Path traversal attack prevention
  - SHA-256 checksum computation
  - File timestamp retrieval (accessed, created, modified)
  - URL generation with configurable base URL
  - Comprehensive directory listing

#### In-Memory Storage

- **InMemoryStorage**: Testing and development storage backend
  - Thread-safe in-memory file storage using Arc<RwLock>
  - Timestamp tracking (accessed, created, modified)
  - Directory-style path organization
  - Configurable file and directory permission modes
  - Django-style deconstruction for serialization
  - Clone support for easy testing

#### Prelude Module

- Re-exports of commonly used types for convenient importing


## utils-core

### Features

### Implemented ✓

#### HTML Utilities

- **HTML escaping** - Escape/unescape HTML special characters for XSS prevention
- **Tag stripping** - Remove HTML tags from text
- **Space normalization** - Strip spaces between HTML tags
- **Attribute escaping** - Escape values for safe use in HTML attributes
- **Template formatting** - Simple placeholder-based HTML templating
- **Conditional escaping** - Context-aware HTML escaping
- **SafeString** - Mark strings as safe to bypass autoescaping
- **HTML truncation** - Truncate HTML content to specified word count while preserving tags

#### Common Utilities

- Type conversion helpers
- String manipulation utilities
- Collection helpers
- Time and date utilities
- Encoding/decoding utilities
- Core abstraction types
