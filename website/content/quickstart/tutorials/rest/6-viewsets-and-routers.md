+++
title = "Tutorial 6: ViewSets and Routers"
weight = 70

[extra]
sidebar_weight = 80
+++

# Tutorial 6: ViewSets and Routers

Use ViewSets and Routers to reduce the amount of code needed to build your API.

## Using ViewSets

ViewSets allow you to implement common RESTful API patterns concisely.

### ModelViewSet

Provides full CRUD operations:

```rust
use reinhardt::prelude::*;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[model(app_label = "snippets", table_name = "snippets")]
struct Snippet {
    #[field(primary_key = true)]
    id: i64,

    #[field(max_length = 10000)]
    code: String,

    #[field(max_length = 50)]
    language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnippetSerializer {
    id: i64,
    code: String,
    language: String,
}

// Create ViewSet
let snippet_viewset = ModelViewSet::<Snippet, SnippetSerializer>::new("snippet");
```

### ReadOnlyModelViewSet

Provides read-only operations:

```rust
use reinhardt::prelude::*;

let snippet_viewset = ReadOnlyModelViewSet::<Snippet, SnippetSerializer>::new("snippet");
```

## Using Routers

Register ViewSets with routers to automatically generate URLs.

Define your ViewSet registrations in `urls.rs`:

```rust
// src/config/urls.rs
use reinhardt::prelude::*;
use reinhardt::routes;
use std::sync::Arc;

#[routes]
pub fn routes() -> UnifiedRouter {
    let snippet_viewset = ModelViewSet::<Snippet, SnippetSerializer>::new("snippet");
    let user_viewset = ReadOnlyModelViewSet::<User, UserSerializer>::new("user");

    UnifiedRouter::new()
        .register_viewset("/snippets", Arc::new(snippet_viewset))
        .register_viewset("/users", Arc::new(user_viewset))
}

// URLs are automatically generated:
// GET/POST    /snippets/           - List/create
// GET/PUT/PATCH/DELETE /snippets/{id}/ - Detail/update/delete
// GET         /users/              - List
// GET         /users/{id}/         - Detail
```

## Automatic URL Generation

Routers automatically generate URL patterns from ViewSets:

| HTTP Method | URL Pattern     | ViewSet Action | Description              |
| ----------- | --------------- | -------------- | ------------------------ |
| GET         | /{prefix}/      | list           | List of objects          |
| POST        | /{prefix}/      | create         | Create new object        |
| GET         | /{prefix}/{id}/ | retrieve       | Retrieve specific object |
| PUT         | /{prefix}/{id}/ | update         | Update object            |
| PATCH       | /{prefix}/{id}/ | partial_update | Partial update           |
| DELETE      | /{prefix}/{id}/ | destroy        | Delete object            |

## ViewSet Benefits

1. **Less Code**: CRUD operations are automatically implemented
2. **Consistency**: Follows standard REST patterns
3. **Maintainability**: Focus on business logic
4. **Automatic URL Generation**: No routing configuration needed

## Views vs ViewSets

### Use Views When:

- Building simple endpoints
- Lots of custom logic required
- Not following standard CRUD patterns

### Use ViewSets When:

- Building standard RESTful APIs
- Multiple endpoints needed (list, detail, etc.)
- Code conciseness is important

## Complete Example

Define models and serializers:

```rust
use reinhardt::prelude::*;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Snippet {
    id: i64,
    title: String,
    code: String,
    language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnippetSerializer {
    id: i64,
    title: String,
    code: String,
    language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    id: i64,
    username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserSerializer {
    id: i64,
    username: String,
}
```

Register ViewSets in `urls.rs`:

```rust
// src/config/urls.rs
use reinhardt::prelude::*;
use reinhardt::routes;
use std::sync::Arc;

#[routes]
pub fn routes() -> UnifiedRouter {
    let snippet_viewset = ModelViewSet::<Snippet, SnippetSerializer>::new("snippet");
    let user_viewset = ReadOnlyModelViewSet::<User, UserSerializer>::new("user");

    UnifiedRouter::new()
        .register_viewset("/snippets", Arc::new(snippet_viewset))
        .register_viewset("/users", Arc::new(user_viewset))
}

// API endpoints:
//   GET/POST    /snippets/
//   GET/PUT/PATCH/DELETE /snippets/{id}/
//   GET         /users/
//   GET         /users/{id}/
```

Start the development server:

```bash
cargo make runserver
```

## Code Comparison: Function-based vs ViewSet

### Function-based Views (Tutorial 1-5)

