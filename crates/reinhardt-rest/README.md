# reinhardt-rest

**Export-only integration layer** for Reinhardt REST API framework.

## Overview

This crate serves as a **convenience layer** that combines multiple Reinhardt crates into a single import. Core functionality is provided by the underlying specialized crates, though this crate also contains its own implementations for browsable API, filters, metadata, serializers, throttling, versioning, authentication, response handling, schema generation, and OpenAPI support.

## Installation

Add `reinhardt` to your `Cargo.toml`:

<!-- reinhardt-version-sync:3 -->
```toml
[dependencies]
reinhardt = { version = "0.1.0-rc.23", features = ["rest"] }

# Or use a preset:
# reinhardt = { version = "0.1.0-rc.23", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.1.0-rc.23", features = ["full"] }      # All features
```

Then import REST features:

```rust
use reinhardt::rest::{ApiResponse, ResponseBuilder, IntoApiResponse};
use reinhardt::rest::{JwtAuth, IsAuthenticated, AllowAny, User, SimpleUser};
use reinhardt::urls::routers::UnifiedRouter;
use reinhardt::rest::{PaginatedResponse};
```

**Note:** REST features are included in the `standard` and `full` feature presets.

## Purpose

- **Unified Interface**: Single import point for REST API functionality
- **Re-export Layer**: Combines authentication, routing, browsable API, and response handling
- **Own Implementations**: Contains browsable API, filters, metadata, serializers, throttling, versioning, authentication, response handling, schema generation, and OpenAPI support
- **Tested Locally**: Core modules have tests; additional coverage in specialized crates

## Features

### Implemented ✓

#### Authentication (from `reinhardt-auth`)

- **JWT Authentication**: Stateless authentication using JSON Web Tokens
  - `JwtAuth` - JWT authentication backend
  - `Claims` - JWT claims structure
- **User Types**:
  - `User` - Base user trait
  - `SimpleUser` - Simple user implementation
  - `AnonymousUser` - Unauthenticated user representation
- **Permission Classes**:
  - `AllowAny` - Allow all users (authenticated or not)
  - `IsAuthenticated` - Require authentication
  - `IsAuthenticatedOrReadOnly` - Read-only for anonymous, full access for authenticated
  - `IsAdminUser` - Require admin privileges
- **REST-specific Utilities**:
  - `AuthResult<U>` - Result type for authentication operations
  - `AuthBackend` - Authentication backend trait

#### Routing (from `reinhardt-urls`)

- **Router Types**:
  - `DefaultRouter` - Default router with automatic ViewSet URL generation
  - `Router` - Base router trait
- **URL Patterns**:
  - `Route` - Individual route definition
  - `UrlPattern` - URL pattern matching

#### Browsable API (module `browsable_api`)

- **HTML Interface**: Interactive API explorer for development and testing
- **Automatic Documentation**: Self-documenting API endpoints

#### Response Handling

- **Response Types**:
  - `ApiResponse<T>` - DRF-style API response wrapper
    - Success responses (`success`, `success_with_status`)
    - Error responses (`error`, `validation_error`)
    - Standard HTTP responses (`not_found`, `unauthorized`, `forbidden`)
  - `ResponseBuilder<T>` - Fluent builder for API responses
- **Utilities**:
  - `IntoApiResponse<T>` - Trait for converting types to API responses
  - `PaginatedResponse` - Paginated response wrapper (from `reinhardt-core`)

#### Schema Generation (from `reinhardt-openapi`)

- **OpenAPI/Swagger**:
  - `OpenApiSchema` - OpenAPI 3.0 schema generation
  - `Components` - Reusable schema components
  - `Operation` - API operation definitions
  - `Parameter` - Request parameter definitions
  - `Server` - Server configuration
  - Auto-schema generation from Rust types
  - `SwaggerUI` - Interactive API documentation

#### Pagination (from `reinhardt-core`)

- **Pagination Strategies**:
  - `PageNumberPagination` - Page-based pagination
  - `LimitOffsetPagination` - Offset-based pagination
  - `CursorPagination` - Cursor-based pagination

#### Filtering (module `filters`)

- **Filter Backends**:
  - `SearchFilter` - Search across multiple fields
  - `OrderingFilter` - Sort results by fields
  - `QueryFilter` - Type-safe query filtering
  - `MultiTermSearch` - Multi-term search operations

#### Throttling/Rate Limiting (from `reinhardt-throttling`)

- **Throttling Classes**:
  - `AnonRateThrottle` - Rate limiting for anonymous users
  - `UserRateThrottle` - Rate limiting for authenticated users
  - `ScopedRateThrottle` - Per-endpoint rate limiting

