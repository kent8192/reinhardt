# reinhardt-shortcuts

Django-style shortcut functions for the Reinhardt framework.

## Overview

Convenient shortcut functions for common HTTP operations, inspired by Django's `django.shortcuts` module. These functions provide simple, intuitive APIs for creating responses, redirects, and handling database queries with automatic 404 error handling.

## Installation

Add `reinhardt` to your `Cargo.toml`:

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", features = ["shortcuts"] }

# Or use a preset:
# reinhardt = { version = "0.1.0-alpha.1", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.1.0-alpha.1", features = ["full"] }      # All features
```

Then import shortcuts features:

```rust
use reinhardt::shortcuts::{redirect, render_json, get_or_404_response};
```

**Note:** Shortcuts features are included in the `standard` and `full` feature presets.

## Implemented ✓

### Redirect Shortcuts

#### `redirect()`

Create a temporary redirect (HTTP 302) response.

**Example:**

```rust
use reinhardt::shortcuts::redirect;

let response = redirect("/new-location");
// Returns HTTP 302 with Location: /new-location
```

#### `redirect_permanent()`

Create a permanent redirect (HTTP 301) response.

**Example:**

```rust
use reinhardt::shortcuts::redirect_permanent;

let response = redirect_permanent("/permanent-location");
// Returns HTTP 301 with Location: /permanent-location
```

### Response Rendering Shortcuts

#### `render_json()`

Render data as JSON and return an HTTP 200 response.

**Example:**

```rust
use reinhardt::shortcuts::render_json;
use serde_json::json;

let data = json!({
    "status": "success",
    "message": "Hello, world!"
});

let response = render_json(&data);
// Returns HTTP 200 with Content-Type: application/json
```

#### `render_json_pretty()`

Render data as pretty-printed JSON with indentation.

**Example:**

```rust
use reinhardt::shortcuts::render_json_pretty;
use serde_json::json;

let data = json!({"key": "value"});
let response = render_json_pretty(&data);
// Returns formatted JSON with newlines and indentation
```

#### `render_html()`

Render HTML content and return an HTTP 200 response.

**Example:**

```rust
use reinhardt::shortcuts::render_html;

let html = "<h1>Hello, World!</h1>";
let response = render_html(html);
// Returns HTTP 200 with Content-Type: text/html; charset=utf-8
```

#### `render_text()`

Render plain text content and return an HTTP 200 response.

**Example:**

```rust
use reinhardt::shortcuts::render_text;

let text = "Plain text content";
let response = render_text(text);
// Returns HTTP 200 with Content-Type: text/plain; charset=utf-8
```

### Database Shortcuts (404 Error Handling)

#### `get_or_404_response()`

Get a single object or return a 404 response if not found.

**Example:**

```rust
use reinhardt::shortcuts::get_or_404_response;

// Simulating database query result
let result = Ok(Some(user));

match get_or_404_response(result) {
    Ok(user) => {
        // User found, continue processing
    }
    Err(response) => {
        // Returns HTTP 404 response
        return response;
    }
}
```

#### `get_list_or_404_response()`

Get a list of objects or return a 404 response if the list is empty.

**Example:**

```rust
use reinhardt::shortcuts::get_list_or_404_response;

let result = Ok(vec![user1, user2]);

match get_list_or_404_response(result) {
    Ok(users) => {
        // Users found, continue processing
    }
    Err(response) => {
        // Returns HTTP 404 if list is empty
        return response;
    }
}
```

#### `exists_or_404_response()`

Check if a record exists or return a 404 response.

**Example:**

```rust
use reinhardt::shortcuts::exists_or_404_response;

let result = Ok(true);

