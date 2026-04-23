# reinhardt-openapi

OpenAPI router wrapper for Reinhardt framework.

## Overview

This crate provides a router wrapper that automatically adds OpenAPI documentation endpoints to any handler. The wrapper intercepts requests to documentation paths and serves them from memory, delegating all other requests to the wrapped handler.

## Features

- **Zero-copy documentation serving**: OpenAPI schema is generated once at wrap time
- **Swagger UI**: Interactive API documentation at `/api/docs`
- **Redoc UI**: Alternative documentation view at `/api/redoc`
- **OpenAPI JSON**: Raw specification at `/api/openapi.json`

## Usage

Apply `OpenApiRouter::wrap` inside the `#[routes]` function:

```rust
use reinhardt::routes;
use reinhardt_openapi::OpenApiRouter;
use reinhardt_urls::routers::UnifiedRouter;

#[routes]
pub fn routes() -> OpenApiRouter<UnifiedRouter> {
    let router = UnifiedRouter::new()
        // Mount your application routes here
        .mount("/", some_app::urls::routes());

    // Wrap with OpenAPI endpoints — served at:
    // - /api/openapi.json (OpenAPI spec)
    // - /api/docs (Swagger UI)
    // - /api/redoc (Redoc UI)
    OpenApiRouter::wrap(router).expect("Failed to wrap router with OpenAPI")
}
```

## Why a Separate Crate?

This crate exists to break a circular dependency chain in the Reinhardt framework:

```
reinhardt-urls → reinhardt-views → reinhardt-rest → reinhardt-urls (cycle!)
```

By placing `OpenApiRouter` in its own crate that depends on both `reinhardt-urls` and `reinhardt-rest`, we avoid this cycle:

```
reinhardt-openapi
    ├── reinhardt-urls (Route, Router trait)
    └── reinhardt-rest (generate_openapi_schema, SwaggerUI, RedocUI)
```

## License

Licensed under the BSD 3-Clause License.
