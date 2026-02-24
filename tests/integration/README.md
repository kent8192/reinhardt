# Reinhardt - Integration Tests

> **Important**: This is the official location for all new integration tests. The legacy `/tests/integration/` directory is being phased out and should not be used for new tests.

This directory contains comprehensive integration tests for multiple Reinhardt crates that require interaction between different components.

## Directory Structure

- `/tests/tests/integration/` - **Official integration test directory** (this directory)
- `/tests/integration/` - Legacy directory (deprecated, being phased out)

## Overview

These integration tests verify that multiple Reinhardt crates work correctly when integrated with:

- **Tower**: Middleware and service layer
- **Hyper**: HTTP implementation for routing and handlers
- **SQLx**: Database operations
- **PostgreSQL**: Database backend

## Features

### Implemented ✓

#### Authentication & Security (67 tests in 9 files)

- **HTTP Authentication**: Basic Auth and Bearer token authentication mechanisms
  - Basic Auth credential encoding and parsing
  - Bearer token validation
  - WWW-Authenticate header handling
- **CSRF Protection**: Cross-Site Request Forgery prevention
  - Token generation and validation
  - Cookie and session-based token storage
  - Form and header token extraction
  - Middleware integration
- **Session Security**: Secure session management
  - Session creation and validation
  - Session expiration handling
  - Secure cookie attributes
- **Content Security Policy (CSP)**: Browser security headers
  - CSP directive configuration
  - Nonce generation for inline scripts/styles
  - Report-only mode support
- **HTTPS Redirect**: Automatic HTTP to HTTPS redirection
  - Middleware-based redirection
  - Configurable port handling
  - X-Forwarded-Proto support
- **Security Middleware**: Comprehensive security headers
  - X-Frame-Options
  - X-Content-Type-Options
  - Strict-Transport-Security
  - Referrer-Policy

#### Database & ORM (40 tests in 14 files)

- **Basic Database Operations**: Core CRUD functionality
  - Connection pool management
  - Query execution
  - Transaction handling
  - Parameterized queries
- **Advanced Database Features**: Complex database scenarios
  - Nested transactions
  - Prepared statements
  - Batch operations
  - Connection pooling under load
- **Database Utilities**: Helper functions and utilities
  - Schema introspection
  - Migration helpers
  - Connection testing
  - Database-specific optimizations
- **ORM Integration**: Object-Relational Mapping with serializers, forms, logging, pagination, storage, and validators
  - Model CRUD operations
  - Relationship handling (one-to-many, many-to-many)
  - Query building and filtering
  - Integration with other framework components

#### Serializers (152 tests in 13 files)

- **Basic Serialization**: Core serialization functionality
  - Field validation
  - Type conversion
  - Nested serializers
  - Custom field types
- **Model Serializers**: ORM model serialization
  - Automatic field detection from models
  - CRUD operation integration
  - Relationship serialization
  - Meta options and field exclusion
- **Model Serializer CRUD**: Full CRUD workflow
  - Create operations with validation
  - Read/retrieve operations
  - Update operations (full and partial)
  - Delete operations
- **Model Serializer Relations**: Relationship handling
  - Foreign key serialization
  - Many-to-many relationships
  - Nested object representation
  - Reverse relationships
- **Model Serializer Meta**: Metadata configuration
  - Field selection and exclusion
  - Read-only and write-only fields
  - Custom field names
  - Depth control for nested serialization
- **Database Integration**: Serializer-database interaction
  - Database-backed validation
  - Unique constraint checking
  - Transaction-safe operations
- **Forms Integration**: Serializer-forms interaction
  - Form field mapping
  - Validation error propagation
  - Form-to-serializer conversion
- **REST Integration**: REST API serialization
  - JSON rendering
  - XML rendering
  - Content negotiation
  - Pagination support
- **Validator Integration**: Custom validator support
  - Field-level validators
  - Object-level validators
  - Async validators
  - Cross-field validation

#### Routing & Views (66 tests in 8 files)

