# reinhardt-macros

Procedural macros for the framework

## Overview

Procedural macros for reducing boilerplate code. Includes derive macros for models, serializers, and forms, as well as attribute macros for endpoints and middleware.

Provides compile-time code generation for common patterns.

## Features

### Implemented ✓

#### Function-based API Views

- **`#[api_view]`** - Convert function to API view
  - Supports multiple HTTP methods via `methods` parameter
  - Validates HTTP methods at compile time (GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS)
  - Defaults to GET if no methods specified
  - Example: `#[api_view(methods = "GET,POST")]`

#### ViewSet Custom Actions

- **`#[action]`** - Define custom ViewSet actions
  - Supports HTTP method specification via `methods` parameter
  - Supports detail/list action via `detail` parameter (required)
  - Optional `url_path` and `url_name` parameters
  - Validates HTTP methods at compile time
  - Example: `#[action(methods = "POST", detail = true)]`

#### HTTP Method Decorators

- **`#[get]`** - GET method decorator with path validation
- **`#[post]`** - POST method decorator with path validation
- **`#[put]`** - PUT method decorator with path validation
- **`#[patch]`** - PATCH method decorator with path validation
- **`#[delete]`** - DELETE method decorator with path validation
- All support compile-time URL pattern validation
- **Dependency Injection**: Use `use_inject = true` option with `#[inject]` parameter attribute
- Path syntax: Simple `{id}` or typed `{<uuid:id>}`, `{<int:id>}`, `{<str:name>}`, `{<slug:title>}`, `{<path:route>}`
- Example: `#[get("/users/{id}")]`
- Example with DI: `#[get("/users/{<uuid:id>}", use_inject = true)]`

#### Permission System

- **`#[permission_required]`** - Permission decorator
  - Validates permission strings at compile time
  - Supports Django-style permission format: `"app.permission"`
  - Uses nom parser for validation
  - Example: `#[permission_required("users.view_user")]`

#### Dependency Injection (FastAPI-style)

##### Injectable Macro (Factory/Provider Pattern)

- **`#[injectable]`** - Transform functions or structs into `Injectable` trait implementations

  **Function Usage**: Factory functions for creating dependencies
  - All parameters must be marked with `#[inject]`
  - Supports both sync and async functions
  - Cache control: `#[inject(cache = false)]`
  - Scope control: `#[inject(scope = Singleton)]` or `#[inject(scope = Request)]`
  - Example:
    ```rust
    use reinhardt::di::injectable;
    use std::sync::Arc;

    #[injectable]
    fn create_user_service(
        #[inject] db: Arc<Database>,
        #[inject] cache: Arc<Cache>,
    ) -> UserService {
        UserService { db, cache }
    }
    ```

  **Struct Usage**: Auto-generate `Injectable` implementation
  - All fields must have either `#[inject]` or `#[no_inject]` attribute
  - Field attributes:
    - `#[inject]` - Inject from DI container (cached by default)
    - `#[inject(cache = false)]` - Inject without caching
    - `#[inject(scope = Singleton)]` - Use singleton scope
    - `#[no_inject(default = Default)]` - Initialize with `Default::default()`
    - `#[no_inject(default = value)]` - Initialize with specific value
    - `#[no_inject]` - Initialize with `None` (field must be `Option<T>`)
  - Struct must implement `Clone` (required by `Injectable` trait)
  - Example:
    ```rust
    use reinhardt::di::injectable;

    #[injectable]
    #[derive(Clone)]
    struct UserViewSet {
        #[inject]
        db: Database,
        #[inject]
        cache: RedisCache,
        #[no_inject(default = Default)]
        config: Config,
    }
    ```

##### HTTP Method Macros with Dependency Injection

