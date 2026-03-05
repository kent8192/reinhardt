# reinhardt-urls

URL routing and proxy utilities for Reinhardt framework

## Overview

`` `reinhardt-urls` `` provides comprehensive URL routing and lazy loading proxy functionality for Reinhardt applications, inspired by Django's URL system.

## Features

### Implemented ✓

This crate provides the following modules:

- **Routers**: Automatic URL routing configuration
  - Django-inspired URL routing
  - Automatic ViewSet URL generation
  - Namespacing and versioning support
  - URL reversal capabilities
  - PathPattern for URL pattern matching
  - DefaultRouter with automatic endpoint generation
  - Custom action support (list and detail-level)

- **Routers Macros**: Routing-related procedural macros
  - Compile-time route validation
  - Type-safe URL pattern generation
  - Route registration macros

- **Proxy**: Lazy loading proxy system
  - Django-style SimpleLazyObject implementation
  - Thread-safe lazy evaluation
  - Integration with ORM for lazy model loading
  - Automatic initialization on first access
  - Support for complex initialization logic
  - Advanced proxy features:
    - Association proxies (SQLAlchemy-style)
    - Scalar proxies with comparison operations
    - Collection proxies for relationship management
    - Query filtering and join operations
    - Lazy/eager loading strategies
    - Relationship caching

- **Advanced URL Pattern Matching**:
  - Compile-time path validation via `path!` macro
  - Runtime pattern matching with parameter extraction
  - Path constraint validation (snake_case parameters, no double slashes, etc.)
  - Regex-based URL matching with named capture groups

## Installation

Add `reinhardt` to your `Cargo.toml`:

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", features = ["urls"] }

# For specific sub-features:
# reinhardt = { version = "0.1.0-alpha.1", features = ["urls-routers", "urls-proxy"] }

# Or use a preset:
# reinhardt = { version = "0.1.0-alpha.1", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.1.0-alpha.1", features = ["full"] }      # All features
```

Then import URLs features:

```rust
use reinhardt::urls::{Router, DefaultRouter, Route};
use reinhardt::urls::routers::{path, re_path, include_routes};
use reinhardt::urls::proxy::{SimpleLazyObject, AssociationProxy};
```

**Note:** URLs features are included in the `standard` and `full` feature presets.

## Usage

### URL Routing

```rust
use reinhardt::urls::{Router, DefaultRouter, Route};

// Create a router
let mut router = DefaultRouter::new();

// Register ViewSet
router.register("users", UserViewSet::new());

// Add custom routes
router.add_route(Route::new("/custom/", custom_handler));

// Match incoming requests
if let Some((handler, params)) = router.match_request(&request) {
    handler.handle(request, params).await?;
}
```

### URL Reversal

```rust
use reinhardt::urls::reverse;

// Reverse URL by name
let url = reverse("user-detail", &[("id", "123")]);
// Returns: /users/123/

// With namespace
let url = reverse("api:v1:user-list", &[]);
// Returns: /api/v1/users/
```

### Lazy Loading Proxy

```rust
use reinhardt::urls::proxy::SimpleLazyObject;

// Create lazy object
let lazy_user = SimpleLazyObject::new(|| {
    // Expensive initialization
    User::from_database(user_id)
});

