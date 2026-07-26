# server_fn Macro Documentation

## Overview

The `server_fn` macro in `reinhardt-pages` provides a seamless way to define server functions that can be called from both server and client (WASM) environments. It automatically generates:

1. Server-side implementation with dependency injection
2. Client-side HTTP request stub (for WASM)
3. Native inventory metadata for automatic endpoint discovery

## Key Features

### Automatic Client Stub Generation

The macro automatically generates client-side code that:
- Serializes function arguments to JSON/URL-encoded/MessagePack
- Sends HTTP POST requests to the server endpoint
- Deserializes responses
- **Excludes `#[inject]` parameters** from the client-side signature

### Dependency Injection Support

Use `#[inject]` attribute to inject dependencies on the server side:

The injected `DatabaseConnection` is a `Copy` ORM handle. Server bootstrap owns
its `DatabaseConnectionLease`; handlers neither construct connections nor own
the lease.

```rust,ignore
use reinhardt::pages::server_fn::{ServerFnError, server_fn};
use reinhardt::DatabaseConnection;

#[server_fn]
pub async fn get_data(
    id: i64,
    #[inject] db: DatabaseConnection,
) -> Result<String, ServerFnError> {
    // Server-side implementation
    // `db` is automatically resolved from DI context
    Ok(format!("Data for id: {}", id))
}
```

The handle can be passed to concurrent operations without cloning:

```rust,ignore
#[server_fn]
async fn dashboard(#[inject] db: DatabaseConnection) -> Result<(), ServerFnError> {
    tokio::try_join!(load_users(db), load_posts(db))?;
    Ok(())
}
```

### Client-Side Usage

The generated client stub automatically excludes `#[inject]` parameters:

```rust
// WASM client code
async fn fetch_data() {
    // Notice: no `db` parameter required
    let result = get_data(42).await;
    match result {
        Ok(data) => log!("Got data: {}", data),
        Err(e) => log!("Error: {:?}", e),
    }
}
```

## Automatic Native Registration

Ordinary `#[server_fn]` declarations automatically submit native inventory
metadata. Each application owns the declarations below the module containing
its `#[app_config]`; a router built inside that application collects only its
own entries. The project root still mounts each application router explicitly.

```rust,ignore
use reinhardt::pages::server_fn::ServerFnRouterExt;
use reinhardt::ServerRouter;

pub fn server_url_patterns() -> ServerRouter {
    ServerRouter::new().auto_server_fns(module_path!())
}
```

The fixed `module_path!()` argument is required. It identifies the router
construction module so the collector can resolve its `#[app_config]` owner.
The collector sorts selected entries before mounting them, making registration
and validation deterministic.

`RunServerCommand` validates both inventory-built routers and routers that an
application pre-registers manually before startup. Manual registration is an
opt-out from inventory construction, not a way to bypass route validation.

### Explicit and Framework-Owned Routes

Use `auto_register = false` when a router must own a function explicitly, such
as a framework router or an endpoint mounted at a deliberate boundary. The
option retains the generated marker and client stub, but omits the native
inventory entry:

```rust,ignore
use reinhardt::pages::server_fn::{ServerFnError, ServerFnRouterExt, server_fn};
use reinhardt::ServerRouter;

#[server_fn(auto_register = false)]
async fn internal_health() -> Result<(), ServerFnError> {
    Ok(())
}

fn framework_router() -> ServerRouter {
    ServerRouter::new().server_fn(internal_health::marker)
}
```

Choose one registration path per endpoint. Registering a default function both
explicitly and through `auto_server_fns` leaves one endpoint mounted and records
a router configuration error. Explicit duplicate markers likewise report a
configuration error instead of replacing the earlier endpoint.

### Ownership and Validation Errors

Native inventory validation is also part of the command system checks. Errors
are deterministic, sorted diagnostics with these identifiers:

- `pages.server_fn.E001`: the router construction module has no owning
  `#[app_config]` module.
- `pages.server_fn.E002`: an automatically registered server function has no
  owning application module.
