# reinhardt-views

View classes and response handling for the Reinhardt framework, inspired by
Django's class-based views and Django REST Framework.

## Overview

This crate provides the view layer for building RESTful APIs in Reinhardt. It
includes base view traits, concrete view implementations for common CRUD
operations, and utilities for OpenAPI schema generation and browsable API
rendering.

## Installation

Add `reinhardt` to your `Cargo.toml`:

<!-- reinhardt-version-sync:3 -->
```toml
[dependencies]
reinhardt = { version = "0.1.0-rc.23", features = ["views"] }

# Or use a preset:
# reinhardt = { version = "0.1.0-rc.23", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.1.0-rc.23", features = ["full"] }      # All features
```

Then import view features:

```rust
use reinhardt::views::{View, ListView, DetailView};
use reinhardt::views::{ListAPIView, CreateAPIView, UpdateAPIView};
use reinhardt::views::viewsets::{ViewSet, ModelViewSet};
```

**Note:** View features are included in the `standard` and `full` feature presets.

## Features

### Implemented ✓

#### Core View Infrastructure

- **View Trait** - Base trait for all views with async dispatch support
  - Automatic OPTIONS method handling
  - Customizable allowed methods
  - Async request/response processing

#### Class-Based Views

- **ListView** - Display paginated lists of objects
  - Configurable pagination with DRF-style metadata
  - Multi-field ordering (ascending/descending)
  - Empty result set handling
  - Custom context object naming
  - Full serializer support
  - HEAD method support
- **DetailView** - Display single objects
  - Primary key (pk) lookup
  - Slug-based lookup
  - QuerySet integration
  - Custom context object naming
  - Full serializer support
  - HEAD method support

#### View Mixins

- **MultipleObjectMixin** - Common functionality for list views
  - Object retrieval
  - Ordering configuration
  - Pagination settings
  - Context data building
- **SingleObjectMixin** - Common functionality for detail views
  - Object retrieval with pk/slug
  - URL parameter configuration
  - Context data building

#### Generic API Views ✓

- **ListAPIView** - List endpoint with pagination, filtering, and ordering
  - GET/HEAD request support
  - QuerySet integration
  - JSON serialization
  - Pagination metadata
- **CreateAPIView** - Create endpoint for object creation
  - POST request support
  - Validation support (stub)
- **UpdateAPIView** - Update endpoint for object modification
  - PUT/PATCH request support
  - Partial update support
  - Lookup field configuration
- **DestroyAPIView** - Delete endpoint for object deletion
  - DELETE request support
  - Lookup field configuration
- **ListCreateAPIView** - Combined list/create endpoint
  - GET/HEAD/POST request support
  - Pagination and ordering for list operations
- **RetrieveUpdateAPIView** - Combined retrieve/update endpoint
  - GET/HEAD/PUT/PATCH request support (stub)
- **RetrieveDestroyAPIView** - Combined retrieve/delete endpoint
  - GET/HEAD/DELETE request support (stub)
- **RetrieveUpdateDestroyAPIView** - Combined retrieve/update/delete endpoint
  - GET/HEAD/PUT/PATCH/DELETE request support (stub)

**Note**: Full ORM integration pending for create/update/delete operations.

#### OpenAPI Schema Generation

- **OpenAPISpec** - OpenAPI 3.0 specification structure
- **Schema** - JSON Schema definitions
- **PathItem** - API path definitions with HTTP methods
- **Operation** - HTTP operation metadata
- **Parameter** - Request parameter definitions (query, header, path, cookie)
- **Response** - Response schema definitions
- **Components** - Reusable schema components
- **SchemaGenerator** - Schema generation utilities
- **EndpointInfo** - Endpoint metadata for documentation

#### Browsable API

- Basic rendering infrastructure (minimal stub)

#### Admin Integration

- Admin change view support
- Integration with Django-style admin interface

#### ViewSets (from `reinhardt-views`)

- **ModelViewSet** - Complete CRUD operations for models
  - List, Create, Retrieve, Update, Partial Update, Destroy actions
  - Automatic HTTP method mapping
  - Custom action support (`ActionType::Custom`)