- **Router Authentication**: Auth-aware routing
  - Protected route middleware
  - Permission-based routing
  - Login required decorators
  - Role-based access control
- **Router Lifecycle**: Request lifecycle management
  - Before-request hooks
  - After-request hooks
  - Teardown handlers
  - Error handling in lifecycle
- **Router Schema**: OpenAPI schema generation from routes
  - Automatic schema extraction
  - Route parameter documentation
  - Response schema generation
  - Tag and description support
- **Router Pages**: Pages-based routing (reinhardt-pages)
  - Component view integration
  - Props passing to components
  - Component selection logic
  - Component error handling
- **Router ViewSet**: ViewSet-based routing
  - Automatic CRUD route generation
  - Custom action registration
  - Route naming conventions
  - Nested route support
- **Router WebSocket**: WebSocket endpoint routing
  - WebSocket upgrade handling
  - Path parameter extraction
  - WebSocket-specific middleware
- **Views API Integration**: RESTful API views
  - Generic views for CRUD
  - List and detail views
  - Create, update, delete views
  - Mixin composition
- **Views Integration**: General view functionality
  - Class-based views
  - Function-based views
  - View decorators
  - Response rendering

#### Pages & Rendering (90 tests in 12 files)

- **Pages Rendering**: Core component rendering functionality (reinhardt-pages)
  - Component loading and compilation
  - Props rendering
  - Component composition
  - Nested components
  - Custom hooks and utilities
- **Advanced Rendering**: Complex rendering scenarios
  - Streaming responses
  - Chunked rendering
  - Async component rendering
  - Component caching
- **Specialized Rendering**: Format-specific renderers
  - JSON renderer
  - XML renderer
  - CSV renderer
  - Custom content types
- **Pages-Renderer Integration**: Pages system with renderer (reinhardt-pages)
  - Content negotiation
  - Format selection
  - Component selection by media type
  - Error component rendering
- **HTTP Renderer Integration**: HTTP-specific rendering
  - Status code handling
  - Header management
  - Cookie handling
  - Redirect responses
- **Pages-i18n Integration**: Internationalization in components (reinhardt-pages)
  - Translation utilities
  - Locale-aware formatting
  - Plural form handling
  - Language switching
- **Pages-Pagination Integration**: Paginated component rendering (reinhardt-pages)
  - Page number display
  - Navigation controls
  - Page size selection
  - Result count display
- **Pages-Static Integration**: Static file serving in components (reinhardt-pages)
  - Static URL generation
  - Asset versioning
  - CDN integration
  - Static file compression
- **Logging-Pages Integration**: Component rendering logging (reinhardt-pages)
  - Component access logging
  - Rendering performance tracking
  - Error logging
  - Debug information

#### API & REST (98 tests in 10 files)

- **OpenAPI Schema Generation**: Automatic API documentation
  - Schema generation from types
  - Path operation documentation
  - Request/response schema
  - Security scheme documentation
  - Example generation
- **Browsable API**: Interactive API explorer
  - HTML form generation
  - API navigation
  - Request/response display
  - Authentication UI
- **Pagination**: Multiple pagination strategies
  - Page number pagination
  - Limit-offset pagination
  - Cursor pagination
  - Custom pagination classes
  - Integration with ORM queries
  - Integration with REST endpoints
- **Parameter Validation**: Type-safe parameter extraction
  - Path parameters
  - Query parameters
  - Header parameters
  - Cookie parameters
  - Request body validation
  - OpenAPI integration for parameters
- **Parsers**: Request body parsing
  - JSON parser
  - Form data parser
  - Multipart parser
  - XML parser
  - Advanced parsing scenarios (nested data, file uploads)
- **Metadata**: API metadata and introspection
  - Endpoint metadata
  - Schema metadata
  - Version information
  - Custom metadata fields
- **Query Encoding**: URL query string handling
  - Query parameter encoding
  - Query parameter decoding
  - Array parameter handling
  - Nested object encoding

#### Server & HTTP (72 tests in 9 files)

- **Basic Server**: Core HTTP server functionality
  - HTTP/1.1 support
  - Request handling
  - Response generation
  - Keep-alive connections
