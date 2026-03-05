# reinhardt-core

Core components for Reinhardt framework

## Overview

`reinhardt-core` provides the fundamental building blocks for the Reinhardt framework. It contains essential types, traits, error handling, signals, security primitives, validators, and backend abstractions that other crates depend on.

This crate serves as the foundation for the entire Reinhardt ecosystem, providing core abstractions and utilities used throughout the framework.

## Features

### Implemented ✓

This crate provides the following modules:

- **Types**: Core type definitions
  - Handler trait for request processing
  - Middleware trait for request/response pipelines
  - MiddlewareChain for composable middleware
  - Type aliases and async trait support

- **Exception**: Exception handling and error types
  - Django-style exception hierarchy
  - HTTP status code exceptions (401, 403, 404, 500, etc.)
  - Validation error handling
  - Database exception types
  - Custom error types (ImproperlyConfigured, ParseError, etc.)

- **Signals**: Event-driven hooks for lifecycle events
  - Type-safe signal system for decoupled communication
  - Lifecycle signals for models, migrations, requests
  - Async and sync signal dispatch patterns
  - Signal composition and middleware
  - Performance monitoring

- **Macros**: Procedural macros for code generation
  - `#[handler]` macro for endpoint definitions
  - `#[middleware]` macro for middleware implementations
  - `#[injectable]` macro for dependency injection

- **Security**: Security primitives and utilities
  - Password hashing and verification
  - CSRF protection
  - XSS prevention
  - Secure random generation
  - Constant-time comparisons

- **Validators**: Data validation utilities
  - Email validation
  - URL validation
  - Length validators
  - Range validators
  - Custom validator support

- **Serializers**: Serialization and deserialization
  - Django REST Framework-inspired field types
  - Validation system with field and object validators
  - Recursive serialization with circular reference detection
  - Arena allocation for high-performance serialization

- **Messages**: Flash messages and user notifications
  - Message levels (Debug, Info, Success, Warning, Error)
  - Storage backends (Memory, Session, Cookie, Fallback)
  - Middleware integration

- **Pagination**: Pagination strategies
  - PageNumberPagination for page-based pagination
  - LimitOffsetPagination for SQL-style pagination
  - CursorPagination for efficient large dataset pagination
  - Database cursor pagination with O(k) performance

- **Parsers**: Request body parsing
  - JSON, XML, YAML, Form, MultiPart parsers
  - File upload handling
  - Content-type negotiation

- **Negotiation**: Content negotiation
  - Media type selection based on Accept headers
  - Language negotiation (Accept-Language)
  - Encoding negotiation (Accept-Encoding)

- **Dependency Injection**: FastAPI-style DI system
  - Automatic dependency resolution
  - Parameter injection
  - Cache control

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
reinhardt-core = "0.1.0-alpha.1"
```

### Optional Features

Enable specific modules based on your needs:

```toml
[dependencies]
reinhardt-core = { version = "0.1.0-alpha.1", features = ["signals", "macros", "security"] }
```

Available features:

- `types` (default): Core type definitions
- `exception` (default): Error handling
- `signals` (default): Event system
- `macros` (default): Procedural macros
- `security` (default): Security primitives
- `validators` (default): Data validation
- `serializers` (default): Serialization utilities
- `http`: HTTP types and traits (requires `types`)
- `messages`: Flash messaging system
- `di`: Dependency injection with parameter extraction
- `negotiation`: Content negotiation
- `parsers`: Request body parsers
- `pagination`: Pagination strategies

## Usage

### Handler and Middleware

```rust
// Import from modules
use reinhardt::core::types::{Handler, Middleware};
use reinhardt::http::{Request, Response};
use reinhardt::core::exception::Result;
use async_trait::async_trait;

// Define a handler
async fn my_handler(req: Request) -> Result<Response> {
    Response::ok().with_body("Hello, world!")
}

// Define middleware
struct LoggingMiddleware;

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn process_request(&self, req: Request) -> Result<Request> {
        println!("Processing request: {:?}", req.uri());
        Ok(req)
    }
}
```

### Error Handling

```rust
use reinhardt::core::exception::{Error, Result};

fn validate_user(authenticated: bool, authorized: bool) -> Result<()> {
    if !authenticated {
        return Err(Error::Authentication("Invalid credentials".into()));
    }
    if !authorized {
        return Err(Error::Authorization("Permission denied".into()));
    }
    Ok(())
}
```

### Signals

```rust
use reinhardt::core::signals::{Signal, SignalDispatcher};
use std::sync::Arc;

