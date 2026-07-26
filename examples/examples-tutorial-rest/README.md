# Reinhardt REST Tutorial Example - Code Snippet Management API

This example demonstrates the concepts covered in the [Reinhardt REST Tutorial](../../website/content/quickstart/tutorials/rest/). It implements a complete RESTful API for managing code snippets.

## What This Example Covers

This example corresponds to the REST tutorial Parts 1-6:

- **Part 1: Project Setup** - Project structure, settings, the `manage` CLI, the development server
- **Part 2: Your First Endpoints** - `#[get]` / `#[post]` / `#[put]` / `#[delete]`, route `name = "..."`, `Path` / `Query` / `Json` extractors, `ViewResult`
- **Part 3: Models and the Database** - `#[model]` `Snippet`, migrations, `migrate`, the builder API
- **Part 4: Dependency Injection** - direct `#[inject] DatabaseConnection`, self-keyed `#[injectable]` provider functions, scopes, `KeyedFactoryOutput<K, T>` registration, and `KeyedDepends<K, Result<T, E>>`. This is where the CRUD handlers are wired to the real ORM
- **Part 5: Serializers and Validation** - `Validate` derive, `pre_validate = true`, error responses and status codes
- **Part 6: Bonus — ViewSets and Routers** - the same CRUD compressed to ~15 lines with `ModelViewSet`; pagination, filtering, ordering

## Features

### Models

- **Snippet**: Code snippet with title, code, and language

### Serializers

- **SnippetSerializer**: Input validation with built-in validation
- **SnippetResponse**: Output serialization

### API Endpoints

```
GET    /api/snippets/         - List all snippets
POST   /api/snippets/         - Create a new snippet
GET    /api/snippets/config/  - DI demonstration: keyed fallible configuration
GET    /api/snippets/<id>/    - Retrieve a specific snippet
PUT    /api/snippets/<id>/    - Update a snippet
DELETE /api/snippets/<id>/    - Delete a snippet
```

All handlers receive a database connection through direct dependency injection
(`#[inject] db: DatabaseConnection`) and query the real ORM. The
`/api/snippets/config/` endpoint is a teaching aid for
`KeyedDepends<K, Result<T, E>>`; see `src/apps/snippets/di.rs`.

`DatabaseConnection` is a copyable ORM handle whose lease is retained by server
bootstrap. Handlers pass it by value without `clone()` and never construct or
own the underlying backend connection.

## Setup

### Prerequisites

- Rust 1.96.0 or later
- Docker, for the disposable PostgreSQL and Redis containers used by
  `cargo make migrate` and `cargo make runserver`
- PostgreSQL, only if you point `settings/local.toml` at an existing database

### Installation

```bash
# From the project root
cd examples/examples-tutorial-rest

# Build the project
cargo build

# Apply migrations against the local disposable database
cargo make migrate

# Run tests
cargo make test
```

## Usage

### Run the Development Server

```bash
# Apply migrations first. `runserver` also depends on `migrate`, but running
# it explicitly makes first-run database problems easier to see.
cargo make migrate

cargo make runserver
```

The server will start at `http://127.0.0.1:8000/`.

### Explore the ORM in the Rust Shell

This example declares `commands-shell` as an opt-in feature, not a default:

```bash
cargo run --bin manage --features commands-shell -- shell
cargo run --bin manage --features commands-shell -- shell -c \
  'println!("{:?}", db.backend())'
```

The feature-gated `config::shell::get_shell_config()` and generated-style
`manage` wiring load the concrete settings, ORM `db` handle, application `di`
context, `framework` alias, and `Snippet` short import. If installed apps
declare the same model name, the short import is skipped and a deterministic
warning lists the concrete registered crate paths; the evaluator's
`project_crate` alias can reference those same types. Projects can add a final
Rust prelude through `ShellConfig::with_prelude(...)`.

Interactive input preserves successful definitions, supports top-level
`.await`, and continues unmatched brackets from `>>> ` at `... `. Ctrl+C
during evaluation, a panic, or evaluator exit clears user state and reloads
settings, database/DI bindings, model imports, and the project prelude.
`shell -c` evaluates once, exits zero only on success, returns non-zero on
failure, and Reinhardt's own diagnostics do not repeat the raw source.
Arbitrary Rust, compiler output, panics, and user code can still print literals;
the shell is not a sandbox.