- **Advanced Server**: Advanced server features
  - HTTP/2 support
  - Server push
  - Graceful shutdown
  - Resource cleanup
- **Async Server**: Asynchronous request handling
  - Concurrent request processing
  - Async handler support
  - Non-blocking I/O
  - Background task execution
- **Protocol Tests**: HTTP protocol compliance
  - Method handling (GET, POST, PUT, DELETE, PATCH, etc.)
  - Header parsing
  - Status code handling
  - Protocol version negotiation
- **Middleware Integration**: Server middleware stack
  - Middleware composition
  - Request/response transformation
  - Error handling middleware
  - Logging middleware
- **End-to-End HTTP**: Full request-response cycles
  - Complete workflow testing
  - Real HTTP client integration
  - Cookie handling
  - Session management

#### Framework Features (161 tests in 33 files)

- **Flatpages**: CMS-like static page management
  - Page serving through views
  - Fallback middleware
  - 404 handling
  - Special character support in URLs
  - Nested URL paths
  - Authentication requirements
- **Messages Framework**: User messaging system
  - Message API
  - Multiple storage backends (session, cookie, fallback)
  - Message levels (info, success, warning, error)
  - Middleware integration
  - View helpers
  - Pages integration (reinhardt-pages)
- **Mail**: Email sending functionality
  - SMTP backend
  - Email composition
  - Attachment handling
  - HTML and text emails
  - Email template-based emails (using email templates, not reinhardt-pages)
- **Logging**: Application logging
  - Database query logging
  - Email logging
  - Pages rendering logging (reinhardt-pages)
  - Custom log handlers
  - Log level configuration
- **Storage**: File storage backends
  - Local file system storage
  - Advanced storage features
  - Forms integration
  - ORM integration
  - Settings configuration
- **WebSocket**: Real-time bidirectional communication
  - WebSocket connection management
  - Room/channel support
  - Message broadcasting
  - Connection lifecycle
- **GraphQL**: GraphQL API support
  - Schema definition
  - Query execution
  - Mutation support
  - Subscription support
  - Full workflow integration
- **i18n (Internationalization)**: Multi-language support
  - Locale activation/deactivation
  - Message translation (gettext, ngettext, pgettext)
  - Pages integration (reinhardt-pages)
  - Framework-level integration
- **Sitemap**: XML sitemap generation
  - Basic sitemap generation
  - Advanced features (priorities, change frequency)
  - Cache integration
  - HTTP serving
  - i18n sitemap support
  - ORM integration
  - Pages rendering (reinhardt-pages)
- **Syndication**: RSS/Atom feed generation
  - Feed creation
  - Item management
  - Multiple feed formats
  - ORM integration
- **Redirects**: URL redirection management
  - Permanent redirects (301)
  - Temporary redirects (302)
  - Redirect middleware
  - Wildcard pattern support
- **Throttling**: Rate limiting
  - Anonymous user throttling
  - Authenticated user throttling
  - Scoped throttling
  - Multiple throttle classes
  - Backend integration
- **Versioning**: API versioning
  - URL path versioning
  - Accept header versioning
  - Custom header versioning
  - Query parameter versioning
- **Background Tasks**: Async task execution
  - Task scheduling
  - Task execution
  - Error handling
  - Integration with HTTP handlers
- **Contrib Modules**: Additional contributed features
  - Health check endpoints
  - Admin integration helpers
  - Common utilities

#### Utilities & Core (91 tests in 11 files)

- **Dependency Injection**: FastAPI-style DI
  - Macros for DI
  - Request scoping
  - Dependency caching
  - Integration with routers
- **Settings System**: Configuration management
  - Settings loading
  - Environment variable support
  - Settings validation
  - Multiple configuration sources
- **Validators**: Data validation
  - ORM model validators
  - Serializer validators
  - Custom validator classes
  - Async validators
- **Utilities**: Common helper functions
  - Comprehensive utility integration
  - String utilities
  - Date/time utilities
  - Collection utilities
  - Encoding/decoding helpers

