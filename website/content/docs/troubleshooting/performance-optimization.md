+++
title = "Performance Optimization"
weight = 20
+++

# Performance Optimization

Guide to optimizing Reinhardt application performance.

## Table of Contents

- [Routing Performance](#routing-performance)
- [Database Query Optimization](#database-query-optimization)
- [Cache Strategies](#cache-strategies)
- [Compression Settings](#compression-settings)

---

## Routing Performance

### matchit Radix Tree

Reinhardt uses matchit Radix Tree for **O(m)** route matching (m = path length).

```rust
// Fast matching even with 1000+ routes
let router = ServerRouter::new()
    .function("/api/resources/{id}", Method::GET, handler)
    .function("/api/resources/{id}/posts", Method::GET, handler)
    .function("/api/posts/{post_id}/comments/{comment_id}", Method::GET, handler);
```

### Route Registration Best Practices

```rust
use reinhardt::ServerRouter;

// ✅ Use hierarchical routing
let api_router = ServerRouter::new()
    .with_prefix("/api/v1")
    .function("/users", Method::GET, list_users);

let router = ServerRouter::new()
    .mount("/api/", api_router);

// ❌ Register all routes flat (inefficient)
let router = ServerRouter::new()
    .function("/api/v1/users", Method::GET, list_users)
    .function("/api/v1/posts", Method::GET, list_posts);
```

### Lazy Compilation

Routes are compiled on first access, so there's no startup overhead.

```rust
let router = ServerRouter::new()
    .function("/api/users", Method::GET, users_handler);

// Routes are compiled on first access
// Can explicitly compile with register_all_routes()
router.register_all_routes();
```

---

## Database Query Optimization

### Efficient Queries with reinhardt-query

```rust
use reinhardt::query::prelude::{Query, Postgres, Expr, Func, Order};

// ✅ Query using indexes
let (limit, offset) = (page_size, (page - 1) * page_size);

let query = Query::select()
    .columns([User::Id, User::Username, User::Email])
    .from(User::Table)
    .and_where(Expr::col(User::IsActive).eq(true))
    .order_by(User::CreatedAt, Order::Desc)  // Sort by indexed column
    .limit(limit as u64)
    .offset(offset as u64)
    .to_owned();

// ✅ Count query is separate
let count_query = Query::select()
    .expr(Func::count(Expr::col(User::Id)))
    .from(User::Table)
    .and_where(Expr::col(User::IsActive).eq(true))
    .to_owned();
```

### Avoid N+1 Problem

```rust
use reinhardt::QuerySet;

// ❌ N+1 problem
let posts = Post::objects().all().await?;
for post in posts {
    let user = post.user().await?;  // Query in each loop
}

// ✅ Use preload
let posts = Post::objects()
    .preload("user")
    .all()
    .await?;
```

---

## Cache Strategies

### HTTP Caching

HTTP caching using `CacheControlMiddleware`.

```rust
use reinhardt::staticfiles::{CacheControlMiddleware, CacheControlConfig};

let config = CacheControlConfig {
    public: true,
    max_age: 3600,        // 1 hour
    s_maxage: Some(86400), // 1 day on CDN
    immutable: true,       // Versioned files
};

let router = ServerRouter::new()
    .with_middleware(CacheControlMiddleware::new(config));
```

### Cache Directives

```rust
use reinhardt::staticfiles::CacheDirective;

// Versioned files (cache for 1 year)
let immutable = CacheDirective::public()
    .with_max_age(31536000)
    .with_immutable(true);

// API responses (cache for 5 minutes)
let api_cache = CacheDirective::public()
    .with_max_age(300)
    .with_private(true);  // User-specific data

// HTML files (don't cache)
let no_cache = CacheDirective::no_cache();
```

### ETag Conditional Requests

```rust
use reinhardt::ETagMiddleware;

let router = ServerRouter::new()
    .with_middleware(ETagMiddleware::new(ETagConfig::default()));

// When client sends If-None-Match header,
// returns 304 Not Modified if unchanged
```

---

## Compression Settings

### GZip Compression

```rust
use reinhardt::{GZipMiddleware, GZipConfig};

let config = GZipConfig {
    level: 6,  // Compression level (1-9, default 6)
    min_size: 1024,  // Minimum file size to compress
};

let router = ServerRouter::new()
    .with_middleware(GZipMiddleware::new(config));
```

### Brotli Compression

```rust
use reinhardt::{BrotliMiddleware, BrotliConfig};

let config = BrotliConfig {
    quality: BrotliQuality::Medium,  // Compression quality
    min_size: 1024,
};

let router = ServerRouter::new()
    .with_middleware(BrotliMiddleware::new(config));
```

### Choosing Compression Algorithm

| Format | Compression | Speed | Use Case |
|--------|-------------|-------|----------|
| GZip | Medium | Fast | General text, JSON, HTML |
| Brotli | High | Medium | JavaScript, CSS, large static files |

### Compression Best Practices

```rust
// ✅ Provide both Brotli and GZip (browser chooses)
let router = ServerRouter::new()
    .with_middleware(BrotliMiddleware::new(brotli_config))
    .with_middleware(GZipMiddleware::new(gzip_config));

// Compressed response:
// Content-Encoding: br, gzip
```

---

## Profiling

### Measuring Request Time

```rust
use reinhardt::{MetricsMiddleware, MetricsConfig};
use std::time::Instant;

async fn timed_handler(req: Request) -> Response {
    let start = Instant::now();

    // Handler processing
    let response = process_request(req).await;

    let duration = start.elapsed();
    println!("Request took {:?}", duration);

    response
}
```

### Metrics Collection

```rust
use reinhardt::{MetricsMiddleware, MetricsStore};

let store = MetricsStore::new();
let middleware = MetricsMiddleware::new(MetricsConfig::default());

let router = ServerRouter::new()
    .with_middleware(middleware);
```

---

## See Also

- [Router API](https://docs.rs/reinhardt-urls/latest/reinhardt_urls/routers/struct.ServerRouter.html)
- [Serving Static Files](/docs/cookbook/static-files/)
- [Middleware Creation](/docs/cookbook/middleware-creation/)