History is best-effort at
`<platform local data directory>/reinhardt/shell/examples-tutorial-rest.history`;
a missing file is a silent first run, while directory-resolution, read, or
write failures warn without preventing startup. `shell-rhai` was removed, and
the `shell` feature now selects Rust rather than the old Rhai syntax.

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
├── .gitignore
├── Cargo.toml
├── Dockerfile
├── Dockerfile.bruno
├── Makefile.toml
├── README.md
├── bruno/
│   ├── Snippets CRUD/
│   │   ├── Create Snippet.bru
│   │   ├── Delete Snippet.bru
│   │   ├── Get Snippet.bru
│   │   ├── List Snippets (Initial).bru
│   │   ├── Update Snippet.bru
│   │   ├── Verify Deletion (404).bru
│   │   └── folder.bru
│   ├── Snippets ViewSet/
│   │   ├── Create Snippet.bru
│   │   ├── Delete Snippet.bru
│   │   ├── Filter by Language.bru
│   │   ├── Get Snippet.bru
│   │   ├── List Snippets (Initial).bru
│   │   ├── Order by Created At Desc.bru
│   │   ├── Update Snippet.bru
│   │   └── folder.bru
│   ├── Validation Tests/
│   │   ├── Create Snippet - Empty Code (400).bru
│   │   ├── Create Snippet - Missing Title (400).bru
│   │   └── folder.bru
│   ├── bruno.json
│   ├── environments/
│   │   └── local.bru
│   └── reports/
│       └── .gitkeep
├── build.rs
├── docker-compose.api-tests.yml
├── migrations/
│   ├── auth/
│   │   └── 0001_initial.rs
│   ├── default/
│   │   └── 0001_initial.rs
│   └── snippets/
│       └── 0001_initial.rs
├── scripts/
│   ├── db_url.sh
│   ├── infra_down.sh
│   ├── infra_up.sh
│   └── parse_local_toml.py
├── settings/
│   ├── base.toml
│   └── ci.toml
├── src/
│   ├── apps/
│   │   ├── snippets/
│   │   │   ├── di.rs
│   │   │   ├── models.rs
│   │   │   ├── serializers.rs
│   │   │   ├── urls.rs
│   │   │   └── views.rs
│   │   └── snippets.rs
│   ├── apps.rs
│   ├── bin/
│   │   └── manage.rs
│   ├── config/
│   │   ├── apps.rs
│   │   ├── shell.rs
│   │   ├── settings.rs
│   │   └── urls.rs
│   ├── config.rs
│   └── lib.rs
└── tests/
    └── integration.rs
```

## Learning Path

This example is designed to be studied alongside the REST tutorial:

1. **Start with the tutorial**: Read the [REST tutorial](../../website/content/quickstart/tutorials/rest/)
2. **Examine the code**: Look at how concepts are implemented in this example
3. **Run the tests**: `cargo make test` to see the functionality in action
4. **Experiment**: Modify the code and see what happens

## Key Concepts Demonstrated

- `src/apps/snippets/models.rs` defines the `Snippet` model with `#[model(app_label = "snippets", table_name = "snippets")]`, typed fields, `created_at`, and the `highlighted()` helper.
- `src/apps/snippets/serializers.rs` defines `SnippetSerializer` with `Validate` length rules and `SnippetResponse::from_model()`.
- `src/apps/snippets/di.rs` registers a self-keyed singleton config provider and a keyed fallible config provider with `KeyedFactoryOutput<K, T>`.
- `src/apps/snippets/views.rs` exposes function-based CRUD handlers with `#[get]`, `#[post(pre_validate = true)]`, `#[put]`, and `#[delete]`, resolving `DatabaseConnection` through direct injection.
- The injected ORM handle is `Copy`, while framework bootstrap retains its owning lease for the server lifetime.
- `src/apps/snippets/views.rs` also exposes a `#[reinhardt::viewset(basename = "snippet")]` `ModelViewSet` with pagination, filtering, and ordering.
- `src/apps/snippets/urls.rs` registers both function-based endpoints and ViewSet endpoints on one `ServerRouter`.
- `src/config/urls.rs` uses `#[routes]` and mounts the snippets router under the literal `/api/` prefix.
- `bruno/` contains executable API collections for CRUD, ViewSet, and validation flows.
- `tests/integration.rs` covers native CRUD, validation, ViewSet, and routing behavior.

## Testing

Run the test suite:

```bash
# Run all tests
cargo make test

# Run specific test
cargo nextest run --all-features test_snippet_model

# Run with output
cargo nextest run --all-features --no-capture
```

## ViewSets (Part 6)

This example demonstrates both function-based views (Parts 2-5) and
ViewSet-based views (Part 6). **Both are mounted simultaneously** on the
same running server; there is no toggle between them. The two endpoint sets
coexist under separate URL prefixes:

```bash
cargo make migrate
cargo make runserver

# Function-based endpoints (Parts 2-5)
curl http://127.0.0.1:8000/api/snippets/

# ViewSet endpoints (Part 6)
curl http://127.0.0.1:8000/api/snippets-viewset/
```

The Bruno collection under `bruno/` contains a `Snippets CRUD` folder for the function-based path and a `Snippets ViewSet` folder for the ViewSet path; both can be exercised back-to-back without restarting the server.

Both endpoint sets issue real database queries. Until rows are inserted, both
paths return empty lists. Create rows with either `POST /api/snippets/` or
`POST /api/snippets-viewset/`, then list or retrieve them through either path.

### Comparison

| Approach | Code Lines | Features |
|----------|------------|----------|
| Function-based (Parts 2-5) | ~200 lines | Full control, explicit implementation |
| ViewSet-based (Part 6) | ~15 lines | CRUD automation, pagination, filtering, ordering |

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

**Function-based views (Parts 2-5)**:
- Simple endpoints with custom logic
- Non-standard RESTful patterns
- When you need fine-grained control
- Learning HTTP handling basics

**ViewSet-based views (Part 6)**:
- Standard RESTful CRUD APIs
- When pagination, filtering, and ordering are needed
- Rapid API development
- When code conciseness is important

## Next Steps

After understanding this example:

1. **Compare both approaches**: Try switching between function-based and ViewSet-based
2. **Understand the trade-offs**: When to use each approach (see above)
3. **Add custom actions**: Extend ViewSets with `#[action]` decorator for non-CRUD endpoints
4. **Add authentication**: Implement JWT, token, or session auth for both approaches
5. **Add permissions**: Implement permission classes to control access

## Related Documentation

- [REST Tutorial](../../website/content/quickstart/tutorials/rest/) - Step-by-step guide
- [API Documentation](https://docs.rs/reinhardt-web) - Complete API reference

## License

This example is part of the Reinhardt project and is licensed under the BSD 3-Clause License.
