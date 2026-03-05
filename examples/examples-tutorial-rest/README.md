# Reinhardt REST Tutorial Example - Code Snippet Management API

This example demonstrates the concepts covered in the [Reinhardt REST Tutorial](../../../docs/tutorials/en/rest/). It implements a complete RESTful API for managing code snippets.

## What This Example Covers

This example corresponds to the REST tutorial Quickstart and Tutorial 1-6:

- **Quickstart** - Project setup, serializers, views, routing
- **Tutorial 1: Serialization** - serde, validation, JSON serialization
- **Tutorial 2: Requests and Responses** - Request object, Response builder, parameter extraction
- **Tutorial 3: Class-Based Views** - Generic views (ListAPIView, CreateAPIView, etc.)
- **Tutorial 4: Authentication & Permissions** - Authentication systems
- **Tutorial 5: Relationships and Hyperlinked APIs** - Relationships and hyperlinked APIs
- **Tutorial 6: ViewSets and Routers** - ViewSets, ModelViewSet, Router

## Features

### Models

- **Snippet**: Code snippet with title, code, and language

### Serializers

- **SnippetSerializer**: Input validation with validator
- **SnippetResponse**: Output serialization

### API Endpoints

```
GET    /api/snippets/       - List all snippets
POST   /api/snippets/       - Create a new snippet
GET    /api/snippets/<id>/  - Retrieve a specific snippet
PUT    /api/snippets/<id>/  - Update a snippet
DELETE /api/snippets/<id>/  - Delete a snippet
```

## Setup

### Prerequisites

- Rust 1.75 or later
- PostgreSQL (optional, for database features)
- Docker (optional, for TestContainers in tests)

### Installation

```bash
# From the project root
cd examples/examples-tutorial-rest

# Build the project
cargo build

# Run tests
cargo test
```

## Usage

### Run the Development Server

```bash
cargo run --bin manage runserver
```

The server will start at `http://127.0.0.1:8000/`.

### API Examples

```bash
# List all snippets
curl http://127.0.0.1:8000/api/snippets/

# Create a new snippet
curl -X POST http://127.0.0.1:8000/api/snippets/ \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Hello World",
    "code": "println!(\"Hello, world!\");",
    "language": "rust"
  }'

# Get a specific snippet
curl http://127.0.0.1:8000/api/snippets/1/

# Update a snippet
curl -X PUT http://127.0.0.1:8000/api/snippets/1/ \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Hello Reinhardt",
    "code": "println!(\"Hello, Reinhardt!\");",
    "language": "rust"
  }'

# Delete a snippet
curl -X DELETE http://127.0.0.1:8000/api/snippets/1/
```

## Project Structure

```
examples-tutorial-rest/
├── Cargo.toml                 # Project configuration
├── build.rs                   # Build script
├── README.md                  # This file
├── src/
│   ├── lib.rs                # Library entry point
│   ├── config.rs             # Config module
│   ├── apps.rs               # Apps module
│   ├── bin/
│   │   └── manage.rs         # Management command
│   ├── config/
│   │   ├── settings.rs       # Settings configuration
│   │   ├── urls.rs           # URL routing
│   │   └── apps.rs           # Installed apps
│   └── apps/
│       └── snippets/
│           ├── lib.rs        # App module
│           ├── models.rs     # Snippet model
│           ├── serializers.rs  # Serializers
│           ├── views.rs      # View handlers
│           └── urls.rs       # URL patterns
└── tests/
    ├── integration.rs        # Integration tests
    └── availability.rs       # Availability tests
```

## Learning Path

This example is designed to be studied alongside the REST tutorial:

1. **Start with the tutorial**: Read [Quickstart](../../../docs/tutorials/en/rest/quickstart.md)
2. **Examine the code**: Look at how concepts are implemented in this example
3. **Run the tests**: `cargo test` to see the functionality in action
4. **Experiment**: Modify the code and see what happens

## Key Concepts Demonstrated

### 1. Models (models.rs)

```rust
pub struct Snippet {
    pub id: Option<i64>,
    pub title: String,
    pub code: String,
    pub language: String,
}
```