#### Signals/Hooks (from `reinhardt-dispatch`)

- **Model Signals**:
  - `pre_save`, `post_save` - Model save signals
  - `pre_delete`, `post_delete` - Model delete signals
  - `m2m_changed` - Many-to-many relationship signals

## Testing

This crate does not contain tests. All functionality is tested in the underlying specialized crates:

- Authentication tests: `reinhardt-auth/tests/`
- Router tests: `reinhardt-urls/tests/`
- Browsable API tests: `reinhardt-rest/src/browsable_api/`
- Response handling tests: Documentation tests in `src/response.rs`
- Integration tests: `tests/integration/`

## Usage

```rust
use reinhardt::rest::{
    // Authentication
    JwtAuth, IsAuthenticated, AllowAny, User, SimpleUser,

    // Response handling
    ApiResponse, ResponseBuilder, IntoApiResponse,

    // Pagination
    PaginatedResponse,
};
// Create a successful response
let user = SimpleUser::new(1, "Alice");
let response = ApiResponse::success(user);

// Build a custom response
let response = ResponseBuilder::new()
    .data("Success")
    .status(201)
    .message("Resource created")
    .build();

// Convert Result to ApiResponse
let result: Result<String, String> = Ok("data".to_string());
let response = result.into_api_response();
```


## browsable-api

### Features

### Implemented ✓

#### Core Rendering

- **BrowsableApiRenderer**: Handlebars-based HTML template renderer
  - Default DRF-inspired template with gradient header design
  - Customizable template registration support
  - JSON response pretty-printing and syntax highlighting
  - Responsive design with modern CSS styling
- **ApiContext**: Complete context structure for API rendering
  - Title, description, endpoint, and HTTP method display
  - Response data with status code
  - Allowed HTTP methods visualization
  - Request headers display in table format
  - Optional form context integration

#### Response Handling

- **BrowsableResponse**: Structured API response type
  - Data payload with serde_json::Value support
  - ResponseMetadata with status, method, path, and headers
  - Convenience constructors (new, success)
  - Full serialization/deserialization support

#### Form Generation

- **FormContext**: Interactive request form rendering
  - Dynamic form field generation
  - Support for multiple input types (text, textarea, etc.)
  - Required field indicators
  - Help text for field guidance
  - Initial value support for form fields
- **FormField**: Individual form field configuration
  - Field name, label, and type specification
  - Required/optional field handling
  - Help text and initial value support

#### Template System

- **ApiTemplate**: Basic HTML template utilities
  - Simple API response rendering
  - Error page generation with status codes
  - Fallback templates for simple use cases

#### Visual Features

- HTTP method badges with color coding (GET, POST, PUT, PATCH, DELETE)
- Monospace endpoint display
- Dark theme code blocks for JSON responses
- Responsive container layout with shadow effects
- Form styling with proper input controls
- Header table display with clean formatting


## openapi

### Features

### Implemented ✓

#### OpenAPI 3.0 Core Types

- **Complete OpenAPI 3.0 Specification**: Full support for OpenAPI 3.0 types via `utoipa` re-exports
  - Info, Contact, License metadata
  - Paths, PathItem, Operation definitions
  - Parameter definitions (Query, Header, Path, Cookie locations)
  - Request/Response body schemas
  - Components and reusable schemas
  - Security schemes (HTTP, ApiKey, OAuth2)
  - Server definitions with variables
  - Tag definitions for API organization

#### Schema Generation

