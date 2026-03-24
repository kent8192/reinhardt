+++
title = "Tutorial 5: Relationships and Hyperlinked APIs"
weight = 60

[extra]
sidebar_weight = 70
+++

# Tutorial 5: Relationships and Hyperlinked APIs

Create APIs with relationships between resources and use hyperlinks for navigation.

## URL Reverse Routing

Generate URLs for named routes using the router's `reverse()` method:

```rust
use reinhardt::prelude::*;

let router = ServerRouter::new();

// Generate URL from route name
let url = router.reverse("snippet-detail", &[("id", "42")]);
// Returns Option<String>
// url: Some("/snippets/42/")
```

## Hyperlinked Relations

Use hyperlinked fields in serializers to reference related resources:

```rust
use reinhardt::prelude::*;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnippetSerializer {
    pub id: i64,
    pub title: String,
    pub code: String,
    pub owner: String,
    pub owner_url: String,  // Hyperlink to owner resource
}

// Generate hyperlink
let owner_url = router.reverse("user-detail", &[("id", &owner_id.to_string())]);
```

## HyperlinkedModelSerializer

Create serializers with automatic hyperlink generation:

```rust
use reinhardt::prelude::*;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    id: i64,
    username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Snippet {
    id: i64,
    title: String,
    code: String,
    owner: User,
}

// Hyperlinked serializer representation
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnippetHyperlinked {
    url: String,           // Self URL
    id: i64,
    title: String,
    code: String,
    owner: String,         // Owner URL
}
```

## Related Object Serialization

Include related objects in responses:

```rust
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserSerializer {
    id: i64,
    username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnippetWithUser {
    id: i64,
    title: String,
    code: String,
    owner: UserSerializer,  // Nested user object
}
```

## URL Patterns

Define URL patterns with route names:

```rust
use reinhardt::prelude::*;

let mut router = ServerRouter::new();

// Register ViewSets with route names
let snippet_viewset = ModelViewSet::<Snippet, SnippetSerializer>::new("snippet");
let user_viewset = ModelViewSet::<User, UserSerializer>::new("user");

router.register_viewset("snippets", snippet_viewset);
router.register_viewset("users", user_viewset);

// Generated routes with names:
// snippet-list:   GET/POST /snippets/
// snippet-detail: GET/PUT/PATCH/DELETE /snippets/{id}/
// user-list:      GET/POST /users/
// user-detail:    GET/PUT/PATCH/DELETE /users/{id}/
```

## Building Hyperlinked Responses

Create responses with hyperlinks:

```rust
use reinhardt::prelude::*;

async fn snippet_detail(
    request: Request,
    router: &ServerRouter,
    id: i64
) -> ViewResult<Response> {
    // Get snippet from database
    let snippet = get_snippet(id).await?;

    // Generate hyperlinks
    let self_url = router.reverse("snippet-detail", &[("id", &snippet.id.to_string())]);
    let owner_url = router.reverse("user-detail", &[("id", &snippet.owner_id.to_string())]);

    // Build response with hyperlinks
    let response_data = SnippetHyperlinked {
        url: self_url.unwrap_or_default(),
        id: snippet.id,
        title: snippet.title,
        code: snippet.code,
        owner: owner_url.unwrap_or_default(),
    };

    Ok(Response::ok(response_data))
}
```

## Many-to-Many Relations

Handle many-to-many relationships:

```rust
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Tag {
    id: i64,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnippetWithTags {
    id: i64,
    title: String,
    code: String,
    tags: Vec<String>,  // List of tag URLs
}

// Generate tag URLs
let tag_urls: Vec<String> = snippet.tags
    .iter()
    .filter_map(|tag| {
        router.reverse("tag-detail", &[("id", &tag.id.to_string())])
    })
    .collect();
```

## Complete Example

Full hyperlinked API implementation:

```rust
use reinhardt::prelude::*;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    id: i64,
    username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Snippet {
    id: i64,
    title: String,
    code: String,
    owner_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserSerializer {
    url: String,
    id: i64,
    username: String,
    snippets: Vec<String>,  // URLs to user's snippets
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnippetSerializer {
    url: String,
    id: i64,
    title: String,
    code: String,
    owner: String,  // URL to owner
}

// In config/urls.rs
pub fn url_patterns() -> ServerRouter {
    let mut router = ServerRouter::new();

    // Register ViewSets
    let snippet_viewset = ModelViewSet::<Snippet, SnippetSerializer>::new("snippet");
    let user_viewset = ModelViewSet::<User, UserSerializer>::new("user");

    router.register_viewset("snippets", snippet_viewset);
    router.register_viewset("users", user_viewset);

    router.into()
}

// Example: Generate snippet URL via router.reverse()
// let snippet_url = router.reverse("snippet-detail", &[("id", "1")]);
// => Some("/api/snippets/1/")
//
// let user_url = router.reverse("user-detail", &[("id", "42")]);
// => Some("/api/users/42/")
```

## Benefits of Hyperlinked APIs

1. **Discoverability**: Clients can navigate the API by following links
2. **Decoupling**: URLs can change without breaking clients
3. **HATEOAS**: Hypermedia as the Engine of Application State
4. **Self-documenting**: Relationships are explicit in responses

## Summary

In this tutorial, you learned:

1. URL reverse routing with `router.reverse()`
2. Creating hyperlinked relations between resources
3. Using `HyperlinkedModelSerializer`
4. Including related objects in responses
5. Defining URL patterns with route names
6. Building hyperlinked API responses
7. Handling many-to-many relationships
8. Benefits of hyperlinked APIs

Next tutorial: [Tutorial 6: ViewSets and Routers](../6-viewsets-and-routers/)