#[derive(Debug, Clone)]
struct User {
    name: String,
}

// Connect a receiver to the signal
async fn setup_signal() {
    let signal = Signal::<User>::new();

    signal.connect(|user: Arc<User>| async move {
        println!("User created: {}", user.name);
        Ok(())
    });

    // Send signal
    let user = User { name: "Alice".to_string() };
    signal.send(user).await.unwrap();
}
```

## Module Organization

`` `reinhardt-core` `` is organized into the following modules:

### Core Modules
- `` `types` `` - Core type definitions (Handler, Middleware, type aliases)
- `` `exception` `` - Error handling and exception types
- `` `signals` `` - Event-driven hooks for lifecycle events
- `` `macros` `` - Procedural macros for code generation

### Utility Modules
- `` `security` `` - Security primitives (hashing, CSRF, XSS)
- `` `validators` `` - Data validation utilities
- `` `serializers` `` - Serialization and deserialization
- `` `messages` `` - Flash messages and user notifications
- `` `pagination` `` - Pagination strategies
- `` `parsers` `` - Request body parsing
- `` `negotiation` `` - Content negotiation

### Using Modules

```rust
use reinhardt::core::types::{Handler, Middleware};
use reinhardt::core::exception::Result;
use reinhardt::core::signals::Signal;
```

**Note**: `` `reinhardt-di` `` and `` `reinhardt-http` `` are separate workspace-level crates that provide dependency injection and HTTP utilities. They can be used independently or alongside `` `reinhardt-core` ``.


## exception

### Features

### Implemented ✓

- **Django-style exception hierarchy** - Comprehensive `Error` enum with categorized error types
- **HTTP status code exceptions** - `Http`, `Authentication` (401), `Authorization` (403), `NotFound` (404), `Internal` (500), etc.
- **Validation error handling** - `Validation` variant with field-level error support
- **Database exception types** - `Database` variant for DB-related errors
- **Custom error types** - `ImproperlyConfigured`, `BodyAlreadyConsumed`, `ParseError`, etc.
- **Error serialization** - All errors implement `Display` and can be converted to HTTP responses via `status_code()` method
- **thiserror integration** - Full integration with `thiserror` for derived error impl
- **anyhow integration** - `Other` variant wraps any `anyhow::Error` for compatibility
- **Error categorization** - `ErrorKind` enum for categorical classification
- **Standard conversions** - `From` implementations for `serde_json::Error`, `std::io::Error`, `http::Error`, `String`, `&str`, `validator::ValidationErrors`
- **Parameter validation context** - `ParamErrorContext` struct with detailed parameter extraction error information
- **Parameter type enumeration** - `ParamType` enum (`Json`, `Query`, `Path`, `Form`, `Header`, `Cookie`, `Body`)
- **Additional error types** - `TemplateNotFound` (404), `MissingContentType` (400), `MethodNotAllowed` (405), `Conflict` (409)
- **Pagination error types** - `InvalidPage`, `InvalidCursor`, `InvalidLimit` variants for pagination validation
- **URL parameter errors** - `MissingParameter` variant for URL reverse operations
- **Helper utilities** - `extract_field_from_serde_error` and `extract_field_from_urlencoded_error` functions
- **Error kind classification** - `kind()` method returns `ErrorKind` for categorical error analysis


## messages

### Features

### Implemented ✓

#### Core Message System

- **Message Levels**: 5 predefined levels (Debug, Info, Success, Warning, Error) with numeric priority values (10, 20, 25, 30, 40)
- **Custom Levels**: Support for user-defined message levels with custom numeric values
- **Message Tags**: Level-based tags and extra custom tags for styling and filtering
- **Message Creation**: Convenience methods for creating messages (`Message::debug()`, `Message::info()`, etc.)
- **Message Configuration**: `MessageConfig` for customizing level tags globally

#### Storage Backends

- **MemoryStorage**: In-memory storage using thread-safe `Arc<Mutex<VecDeque>>` for testing and temporary messages
- **SessionStorage**: Session-based persistent storage with JSON serialization
  - Customizable session key (default: `"_messages"`)
  - Session availability validation
  - Serialization/deserialization for session integration
- **CookieStorage**: Cookie-based storage with automatic size management
  - Configurable cookie name and size limit (default: 4KB)
  - Automatic message truncation using binary search when exceeding size limits
  - Drops oldest messages first when size limit is exceeded
- **FallbackStorage**: Intelligent fallback between Cookie and Session storage
  - Attempts cookie storage first for better performance
  - Automatically falls back to session storage when cookie size is exceeded
  - Tracks which storage backend(s) were used
  - Supports flushing messages from both backends

#### Utilities

- **Binary Search Algorithms**: Efficient size-limited message management
  - `bisect_keep_left()`: Keep maximum messages from the beginning within size limit
  - `bisect_keep_right()`: Keep maximum messages from the end within size limit
- **SafeData**: HTML-safe string wrapper for rendering pre-sanitized HTML content
  - Prevents double-escaping of HTML in messages
  - Serializable with serde support

#### Storage Trait

- **MessageStorage Trait**: Unified interface for all storage backends
  - `add()`: Add a message to storage
  - `get_all()`: Retrieve and clear all messages
  - `peek()`: View messages without clearing
  - `clear()`: Remove all messages

#### Middleware Integration

- **MessagesMiddleware**: Request/response middleware for automatic message handling
  - Automatic message retrieval and storage during request lifecycle
  - Thread-safe message container with Arc-based sharing
- **MessagesContainer**: Container for messages during request processing
  - `add()`: Add messages during request
  - `get_messages()`: Retrieve all messages
  - `add_from_storage()`: Load messages from storage backend

#### Context Processor

- **MessagesContext**: Context for template rendering integration
  - `get_messages()`: Retrieve messages for rendering
  - `add_message()`: Add messages to context
- **get_messages_context()**: Helper function to create messages context
- **add_message()**: Convenience function to add messages to context

#### Message Filtering

- **filter_by_level()**: Filter messages by exact level match
- **filter_by_min_level()**: Filter messages above or equal to minimum level
- **filter_by_max_level()**: Filter messages below or equal to maximum level
- **filter_by_level_range()**: Filter messages within a level range (inclusive)
- **filter_by_tag()**: Filter messages by tag match


## security

### Features

### Implemented ✓

#### CSRF Protection

- **Token Generation & Validation**:
  - `get_secret_bytes()`: Generate cryptographically secure 32-byte secret for HMAC
  - `generate_token_hmac()`: Generate HMAC-SHA256 token from secret and message
  - `get_token_hmac()`: High-level token generation using secret and session ID
  - `verify_token_hmac()`: Constant-time HMAC verification
  - `check_token_hmac()`: Token validation with detailed error reporting
- **Token Rotation Support**:
  - `generate_token_with_timestamp()`: Generate token with timestamp for rotation tracking
  - `verify_token_with_timestamp()`: Verify timestamped token and extract timestamp
  - `get_token_timestamp()`: Get current Unix timestamp for rotation logic
  - `should_rotate_token()`: Determine if token rotation is due based on interval
  - Configurable via `CsrfConfig::with_token_rotation(interval)`
- **Origin/Referer Checking**: `check_origin()` and `check_referer()` validate request sources
- **Domain Validation**: `is_same_domain()` for cross-domain request protection
- **Configurable Cookie Settings**: Full control over SameSite, Secure, HttpOnly, Domain, Path, and Max-Age
- **Production-Ready Config**: `CsrfConfig::production()` with security hardening (includes token rotation)
- **Middleware**: `CsrfMiddleware` with customizable configuration
- **Error Handling**: Detailed rejection reasons for debugging (bad origin, bad referer, missing token, etc.)
- **Constants**: `CSRF_TOKEN_LENGTH`, `CSRF_SECRET_LENGTH`, `CSRF_SESSION_KEY`, rejection reason constants

#### XSS Prevention

- **HTML Escaping**:
  - `escape_html()`: Escapes dangerous characters (`<`, `>`, `&`, `"`, `'`)
  - `escape_html_attr()`: Escapes HTML attributes including newlines and control characters