- `pages.server_fn.E003`: more than one application owns a module at the same
  specificity.
- `pages.server_fn.E004`: two automatically registered functions in one
  application use the same endpoint path.
- `pages.server_fn.E005`: two automatically registered functions in one
  application use the same route name.

Move application code under exactly one `#[app_config]` module, choose unique
paths and names, or opt out and mount the marker explicitly where that is the
intended ownership boundary.

### Cross-Target Hygiene

Inventory entries, the collector, and native route handlers are excluded from
browser WASM. Generated markers, metadata, client stubs, and wire DTOs remain
cross-target. Keep request and response DTOs in a module available to both
targets, and keep imports used only by a server implementation in that
application's native-only import block. The macro uses its resolved framework
paths internally, so application crates do not need a framework crate alias,
an `inventory` dependency, or a custom cfg alias.

## Structured errors (version 1)

Every failed server-function response uses a JSON error envelope independent of
the selected success codec. Version 1 uses stable lowercase error kinds and
always includes `field_errors`:

```json
{
  "version": 1,
  "kind": "validation",
  "status": 422,
  "message": "Please correct the submitted values",
  "field_errors": [
    {
      "field": "email",
      "message": "Enter a valid email address"
    }
  ]
}
```

Client code receives the same typed [`ServerFnError`] for every server-function
failure. Use constructors on the server and accessors on the client instead of
matching Rust variants or parsing JSON:

```rust,ignore
use reinhardt::pages::server_fn::{ServerFnError, ServerFnErrorKind};

let validation = ServerFnError::validation_with_message(
    "Please correct the submitted values",
    [("email", "Enter a valid email address")],
);
let empty_validation = ServerFnError::validation(std::iter::empty::<(&str, &str)>());
let forbidden = ServerFnError::auth(403, "Permission denied");
let application = ServerFnError::application("The operation failed");
let server = ServerFnError::server(500, "Internal server error");
let transport = ServerFnError::transport("Connection timeout");
let deserialization = ServerFnError::deserialization("Invalid response");

match validation.kind() {
    ServerFnErrorKind::Validation => {
        for field_error in validation.field_errors() {
            show_field_error(field_error.field(), field_error.message());
        }
    }
    ServerFnErrorKind::Auth => redirect_to_login(),
    _ => show_message(validation.user_message()),
}
```

`message()` remains an alias for `user_message()`. `network()` and
`serialization()` remain migration constructors and produce the `Transport`
kind.

Server-side DTO validation converts directly into `ServerFnError`, so a handler
that returns `Result<_, ServerFnError>` can use `?` without rebuilding field
errors:

```rust,ignore
use reinhardt::core::validators::Validate;
use reinhardt::pages::server_fn::ServerFnError;

#[server_fn]
async fn create_profile(request: ProfileRequest) -> Result<Profile, ServerFnError> {
    request.validate()?;
    save_profile(request).await
}
```

The conversion sets kind `Validation`, status `422`, and one ordered
`ServerFnFieldError` per `ValidationErrors` entry. Generated server-function
actions and forms retain this type end-to-end:

```rust,ignore
use reinhardt::pages::prelude::*;

let save: Action<Profile, ServerFnError> = use_action(save_profile);
let outcome = runtime
    .submit_server_fn(|| save_profile(ProfileRequest::from(&runtime)))
    .await?;
```

`submit_server_fn` routes matching validation field errors into the form state.
Unmatched field errors and non-validation failures remain available through the
form-level error signal, while the returned `Err(ServerFnError)` preserves the
typed error for caller-specific handling.

## How It Works

### Code Generation Flow

1. **Parse Function**: The macro analyzes the function signature and identifies `#[inject]` parameters
2. **Generate Server Code**: Creates server-side function with DI resolution
3. **Generate Client Stub**: Creates WASM client stub with filtered parameters (excluding `#[inject]`)
4. **Register Route**: Automatically registers the endpoint for routing

### Example Expansion

