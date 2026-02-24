+++
title = "Pagination"
weight = 40
+++

# Pagination

Guide to paginating large datasets.

## Table of Contents

- [Page Number-Based Pagination](#page-number-based-pagination)
- [Limit/Offset-Based Pagination](#limitoffset-based-pagination)
- [Cursor-Based Pagination](#cursor-based-pagination)
- [Pagination Metadata](#pagination-metadata)
- [Best Practices](#best-practices)

---

## Page Number-Based Pagination

### `PageNumberPagination`

Traditional page number-based pagination.

```rust
use reinhardt::pagination::{PageNumberPagination, Page};
use reinhardt::{Query, Path};

#[derive(serde::Deserialize)]
struct PageQuery {
    page: Option<usize>,
    page_size: Option<usize>,
}

async fn list_users(
    Query(query): Query<PageQuery>,
) -> reinhardt::Response {
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(10);

    let pagination = PageNumberPagination::new(page, page_size);

    // Fetch data from database
    let total_count = fetch_user_count().await;
    let users = fetch_users_page(page, page_size).await;

    let page_data = Page::new(users, page, total_count, page_size);
    let num_pages = (total_count + page_size - 1) / page_size;

    let metadata = PaginationMetadata {
        count: total_count,
        next: if page < num_pages {
            Some(format!("/api/users?page={}", page + 1))
        } else {
            None
        },
        previous: if page > 1 {
            Some(format!("/api/users?page={}", page - 1))
        } else {
            None
        },
    };

    let response = PaginatedResponse::new(users, metadata);
    reinhardt::Response::ok().with_json(&response).unwrap()
}
```

### Page Number Validation

```rust
use reinhardt::pagination::PageNumberPagination;

let pagination = PageNumberPagination::new(page, page_size)
    .with_max_page_size(100); // Maximum 100 items

if let Err(e) = pagination.validate() {
    return reinhardt::Response::bad_request()
        .with_json(&serde_json::json!({
            "error": "Invalid page parameters",
            "details": e.to_string()
        }))
        .unwrap();
}
```

---

## Limit/Offset-Based Pagination

### `LimitOffsetPagination`

Flexible limit/offset-based pagination.

```rust
use reinhardt::pagination::{LimitOffsetPagination, PaginatedResponse};

#[derive(serde::Deserialize)]
struct OffsetQuery {
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn list_items(
    Query(query): Query<OffsetQuery>,
) -> reinhardt::Response {
    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);

    let pagination = LimitOffsetPagination::new(limit, offset);

    let total_count = fetch_item_count().await;
    let items = fetch_items_limit_offset(limit, offset).await;

    let metadata = PaginationMetadata {
        count: total_count,
        next: if offset + limit < total_count {
            Some(format!("/api/items?limit={}&offset={}", limit, offset + limit))
        } else {
            None
        },
        previous: if offset > 0 {
            let prev_offset = (offset - limit).max(0);
            Some(format!("/api/items?limit={}&offset={}", limit, prev_offset))
        } else {
            None
        },
    };

    let response = PaginatedResponse::new(items, metadata);
    reinhardt::Response::ok().with_json(&response).unwrap()
}
```

---

## Cursor-Based Pagination

### `CursorPagination`

Best for infinite scroll or real-time feeds.

```rust
use reinhardt::pagination::{CursorPagination, Cursor};

#[derive(serde::Deserialize, Serialize)]
struct ItemCursor {
    id: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

async fn feed(
    Query(query): Query<serde_json::Value>,
) -> reinhardt::Response {
    let cursor = query.get("cursor")
        .and_then(|v| v.as_str())
        .map(|s| Cursor::new(s.to_string()));

    let limit = query.get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(20) as usize;

    let pagination = CursorPagination::new(limit, cursor);

    let items = fetch_items_cursor(pagination).await;
    let next_cursor = items.last().map(|item: &Item| item.id.clone());
    let has_more = items.len() == limit;

    let response = serde_json::json!({
        "results": items,
        "next_cursor": next_cursor,
        "has_more": has_more
    });

    reinhardt::Response::ok().with_json(&response).unwrap()
}
```

### Encoded Cursors

```rust
use reinhardt::pagination::{Cursor, CursorPagination};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

// Encode cursor
let cursor_data = format!("{}|{}", item_id, timestamp);
let encoded = URL_SAFE_NO_PAD.encode(cursor_data);

// Decode cursor
let decoded = URL_SAFE_NO_PAD.decode(encoded).unwrap();
let cursor_data = String::from_utf8(decoded).unwrap();
```

---

## Pagination Metadata

### `PaginatedResponse`

Standard paginated response format.

```rust
use reinhardt::pagination::{PaginatedResponse, PaginationMetadata};
use serde::Serialize;

#[derive(Serialize)]
struct User {
    id: u32,
    username: String,
}

let response = PaginatedResponse {
    count: 100,
    next: Some("/api/users?page=2".to_string()),
    previous: None,
    results: users,
};

// JSON output:
// {
//   "count": 100,
//   "next": "/api/users?page=2",
//   "previous": null,
//   "results": [...]
// }
```

### `Page` Struct

Contains detailed page information.

```rust
use reinhardt::pagination::Page;

let page = Page::new(
    items,
    2,      // Current page number
    10,     // Total pages
    100,    // Total items
    10,     // Page size
);

// Available information:
// - page.number: Current page number
// - page.num_pages: Total number of pages
// - page.count: Total items
// - page.start_index(): First item index
// - page.end_index(): Last item index
// - page.has_next(): Whether next page exists
// - page.has_previous(): Whether previous page exists
```

---

## Best Practices

### Default Page Size

```rust
const DEFAULT_PAGE_SIZE: usize = 20;
const MAX_PAGE_SIZE: usize = 100;

let page_size = query.page_size
    .unwrap_or(DEFAULT_PAGE_SIZE)
    .min(MAX_PAGE_SIZE);
```

### Caching Total Count

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

let count_cache = Arc::new(RwLock::new(HashMap::new()));

async fn get_count_with_cache(key: &str) -> usize {
    // Check cache
    {
        let cache = count_cache.read().await;
        if let Some(&count) = cache.get(key) {
            return count;
        }
    }

    // Fetch from database
    let count = fetch_count_from_db().await;

    // Store in cache
    let mut cache = count_cache.write().await;
    cache.insert(key.to_string(), count);

    count
}
```

### Efficient Queries

```rust
use reinhardt::query::prelude::{Query, Postgres};

// Efficient pagination query (PostgreSQL)
let (limit, offset) = (page_size, (page - 1) * page_size);

let query = Query::select()
    .columns([
        User::Id,
        User::Username,
        User::Email,
    ])
    .from(User::Table)
    .order_by(User::CreatedAt, Order::Desc)
    .limit(limit as u64)
    .offset(offset as u64)
    .to_owned();

// Separate query for total count
let count_query = Query::select()
    .expr(Func::count(Expr::col(User::Id)))
    .from(User::Table)
    .to_owned();
```

---

## See Also

- [Request API](https://docs.rs/reinhardt-http/latest/reinhardt_http/struct.Request.html)
- [Response API](https://docs.rs/reinhardt-http/latest/reinhardt_http/struct.Response.html)
- [Response Serialization](../response-serialization/)