// Access triggers initialization
let name = lazy_user.name; // Initialization happens here
```

## Module Organization

`` `reinhardt-urls` `` is organized into the following modules:

- `` `routers` `` - URL routing system
- `` `routers_macros` `` - Routing procedural macros
- `` `proxy` `` - Lazy loading proxy utilities

### Using Modules

```rust
use reinhardt::urls::routers::{DefaultRouter, PathPattern};
use reinhardt::urls::proxy::LazyObject;
```

## proxy

### Features

### Implemented ✓

#### Core Association Proxy (`proxy.rs`)

- `AssociationProxy<T, U>` - Main proxy type for relationship traversal
- `ProxyAccessor` trait - Interface for getting/setting proxy targets
- `ProxyTarget` enum - Represents scalar or collection proxy results
- `ScalarValue` enum - Type-safe scalar value representation (String, Integer, Float, Boolean, Null)
- Creator function support for new associations
- Comprehensive type conversion methods (`as_string()`, `as_integer()`, `as_float()`, `as_boolean()`)

#### Scalar Proxies (`scalar.rs`)

- `ScalarProxy` - For one-to-one and many-to-one relationships
- `ScalarComparison` enum - Rich comparison operators (Eq, Ne, Gt, Gte, Lt, Lte, In, NotIn, IsNull, IsNotNull, Like, NotLike)
- Async get/set operations for scalar values
- Builder methods for all comparison types

#### Collection Proxies (`collection.rs`)

- `CollectionProxy` - For one-to-many and many-to-many relationships
- Unique value support with deduplication
- Collection manipulation methods:
  - `get_values()` - Extract all values from related objects
  - `set_values()` - Replace entire collection
  - `append()` - Add single value
  - `remove()` - Remove matching values
  - `contains()` - Check for value existence
  - `count()` - Get collection size
- Advanced filtering:
  - `filter()` - Filter with FilterCondition
  - `filter_by()` - Filter with custom predicate
- `CollectionOperations` - Wrapper for transformation operations (filter, map, sort, distinct)
- `CollectionAggregations` - Wrapper for aggregation operations (sum, avg, min, max)

#### Query Filtering (`query.rs`)

- `FilterOp` enum - Filter operations (Eq, Ne, Lt, Le, Gt, Ge, In, NotIn, Contains, StartsWith, EndsWith)
- `FilterCondition` - Condition with field, operator, and value
- `QueryFilter` - Container for multiple conditions
- `matches()` method for evaluating conditions against ScalarValue

#### Join Operations (`joins.rs`)

- `JoinConfig` - Configuration for eager/lazy loading
- `LoadingStrategy` enum - Eager, Lazy, Select strategies
- `NestedProxy` - Multi-level relationship traversal
- `RelationshipPath` - Path representation for relationships
- Helper functions:
  - `extract_through_path()` - Parse dot-separated paths
  - `filter_through_path()` - Filter path segments
  - `traverse_and_extract()` - Extract from nested proxies
  - `traverse_relationships()` - Navigate relationship paths

#### Builder Pattern (`builder.rs`)

- `ProxyBuilder<T, U>` - Fluent API for proxy construction
- Method chaining for configuration:
  - `relationship()` - Set relationship name
  - `attribute()` - Set attribute name
  - `creator()` - Set creator function
- Safe construction methods:
  - `build()` - Build with panic on missing config
  - `try_build()` - Build returning Option
- `association_proxy()` helper function

#### Reflection System (`reflection.rs`)

- `Reflectable` trait - Core trait for runtime introspection
  - `get_relationship()` / `get_relationship_mut()` - Access relationships
  - `get_attribute()` / `set_attribute()` - Access attributes
  - `get_relationship_attribute()` / `set_relationship_attribute()` - Nested access
  - `has_relationship()` / `has_attribute()` - Existence checks
- `ProxyCollection` trait - Unified collection interface
  - Generic implementation for `Vec<T>`
  - Methods: `items()`, `add()`, `remove()`, `contains()`, `len()`, `clear()`
- `AttributeExtractor` trait - Scalar value extraction interface
- Helper functions:
  - `downcast_relationship()` - Type-safe downcasting
  - `extract_collection_values()` - Bulk value extraction

#### Error Handling

- `ProxyError` enum with comprehensive error types:
  - `RelationshipNotFound` - Missing relationship
  - `AttributeNotFound` - Missing attribute
  - `TypeMismatch` - Type conversion errors
  - `InvalidConfiguration` - Configuration errors
  - `DatabaseError` - Database operation errors
  - `SerializationError` - Serialization errors
- `ProxyResult<T>` type alias for Result with ProxyError

## License

Licensed under the BSD 3-Clause License.