**Input:**
```rust
#[server_fn]
pub async fn create_user(
    username: String,
    email: String,
    #[inject] db: DatabaseConnection,
) -> Result<User, ServerFnError> {
    // Implementation
}
```

**Generates (conceptual):**

```rust
// Server-side (cfg not wasm32)
pub async fn create_user(
    username: String,
    email: String,
    db: DatabaseConnection,
) -> Result<User, ServerFnError> {
    // Original implementation
}

// Client-side (cfg wasm32)
pub async fn create_user(
    username: String,
    email: String,
) -> Result<User, ServerFnError> {
    #[derive(Serialize)]
    struct CreateUserArgs {
        username: String,
        email: String,
        // Note: `db` is NOT included
    }

    let endpoint = "/api/server_fn/create_user";
    let args = CreateUserArgs { username, email };
    let body = serde_json::to_string(&args)?;

    let response = reinhardt_pages::__private::fetch::request_with_credentials(
        "POST",
        endpoint,
        Some(&body),
        vec![("Content-Type".to_string(), "application/json".to_string())],
        reinhardt_pages::__private::fetch::FetchCredentials::Include,
    )
        .await?;

    response.json()
}

// Server handler (cfg not wasm32)
pub async fn __server_fn_handler_create_user(
    req: Request,
) -> Result<String, String> {
    // Parse request body
    let args: CreateUserArgs = serde_json::from_str(&body)?;

    // Resolve dependencies from DI context
    let db = resolve_dependency::<DatabaseConnection>(&di_ctx).await?;

    // Call original function
    let result = create_user(args.username, args.email, db).await;

    // Serialize response
    serde_json::to_string(&result)
}
```

## Migration from Manual Double Definition

### Before (v0.1.0-alpha.1 and earlier)

Previously, you had to manually define the function twice:

```rust
// Server-side
#[cfg(not(target_arch = "wasm32"))]
#[server_fn]
pub async fn get_questions(
    #[inject] _db: DatabaseConnection,
) -> Result<Vec<QuestionInfo>, ServerFnError> {
    // Server implementation
}

// Client-side (manual stub)
#[cfg(target_arch = "wasm32")]
#[server_fn]
pub async fn get_questions() -> Result<Vec<QuestionInfo>, ServerFnError> {
    unreachable!("Replaced by macro")
}
```

### After (v0.1.0-alpha.2+)

Now you only need a single definition:

```rust
// Single definition works for both environments
#[server_fn]
pub async fn get_questions(
    #[inject] _db: DatabaseConnection,
) -> Result<Vec<QuestionInfo>, ServerFnError> {
    // Server implementation
}
```

**The macro automatically:**
- Generates server-side code with `#[inject]` parameters
- Generates client-side stub **without** `#[inject]` parameters
- Handles conditional compilation for both targets
- Generates typed query-key helpers for `use_query` / `use_mutation`

## Query Key Helpers

Every `#[server_fn]` emits a marker module with a `key(...)` helper that returns
a typed `QueryKey<T, ServerFnError>` for `use_query`:

```rust
use reinhardt::pages::prelude::*;
use reinhardt::pages::server_fn::{ServerFnError, server_fn};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct JobSnapshot {
    pub id: i64,
}

#[server_fn]
pub async fn list_project_jobs(project_id: i64) -> Result<Vec<JobSnapshot>, ServerFnError> {
    Ok(Vec::new())
}

#[server_fn]
pub async fn retry_job(project_id: i64, job_id: i64) -> Result<(), ServerFnError> {
    Ok(())
}

let jobs = use_query(list_project_jobs::key(42));
let retry = use_mutation(|job_id: i64| async move { retry_job(42, job_id).await })
    .invalidates(list_project_jobs::key(42));
```

The key ID is derived from the server function endpoint, codec, and a SHA-256
digest of canonical JSON arguments. Raw arguments are not embedded in cache or
hydration IDs. Mounted queries with logically equivalent object arguments share
one cache entry regardless of map iteration order. Queries with the same key
share one cache entry and in-flight request, `refetch()` refreshes manually,
and `poll(duration)` keeps a query current while the handle is alive.