- **JavaScript Context Escaping**: `escape_javascript()` for safe embedding in JavaScript strings
- **URL Encoding**: `escape_url()` for URL encoding to prevent injection
- **HTML Sanitization**: `sanitize_html()` for basic HTML input sanitization
- **XSS Pattern Detection**: `detect_xss_patterns()` detects dangerous patterns (script tags, event handlers, etc.)
- **URL Validation**: `is_safe_url()` validates URLs and allows only safe protocols (http, https, mailto, ftp)
- **Safe Output**: Prevents script injection in user-generated content across multiple contexts

#### Security Headers

- **Content Security Policy (CSP)**: Configurable CSP with granular control over:
  - `default-src`, `script-src`, `style-src`, `img-src`
  - `connect-src`, `font-src`, `object-src`, `media-src`, `frame-src`
  - **CSP Reporting**: `report-uri` and `report-to` for violation reporting via `with_report_uri()` and `with_report_to()`
  - **Nonce Generation**: `generate_nonce()` for inline script/style nonces
  - **Auto Nonce**: Automatic nonce injection with `with_auto_nonce()`
- **Security Headers Middleware**: `SecurityHeadersMiddleware` with comprehensive defaults
- **Configurable Headers**:
  - `X-Content-Type-Options: nosniff`
  - `X-Frame-Options: DENY` (clickjacking protection)
  - `X-XSS-Protection: 1; mode=block`
  - `Strict-Transport-Security` (HSTS)
  - `Referrer-Policy: strict-origin-when-cross-origin`
  - `Permissions-Policy` (optional)
  - **Cross-Origin Policies**:
    - `Cross-Origin-Embedder-Policy: require-corp`
    - `Cross-Origin-Opener-Policy: same-origin`
    - `Cross-Origin-Resource-Policy: same-origin`