- **ViewSet Builder** - Fluent API for ViewSet configuration
- **Action System** - Type-safe action definitions
  - Built-in actions (List, Retrieve, Create, Update, PartialUpdate, Destroy)
  - Custom action support
  - Detail/non-detail action distinction
- **Handler System** - Request routing and dispatch
- **Dependency Injection** - Field and method injection for ViewSets
- **Middleware Support** - ViewSet-specific middleware
- **Registry** - ViewSet registration and discovery

#### Enhanced Browsable API

- HTML rendering for API exploration
- Interactive API documentation
- Form generation for POST/PUT/PATCH methods
- Syntax highlighting for responses

#### Schema Generation Enhancements

- Automatic schema inference from models
- Type-safe schema generation from Rust types
- Request/response example generation
- Schema validation utilities

#### Advanced ViewSet Features

- Nested resources handling
- Batch operations support
- Optimistic locking support

#### Component-Based Rendering (reinhardt-pages)

- SSR (Server-Side Rendering) with WASM components
- Reactive component rendering
- Client-side hydration support
- Type-safe view composition

## Usage

### ListView Example

```rust
use reinhardt::views::{ListView, View};
use reinhardt::core::serializers::JsonSerializer;
use reinhardt::db::orm::Model;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Article {
    id: Option<i64>,
    title: String,
    content: String,
}

impl Model for Article {
    type PrimaryKey = i64;
    fn table_name() -> &'static str { "articles" }
    fn primary_key(&self) -> Option<&Self::PrimaryKey> { self.id.as_ref() }
    fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
}

// Create a paginated list view
let articles = vec![
    Article { id: Some(1), title: "First".into(), content: "...".into() },
    Article { id: Some(2), title: "Second".into(), content: "...".into() },
];

let view = ListView::<Article, JsonSerializer<Article>>::new()
    .with_objects(articles)
    .with_paginate_by(10)
    .with_ordering(vec!["-id".into()])
    .with_context_object_name("articles");
```

### DetailView Example

```rust
use reinhardt::views::{DetailView, View};
use reinhardt::core::serializers::JsonSerializer;

let article = Article {
    id: Some(1),
    title: "Hello".into(),
    content: "World".into()
};

let view = DetailView::<Article, JsonSerializer<Article>>::new()
    .with_object(article)
    .with_pk_url_kwarg("article_id")
    .with_context_object_name("article");
```

### Generic API Views Examples

```rust
use reinhardt::views::{
    ListAPIView, CreateAPIView, UpdateAPIView, DestroyAPIView,
    ListCreateAPIView, View
};
use reinhardt::core::serializers::JsonSerializer;

// List endpoint
let list_view = ListAPIView::<Article, JsonSerializer<Article>>::new()
    .with_paginate_by(10)
    .with_ordering(vec!["-created_at".into()]);

// Create endpoint
let create_view = CreateAPIView::<Article, JsonSerializer<Article>>::new();

// Update endpoint
let update_view = UpdateAPIView::<Article, JsonSerializer<Article>>::new()
    .with_lookup_field("id".into())
    .with_partial(true);

// Delete endpoint
let destroy_view = DestroyAPIView::<Article>::new()
    .with_lookup_field("id".into());

// Combined list/create endpoint
let list_create_view = ListCreateAPIView::<Article, JsonSerializer<Article>>::new()
    .with_paginate_by(20)
    .with_ordering(vec!["-id".into()]);
```

### OpenAPI Schema Generation

<!-- reinhardt-version-sync -->
```rust
use reinhardt::views::{OpenAPISpec, Info, PathItem, Operation};

let spec = OpenAPISpec::new(Info::new(
    "My API".into(),
    "0.1.0-rc.23".into()
));
```

## Dependencies

- `reinhardt-core` - Core types, serializers, pagination, and exception handling
- `reinhardt-db` - Database and ORM integration
- `reinhardt-http` - HTTP request/response types
- `reinhardt-di` - Dependency injection
- `async-trait` - Async trait support

## Testing

The crate includes comprehensive unit tests covering:

- Basic view functionality
- ListView pagination and ordering
- DetailView object retrieval
- Error handling
- ViewSet patterns
- API view behavior
- Admin change views


## viewsets

### Features

### Implemented ✓

#### Core ViewSet Types