- **SchemaGenerator**: Builder pattern for creating OpenAPI schemas
  - Fluent API for setting title, version, description
  - Direct generation to `OpenApiSchema` (utoipa's `OpenApi` type)

#### Documentation UI

- **Swagger UI Integration**: Built-in Swagger UI via `utoipa-swagger-ui`
  - HTML rendering with customizable title and spec URL
  - Request handler for serving Swagger UI pages
  - Automatic OpenAPI spec serving at `/api/openapi.json`
  - Schema JSON export functionality
- **Redoc UI Support**: Alternative documentation interface
  - HTML rendering for Redoc
  - Request handler for serving Redoc pages
  - Uses same OpenAPI spec endpoint

#### Format Export

- **JSON Export**: Serialize OpenAPI schemas to JSON format
- **YAML Export**: Support via `serde_yaml` dependency (capability present in dependencies)

#### utoipa Compatibility Layer

- **Bidirectional Type Conversion**: Complete conversion utilities between Reinhardt and utoipa types
  - Schema type conversions (Object, Array, primitives)
  - Parameter and request/response body conversions
  - Security scheme conversions (HTTP, ApiKey, OAuth2)
  - Server and tag conversions
  - Format and schema type mappings
  - Comprehensive test coverage

#### Auto-Schema Derivation

- **ToSchema Trait**: Core trait for types that can generate OpenAPI schemas
  - `schema()` method returns OpenAPI schema representation
  - `schema_name()` method returns optional schema identifier
  - Implemented for all Rust primitive types (i8-i64, u8-u64, f32-f64, bool, String)
  - Generic implementations for `Option<T>` and `Vec<T>`

- **Schema Derive Macro**: `#[derive(Schema)]` procedural macro for automatic schema generation
  - Automatic field metadata extraction (type, required, nullable)
  - Support for struct types with named fields
  - Automatic required field detection (`Option<T>` fields are optional)
  - Doc comment extraction for field descriptions
  - Support for enum types with string variant generation
  - Compatible with utoipa 5.4 ObjectBuilder pattern

#### Extended Auto-Schema Features

- **Attribute Macro Support**: Advanced schema customization
  - Field configuration: `#[schema(example = "...", description = "...")]`
  - Nested schema generation with `$ref` references
  - Advanced enum handling (tagged, adjacently tagged, untagged)
  - Integration with serde attributes (`#[serde(rename)]`, `#[serde(skip)]`)
  - Schema registry for component reuse
  - Validation constraint reflection (min, max, pattern)
  - Example value generation
- **HashMap Support**: `HashMap<K,V>` schema generation
- **Tuple Struct Support**: Schema generation for tuple structs

#### ViewSet Integration

- **ViewSet Inspector**: Automatic schema extraction from ViewSets
  - Introspect ViewSet methods and serializers
  - Generate paths and operations from ViewSet definitions
  - Extract parameter information from method signatures
  - Automatic request/response schema generation from serializers


## serializers

### Features

### Implemented ✓

#### Core Serialization

- **`Serializer` trait**: Generic trait for data serialization and deserialization
  - `serialize()`: Convert Rust types to output format
  - `deserialize()`: Parse output format back to Rust types
  - `SerializerError`: Type-safe error handling for serialization failures

- **`JsonSerializer<T>`**: JSON serialization implementation
  - Built on `serde_json` for efficient JSON handling
  - Supports any type implementing `Serialize` and `Deserialize`
  - Automatic conversion between Rust types and JSON strings

- **`Deserializer` trait**: Dedicated deserialization interface
  - Separate trait for read-only deserialization operations
  - Enables more flexible data parsing pipelines

#### Model Serialization

- **`ModelSerializer<M>`**: Automatic serialization for ORM models
  - Django-style automatic field mapping from model definitions
  - Built-in validation support with `validate()` method
  - Seamless integration with `reinhardt-db::Model` trait
  - JSON serialization/deserialization for database models
  - Extensible validation system for custom business logic
  - **Meta Configuration**: Field inclusion/exclusion, read-only/write-only fields
  - **Field Introspection**: Automatic detection of model fields and types
  - **Nested Serializer Support**: Configure and serialize related objects
  - **Validator Integration**: Built-in database constraint validation

#### Meta Configuration

- **`MetaConfig`**: Django REST Framework-style Meta options
  - `fields`: Explicitly include specific fields
  - `exclude`: Exclude specific fields
  - `read_only_fields`: Mark fields as read-only
  - `write_only_fields`: Mark fields as write-only (e.g., passwords)
  - Builder pattern with chainable methods
  - Comprehensive doctests (4 tests) and unit tests (8 tests)

#### Field Introspection

- **`FieldIntrospector`**: Automatic field discovery and type inference
  - Register fields with `FieldInfo` (name, type, optional, collection, primary key)
  - Query fields: `field_names()`, `required_fields()`, `optional_fields()`, `primary_key_field()`
  - Type mapping with `TypeMapper` for common Rust types
  - Integration with ModelSerializer for automatic field detection
  - Comprehensive unit tests (14 tests) and integration tests (10 tests)

- **`FieldInfo`**: Rich field metadata
  - Field name, type name, optionality, collection status
  - Primary key identification
  - Builder pattern for configuration

#### Nested Serialization

- **`NestedSerializerConfig`**: Configure nested object serialization
  - Per-field depth control
  - Read-only vs writable nested fields
  - Create/update permissions (`allow_create`, `allow_update`)
  - Flexible nested field configuration
  - Comprehensive unit tests (11 tests)

- **`NestedFieldConfig`**: Individual nested field configuration
  - `depth()`: Set nesting depth (default: 1)
  - `read_only()`: Mark as read-only
  - `writable()`: Enable create/update operations
  - `allow_create()`, `allow_update()`: Fine-grained permissions

- **`SerializationContext`**: Circular reference and depth management
  - Track visited objects to prevent infinite loops
  - Max depth enforcement
  - Context-aware traversal methods
  - Comprehensive unit tests (15 tests) and integration tests (17 tests)

- **`RecursiveError`**: Error handling for nested serialization
  - `MaxDepthExceeded`: Nesting too deep
  - `CircularReference`: Circular dependency detected
  - `SerializationError`: Generic serialization failures

#### Validator Configuration

- **`ValidatorConfig<M>`**: Manage validators for ModelSerializer
  - Register `UniqueValidator` and `UniqueTogetherValidator`
  - Query registered validators
  - Type-safe validator management
  - Comprehensive unit tests (4 tests) and integration tests (17 tests)

#### Database Validators

- **`UniqueValidator<M>`**: Enforce field uniqueness in database
  - Async validation against PostgreSQL database
  - Supports update operations (excludes current instance from uniqueness check)
  - Customizable field names and error messages
  - Database-level uniqueness verification
  - Builder pattern with `with_message()` for custom error messages
  - Cloneable and debuggable
  - Comprehensive unit tests (4 tests)

- **`UniqueTogetherValidator<M>`**: Ensure unique field combinations
  - Multi-field uniqueness constraints
  - Async PostgreSQL validation
  - Support for update operations
  - Customizable error messages with `with_message()`
  - Flexible field combinations
  - Cloneable and debuggable
  - Comprehensive unit tests (4 tests)

#### Error Handling

- **`SerializerError`**: Comprehensive error type for all serialization operations
  - `Validation(ValidatorError)`: Validation errors with detailed context
  - `Serde { message }`: Serialization/deserialization errors
  - `Other { message }`: Generic errors
  - Helper constructors: `unique_violation()`, `unique_together_violation()`, `required_field()`, `database_error()`
  - `is_validation_error()`, `as_validator_error()` for error inspection
  - Comprehensive error handling tests (22 tests)

- **`ValidatorError`**: Detailed validation error information
  - `UniqueViolation`: Single field uniqueness violation
  - `UniqueTogetherViolation`: Multi-field uniqueness violation
  - `RequiredField`: Missing required field
  - `FieldValidation`: Field constraint violation (regex, range, etc.)
  - `DatabaseError`: Database operation errors
  - `Custom`: Generic validation errors
  - Rich error context with field names, values, and constraints
  - Methods: `message()`, `field_names()`, `is_database_error()`, `is_uniqueness_violation()`

#### Content Negotiation (Re-exported)

- **`ContentNegotiator`**: Select appropriate response format based on client request
- **`MediaType`**: Parse and compare media type strings

#### Parsers (Re-exported from `reinhardt-core`)

- **`JSONParser`**: Parse JSON request bodies
- **`FormParser`**: Parse form-encoded data
- **`MultiPartParser`**: Handle multipart/form-data (file uploads)
- **`FileUploadParser`**: Direct file upload handling
- **`ParseError`**: Error type for parsing failures

#### Field Types

- **`FieldError`**: Comprehensive error types for field validation failures
  - 14 error variants covering all validation scenarios
  - Display implementation for user-friendly error messages
- **`CharField`**: String field with length validation
  - Builder pattern with `min_length()`, `max_length()`, `required()`, `allow_blank()`
  - Default value support
  - Comprehensive doctests (7 tests) and unit tests (3 tests)
- **`IntegerField`**: Integer field with range validation
  - Builder pattern with `min_value()`, `max_value()`, `required()`, `allow_null()`
  - i64 value support
  - Comprehensive doctests (6 tests) and unit tests (3 tests)
- **`FloatField`**: Floating-point field with range validation
  - Builder pattern with `min_value()`, `max_value()`, `required()`, `allow_null()`
  - f64 value support
  - Comprehensive doctests (6 tests) and unit tests (1 test)
- **`BooleanField`**: Boolean field handling
  - Builder pattern with `required()`, `allow_null()`, `default()`
  - Always valid validation (booleans can't be invalid)
  - Comprehensive doctests (3 tests) and unit tests (1 test)
- **`EmailField`**: Email format validation
  - Builder pattern with `required()`, `allow_blank()`, `allow_null()`
  - Basic RFC-compliant email validation (@ sign, domain with dot)
  - Comprehensive doctests (4 tests) and unit tests (2 tests)
- **`URLField`**: URL format validation
  - Builder pattern with `required()`, `allow_blank()`, `allow_null()`
  - HTTP/HTTPS protocol validation
  - Comprehensive doctests (4 tests) and unit tests (2 tests)
- **`ChoiceField`**: Enumerated value validation
  - Builder pattern with `required()`, `allow_blank()`, `allow_null()`
  - Configurable list of valid choices
  - Comprehensive doctests (3 tests) and unit tests (2 tests)

#### Advanced Serialization

- **`SerializerMethodField`**: Compute custom read-only fields
  - Method-based computed fields for serializers
  - Custom method names with `.method_name()`
  - HashMap-based context for method values
  - Read-only field support (always `read_only: true`)
  - Example: `full_name` field computed from `first_name` + `last_name`
  - Comprehensive doctests (2 tests) and unit tests (7 tests)

- **`MethodFieldProvider` trait**: Support for serializers with method fields
  - `compute_method_fields()`: Generate all method field values
  - `compute_method()`: Generate single method field value
  - Integration with serializer context

- **`MethodFieldRegistry`**: Manage multiple method fields
  - Register method fields with `.register()`
  - Retrieve fields with `.get()` and `.contains()`
  - Access all fields with `.all()`

#### Validation System

- **`ValidationError`**: Structured validation error messages
  - `FieldError`: Single field validation errors with field name and message
  - `MultipleErrors`: Collection of multiple validation errors
  - `ObjectError`: Object-level validation errors
  - Helper methods: `field_error()`, `object_error()`, `multiple()`
  - thiserror integration for error handling

- **`FieldValidator` trait**: Field-level validation
  - `validate()`: Validate individual field values
  - Implemented by custom validators (EmailValidator, AgeValidator, etc.)
  - JSON Value-based validation

- **`ObjectValidator` trait**: Object-level validation
  - `validate()`: Validate entire objects with multiple fields
  - Cross-field validation support
  - Example: Password confirmation matching

- **`FieldLevelValidation` trait**: Serializer field-level validation
  - `validate_field()`: Validate specific field by name
  - `get_field_validators()`: Get all registered field validators
  - Django-style `validate_<field>()` pattern support

- **`ObjectLevelValidation` trait**: Serializer object-level validation
  - `validate()`: Validate entire serialized object
  - Called after all field validations pass

- **`validate_fields()` helper**: Validate all fields in a data object
  - Takes HashMap of field validators
  - Returns single error or MultipleErrors
  - Comprehensive doctests (3 tests) and unit tests (13 tests)

### Advanced Relations

#### Hyperlinked Model Serializer

- **HyperlinkedModelSerializer<M>**: Django REST Framework-style hyperlinked serialization
- **UrlReverser Trait**: Automatic URL generation for resources
- **View Name Mapping**: Generate URLs based on view names
- **Custom URL Fields**: Configurable URL field names

```rust
use reinhardt::rest::serializers::HyperlinkedModelSerializer;

let serializer = HyperlinkedModelSerializer::<User>::new("user-detail", None);
// Generates URLs like: {"url": "/api/users/123/", "username": "alice"}

// When using UrlReverser
// let reverser: Arc<dyn UrlReverser> = Arc::new(MyUrlReverser);
// let serializer = HyperlinkedModelSerializer::<User>::new("user-detail", Some(reverser));
```

#### Nested Serializers

- **NestedSerializer<M, R>**: Handle nested object serialization
- **Relationship Fields**: Serialize related models inline
- **Depth Control**: Configure nesting depth
- **Bidirectional Relations**: Support for parent-child relationships

```rust
use reinhardt::rest::serializers::NestedSerializer;

let serializer = NestedSerializer::<Post, User>::new("author", 2);
// Serializes: {"title": "Post", "author": {"id": 1, "username": "alice"}}
```

#### Relation Fields

- **PrimaryKeyRelatedField<T>**: Represent relations using primary keys
- **SlugRelatedField<T>**: Represent relations using slug fields
- **StringRelatedField<T>**: Read-only string representation of related objects
- **Flexible Representation**: Choose the best representation for your API

```rust
use reinhardt::rest::serializers::{PrimaryKeyRelatedField, SlugRelatedField};

// Primary key relation: {"author": 123}
let pk_field = PrimaryKeyRelatedField::<User>::new();

// Slug relation: {"author": "alice-smith"}
let slug_field = SlugRelatedField::<User>::new("slug");
```

### ORM Integration

#### QuerySet Integration

- **`SerializerSaveMixin` trait**: Django-style save interface for serializers
- **`SaveContext`**: Transaction-aware context for save operations
- **Manager Integration**: Automatic ORM create/update operations

```rust
use reinhardt::rest::serializers::{SerializerSaveMixin, SaveContext};
use reinhardt::db::orm::{Model, Manager};

// Create new instance
let context = SaveContext::new();
let user = serializer.save(context).await?;

// Update existing instance
let context = SaveContext::with_instance(existing_user);
let updated_user = serializer.update(validated_data, existing_user).await?;
```

#### Transaction Management

- **`TransactionHelper`**: RAII-based transaction management
- **Automatic Rollback**: Drop-based cleanup on errors
- **Savepoint Support**: Nested transaction handling

```rust
use reinhardt::rest::serializers::TransactionHelper;

// Wrap operations in transaction
TransactionHelper::with_transaction(|| async {
    // All database operations here are atomic
    let user = manager.create(user_data).await?;
    let profile = manager.create(profile_data).await?;
    Ok((user, profile))
}).await?;

// Nested transactions with savepoints
TransactionHelper::savepoint(depth, || async {
    // Nested operation with automatic savepoint
    manager.update(instance).await
}).await?;
```

#### Nested Save Context

- **`NestedSaveContext`**: Depth-aware transaction management
- **Automatic Scope Selection**: Transaction vs savepoint based on depth
- **Hierarchical Operations**: Support for deeply nested serializers

```rust
use reinhardt::rest::serializers::NestedSaveContext;

let context = NestedSaveContext::new(depth);

// Automatically uses transaction (depth=0) or savepoint (depth>0)
context.with_scope(|| async {
    // Nested serializer save operations
    nested_serializer.save(data).await
}).await?;
```

#### Many-to-Many Relationship Management

- **`ManyToManyManager`**: Junction table operations
- **Bulk Operations**: Efficient batch insert/delete
- **Set Operations**: Replace all relationships atomically

```rust
use reinhardt::rest::serializers::ManyToManyManager;

let m2m_manager = ManyToManyManager::<User, Tag>::new(
    "user_tags",      // Junction table
    "user_id",        // Source FK
    "tag_id"          // Target FK
);

// Add multiple relationships
m2m_manager.add_bulk(&user_id, vec![tag1_id, tag2_id, tag3_id]).await?;

// Remove specific relationships
m2m_manager.remove_bulk(&user_id, vec![tag1_id]).await?;

// Replace all relationships atomically
m2m_manager.set(&user_id, vec![tag4_id, tag5_id]).await?;

// Clear all relationships
m2m_manager.clear(&user_id).await?;
```

#### Relation Field Database Lookups

- **`PrimaryKeyRelatedFieldORM`**: Database-backed primary key relations
- **`SlugRelatedFieldORM`**: Database-backed slug field relations
- **Batch Query Optimization**: IN clause for multiple lookups
- **Custom QuerySet Filters**: Additional filtering constraints

```rust
use reinhardt::rest::serializers::{PrimaryKeyRelatedFieldORM, SlugRelatedFieldORM};
use reinhardt::db::orm::{Filter, FilterOperator, FilterValue};

// Primary key relation with database validation
let pk_field = PrimaryKeyRelatedFieldORM::<User>::new();

// Validate existence in database
pk_field.validate_exists(&user_id).await?;

// Fetch single instance
let user = pk_field.get_instance(&user_id).await?;

// Batch fetch (prevents N+1 queries)
let users = pk_field.get_instances(vec![id1, id2, id3]).await?;

// Slug field relation with custom filter
let slug_field = SlugRelatedFieldORM::<User>::new("username")
    .with_queryset_filter(Filter::new(
        "is_active",
        FilterOperator::Eq,
        FilterValue::Bool(true)
    ));

// Validate slug exists
slug_field.validate_exists(&slug_value).await?;

// Fetch by slug
let user = slug_field.get_instance(&slug_value).await?;

// Batch fetch by slugs
let users = slug_field.get_instances(vec!["alice", "bob", "charlie"]).await?;
```

#### Performance Optimization

- **`IntrospectionCache`**: Cache field metadata to avoid repeated introspection
- **`QueryCache`**: TTL-based query result caching
- **`BatchValidator`**: Combine multiple database checks into single queries
- **`PerformanceMetrics`**: Track serialization and validation performance

```rust
use reinhardt::rest::serializers::{IntrospectionCache, QueryCache, BatchValidator, PerformanceMetrics};
use std::time::Duration;

// Cache field introspection results
let cache = IntrospectionCache::new();
if let Some(fields) = cache.get("User") {
    // Use cached fields
} else {
    let fields = introspect_fields();
    cache.set("User".to_string(), fields);
}

// Query result caching with TTL
let query_cache = QueryCache::new(Duration::from_secs(300));
query_cache.set("user:123".to_string(), user_data);

// Batch validation
let mut validator = BatchValidator::new();
validator.add_unique_check("users", "email", "alice@example.com");
validator.add_unique_check("users", "username", "alice");
let failures = validator.execute().await?;

// Performance tracking
let metrics = PerformanceMetrics::new();
metrics.record_serialization(50); // 50ms
let stats = metrics.get_stats();
println!("Average: {}ms", stats.avg_serialization_ms);
```

**Note**: ORM integration features (Django-like QuerySet API and SQLAlchemy-like query builder) are available by default. Both patterns can be used interchangeably for database operations.

#### Additional Field Types

- `DateField`, `DateTimeField`: Date and time handling with chrono integration

#### Advanced Serialization

- `WritableNestedSerializer`: Support updates to nested objects
- `ListSerializer`: Serialize collections of objects

#### Additional Renderers

- `YAMLRenderer`: Render data as YAML
- `CSVRenderer`: Render data as CSV (for list endpoints)
- `OpenAPIRenderer`: Generate OpenAPI/Swagger specifications

#### Meta Options

- Field inclusion/exclusion
- Read-only/write-only fields
- Custom field mappings
- Depth control for nested serialization


## versioning

### Features

### ✅ Implemented

#### Core Versioning Strategies

1. **URLPathVersioning** - Version detection from URL path
   - Extracts version from path segments (e.g., `/v1/users/`, `/v2.0/api/`)
   - Customizable regex patterns for flexible version extraction
   - Pattern configuration with `with_pattern()` method
   - Default fallback version support
   - Allowed versions validation
   - Examples: `/v1/`, `/v2.0/`, `/api/v3/`

2. **AcceptHeaderVersioning** - Version detection from Accept header
   - Parses Accept header media type parameters (e.g., `Accept: application/json; version=2.0`)
   - Configurable version parameter name
   - Strict version validation
   - Default version fallback
   - Compatible with standard media type negotiation
   - Example: `Accept: application/json; version=1.0`

3. **QueryParameterVersioning** - Version detection from query parameters
   - Extracts version from query string (e.g., `?version=1.0`, `?v=2.0`)
   - Customizable parameter name with `with_version_param()`
   - Multiple parameter support
   - Default version fallback
   - Examples: `?version=1.0`, `?v=2.0`

4. **HostNameVersioning** - Version detection from subdomain
   - Extracts version from hostname subdomain (e.g., `v1.api.example.com`)
   - Customizable regex patterns for hostname parsing
   - Host format configuration with `with_host_format()`
   - Hostname pattern mapping with `with_hostname_pattern()`
   - Port handling support
   - Examples: `v1.api.example.com`, `api-v2.example.com`

5. **NamespaceVersioning** - Version detection from URL namespace
   - Router namespace integration for version extraction
   - Configurable namespace patterns (e.g., `/v{version}/`)
   - Namespace prefix support with `with_namespace_prefix()`
   - Router integration methods:
     - `extract_version_from_router()` - Extract version from router path
     - `get_available_versions_from_router()` - Get all registered versions
   - Pattern-based version extraction
   - Examples: `/v1/users/`, `/api/v2/posts/`

#### Middleware System

6. **VersioningMiddleware** - Automatic version detection and injection
   - Integrates with any `BaseVersioning` strategy
   - Automatic version extraction from requests
   - Stores version in request extensions
   - Error handling for invalid versions
   - Clone support for middleware composition
   - Zero-cost abstraction over versioning strategies

7. **RequestVersionExt** - Type-safe version access from requests
   - `version()` - Get version as `Option<String>`
   - `version_or()` - Get version with fallback default
   - Seamless integration with request extensions
   - Type-safe version retrieval

8. **ApiVersion** - Version data type
   - `as_str()` - Get version as string slice
   - `to_string()` - Get version as owned String
   - `new()` - Create new version instance
   - Clone and Debug support

#### Handler Integration

9. **VersionedHandler** - Trait for version-aware handlers
   - `handle_versioned()` - Handle request with version context
   - `supported_versions()` - Get list of supported versions
   - `supports_version()` - Check version support

10. **VersionedHandlerWrapper** - Handler trait adapter
    - Makes `VersionedHandler` compatible with standard `Handler` trait
    - Automatic version determination
    - Version validation before handling
    - Error handling for unsupported versions

11. **SimpleVersionedHandler** - Simple version-to-response mapper
    - Map versions to static responses
    - `with_version_response()` - Add version-specific response
    - `with_default_response()` - Set fallback response
    - HashMap-based response lookup

12. **ConfigurableVersionedHandler** - Advanced handler configuration
    - Map versions to different handler implementations
    - `with_version_handler()` - Add version-specific handler
    - `with_default_handler()` - Set fallback handler
    - Dynamic handler dispatch based on version

13. **VersionedHandlerBuilder** - Builder pattern for handlers
    - Fluent API for handler construction
    - Version-handler mapping
    - Default handler configuration
    - Automatic wrapper integration

14. **VersionResponseBuilder** - Response builder with version metadata
    - `with_data()` - Add response data
    - `with_field()` - Add individual fields
    - `with_version_info()` - Add version metadata
    - `version()` - Get current version
    - JSON serialization support

15. **versioned_handler!** - Macro for easy handler creation
    - Declarative syntax for version mapping
    - Optional default handler
    - Compile-time version checking

#### Configuration System

16. **VersioningConfig** - Global configuration
    - Centralized versioning settings
    - Strategy configuration
    - Default and allowed versions
    - Strict mode enforcement
    - Version parameter customization
    - Hostname pattern mapping
    - Builder pattern API
    - Serialization/deserialization support

17. **VersioningStrategy** - Strategy enumeration
    - Five strategy variants:
      - `AcceptHeader` - Accept header versioning
      - `URLPath { pattern }` - URL path with custom pattern
      - `QueryParameter { param_name }` - Query parameter with custom name
      - `HostName { patterns }` - Hostname with pattern mapping
      - `Namespace { pattern }` - Namespace with custom pattern
    - Serde support for configuration files
    - JSON/YAML compatible

18. **VersioningManager** - Configuration management
    - Create versioning instances from configuration
    - Dynamic configuration updates
    - `config()` - Get current configuration
    - `versioning()` - Get versioning instance
    - `update_config()` - Update configuration at runtime
    - Environment variable support with `from_env()`

19. **Environment Configuration** - Env var support
    - `REINHARDT_VERSIONING_DEFAULT_VERSION` - Default version
    - `REINHARDT_VERSIONING_ALLOWED_VERSIONS` - Comma-separated allowed versions
    - `REINHARDT_VERSIONING_STRATEGY` - Strategy type
    - `REINHARDT_VERSIONING_STRICT_MODE` - Enable/disable strict mode

#### URL Reverse System

20. **VersionedUrlBuilder** - Versioned URL construction
    - Build URLs with version in appropriate location
    - Strategy-aware URL generation
    - `build()` - Build URL with default version
    - `build_with_version()` - Build URL with specific version
    - `build_all_versions()` - Build URLs for all allowed versions
    - Support for all five versioning strategies

21. **UrlReverseManager** - Multiple builder management
    - Named builder registration
    - Default builder support
    - `add_builder()` - Register named builder
    - `with_default_builder()` - Set default builder
    - `build_url()` - Build URL with named builder
    - `build_default_url()` - Build URL with default builder
    - `build_all_urls()` - Build URLs with all builders

22. **ApiDocUrlBuilder** - API documentation URL builder
    - OpenAPI schema URLs
    - Swagger UI URLs
    - ReDoc URLs
    - Custom format support
    - Version-specific documentation paths
    - Examples:
      - `/v1.0/openapi.json`
      - `/v2.0/swagger-ui/`
      - `/v1.0/redoc/`

23. **ApiDocFormat** - Documentation format enum
    - `OpenApi` - OpenAPI 3.0 JSON
    - `Swagger` - Swagger UI
    - `ReDoc` - ReDoc documentation
    - `Custom(String)` - Custom format

24. **versioned_url!** - Macro for URL building
    - Simple syntax for URL construction
    - Version override support
    - Type-safe URL generation

#### Testing & Quality

25. **Comprehensive Test Coverage**
    - 29+ unit tests across all modules
    - 11+ integration tests
    - All versioning strategies tested
    - Middleware integration tests
    - Handler system tests
    - URL building tests
    - Configuration serialization tests

26. **Test Utilities** - `test_utils` module
    - `create_test_request()` - Create mock requests for testing
    - Header customization
    - URI customization
    - Reusable across test suites

27. **Full Documentation**
    - Comprehensive rustdoc comments
    - Code examples for all public APIs
    - Usage examples in each module
    - Integration examples

#### Error Handling

28. **VersioningError** - Comprehensive error types
    - `InvalidAcceptHeader` - Malformed Accept header
    - `InvalidURLPath` - Invalid URL path format
    - `InvalidNamespace` - Invalid namespace format
    - `InvalidHostname` - Invalid hostname format
    - `InvalidQueryParameter` - Invalid query parameter
    - `VersionNotAllowed` - Version not in allowed list
    - Integration with `reinhardt_apps::Error`

#### Traits & Abstractions

29. **BaseVersioning** - Core versioning trait
    - `determine_version()` - Extract version from request
    - `default_version()` - Get default version
    - `allowed_versions()` - Get allowed versions
    - `is_allowed_version()` - Check version validity
    - `version_param()` - Get version parameter name
    - Async trait for async version detection
    - Send + Sync for thread safety