- **Environment Presets**:
  - `SecurityHeadersConfig::production()`: Strict security headers for production
  - `SecurityHeadersConfig::development()`: Relaxed headers for development (no HSTS, no CSP)

#### HSTS (HTTP Strict Transport Security)

- **HSTS Configuration**: `HstsConfig` with builder pattern
- **Configurable Options**:
  - `max_age`: Configurable duration in seconds
  - `includeSubDomains`: Optional subdomain protection
  - `preload`: HSTS preload list support
- **Header Generation**: `build_header()` for automatic header value construction
- **Secure Defaults**: 1-year max-age default configuration

#### Security Utilities

The `utils` module provides internal security utilities:

- **Secure Token Generation**: `generate_token()` creates cryptographically random tokens (internal use)
- **SHA-256 Hashing**: `hash_sha256()` for secure string hashing (internal use)
- **Random Number Generation**: Built on `rand` crate for security

**Note**: These utilities are available through the `utils` module but are not re-exported at the crate root. They are primarily used internally by CSRF and other security features.

#### Error Handling

- **Comprehensive Error Types**: `SecurityError` enum with specific variants
- **CSRF Validation Errors**: Detailed error messages for debugging
- **XSS Detection**: Error type for potential XSS attempts
- **Configuration Errors**: Validation for security configurations

#### IP Filtering

- **Whitelist/Blacklist Modes**: `IpFilterMode` enum for configurable filtering strategy
  - `Whitelist`: Only allow IPs in the allowed list
  - `Blacklist`: Deny IPs in the blocked list (default)
- **IP Range Support**: Add individual IPs or CIDR ranges (e.g., `192.168.1.0/24`)
- **IPv4 and IPv6**: Full support for both IP versions
- **Flexible Configuration**: `IpFilterConfig` with builder-style methods
  - `new(mode)`: Create with specified mode
  - `whitelist()`: Create with whitelist mode
  - `blacklist()`: Create with blacklist mode
  - `add_allowed_ip(ip_or_range)`: Add IP addresses or ranges to whitelist
  - `add_blocked_ip(ip_or_range)`: Add IP addresses or ranges to blacklist
  - `is_allowed(&ip)`: Check if an IP address is permitted
- **Blacklist Override**: Blocked IPs take precedence over allowed IPs
- **Middleware**: `IpFilterMiddleware` for request filtering based on IP address


## types

### Features

### Implemented ✓

- **Handler trait** - Core abstraction for async request processing
  - `async fn handle(&self, request: Request) -> Result<Response>`
  - Blanket implementation for `Arc<T>` to enable `Arc<dyn Handler>`
- **Middleware trait** - Request/response pipeline processing
  - `async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response>`
  - `fn should_continue(&self, request: &Request) -> bool` - Conditional execution
- **MiddlewareChain** - Composable middleware system with automatic chaining
  - Builder pattern: `with_middleware()` for method chaining
  - Mutable API: `add_middleware()` for imperative style
  - Performance optimizations:
    - O(k) complexity where k ≤ n (skips unnecessary middleware)
    - Short-circuiting with `Response::with_stop_chain(true)`