Generated keys support direct `Result<T, E>` returns and common result aliases
such as `AppResult<T> = Result<T, ServerFnError>`. Server functions with
request extractors or `#[inject]` parameters do not run their fetcher during
native SSR prefetch; the key remains usable for browser fetches and native
component-test server-function mocks, including result aliases.

Use `server_fn_module::key(...)` for generated keys. The module-qualified helper
binds the key to the selected server function even when another function has the
same argument and return types.

## Typed Server Function Sets

`#[server_fnset]` groups existing server functions without changing their
markers, codecs, CSRF behavior, extractors, injected parameters, or mock
identity. A developer-authored function that is explicitly mounted through a
set must declare `auto_register = false`; otherwise its default inventory
registration is collected automatically. Do not mount the same function through
both `.auto_server_fns(module_path!())` and `.server_fnset(...)`. Members may
use different codecs:

```rust,no_run
use reinhardt_pages::server_fn::{
    ServerFnError, ServerFnRouterExt, ServerFnSet, ServerFnSetChainExt,
    ServerFnSetRegistration, server_fn, server_fnset,
};
use reinhardt_urls::routers::ServerRouter;

#[server_fn(codec = "json", auto_register = false)]
async fn dashboard() -> Result<String, ServerFnError> {
    Ok(String::new())
}

#[server_fn(codec = "url", auto_register = false)]
async fn export_data(format: String) -> Result<Vec<u8>, ServerFnError> {
    let _ = format;
    Ok(Vec::new())
}

#[server_fnset(name = "admin")]
pub fn admin_fns() -> impl ServerFnSetRegistration {
    ServerFnSet::new()
        .server_fn(dashboard::marker)
        .server_fn(export_data::marker)
}

fn routes() -> ServerRouter {
    ServerRouter::new().server_fnset(admin_fns())
}

fn main() {
    let _ = routes();
}
```

The set name must be nonempty and contain only ASCII letters, digits, hyphens,
or underscores. Slashes, dots, percent escapes, whitespace, non-ASCII text, and
URL delimiter characters are rejected, so a name remains one safe path segment.

Model-generated `#[server_fnset]` action functions are internally opted out of
automatic registration. Mount a model set with `.server_fnset(...)`; its typed
set registration is the single native routing boundary.

### Model-backed CRUD

Enable `model-server-fnset` to generate model-backed RPCs. The resource keeps
wire DTOs separate from the ORM model, selects a policy explicitly, and proves
that its lookup is unique. `AllowAllPolicy` is an intentional public-access
choice; omitting `Policy` is a compile error.

A cross-target package can keep the browser-safe dependencies in the common
table and declare the ORM dependency only for native targets:

```toml
[dependencies]
async-trait = "0.1"
reinhardt-core = { version = "0.3.1", default-features = false, features = ["macros"] }
reinhardt-pages = { version = "0.3.1", features = ["model-server-fnset"] }
reinhardt-urls = { version = "0.3.1", default-features = false, features = ["client-router"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_urlencoded = "0.7"

[target.'cfg(not(all(target_family = "wasm", target_os = "unknown")))'.dependencies]
ctor = "0.8"
reinhardt-db = { version = "0.3.1", features = ["orm", "di"] }
reinhardt-di = { version = "0.3.1", features = ["macros", "params"] }
reinhardt-http = "0.3.1"
reinhardt-urls = { version = "0.3.1", default-features = false, features = ["routers", "client-router"] }
```

The native macro expansion resolves DI, HTTP transport, model registration,
and JSON codec support through these direct dependencies. The `di` feature on
`reinhardt-db` supplies injection for the generated transaction-bound database
connection. `info = false` below omits optional application metadata while
retaining the generated typed field accessors.