- **`#[get("/path", use_inject = true)]`** - GET with DI enabled
- **`#[post("/path", use_inject = true)]`** - POST with DI enabled
- **`#[put("/path", use_inject = true)]`** - PUT with DI enabled
- **`#[patch("/path", use_inject = true)]`** - PATCH with DI enabled
- **`#[delete("/path", use_inject = true)]`** - DELETE with DI enabled
  - FastAPI-style parameter attributes with `#[inject]`
  - Automatic resolution from `InjectionContext`
  - `use_inject = true` is **required** when using `#[inject]` parameters
  - Example:
    ```rust
    use reinhardt::views::{get, post};
    use reinhardt::http::{Response, ViewResult};
    use reinhardt::extractors::{Path, Json};
    use uuid::Uuid;

    #[get("/users/{<uuid:id>}", use_inject = true)]
    async fn get_user(
        Path(id): Path<Uuid>,
        #[inject] db: DatabaseConnection,  // Injected from context
    ) -> ViewResult<Response> {
        // ...
    }

    #[post("/users", use_inject = true)]
    async fn create_user(
        Json(data): Json<CreateUserRequest>,
        #[inject] db: DatabaseConnection,
    ) -> ViewResult<Response> {
        // ...
    }
    ```

  A raw `Request` can be combined with typed extractors when a handler needs
  direct access to headers or other request metadata:

  ```rust
  use reinhardt::{get, Path, Request, Response, ViewResult};
  use uuid::Uuid;

  #[get("/books/import/{job_id}")]
  async fn get_job(
      req: Request,
      Path(job_id): Path<Uuid>,
  ) -> ViewResult<Response> {
      let cookie = req.get_header("cookie");
      // Use `cookie` and `job_id` to authorize and load the import job.
      Ok(Response::ok())
  }
  ```

  Injected parameters may use mutable bindings and destructuring patterns:

  ```rust,ignore
  #[inject] mut db: DatabaseConnection
  #[inject] Wrapper(mut value): Wrapper<Data>
  ```

  Mutability applies only to the function's internal binding; it does not
  change resolver ownership or caching.

**Pattern Comparison:**
- `#[injectable]` - Creates an `Injectable` implementation for the return type (Factory/Provider pattern)
- `#[<http_method>(..., use_inject = true)]` - Injects dependencies into function parameters

#### Configuration Macros

- **`installed_apps!`** - Define installed applications
  - Compile-time validation of application paths
  - Type-safe enum generation for all installed apps
  - Validates `reinhardt.*` module paths at compile time
  - Generates `Display` and `FromStr` implementations
  - Example: `installed_apps! { polls: "polls", }` (user apps only)
  - Note: Built-in framework features are enabled via Cargo feature flags, not `installed_apps!`

#### URL Pattern Validation

- **`path!`** - Validate URL patterns at compile time
  - Uses nom parser for pattern validation
  - Supports simple parameters: `{id}`
  - Supports Django-style typed parameters: `{<int:id>}`
  - Validates parameter names and type specifiers
  - Supported types: `int`, `str`, `uuid`, `slug`, `path`
  - Detailed error messages with position information
  - Example: `path!("users/{<int:user_id>}/posts/{post_id}/")`

#### Signal System

- **`#[receiver]`** - Connect receiver function to signal
  - Django-style `@receiver` decorator functionality
  - Supports signal and sender parameters
  - Marker macro for signal registration
  - Example: `#[receiver(signal = post_save::<User>())]`

#### Type-safe Query Fields

- **`#[derive(QueryFields)]`** - Generate field accessor methods
  - Automatic field accessor generation for models
  - Compile-time validated field lookups
  - Type-specific lookup methods based on field type
  - String fields: `lower()`, `upper()`, `trim()`, `contains()`
  - Numeric fields: `abs()`, `ceil()`, `floor()`, `round()`
  - DateTime fields: `year()`, `month()`, `day()`, `hour()`
  - All fields: `eq()`, `ne()`, `gt()`, `gte()`, `lt()`, `lte()`
  - Example: `QuerySet::<User>::new().filter(User::email().lower().contains("example.com"))`

#### Model Definition

- **`#[model(...)]`** - Attribute macro for Django-style model definition
  - Automatically adds `#[derive(Model)]`
  - Cleaner syntax without explicit `#[derive(Model)]`
  - Same attributes as `#[derive(Model)]`
  - Requires an explicit `app_label`
  - Enables generated model-form schema and generic payload types only with the
    explicit `form = true` opt-in
  - Defaults `table_name` to the app label plus struct name in snake_case without pluralization
  - Example: `#[model(app_label = "polls", form = true)]` generates
    `QuestionFormSchema`, `QuestionModelFormData<P>`, and
    `CleanedQuestionModelFormData<P>` for a `Question` model
  - Models without `form = true` generate no model-form symbols
  - `#[form(validate = path)]` declares one synchronous validator over the
    generated cleaned payload
  - `#[form(trim)]` opts a generated text, email, or URL field into trimming;
    generated fields otherwise preserve surrounding whitespace
  - `#[field(...)]` remains database and model metadata; form-only behavior
    belongs in `#[form(...)]`
  - Native create code consumes raw payloads with `clean_and_validate()`, then
    uses cleaned `into_model(context)`; update code uses
    `clean_and_validate_for_update(&existing)` before `apply_to(existing)` so
    synchronous cross-field validation observes the post-merge candidate