- **Type aliases** - Re-export of `Request` and `Response` from `reinhardt-http`
- **Async trait support** - Full async/await support via `async_trait`
- **Zero-cost abstractions** - All traits compile to efficient code with no runtime overhead


## validators

### Features

### Implemented ✓

#### Core Validation Framework

- **Validator Trait**: Generic validation interface `Validator<T>` for implementing custom validators
- **OrmValidator Trait**: Extension trait for ORM validators with custom error messages
- **SettingsValidator Trait**: Extension trait for validating configuration settings
- **ValidationError**: Comprehensive error types with descriptive messages
- **ValidationResult<T>**: Type-safe result type for validation operations
- **Prelude Module**: Convenient re-exports of all validators and error types

#### String Validators

- **MinLengthValidator**: Validates minimum string length
  - Works with both `String` and `&str` types
  - Provides detailed error messages with actual and expected lengths
  - Unicode-aware length checking
- **MaxLengthValidator**: Validates maximum string length
  - Works with both `String` and `&str` types
  - Provides detailed error messages with actual and expected lengths
  - Unicode-aware length checking
- **RegexValidator**: Pattern matching with regular expressions
  - Custom error message support via `with_message()`
  - Full regex syntax support
  - Works with both `String` and `&str` types

#### Numeric Validators

- **MinValueValidator**: Validates minimum numeric values
  - Generic over any `PartialOrd + Display + Clone` type
  - Supports integers (i8, i16, i32, i64, isize, u8, u16, u32, u64, usize)
  - Supports floating-point numbers (f32, f64)
  - Provides detailed error messages with actual and expected values
- **MaxValueValidator**: Validates maximum numeric values
  - Generic over any `PartialOrd + Display + Clone` type
  - Supports all integer and floating-point types
  - Provides detailed error messages with actual and expected values
- **RangeValidator**: Validates values within a range (inclusive)
  - Generic over any `PartialOrd + Display + Clone` type
  - Supports all numeric types
  - Reports whether value is too small or too large

#### Email Validator

- **EmailValidator**: RFC 5322 compliant email validation
  - Case-insensitive validation
  - Local part validation (max 64 characters)
    - Allows alphanumeric characters, dots, underscores, percent signs, plus and minus signs
    - Prevents consecutive dots
    - Prevents leading/trailing dots
  - Domain part validation (max 255 characters)
    - Supports subdomains
    - Each label max 63 characters
    - TLD minimum 2 characters
    - Prevents leading/trailing hyphens in domain labels
  - Total length limit (max 320 characters)
  - Works with both `String` and `&str` types

#### URL Validator

- **UrlValidator**: HTTP/HTTPS URL validation
  - Scheme validation (http, https)
  - Port number support (1-5 digits)
  - Path validation
  - Query string support
  - Fragment identifier support
  - Subdomain support
  - Hyphen support in domain names (not at start/end of labels)
  - Works with both `String` and `&str` types

#### Error Types

- `InvalidEmail(String)`: Invalid email address format
- `InvalidUrl(String)`: Invalid URL format
- `TooSmall { value: String, min: String }`: Value below minimum
- `TooLarge { value: String, max: String }`: Value above maximum
- `TooShort { length: usize, min: usize }`: String shorter than minimum
- `TooLong { length: usize, max: usize }`: String longer than maximum
- `PatternMismatch(String)`: Regex pattern did not match
- `Custom(String)`: Custom validation error

#### Additional Validators (Implemented ✓)

- **SlugValidator**: Validate URL-safe slugs
- **UUIDValidator**: Validate UUID formats (v1-v5)
- **IPAddressValidator**: Validate IPv4/IPv6 addresses
- **DateValidator**: Validate date formats
- **TimeValidator**: Validate time formats
- **DateTimeValidator**: Validate datetime formats
- **JSONValidator**: Validate JSON structure
- **ColorValidator**: Validate color codes (hex, rgb, rgba, etc.)
- **PhoneNumberValidator**: Validate phone numbers (E.164 format)
- **CreditCardValidator**: Validate credit card numbers (Luhn algorithm)
- **IBANValidator**: Validate international bank account numbers
- **CustomRegexValidator**: User-defined regex pattern validation

#### File Validators (Implemented ✓)