match exists_or_404_response(result) {
    Ok(_) => {
        // Record exists, continue
    }
    Err(response) => {
        // Returns HTTP 404 if not found
        return response;
    }
}
```

### Error Types

#### `GetError`

Error type for database query operations.

**Variants:**

- `NotFound` - Object not found in database
- `MultipleObjectsReturned` - Query returned multiple objects when one was expected
- `DatabaseError(String)` - Database operation error

## Implemented with Feature Gates ✓

### ORM Integration (requires `database` feature)

#### `get_object_or_404<M>(pk: M::PrimaryKey) -> Result<M, Response>`

Direct database integration for single object retrieval. Queries the database using
the ORM and returns HTTP 404 if the object is not found.

**Example:**

```rust
use reinhardt::shortcuts::get_object_or_404;

async fn user_detail(user_id: i64) -> Result<Response, Response> {
    let user = get_object_or_404::<User>(user_id).await?;
    render_json(&user)
}
```

#### `get_list_or_404<M>(queryset: QuerySet<M>) -> Result<Vec<M>, Response>`

Direct database integration for list retrieval. Executes a QuerySet and returns HTTP 404
if the result list is empty.

**Example:**

```rust
use reinhardt::shortcuts::get_list_or_404;

async fn user_list(status: &str) -> Result<Response, Response> {
    let queryset = User::objects()
        .filter("status", FilterOperator::Eq, FilterValue::String(status.to_string()));

    let users = get_list_or_404(queryset).await?;
    render_json(&users)
}
```

### Server-Side Rendering (use `reinhardt-pages`)

For server-side rendering with components, use `reinhardt::pages` directly instead of template shortcuts.
reinhardt-pages provides a modern WASM-based component system with SSR support.

**Migration Note**: The `render_template` and `render_to_response` functions have been removed.
Use `reinhardt::pages::ssr::SsrRenderer` for component-based SSR instead.

#### Example: Component-Based SSR

```rust
use reinhardt::pages::prelude::*;
use reinhardt::pages::ssr::{SsrRenderer, SsrOptions};
use reinhardt::http::Response;

#[component]
fn IndexPage(title: String, user: String) -> impl IntoView {
    div()
        .child(h1().text(title))
        .child(p().text(format!("Welcome, {}!", user)))
}

async fn index_view(request: Request) -> Result<Response, Response> {
    let renderer = SsrRenderer::new(SsrOptions {
        include_hydration_markers: true,
        serialize_state: true,
        ..Default::default()
    });

    let html = renderer.render_page(IndexPage {
        title: "Welcome".to_string(),
        user: request.user().name().to_string(),
    })?;

    Ok(Response::html(html))
}
```

**Key Differences from Old Template System:**

| Feature | Old (Tera Template System) | New (reinhardt-pages) |
|---------|----------------|----------------------|
| Rendering | Server-side only | SSR + Client hydration |
| Syntax | Template files (`.html`) | Rust components |
| Type Safety | Runtime errors | Compile-time checking |
| Interactivity | None | Built-in with WASM |

See [`reinhardt-pages` documentation](../../docs/api/README.md#reinhardt-pages) for more details.

## Usage Patterns

### Combining Shortcuts

```rust
use reinhardt::shortcuts::{render_json, get_or_404_response};

async fn get_user_handler(user_id: i64) -> Response {
    let result = database::find_user(user_id).await;

    match get_or_404_response(result) {
        Ok(user) => render_json(&user),
        Err(not_found_response) => not_found_response,
    }
}
```

### Redirect After Action

```rust
use reinhardt::shortcuts::{redirect, redirect_permanent};

async fn create_user_handler(user_data: UserData) -> Response {
    database::create_user(user_data).await;
    redirect("/users/")
}

async fn old_url_handler() -> Response {
    redirect_permanent("/new-url/")
}
```

## Dependencies

- `reinhardt-http` - HTTP types (Request, Response)
- `serde` - Serialization support
- `serde_json` - JSON rendering
- `bytes` - Efficient byte buffer handling
- `thiserror` - Error type definitions

## Related Crates

- `reinhardt-http` - HTTP primitives
- `reinhardt-views` - View layer
- `reinhardt-orm` - Database ORM (for future direct integration)

## License

Licensed under the BSD 3-Clause License.