### 2. Serializers (serializers.rs)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SnippetSerializer {
    #[validate(length(min = 1, max = 100))]
    pub title: String,

    #[validate(length(min = 1))]
    pub code: String,

    #[validate(length(min = 1, max = 50))]
    pub language: String,
}
```

### 3. Views (views.rs)

```rust
pub async fn list(_req: Request) -> Result<Response, Box<dyn std::error::Error + Send + Sync>> {
    // List all snippets
}

pub async fn create(mut req: Request) -> Result<Response, Box<dyn std::error::Error + Send + Sync>> {
    // Create a new snippet with validation
    let serializer: SnippetSerializer = serde_json::from_slice(&req.body)?;
    serializer.validate()?;
    // ...
}
```

### 4. URL Routing (urls.rs)

```rust
UnifiedRouter::new()
    .function("/", Method::GET, super::views::list)
    .function("/", Method::POST, super::views::create)
    .function("/{id}/", Method::PUT, super::views::update)
    .function("/{id}/", Method::GET, super::views::retrieve)
    .function("/{id}/", Method::DELETE, super::views::delete)
```

### 5. Validation

```rust
use validator::Validate;

// In view handler
serializer.validate()?;  // Returns validation errors if invalid
```

## Testing

Run the test suite:

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_snippet_model

# Run with output
cargo test -- --nocapture
```

## ViewSets (Tutorial 6)

This example demonstrates both function-based views (Tutorial 1-5) and ViewSet-based views (Tutorial 6).

### Switching Between Approaches

You can switch between the two approaches using the `USE_VIEWSET` environment variable:

```bash
# Function-based views (default) - Tutorial 1-5 approach
cargo run --bin manage runserver
# Visit http://127.0.0.1:8000/api/snippets/

# ViewSet-based views - Tutorial 6 approach
USE_VIEWSET=1 cargo run --bin manage runserver
# Visit http://127.0.0.1:8000/api/snippets-viewset/
```

### Comparison

| Approach | Code Lines | Features |
|----------|------------|----------|
| Function-based (Tutorial 1-5) | ~200 lines | Full control, explicit implementation |
| ViewSet-based (Tutorial 6) | ~15 lines | CRUD automation, pagination, filtering, ordering |

### ViewSet Features

The ViewSet implementation provides:

- **Automatic CRUD operations**: list, create, retrieve, update, delete
- **Pagination**: `?page=1&page_size=10` (10 items per page, max 100)
- **Filtering**: `?language=rust&title=hello` (filter by language and title fields)
- **Ordering**: `?ordering=created_at,-title` (order by created_at ascending, title descending)

### Testing ViewSet Features

```bash
# List with pagination
curl "http://127.0.0.1:8000/api/snippets-viewset/?page=1&page_size=10"

# Filter by language
curl "http://127.0.0.1:8000/api/snippets-viewset/?language=rust"

# Order by created_at (descending)
curl "http://127.0.0.1:8000/api/snippets-viewset/?ordering=-created_at"

# Combine: Filter + Order + Paginate
curl "http://127.0.0.1:8000/api/snippets-viewset/?language=rust&ordering=-title&page=1&page_size=5"
```

### When to Use Each Approach

**Function-based views (Tutorial 1-5)**:
- Simple endpoints with custom logic
- Non-standard RESTful patterns
- When you need fine-grained control
- Learning HTTP handling basics

**ViewSet-based views (Tutorial 6)**:
- Standard RESTful CRUD APIs
- When pagination, filtering, and ordering are needed
- Rapid API development
- When code conciseness is important

## Next Steps

After understanding this example:

1. **Compare both approaches**: Try switching between function-based and ViewSet-based
2. **Understand the trade-offs**: When to use each approach (see above)
3. **Add custom actions**: Extend ViewSets with `#[action]` decorator for non-CRUD endpoints
4. **Add database integration**: Implement actual database storage instead of in-memory sample data
5. **Add authentication**: Implement JWT/Token/Session auth for both approaches
6. **Add permissions**: Implement permission classes to control access

## Related Documentation

- [REST Tutorial](../../../docs/tutorials/en/rest/) - Step-by-step guide
- [Feature Flags Guide](../../../docs/FEATURE_FLAGS.md) - Available features
- [Getting Started](../../../docs/GETTING_STARTED.md) - Quick start guide
- [API Documentation](https://docs.rs/reinhardt) - Complete API reference

## License

This example is part of the Reinhardt project and is licensed under the BSD 3-Clause License.