- **FileTypeValidator**: Comprehensive file type validation
  - **Extension validation**: `FileTypeValidator::with_extensions()`
    - Case-insensitive extension matching
    - Multiple extensions support
    - Whitelist-based filtering
  - **MIME type validation**: `FileTypeValidator::with_mime_types()`
    - Validates file MIME types
    - Multiple MIME types support
  - **Preset validators**:
    - `FileTypeValidator::images_only()`: Supports JPEG, PNG, GIF, WebP, SVG, BMP, TIFF, ICO, AVIF
    - `FileTypeValidator::documents_only()`: Supports PDF, DOC, DOCX, XLS, XLSX, PPT, PPTX, TXT

**Example**:
```rust
use reinhardt_validators::{FileTypeValidator, Validator};

// Extension validation
let validator = FileTypeValidator::with_extensions(vec![
    "jpg".to_string(),
    "png".to_string(),
]);
assert!(validator.validate_filename("photo.jpg").is_ok());
assert!(validator.validate_filename("photo.JPG").is_ok()); // Case-insensitive
assert!(validator.validate_filename("document.pdf").is_err());

// Preset validator
let image_validator = FileTypeValidator::images_only();
assert!(image_validator.validate_filename("photo.png").is_ok());
```

#### Async Validators (Implemented ✓)

- **ExistsValidator**: Asynchronous foreign key existence validation
  - Custom async check function support
  - Database table reference validation
  - Validates that referenced records exist in the database

- **UniqueValidator**: Asynchronous uniqueness constraint validation
  - Prevent duplicate entries
  - Instance exclusion during updates with `exclude_id` parameter
  - Custom async uniqueness check function

**Example**:
```rust
use reinhardt_validators::{ExistsValidator, UniqueValidator, Validator};

// Foreign key existence check
let exists_validator = ExistsValidator::new(
    "user_id",
    "users",
    Box::new(|value| Box::pin(async move {
        // Database check logic here
        // Return true if record exists, false otherwise
        true
    }))
);

// Async validation
let result = exists_validator.validate_async("123").await;
assert!(result.is_ok());

// Uniqueness check with instance exclusion
let unique_validator = UniqueValidator::new(
    "email",
    Box::new(|value, exclude_id| Box::pin(async move {
        // Check if email is unique, excluding the given ID
        true
    }))
);

// Validate new record (no exclusion)
let result = unique_validator.validate_async("new@example.com").await;
assert!(result.is_ok());
```

#### Database Identifier Validators (Implemented ✓)

- **TableName**: Compile-time and runtime validated table names
  - SQL reserved word checking via `is_sql_reserved_word()`
  - Snake_case format validation
  - Length validation (max 63 characters for PostgreSQL compatibility)
  - Compile-time validation with `new_const()` constant function
  - Runtime validation with `new()` method

- **FieldName**: SQL-safe field/column name validation
  - Same validation rules as TableName
  - Prevents SQL injection through identifier validation

- **ConstraintName**: SQL-safe constraint name validation
  - Validates constraint identifiers for CREATE/ALTER statements

**Example**:
```rust
use reinhardt_validators::{TableName, FieldName};

// Runtime validation
let table = TableName::new("users")?;
assert!(TableName::new("select").is_err()); // SQL reserved word
assert!(TableName::new("User-Table").is_err()); // Not snake_case

// Compile-time validation
const VALID_TABLE: TableName = TableName::new_const("users");
const VALID_FIELD: FieldName = FieldName::new_const("email_address");
```

#### Custom Error Messages (Partial Implementation ✓)

Currently supported by:
- **RegexValidator**: `.with_message("Custom message")`
- **CustomRegexValidator**: Built-in custom message support

**Planned Extension**: Extend to all validators (see lib.rs for planned features)

**Example**:
```rust
use reinhardt_validators::{RegexValidator, Validator};

let validator = RegexValidator::new(r"^\d{3}-\d{4}$")
    .unwrap()
    .with_message("Phone number must be in format XXX-XXXX");

match validator.validate("invalid") {
    Err(e) => assert_eq!(e.to_string(), "Phone number must be in format XXX-XXXX"),
    Ok(_) => panic!("Expected validation error"),
}
```

#### File Size Validator (Implemented ✓)

- **FileSizeValidator**: Validate file sizes with minimum, maximum, or range constraints
  - **Min size validation**: `FileSizeValidator::min(bytes)`
  - **Max size validation**: `FileSizeValidator::max(bytes)`
  - **Range validation**: `FileSizeValidator::range(min_bytes, max_bytes)`
  - **Helper methods for unit conversion**:
    - `FileSizeValidator::from_kb(kb)`: Convert KB to bytes
    - `FileSizeValidator::from_mb(mb)`: Convert MB to bytes
    - `FileSizeValidator::from_gb(gb)`: Convert GB to bytes
  - Integrates well with `FileTypeValidator` for comprehensive file validation
  - Generic over `u64` type for file size values