The complete example below is native-only because it implements the ORM
resource and mounts native server routes. It uses the same contracts exercised
by the macro compile fixtures, selects the generated typed unique accessor for
`slug`, replaces the checked `update` action, and adds a transactional detail
action:

```rust,no_run
use reinhardt_core::macros::model;
use reinhardt_db::orm::{Model, TransactionExecutor, UniqueFieldRef};
use reinhardt_pages::server_fn::{
    AllowAllPolicy, CreateModelInput, DetailActionContext, ModelServerFnResource,
    ModelServerFnSet, PageRequest, PatchModelInput, ServerFnListQuery,
    ServerFnResource, ServerFnRouterExt, ServerFnSetError, UpdateModelInput,
    server_fnset,
};
use reinhardt_urls::routers::ServerRouter;
use serde::{Deserialize, Serialize};

#[model(app_label = "articles", table_name = "articles", info = false)]
#[derive(Clone, Serialize, Deserialize)]
pub struct Article {
    #[field(primary_key = true)]
    pub id: Option<i64>,
    #[field(max_length = 120, unique = true)]
    pub slug: String,
    #[field(max_length = 255)]
    pub title: String,
    pub published: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ArticleListQuery {
    pub limit: Option<u32>,
    pub offset: u64,
}

impl ServerFnListQuery for ArticleListQuery {
    fn page_request(&self) -> PageRequest {
        PageRequest { limit: self.limit, offset: self.offset }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ArticleDto {
    pub slug: String,
    pub title: String,
    pub published: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CreateArticle {
    pub slug: String,
    pub title: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct UpdateArticle {
    pub title: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PatchArticle {
    pub title: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PublishArticle;

impl CreateModelInput<Article> for CreateArticle {
    fn build(self) -> Result<Article, ServerFnSetError> {
        Ok(Article {
            id: None,
            slug: self.slug,
            title: self.title,
            published: false,
        })
    }
}

impl UpdateModelInput<Article> for UpdateArticle {
    fn apply(self, article: &mut Article) -> Result<(), ServerFnSetError> {
        article.title = self.title;
        Ok(())
    }
}

impl PatchModelInput<Article> for PatchArticle {
    fn apply_patch(self, article: &mut Article) -> Result<(), ServerFnSetError> {
        if let Some(title) = self.title {
            article.title = title;
        }
        Ok(())
    }
}

pub struct ArticleResource;

impl ServerFnResource for ArticleResource {
    type Lookup = String;
    type Read = ArticleDto;
    type Create = CreateArticle;
    type Update = UpdateArticle;
    type Patch = PatchArticle;
    type ListQuery = ArticleListQuery;
}

#[async_trait::async_trait]
impl ModelServerFnResource for ArticleResource {
    type Model = Article;
    type Policy = AllowAllPolicy;
    const PUBLIC_NAME: &'static str = "article";

    fn lookup_field() -> UniqueFieldRef<Article, String> {
        Article::unique_slug()
    }

    async fn to_read(
        article: &Article,
        _executor: Option<&mut dyn TransactionExecutor>,
    ) -> Result<ArticleDto, ServerFnSetError> {
        Ok(ArticleDto {
            slug: article.slug.clone(),
            title: article.title.clone(),
            published: article.published,
        })
    }
}

pub struct ArticleActions;

#[server_fnset(name = "article-api", actions = ArticleActions)]
pub fn article_fns() -> ModelServerFnSet<ArticleResource> {
    ModelServerFnSet::new()
}

#[server_fnset(for = article_fns)]
impl ArticleActions {
    pub async fn update(
        _lookup: String,
        input: UpdateArticle,
        #[inject] mut context: DetailActionContext<ArticleResource>,
    ) -> Result<ArticleDto, ServerFnSetError> {
        let (article, executor) = context.parts_mut();
        ArticleResource::perform_update(input, article, executor).await?;
        ArticleResource::to_read(article, Some(executor)).await
    }

    #[action(detail = true, transactional = true)]
    pub async fn publish(
        _lookup: String,
        _input: PublishArticle,
        #[inject] mut context: DetailActionContext<ArticleResource>,
    ) -> Result<ArticleDto, ServerFnSetError> {
        let (article, executor) = context.parts_mut();
        article.published = true;
        article
            .save_with_executor(executor)
            .await
            .map_err(|_| ServerFnSetError::Internal)?;
        ArticleResource::to_read(article, Some(executor)).await
    }
}

fn routes() -> ServerRouter {
    ServerRouter::new().server_fnset(article_fns())
}

fn main() {
    let _ = routes();
}
```