- **`#[derive(Model)]`** - Derive macro for automatic Model implementation
  - Implements `Model` trait
  - Registers model with global ModelRegistry for migrations
  - Model attributes: `app_label`, `table_name`, `constraints`, `form`
  - Field attributes: `primary_key`, `max_length`, `null`, `blank`, `unique`, `default`, `db_column`, `editable`
  - Supported types: `i32`, `i64`, `String`, `bool`, `DateTime<Utc>`, `NaiveDateTime`, `Date`, `Time`, `f32`, `f64`, `Option<T>`
  - Requires: Named fields, `Serialize`/`Deserialize`, exactly one `primary_key`, `max_length` for String fields

#### ORM Reflection

- **`#[derive(OrmReflectable)]`** - Automatic OrmReflectable implementation
  - Enables reflection-based field and relationship access
  - Automatic type inference: `Vec<T>` → collection, `Option<T>` → scalar, primitives → fields
  - Field attributes: `#[orm_field(type = "Integer")]`, `#[orm_relationship(type = "collection")]`, `#[orm_ignore]`
  - Supported types: Integer, Float, Boolean, String

#### OpenAPI Schema Generation

- **`#[derive(Schema)]`** - Automatic OpenAPI 3.0 schema generation
  - Implements `ToSchema` trait
  - Supports primitives, `Option<T>`, `Vec<T>`, custom types
  - Documentation comments become field descriptions
  - Automatic required/optional field detection

#### Shared DTOs

- **`#[dto]`** - Target-neutral DTO validation boilerplate
  - Emits a shared `Validate` derive for native and WASM builds
  - Leaves `#[validate(...)]` attributes active on both targets
- **`#[dto(schema)]`** - Opts the DTO into native-only OpenAPI `Schema` generation
  - Requires the consumer's `openapi` feature, which also enables the core validation surface
  - Gates container and field `#[schema(...)]` customizations to native builds
  - Leaves the plain `#[dto]` behavior unchanged

#### Application Configuration

- **`#[derive(AppConfig)]`** - AppConfig factory method generation
  - Generates `config()` method returning `AppConfig`
  - Attributes: `name` (required), `label` (required), `verbose_name` (optional)
  - Example: `#[derive(AppConfig)] #[app_config(name = "auth", label = "auth")]`

#### Admin Panel Configuration

- **`#[admin(...)]`** - ModelAdmin configuration with compile-time validation
  - Implements `ModelAdmin` trait
  - Required: `for = ModelType`, `name = "ModelName"`
  - Optional: `list_display`, `list_filter`, `search_fields`, `fields`, `readonly_fields`, `ordering`, `list_per_page`
  - Compile-time field validation against model
  - Example: `#[admin(for = User, name = "User", list_display = [id, email, username])]`

#### URL Pattern Registration

- **`#[routes]`** - Attribute macro for automatic URL pattern registration
  - Registers URL pattern function for framework discovery (via `inventory` crate)
  - Apply to project-level `routes()` function in `src/config/urls.rs`
  - Return type must be `UnifiedRouter` (framework handles Arc wrapping internally)
  - Example:
    ```rust
    use reinhardt::prelude::*;
    use reinhardt::routes;

    #[routes]
    pub fn routes() -> UnifiedRouter {
        UnifiedRouter::new()
            .mount("/api/", api_router())
    }
    ```

#### Migration Collection

- **`collect_migrations!`** - Migration registration with global registry
  - Generates `MigrationProvider` implementation
  - Registers with global migration registry via `linkme::distributed_slice`
  - Requires migration modules to export `migration()` function

#### Generic Dependency Injection

- **`#[use_inject]`** - Standalone dependency injection for any function
  - Transforms functions with `#[inject]` parameters
  - Removes `#[inject]` parameters from signature
  - Adds `InjectionContext` parameter
  - Injects dependencies at function start
  - Can be used independently of HTTP method macros

## Installation

Add `reinhardt` to your `Cargo.toml`:

```toml
[dependencies]
reinhardt = { version = "0.1.2", features = ["core"] }

# Or use a preset:
# reinhardt = { version = "0.1.2", features = ["standard"] }  # Default preset
# reinhardt = { version = "0.1.2", features = ["full"] }      # Broad preset
```

**Note:** The `core` feature (included in `standard` and `full`) is required to use the macros from this crate.