**Example**:
```rust
use reinhardt_validators::{FileSizeValidator, Validator};

// Validate minimum file size
let min_validator = FileSizeValidator::min(FileSizeValidator::from_kb(100)); // 100 KB minimum
assert!(min_validator.validate(&(150 * 1024)).is_ok()); // 150 KB passes
assert!(min_validator.validate(&(50 * 1024)).is_err()); // 50 KB fails

// Validate maximum file size
let max_validator = FileSizeValidator::max(FileSizeValidator::from_mb(5)); // 5 MB maximum
assert!(max_validator.validate(&(3 * 1024 * 1024)).is_ok()); // 3 MB passes
assert!(max_validator.validate(&(10 * 1024 * 1024)).is_err()); // 10 MB fails

// Validate file size range
let range_validator = FileSizeValidator::range(
    FileSizeValidator::from_kb(100),  // 100 KB minimum
    FileSizeValidator::from_mb(10),   // 10 MB maximum
);
assert!(range_validator.validate(&(5 * 1024 * 1024)).is_ok()); // 5 MB passes
```

#### Validator Composition (Implemented ✓)

- **AndValidator**: Combine multiple validators with AND logic
  - All contained validators must pass for validation to succeed
  - Short-circuits on first failure for better performance
  - Supports nested composition (AND within OR, etc.)
  - Generic over any type `T` that validators can validate

- **OrValidator**: Combine multiple validators with OR logic
  - At least one contained validator must pass
  - Optional error collection from all validators when all fail
  - Supports nested composition (OR within AND, etc.)
  - Generic over any type `T` that validators can validate

**Example**:
```rust
use reinhardt_validators::{AndValidator, OrValidator, MinLengthValidator, MaxLengthValidator, EmailValidator, UrlValidator, Validator};

// AND composition - Username must be 3-20 characters
let username_validator = AndValidator::new(vec![
    Box::new(MinLengthValidator::new(3)),
    Box::new(MaxLengthValidator::new(20)),
]);
assert!(username_validator.validate("john").is_ok());
assert!(username_validator.validate("jo").is_err()); // Too short
assert!(username_validator.validate("verylongusernamethatexceedslimit").is_err()); // Too long

// OR composition - Contact must be either email OR URL
let contact_validator = OrValidator::new(vec![
    Box::new(EmailValidator::new()),
    Box::new(UrlValidator::new()),
]);
assert!(contact_validator.validate("user@example.com").is_ok()); // Valid email
assert!(contact_validator.validate("http://example.com").is_ok()); // Valid URL
assert!(contact_validator.validate("invalid").is_err()); // Neither email nor URL

// Nested composition - Complex validation logic
let complex_validator = OrValidator::new(vec![
    Box::new(AndValidator::new(vec![
        Box::new(MinLengthValidator::new(3)),
        Box::new(MaxLengthValidator::new(10)),
    ])),
    Box::new(MinLengthValidator::new(20)), // Or very long string
]);
assert!(complex_validator.validate("hello").is_ok()); // Passes first (3-10 chars)
assert!(complex_validator.validate("verylongusernameexceeds20chars").is_ok()); // Passes second (20+ chars)
assert!(complex_validator.validate("hi").is_err()); // Fails both

// Error collection with OrValidator
let collecting_validator = OrValidator::new(vec![
    Box::new(MinLengthValidator::new(10)),
    Box::new(MinLengthValidator::new(20)),
])
.with_error_collection(true);

match collecting_validator.validate("short") {
    Err(e) => println!("All validators failed: {}", e), // Shows all error messages
    Ok(_) => unreachable!(),
}
```

#### Postal Code Validator (Implemented ✓)

- **PostalCodeValidator**: Country-specific postal code format validation
  - **Supported countries**: US, UK, JP, CA, DE (5 countries)
  - **Country restriction**: `with_countries(vec![Country::US, Country::JP])`
  - **Single country**: `for_country(Country::US)`
  - **Country detection**: `validate_with_country()` returns detected country
  - **Case-insensitive validation**: Automatically handles uppercase/lowercase
  - **Whitespace trimming**: Handles leading/trailing spaces
  - **Priority-based pattern matching**: Resolves ambiguous formats correctly