Every model set emits exactly six standard POST RPCs: `list`, `retrieve`,
`create`, `update`, `partial_update`, and `destroy`. The generated namespace is
the fn-form name for an unqualified `for` link. For example,
`article_fns::partial_update` posts to
`/api/server_fn/article-api/partial-update`. Rust underscores normalize to
hyphens for action path and metadata segments. Standard overrides are checked
against their generated input, output, and error contracts; custom actions use
`#[action(detail = ..., transactional = ...)]`.

Qualified `for` links use a private, path-derived internal action module so
separate links such as `admin::article_fns` and `public::article_fns` cannot
collide. The generated module does not replay a linked declaration's relative
visibility at a different expansion site. Applications should register the
linked fn-form rather than depend on that internal module name.

`ServerFnResource::Lookup` must implement `Clone` because detail overrides keep
the decoded lookup while the framework performs the authorized object lookup.
A resource should set `ModelServerFnResource::PUBLIC_NAME` to the stable name
used in client-visible not-found errors; the default is the generic
`"resource"` and never exposes the physical database table name.
A standard create override receives `CreateActionContext`, which exposes the
active transaction executor and a principal-aware `authorize_object` method.
Overrides that persist a model must call `authorize_object` before using the
executor to write it; create overrides do not run queryset scoping. Transactional
collection custom actions instead receive a policy-scoped
`CollectionActionContext`.

List pagination defaults to 25 items, accepts limits in `1..=100`, and reports
the policy-scoped total before applying offset and limit. Collection and detail
policies run before data becomes visible. Mutations and transactional custom
actions use one transaction-bound executor and roll back on error. Full and
partial updates authorize the mutated object again before read mapping and
commit.

Model action failures map deterministically: validation and application errors
to 400, unauthenticated to 401, forbidden to 403, missing objects to 404,
conflicts to 409, and transport or internal failures to 500. Internal transport
details are logged server-side and serialized as the sanitized `Internal`
variant.

### Native, WASM, and mocks

The cross-target surface contains wire contracts (`ServerFnResource`, list and
page types, `ServerFnSetError`), metadata, generated markers, and client stubs.
The ORM resource implementation, policies, action contexts, database executors,
native CRUD handlers, and `ModelServerFnSet` constructor are native-only and
require `model-server-fnset`. This keeps `reinhardt-db` and `reinhardt-views`
out of the browser dependency graph.

The generated six standard client signatures remain available to browser WASM.
This compile-only shape is kept in sync by
`tests/wasm/server_fnset_wasm_compile_test.rs`:

```rust,ignore
async fn assert_standard_client_signatures() {
    let _ = article_fns::list(ArticleListQuery { limit: None, offset: 0 }).await;
    let _ = article_fns::retrieve("typed-rpc".to_owned()).await;
    let _ = article_fns::create(CreateArticle {
        slug: "typed-rpc".to_owned(),
        title: "Typed RPC".to_owned(),
    }).await;
    let _ = article_fns::update(
        "typed-rpc".to_owned(),
        UpdateArticle { title: "Updated".to_owned() },
    ).await;
    let _ = article_fns::partial_update(
        "typed-rpc".to_owned(),
        PatchArticle { title: None },
    ).await;
    let _ = article_fns::destroy("typed-rpc".to_owned()).await;
}
```

Each generated action retains an independent marker, so component and MSW mocks
target one action rather than the whole set. Enable `msw` for application-side
marker mocks. Tests in `reinhardt-test` that mock model actions use its dedicated
`model-server-fnset-msw` feature:

```rust,ignore
worker.handle_server_fn::<article_fns::retrieve::marker>(|args| {
    Ok(ArticleDto {
        slug: args.lookup,
        title: "Mock article".to_owned(),
        published: false,
    })
});
```

The first release does not provide action subsets, a read-only model set type,
REST or OpenAPI generation, cursor pagination, bulk or nested actions,
composite lookups, global discovery, or automatic model-to-DTO derivation.

## Macro Attributes

### `#[server_fn]`

Basic usage without options:

```rust
#[server_fn]
pub async fn simple_function() -> Result<String, ServerFnError> {
    Ok("Hello".to_string())
}
```

### `#[server_fn]` with `#[inject]`

`#[inject]` parameters are auto-detected. No additional options are needed:

```rust
#[server_fn]
pub async fn with_di(
    #[inject] db: DatabaseConnection,
) -> Result<(), ServerFnError> {
    Ok(())
}
```

### `#[server_fn(codec = "json")]`

Specify serialization codec (default: "json"):

```rust
#[server_fn(codec = "json")]  // JSON (default)
#[server_fn(codec = "url")]   // URL-encoded
#[server_fn(codec = "msgpack")] // MessagePack
```

### `#[server_fn(no_csrf = true)]`

Disable CSRF protection:

```rust
#[server_fn(no_csrf = true)]
pub async fn public_endpoint() -> Result<String, ServerFnError> {
    Ok("Public data".to_string())
}
```

## Best Practices

### 1. Use `#[inject]` for Server-Only Dependencies

For ORM access, inject `DatabaseConnection` directly. The framework retains the
owning lease for the server lifetime, so a server function must not create or
store a `DatabaseConnectionLease`.

```rust
#[server_fn]
pub async fn get_user(
    user_id: i64,
    #[inject] db: DatabaseConnection,
    #[inject] session: SessionData,
) -> Result<UserInfo, ServerFnError> {
    // `db` and `session` are injected on server
    // Client calls: get_user(user_id)
}
```

### 2. Keep Function Signatures Simple

```rust
// ✅ Good: Simple types that serialize well
#[server_fn]
pub async fn create_post(
    title: String,
    content: String,
) -> Result<PostInfo, ServerFnError> { }

// ❌ Avoid: Complex types that don't serialize
#[server_fn]
pub async fn bad_function(
    callback: Box<dyn Fn()>, // Won't serialize!
) -> Result<(), ServerFnError> { }
```

### 3. Use Shared Types

Define shared types in a module accessible to both server and client:

```rust
// src/shared/types.rs
#[derive(Serialize, Deserialize)]
pub struct CreatePostRequest {
    pub title: String,
    pub content: String,
}

#[derive(Serialize, Deserialize)]
pub struct PostInfo {
    pub id: i64,
    pub title: String,
}

// src/apps/posts/server_fn.rs
use crate::shared::types::{CreatePostRequest, PostInfo};

#[server_fn]
pub async fn create_post(
    request: CreatePostRequest,
    #[inject] db: DatabaseConnection,
) -> Result<PostInfo, ServerFnError> {
    // Implementation
}
```

### 4. Handle Errors Properly

```rust
#[server_fn]
pub async fn safe_operation(
    #[inject] db: DatabaseConnection,
) -> Result<Data, ServerFnError> {
    let data = fetch_data(&db)
        .await
        .map_err(|e| ServerFnError::application(e.to_string()))?;

    Ok(data)
}
```

## Troubleshooting

### Issue: "cannot find type `X` in this scope" in WASM build

**Cause**: Server-only types are not available in WASM environment.

**Solution**: Use conditional imports:

```rust
// Common imports
use reinhardt::pages::server_fn::{ServerFnError, server_fn};

// Server-only imports
#[cfg(not(target_arch = "wasm32"))]
use {
    reinhardt::DatabaseConnection,
    crate::models::User,
};

// WASM-only imports (if needed)
#[cfg(target_arch = "wasm32")]
use crate::shared::types::UserInfo;
```

