# Advanced ViewSet Example

This example demonstrates advanced ViewSet features beyond basic CRUD operations.

## Apps

### Authors (`/api/authors/`)
- **GenericViewSet** with composable Mixins (List + Retrieve only)
- **Custom Actions**: `POST /authors/{id}/activate/`, `GET /authors/recent/`

### Books (`/api/books/`)
- **ReadOnlyModelViewSet** (list and retrieve only)
- **Caching** with `CacheConfig` (TTL: 5 minutes)
- Write operations return 405 Method Not Allowed

### Articles (`/api/articles/`)
- **ModelViewSet** with full CRUD
- **Batch Operations**: `POST /articles/bulk/`
- **Nested Resources**: `GET /authors/{author_id}/articles/`
- **Partial Update (PATCH)**: Update specific fields only
- **Middleware**: Authentication and permission examples
- **Dependency Injection**: Database connection injection

## Running

```bash
cd examples/examples-viewset-advanced
cargo run
```

## API Endpoints

| Method | URL | Description |
|--------|-----|-------------|
| GET | `/api/authors/` | List authors |
| GET | `/api/authors/{id}/` | Get author |
| POST | `/api/authors/{id}/activate/` | Activate author (custom action) |
| GET | `/api/authors/recent/` | Recent authors (custom action) |
| GET | `/api/books/` | List books (cached) |
| GET | `/api/books/{id}/` | Get book (cached) |
| POST | `/api/books/` | 405 Method Not Allowed |
| GET | `/api/articles/` | List articles |
| GET | `/api/articles/{id}/` | Get article |
| POST | `/api/articles/` | Create article |
| PUT | `/api/articles/{id}/` | Full update |
| PATCH | `/api/articles/{id}/` | Partial update |
| DELETE | `/api/articles/{id}/` | Delete article |
| POST | `/api/articles/bulk/` | Batch create |
| GET | `/api/authors/{id}/articles/` | Nested articles |
