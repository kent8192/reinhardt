+++
title = "Tutorial 3: Class-Based Views"
weight = 40

[extra]
sidebar_weight = 50
+++

# Tutorial 3: Class-Based Views

Use struct-based generic views instead of writing function-based views.

## Using Generic Views

Reinhardt provides generic views for common REST patterns.

### ListAPIView

View for displaying a list of objects:

```rust
use reinhardt::prelude::*;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Snippet {
    id: i64,
    code: String,
    language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnippetSerializer {
    id: i64,
    code: String,
    language: String,
}

let snippets = vec![
    Snippet { id: 1, code: "print('hello')".to_string(), language: "python".to_string() },
];

let view = ListAPIView::<Snippet, SnippetSerializer>::new()
    .with_objects(snippets)
    .with_paginate_by(10);
```

## Available Generic Views

Reinhardt provides the following generic views:

### Single Operation Views

- `ListAPIView` - Display list of objects (GET)
- `CreateAPIView` - Create object (POST)
- `RetrieveAPIView` - Retrieve single object (GET)
- `UpdateAPIView` - Update object (PUT/PATCH)
- `DestroyAPIView` - Delete object (DELETE)

### Combined Operation Views

- `ListCreateAPIView` - List and create (GET, POST)
- `RetrieveUpdateAPIView` - Retrieve and update (GET, PUT, PATCH)
- `RetrieveDestroyAPIView` - Retrieve and delete (GET, DELETE)
- `RetrieveUpdateDestroyAPIView` - Retrieve, update, delete (GET, PUT, PATCH, DELETE)

## Moving to ViewSets

ViewSets provide a powerful way to build RESTful APIs with significantly less code. They combine multiple CRUD actions into a single struct:

```rust
use reinhardt::prelude::*;

// ModelViewSet automatically provides all CRUD operations
let viewset = ModelViewSet::<Snippet, SnippetSerializer>::new("snippet");

// ReadOnlyModelViewSet for read-only endpoints
let readonly_viewset = ReadOnlyModelViewSet::<Snippet, SnippetSerializer>::new("snippet");
```

**ViewSets vs Generic Views:**

| Feature | Generic Views | ViewSets |
|---------|--------------|----------|
| **Code Amount** | ~200 lines for full CRUD | ~15 lines for full CRUD |
| **Automatic URL Generation** | Manual configuration | Automatic via routers |
| **Built-in Features** | Basic operations | CRUD + pagination + filtering + ordering |
| **Best For** | Custom logic, simple endpoints | Standard RESTful APIs |

**Available ViewSets:**
- ✅ `ModelViewSet` - Full CRUD operations (list, create, retrieve, update, delete)
- ✅ `ReadOnlyModelViewSet` - Read-only operations (list, retrieve)
- ✅ `GenericViewSet` - Base class for custom ViewSets

For detailed examples and advanced usage, see [Tutorial 6: ViewSets and Routers](../6-viewsets-and-routers/).

## Summary

In this tutorial, you learned:

1. How to use generic views
2. Differences between single and combined operation views
3. Moving to ViewSets

Next tutorial: [Tutorial 4: Authentication and Permissions](../4-authentication-and-permissions/)