```rust
// ~200 lines of code for full CRUD operations
use reinhardt::{get, post, put, delete, Json, Path, Response, StatusCode, ViewResult};
use validator::Validate;

#[get("/snippets/", name = "snippets_list")]
pub async fn list() -> ViewResult<Response> {
    // Manual implementation of list logic
    // - Fetch all snippets
    // - Format response
    // Total: ~20 lines
}

#[post("/snippets/", name = "snippets_create")]
pub async fn create(Json(serializer): Json<SnippetSerializer>) -> ViewResult<Response> {
    serializer.validate()?;
    // Manual implementation of create logic
    // - Validate input
    // - Create snippet
    // - Return response
    // Total: ~30 lines
}

#[get("/snippets/{id}/", name = "snippets_retrieve")]
pub async fn retrieve(Path(snippet_id): Path<i64>) -> ViewResult<Response> {
    // Manual implementation of retrieve logic
    // Total: ~20 lines
}

#[put("/snippets/{id}/", name = "snippets_update")]
pub async fn update(Path(snippet_id): Path<i64>, Json(serializer): Json<SnippetSerializer>) -> ViewResult<Response> {
    serializer.validate()?;
    // Manual implementation of update logic
    // Total: ~40 lines
}

#[delete("/snippets/{id}/", name = "snippets_delete")]
pub async fn delete(Path(snippet_id): Path<i64>) -> ViewResult<Response> {
    // Manual implementation of delete logic
    // Total: ~15 lines
}

// URL registration in urls.rs
UnifiedRouter::new()
    .endpoint(views::list)
    .endpoint(views::create)
    .endpoint(views::retrieve)
    .endpoint(views::update)
    .endpoint(views::delete)
```

**Total**: ~200 lines for basic CRUD (no pagination, filtering, or ordering)

### ViewSet-based (Tutorial 6)

```rust
// ~15 lines for the same functionality PLUS pagination, filtering, and ordering!
use reinhardt::viewsets::{ModelViewSet, PaginationConfig, FilterConfig, OrderingConfig};

pub struct SnippetViewSet;

impl SnippetViewSet {
    pub fn new() -> ModelViewSet<Snippet, SnippetSerializer> {
        ModelViewSet::new("snippet")
            .with_pagination(PaginationConfig::page_number(10, Some(100)))
            .with_filters(FilterConfig::new()
                .with_filterable_fields(vec!["language".to_string(), "title".to_string()]))
            .with_ordering(OrderingConfig::new()
                .with_ordering_fields(vec!["created_at".to_string(), "title".to_string()]))
    }
}

// URL registration in urls.rs
UnifiedRouter::new().register_viewset("/snippets-viewset", Arc::new(SnippetViewSet::new()))
```

**Total**: ~15 lines for full CRUD + pagination + filtering + ordering

**Result**: **~13x less code** with ViewSets while providing **more features**!

## Try it Yourself

The complete working example is available in `examples-tutorial-rest`:
- [examples-tutorial-rest](../../../examples/examples-tutorial-rest/)

### Running the Example

```bash
cd examples/examples-tutorial-rest

# Option 1: Function-based views (Tutorial 1-5)
cargo make runserver
# Visit http://127.0.0.1:8000/api/snippets/

# Option 2: ViewSet-based views (Tutorial 6)
USE_VIEWSET=1 cargo make runserver
# Visit http://127.0.0.1:8000/api/snippets-viewset/
```

### Testing the ViewSet Features

```bash
# List with pagination
curl "http://127.0.0.1:8000/api/snippets-viewset/?page=1&page_size=10"

# Filter by language
curl "http://127.0.0.1:8000/api/snippets-viewset/?language=rust"

# Order by created_at (descending)
curl "http://127.0.0.1:8000/api/snippets-viewset/?ordering=-created_at"

# Combine: Filter + Order + Paginate
curl "http://127.0.0.1:8000/api/snippets-viewset/?language=rust&ordering=-title&page=1&page_size=5"

# Create a new snippet
curl -X POST http://127.0.0.1:8000/api/snippets-viewset/ \
  -H "Content-Type: application/json" \
  -d '{"title":"Test ViewSet","code":"fn main() {}","language":"rust"}'

# Retrieve a specific snippet
curl http://127.0.0.1:8000/api/snippets-viewset/1/

# Update a snippet
curl -X PUT http://127.0.0.1:8000/api/snippets-viewset/1/ \
  -H "Content-Type: application/json" \
  -d '{"title":"Updated","code":"fn main() { println!(\"Hello!\"); }","language":"rust"}'

# Delete a snippet
curl -X DELETE http://127.0.0.1:8000/api/snippets-viewset/1/
```

## Summary

Throughout this tutorial series, you learned:

1. **Serialization** - Data serialization and validation
2. **Requests and Responses** - HTTP handling basics
3. **Class-Based Views** - Using generic views
4. **Authentication and Permissions** - API protection
5. **Hyperlinked APIs** - URL reverse routing and relationships
6. **ViewSets and Routers** - Efficient API building

You can now build production-ready RESTful APIs with this knowledge!

## Next Steps

- For more advanced topics, see the [API Reference](/docs/api/)
- Learn about Dependency Injection (documentation coming soon)
- Check out the [Feature Flags Guide](/docs/feature-flags/) for customization
- Try the working example in [examples-tutorial-rest](https://github.com/kent8192/reinhardt-web/tree/main/examples/examples-tutorial-rest)
- Join the community to ask questions