### Issue: "failed to resolve: could not find `fetch`"

**Cause**: The Pages runtime dependency is unavailable to the browser target.

**Solution**: Keep either the direct `reinhardt-pages` dependency or the
`reinhardt` facade dependency available to the target compiling the client
stub. The macro resolves a renamed direct `reinhardt-pages` dependency and a
renamed `reinhardt` facade automatically. No source-level framework crate
alias, macro crate-path option, `inventory` dependency, or custom cfg alias is
required. If the error remains, verify that the selected target dependency and
its Pages client features have not been disabled.

### Issue: Function signature mismatch between server and client

**Cause**: Manually defined double definitions are no longer needed.

**Solution**: Remove manual `#[cfg(target_arch = "wasm32")]` stubs:

```rust
// ❌ Remove this
#[cfg(target_arch = "wasm32")]
#[server_fn]
pub async fn my_function() -> Result<(), ServerFnError> {
    unreachable!()
}

// ✅ Keep only this
#[server_fn]
pub async fn my_function(
    #[inject] db: DatabaseConnection,
) -> Result<(), ServerFnError> {
    // Implementation
}
```

## Implementation Details

### Macro Location

- **Crate**: `reinhardt-pages-macros`
- **File**: `crates/reinhardt-pages/macros/src/server_fn.rs`

### Key Functions

1. **`expand()`** (line 348-359): Main expansion logic
   - Removes `#[inject]` attributes from server function
   - Generates client stub with filtered parameters
   - Generates server handler with DI resolution

2. **`generate_client_stub()`** (line 388-555): Client stub generation
   - **Line 405-417**: Filters out `#[inject]` parameters
   - **Line 430-440**: Creates new signature without `#[inject]` parameters
   - **Line 520**: Uses filtered signature for client stub

3. **`generate_server_handler()`**: Server handler generation
   - Deserializes request body
   - Resolves `#[inject]` dependencies from DI context
   - Calls original function with resolved dependencies

### Conditional Compilation Strategy

The macro uses `#[cfg(target_arch = "wasm32")]` and `#[cfg(not(target_arch = "wasm32"))]` to generate target-specific code:

```rust
quote! {
    // Server-side: Original function (with #[inject] attributes removed)
    #[cfg(not(target_arch = "wasm32"))]
    #clean_func

    // Client-side: HTTP request stub (without #[inject] parameters)
    #client_stub

    // Server-side: Route handler and registration
    #server_handler
}
```

## Version History

### Upcoming

**Deprecation**: `use_inject = true` is deprecated

- **Changed**: `#[inject]` parameters are now auto-detected unconditionally, matching route macro behavior
- **Deprecated**: `use_inject = true` option (emits deprecation warning)
- **Migration**: Remove `use_inject = true` from `#[server_fn(...)]` attributes

### v0.1.0-alpha.2 (2026-01-09)

**Breaking Change**: Automatic `#[inject]` parameter exclusion in client stubs

- **Added**: Client-side signature filtering (removes `#[inject]` parameters)
- **Removed**: Need for manual double definition with conditional compilation
- **Fixed**: Parameter count mismatch between server and client calls

**Migration**: Remove manual `#[cfg(target_arch = "wasm32")]` function stubs.

### v0.1.0-alpha.1 (Initial Release)

- Initial implementation with manual double definition requirement

## See Also

- [reinhardt-pages README](../README.md)
- [Dependency Injection Guide](../../reinhardt-di/README.md)

## Contributing

If you find issues with the `server_fn` macro or have suggestions for improvements, please:

1. Check existing issues: https://github.com/kent8192/reinhardt-web/issues
2. Open a new issue with:
   - Clear description of the problem
   - Minimal reproducible example
   - Expected vs. actual behavior
3. Submit a pull request if you have a fix

## License

This documentation is part of the Reinhardt project and is licensed under the BSD 3-Clause License.