- **ViewSet Trait** - Base trait for all ViewSet implementations with dispatch, middleware support, and action routing
- **GenericViewSet** - Generic ViewSet implementation with composable handler pattern
- **ModelViewSet** - Full CRUD operations (list, retrieve, create, update, destroy) for model-based APIs
- **ReadOnlyModelViewSet** - Read-only operations (list, retrieve) for immutable resources

#### Action System

- **Action Types** - Comprehensive action type system supporting standard CRUD operations and custom actions
  - Standard actions: List, Retrieve, Create, Update, PartialUpdate, Destroy
  - Custom action support with configurable detail/list behavior
- **Action Metadata** - Rich metadata system for actions including:
  - Custom display names and suffixes
  - URL path and name configuration
  - HTTP method filtering
  - Action handler integration
- **Action Registry** - Global and local action registration systems
  - Manual registration API with `register_action()`
  - Macro-based registration with `register_viewset_actions!`
  - Inventory-based automatic collection of actions

#### Mixin System

- **ListMixin** - Provides list() action for querying collections
- **RetrieveMixin** - Provides retrieve() action for fetching single objects
- **CreateMixin** - Provides create() action for object creation
- **UpdateMixin** - Provides update() action for object modification
- **DestroyMixin** - Provides destroy() action for object deletion
- **CrudMixin** - Composite trait combining all CRUD operations

#### Middleware Support

- **ViewSetMiddleware Trait** - Middleware integration for cross-cutting concerns
  - `process_request()` - Pre-processing with early response capability
  - `process_response()` - Post-processing and response modification
- **AuthenticationMiddleware** - Login requirement enforcement
  - Configurable login_required behavior
  - Login URL redirection support
  - Session and header-based authentication detection
- **PermissionMiddleware** - Permission-based access control
  - Per-ViewSet permission requirements
  - Automatic 403 Forbidden responses for unauthorized access
- **CompositeMiddleware** - Middleware composition and chaining
  - Builder pattern for middleware configuration
  - Sequential middleware execution

#### Handler Integration

- **ViewSetHandler** - Converts ViewSets to Handlers for routing integration
  - HTTP method to action mapping
  - Path parameter extraction
  - Request attribute management (args, kwargs)
  - Middleware processing pipeline
- **ViewSetBuilder** - Fluent builder API for Handler creation
  - Action mapping configuration with `with_actions()` and `action()`
  - Custom name/suffix support (mutually exclusive)
  - Validation of action mappings
  - Macro support via `viewset_actions!`

#### Dependency Injection (FastAPI-Style)

- **Three DI Patterns** - Multiple ways to inject dependencies into ViewSets:
  1. **Field-Level Injection** - Use `#[derive(Injectable)]` with `#[inject]` attributes on struct fields
  2. **Method-Level Injection** - Use `#[endpoint]` with `#[inject]` attributes on method parameters
  3. **Dispatch-Level Injection** - Override `dispatch_with_context()` with `#[inject]` parameters
- **DiViewSet** - ViewSet wrapper with full DI support
  - Automatic dependency resolution via `Depends<V>`
  - Integration with reinhardt-di framework
- **ViewSetFactory Trait** - Factory pattern for ViewSet creation with DI
- **Injectable Dependencies** - Example implementations (DatabaseConnection)
- **Cache Control** - Fine-grained control with `#[inject(cache = false)]`
- **Backward Compatibility** - Non-DI ViewSets continue to work without changes

#### Testing Utilities

- **TestViewSet** - Configurable test ViewSet with middleware support
  - Configurable login_required behavior
  - Permission configuration
  - Middleware integration testing
- **SimpleViewSet** - Minimal ViewSet for basic testing scenarios

#### Advanced Features

- **Pagination Integration** - Automatic pagination support for list actions
- **Filtering System** - Query parameter-based filtering for collections
- **Ordering Support** - Sortable collections with multiple field support
- **Bulk Operations** - Batch create/update/delete operations
- **Nested ViewSets** - Parent-child resource relationships
- **ViewSet Schema Generation** - OpenAPI schema generation from ViewSet definitions
- **Caching Support** - Response caching for read-only operations
- **Rate Limiting** - Per-ViewSet or per-action rate limiting
- **WebSocket ViewSets** - Real-time action support via WebSockets