## Test Count Summary

| Category                  | Tests   | Files   |
| ------------------------- | ------- | ------- |
| Authentication & Security | 67      | 9       |
| Database & ORM            | 40      | 14      |
| Serializers               | 152     | 13      |
| Routing & Views           | 66      | 8       |
| Pages & Rendering         | 90      | 12      |
| API & REST                | 98      | 10      |
| Server & HTTP             | 72      | 9       |
| Framework Features        | 161     | 33      |
| Utilities & Core          | 91      | 11      |
| **Total**                 | **594** | **119** |

## Prerequisites

### 1. PostgreSQL Database

You need a running PostgreSQL instance. The tests will create and clean up their own tables.

**Using Docker:**

```bash
docker run -d \
  --name reinhardt-test-db \
  -e POSTGRES_PASSWORD=postgres \
  -e POSTGRES_DB=reinhardt_test \
  -p 5432:5432 \
  postgres:17
```

**Using local PostgreSQL:**

```bash
createdb reinhardt_test
```

## Running Tests

### Run All Integration Tests

From the project root:

```bash
cargo test --package reinhardt-integration-tests
```

From this directory:

```bash
cargo test
```

### Run Specific Test Files

```bash
# Authentication tests
cargo test --test auth_security_integration

# Serializer tests
cargo test --test serializer_model_integration_tests

# Server tests
cargo test --test server_integration_tests
```

### Run Specific Tests

```bash
# Run a single test
cargo test test_basic_auth_encoding

# Run tests matching a pattern
cargo test serializer
```

### Include Ignored Tests

Some tests are marked with `#[ignore]` because they require middleware that hasn't been integrated yet:

```bash
# Run all tests including ignored ones
cargo test -- --ignored

# Run only ignored tests
cargo test -- --ignored --test-threads=1
```

## Implementation Details

### Test Utilities

**`src/lib.rs`** provides:

- `setup_test_db()`: Create test database connection pool
- `create_flatpages_tables()`: Set up required database tables
- `cleanup_test_tables()`: Clean up after tests
- `make_request()`: Helper for making HTTP requests to test apps

**Helper modules** provide:

- `src/security_test_helpers.rs`: Authentication testing utilities
- `src/test_helpers.rs`: General testing helpers
- `src/flatpages_common.rs`: Flatpages test utilities
- `src/messages_helpers.rs`: Message framework helpers
- `src/validator_test_common.rs`: Validator testing utilities

### Test Database Isolation

Each test:

1. Creates its own database connection pool
2. Sets up required tables
3. Runs test operations
4. Cleans up all test data

Tests can run in parallel safely due to proper cleanup.

## Troubleshooting

### Permission Denied

```
Error: permission denied to create database
```

**Solution**: Ensure the PostgreSQL user has sufficient privileges:

```sql
ALTER USER postgres CREATEDB;
```

### Port Already in Use

```
Error: Address already in use (os error 48)
```

**Solution**: The test server couldn't bind to a port. This usually resolves itself, but you can run tests sequentially:

```bash
cargo test -- --test-threads=1
```

## Contributing

When adding new integration tests:

1. **Follow Django/DRF/FastAPI test patterns**: Our tests mirror patterns from these frameworks
2. **Clean up resources**: Always clean up database tables, files, and other resources at the end
3. **Use helpers**: Leverage existing test utilities and app builders
4. **Mark pending tests**: Use `#[ignore]` with clear TODO comments for features requiring unimplemented components
5. **Test integration**: Focus on interactions between multiple crates
6. **Document test purpose**: Add comments explaining what integration is being tested

## References

- [Django Test Suite](https://github.com/django/django/tree/main/tests)
- [Django REST Framework Tests](https://github.com/encode/django-rest-framework/tree/master/tests)
- [FastAPI Tests](https://github.com/tiangolo/fastapi/tree/master/tests)
- [Tower Documentation](https://docs.rs/tower/)
- [Hyper Documentation](https://docs.rs/hyper/)
- [SQLx Documentation](https://docs.rs/sqlx/)