**Supported Formats**:
- **US**: ZIP (12345) and ZIP+4 (12345-6789) formats
- **UK**: Complex alphanumeric format (SW1A 1AA, M1 1AE, etc.)
- **JP**: 7-digit with hyphen (123-4567)
- **CA**: Alphanumeric format (K1A 0B1, M5W 1E6)
- **DE**: 5-digit format (10115, 80331)

**Example**:
```rust
use reinhardt_validators::{PostalCodeValidator, Country, Validator};

// Validate with country restriction
let validator = PostalCodeValidator::with_countries(vec![
    Country::US,
    Country::JP,
]);
assert!(validator.validate("12345").is_ok()); // US ZIP
assert!(validator.validate("12345-6789").is_ok()); // US ZIP+4
assert!(validator.validate("123-4567").is_ok()); // Japan
assert!(validator.validate("SW1A 1AA").is_err()); // UK not allowed

// Single country validation
let us_validator = PostalCodeValidator::for_country(Country::US);
assert!(us_validator.validate("90210").is_ok());
assert!(us_validator.validate("123-4567").is_err()); // Not US format

// Country detection
let detector = PostalCodeValidator::new(); // Accepts all countries
let country = detector.validate_with_country("12345-6789").unwrap();
assert_eq!(country, Country::US);

let country = detector.validate_with_country("SW1A 1AA").unwrap();
assert_eq!(country, Country::UK);

// Case-insensitive and whitespace handling
assert!(detector.validate("  sw1a 1aa  ").is_ok()); // UK lowercase with spaces
assert!(detector.validate("k1a 0b1").is_ok()); // Canada lowercase
```

#### Image Dimension Validator (Implemented ✓)

- **ImageDimensionValidator**: Validate image width/height dimensions
  - **Min/max width constraints**: `min_width()`, `max_width()`
  - **Min/max height constraints**: `min_height()`, `max_height()`
  - **Aspect ratio validation**: `aspect_ratio()` with configurable tolerance
  - **File validation**: `validate_file()` for file paths
  - **Bytes validation**: `validate_bytes()` for in-memory images
  - **Supported formats**: JPEG, PNG, GIF, WebP, BMP, TIFF, ICO, and more via `image` crate

**Example**:
```rust
use reinhardt::validators::ImageDimensionValidator;

// Basic dimension constraints
let validator = ImageDimensionValidator::new()
	.with_min_width(100)
	.with_max_width(1920)
	.with_min_height(100)
	.with_max_height(1080);

// With aspect ratio validation (16:9 with 1% tolerance)
let hd_validator = ImageDimensionValidator::new()
	.with_min_width(1280)
	.with_min_height(720)
	.with_aspect_ratio(16, 9)
	.with_aspect_ratio_tolerance(0.01);

// Validate from file path
let result = validator.validate_file("image.jpg");

// Validate from bytes (in a function context)
# fn example() -> Result<(), Box<dyn std::error::Error>> {
let image_bytes: Vec<u8> = std::fs::read("image.png")?;
let result = validator.validate_bytes(&image_bytes);
# Ok(())
# }
```

#### Conditional Validation (Implemented ✓)

- **ConditionalValidator**: Apply validators based on runtime conditions
  - **`when` condition**: Apply validator only when condition is true
  - **`unless` condition**: Apply validator only when condition is false
  - **Closure-based conditions**: Use custom logic for condition evaluation
  - **Chainable API**: Combine with other validators

**Example**:
```rust
use reinhardt::validators::{ConditionalValidator, MinLengthValidator, Validator};

// Apply validation only when condition is true
// Condition receives &T parameter, validator is boxed
let validator = ConditionalValidator::when(
	|value: &str| value.starts_with("admin_"), // Fn(&T) -> bool
	Box::new(MinLengthValidator::new(10)),      // Box<dyn Validator<T>>
);

// Validate admin username (must be at least 10 chars)
assert!(validator.validate("admin_john").is_ok());
assert!(validator.validate("admin_j").is_err()); // Too short

// Regular username (no validation applied)
assert!(validator.validate("john").is_ok());

// Apply validation unless condition is true
let validator = ConditionalValidator::unless(
	|value: &str| value.is_empty(),
	Box::new(MinLengthValidator::new(5)),
);
```

## License

Licensed under the BSD 3-Clause License.
