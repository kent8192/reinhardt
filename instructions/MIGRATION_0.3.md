# Migration Guide: 0.2.x to 0.3.0

Related release-readiness tracking:
[#5405](https://github.com/kent8192/reinhardt-web/issues/5405) and
[#5410](https://github.com/kent8192/reinhardt-web/issues/5410).

This guide covers the delta from the public 0.2.x line to the final 0.3.0
line. It assumes the application already follows
[`MIGRATION_0.2.md`](MIGRATION_0.2.md). It does not repeat changes that were
already required to leave 0.1.x.

0.3.0 is a major-version upgrade. The main work is:

- remove 0.2 compatibility APIs that are gone in 0.3,
- migrate dependency providers to keyed outputs where the value type is not a
  unique dependency identity,
- replace raw server-route registration with endpoint metadata,
- review generated model-info relation fields and browser-visible user models,
- update Pages projects to the route-backed component and split
  client/server module layout,
- regenerate migrations when model metadata, field names, or unique
  constraints changed.

## Recommended order

1. Update `Cargo.toml` to the target 0.3 release.
2. Fix removed API references listed in the table below.
3. Migrate dependency providers and injection sites that need keyed identity.
4. Replace raw server-route registration with endpoint-based registration.
5. Review generated `{Model}Info` relation fields and DTO conversions.
6. Update Pages app layout and component route wrappers.
7. Regenerate migrations and review the diff.
8. Run the verification commands at the end of this guide.

## PR coverage

This guide is grouped by migration surface rather than merge order.

| Area | PRs | Migration action |
|---|---|---|
| Removed 0.2 compatibility APIs | #5362 | Follow "Removed API"; replace `AuthUser`, `create_resource*`, and `use_effect_event*`. |
| Dependency injection identity and WASM stubs | #5349, #5358, #5341 | Follow "Keyed dependency providers"; use `#[injectable]`, `FactoryOutput<K, T>`, `#[injectable_key]`, and `Depends<K, T>` when a provider output needs an explicit key. |
| WASM/native public API parity | #5338, #5324, #5342, #5417 | Follow "WASM/native parity"; keep shared app modules cfg-clean and rely on inert WASM stubs for `#[user]`, client pages, and provider symbols where documented. |
| URL routing | #5317 | Follow "Server route registration"; remove raw `function`, `route`, and method-specific handler registration calls. |
| Pages components, resources, portals, assets, activity boundaries, widgets | #5330, #5327, #5329, #5332, #5318, #5344, #5348 | Follow "Pages and components"; move route wrappers to `#[component]` modules and keep `use_resource(fetcher, deps)`. |
| Model metadata, ORM, and migrations | #5307, #5339, #5372, #5368 | Follow "Model info, partial updates, and migrations"; review relation-shaped info DTOs, `update_fields`, and replayed migration drift. |
| Pages scaffolding and tutorials | #5415, #5421, #5430, #5431 | Follow "Generated Pages project layout"; remove obsolete `pages.rs` wrappers and use app-local split directories. |

## Removed API

These APIs were deprecated or compatibility-only in 0.2 and are removed in
0.3. Replace the symbol first, then let `cargo check` surface surrounding type
changes.

| Crate | Removed API | Replacement |
|---|---|---|
| `reinhardt-auth` | `AuthUser<U>` | `CurrentUser<U>` |
| `reinhardt-pages` | `create_resource(fetcher)` | `use_resource(fetcher, ())` |
| `reinhardt-pages` | `create_resource_with_deps(fetcher, deps)` | `use_resource(fetcher, deps)` |
| `reinhardt-pages` | `use_effect_event(f)` | `use_callback(f, deps)` or `.get_untracked()` inside the effect |
| `reinhardt-pages` | `use_effect_event_with(f, deps)` | `use_callback_with(f, deps)` or `.get_untracked()` inside the effect |
| `reinhardt-urls` | `ServerRouter::function`, `ServerRouter::route`, `ServerRouter::handler_with_method`, and named variants | `#[get]` / `#[post]` / endpoint macros plus `.endpoint(factory)` |
| `reinhardt-urls` | `FunctionHandler` public re-export | endpoint-generated handler types or a custom `Handler` type registered with `.view(...)` |
| `reinhardt-di` | value-type-only provider identity for provider functions | `FactoryOutput<K, T>` with `Depends<K, T>` |
| `reinhardt-di` | `DependsResult` / `DependsOption` sugar aliases | `Depends<K, Result<T, E>>` / `Depends<K, Option<T>>` |

Quick scan:

```bash
rg -n "AuthUser|create_resource|create_resource_with_deps|use_effect_event|use_effect_event_with" src crates examples
rg -n "\\.(function|route|handler_with_method)(_named)?\\(|FunctionHandler|Depends(Result|Option)" src crates examples
rg -n "FactoryOutput<|Depends<[^,>]+>|injectable_factory|InjectableKey" src crates examples
rg -n "pages\\.rs|server_urls|client/pages|src/shared/(forms|types)\\.rs" src examples
```

## Auth extractor

`CurrentUser<U>` is the only full authenticated-user extractor.

```rust
// Before
use reinhardt_auth::AuthUser;

async fn profile(AuthUser(user): AuthUser<MyUser>) -> Response {
    let id = user.id();
    // ...
}

// After
use reinhardt_auth::CurrentUser;

async fn profile(CurrentUser(user): CurrentUser<MyUser>) -> Response {
    let id = user.id();
    // ...
}
```

Cookie-backed session apps should register `SessionMiddleware` once in
`urls.rs`. In 0.3, `SessionMiddleware` derives `AuthState` from
`USER_ID_SESSION_KEY` when the active session is authenticated and preserves an
existing `AuthState` inserted by earlier middleware. Standard
`SessionData` + `SessionAuthExt` + `CurrentUser<U>` setups do not need an
additional session-auth middleware layer.

`CookieSessionAuthMiddleware` remains available for projects that plug a
custom `AsyncSessionBackend` directly.

## Keyed dependency providers

0.3 makes provider identity explicit. Use direct `Injectable` values when the
type itself is the unique dependency identity. Use `FactoryOutput<K, T>` when a
provider function returns a value type that might have multiple meanings in one
application.

```rust
// Before: provider identity was the value type.
#[injectable_factory(scope = "singleton")]
async fn database(
    #[inject] settings: ProjectSettings,
) -> DatabaseConnection {
    DatabaseConnection::connect(&settings.database_url).await.unwrap()
}

async fn health(
    #[inject] db: Depends<DatabaseConnection>,
) -> Response {
    Response::ok()
}
```

```rust
// After: provider identity is the explicit key.
use reinhardt::di::{Depends, FactoryOutput, injectable, injectable_key};

#[injectable_key]
struct PrimaryDatabase;

#[injectable(scope = "singleton")]
async fn database(
    #[inject] settings: ProjectSettings,
) -> FactoryOutput<PrimaryDatabase, DatabaseConnection> {
    FactoryOutput::new(
        DatabaseConnection::connect(&settings.database_url)
            .await
            .unwrap(),
    )
}

async fn health(
    #[inject] db: Depends<PrimaryDatabase, DatabaseConnection>,
) -> Response {
    Response::ok()
}
```

If initialization can fail, keep the key in the first position and put
`Result<T, E>` in the provider value position:

```rust
#[injectable(scope = "singleton")]
async fn database(
    #[inject] settings: ProjectSettings,
) -> FactoryOutput<PrimaryDatabase, Result<DatabaseConnection, DbError>> {
    FactoryOutput::new(DatabaseConnection::connect(&settings.database_url).await)
}

async fn health(
    #[inject] db: Depends<PrimaryDatabase, Result<DatabaseConnection, DbError>>,
) -> Response {
    Response::ok()
}
```

`#[injectable_factory]` is retained only as a deprecated compatibility alias for
provider functions. New code should use `#[injectable]`.

## Server route registration

Raw server route registration was removed from the public migration surface.
Move free functions to endpoint macros and register the endpoint factory with
`.endpoint(...)`.

```rust
// Before
async fn health(_request: Request) -> Result<Response> {
    Ok(Response::ok())
}

let router = ServerRouter::new()
    .function("/health", Method::GET, health);
```

```rust
// After
#[get("/health", name = "health")]
async fn health() -> ViewResult<Response> {
    Ok(Response::ok())
}

let router = ServerRouter::new()
    .endpoint(health);
```

For named class-style handlers that already implement `Handler`, keep
`.view(...)` / `.view_named(...)` when the handler is intentionally
method-agnostic. For HTTP-method endpoints, prefer the endpoint macros so path,
method, and route name stay attached to the handler type.

## Resource hooks and effect callbacks

Use `use_resource(fetcher, deps)` for both mount-only and dependency-driven
resource loading.

```rust
// Before
let questions = create_resource_with_deps(fetch_questions, (page,));

// After
let questions = use_resource(fetch_questions, (page,));
```

For mount-only loading, pass `()`:

```rust
let user = use_resource(fetch_user, ());
```

Replace `use_effect_event` with a callback when the effect should call a
stable function, or read the latest signal value with `.get_untracked()` when
the value should not become a dependency of the effect.

```rust
// Before
let submit = use_effect_event(move || save(form.get()));

// After
let submit = use_callback(move || save(form.get()), (form,));
```

## Pages and components

0.3 adds route-backed component macros. Route wrappers should live under
`src/apps/<app>/client/components/`, not in a separate `pages.rs` wrapper.

```rust
use reinhardt::pages::component;
use reinhardt::pages::component::Page;

use crate::client::components::nav::with_nav;

#[component("/polls/{question_id}/", "detail")]
pub fn polls_detail(question_id: Path<i64>) -> Page {
    with_nav(super::polls_detail(question_id.into_inner()))
}
```

Register the component from the app-local client router:

```rust
use crate::apps::polls::client::components;
use reinhardt::ClientRouter;

pub fn client_url_patterns() -> ClientRouter {
    ClientRouter::new()
        .component(components::polls_detail::polls_detail)
}
```

Use the 0.3 Pages primitives directly where relevant:

- `#[wasm_server_api]` for cross-target server API signature parity,
- `Portal` / `mount_portal` for explicit portal lifetimes,
- `ActivityBoundary` and `ViewTransitionBoundary` for preserved hidden state
  and View Transition API coordination,
- `FieldArray` for dynamic form collections,
- generated file-field runtime support through `use_form`.

## Generated Pages project layout

Generated Pages apps now split target-specific implementation behind app-local
module declarations. App roots are declaration files; implementation lives in
subdirectories.

```text
src/apps/polls.rs
src/apps/polls/
├── client.rs
├── client/
│   ├── components.rs
│   └── components/
│       └── placeholder.rs
├── server.rs
├── server/
│   ├── admin.rs
│   ├── forms.rs
│   ├── models.rs
│   └── views.rs
├── serializers.rs
├── serializers/.gitkeep
├── server_fn.rs
├── server_fn/.gitkeep
├── services.rs
├── services/client.rs
├── services/client/.gitkeep
├── services/server.rs
├── services/server/.gitkeep
├── urls.rs
└── urls/
    ├── client_router.rs
    └── server_router.rs
```

Migration actions for existing generated Pages projects:

- delete app-local `pages.rs` and `client/pages.rs` wrappers after moving route
  wrappers into `client/components/*.rs`,
- rename `urls/server_urls.rs` to `urls/server_router.rs`,
- keep `urls/client_router.rs` behind `#[cfg(client)]` and
  `urls/server_router.rs` behind `#[cfg(server)]`,
- move server-only `admin`, `forms`, `models`, and `views` under the app-local
  `server/` directory,
- split app services into `services/client.rs` and `services/server.rs` when
  the service has target-specific dependencies,
- preserve empty generated directories with `.gitkeep` when the template
  expects users to add submodules later,
- remove the broad project-level `shared/forms.rs` and `shared/types.rs` files
  unless the application still has hand-written cross-app DTOs there.

## WASM/native parity

0.3 documents public API parity levels for symbols that applications can name
from both native and `wasm32-unknown-unknown` builds:

- P2: behavior exists on both targets,
- P1: symbol exists on both targets and one side is inert by design,
- P0: target-only behavior; misuse should fail at compile time.

Application code under `src/apps/**`, `src/config/**`, and shared DTO modules
should compile without broad call-site `#[cfg]` workarounds. The major 0.3
parity migrations are:

- `#[user]` is inert on WASM, so shared user model declarations can stay
  visible to browser builds while native builds get auth runtime behavior,
- `#[client_page]` emits native route-table stubs so client routes can be named
  from shared route aggregates,
- `#[injectable]` emits inert WASM provider stubs and skips native-only
  registration side effects.

## Model info, partial updates, and migrations

Generated `{Model}Info` relation fields now expose relation-shaped payloads:

- one-to-one / foreign-key fields use `RelationInfo<T>`,
- many-to-many fields use `ManyToManyInfo<Source, Target>`.

Review API DTOs, serializer outputs, and browser tests that previously
expected flattened `*_id` scalar fields.

`QuerySet` now supports atomic conditional partial updates:

```rust
let updated = User::objects()
    .filter(User::field_id().eq(user_id))
    .filter(User::field_is_active().eq(true))
    .update_fields([User::field_last_seen_at().assign(Utc::now())])
    .await?;
```

The update preserves the predicates already attached to the `QuerySet`.
Empty assignment lists and predicate-less partial updates are rejected at the
API boundary to avoid accidental table-wide updates.

Migration generation also has stricter drift handling. Regenerate and review
migrations after:

- model relations changed and generated info types changed shape,
- field renames are now unambiguous enough to emit `RenameColumn`,
- replayed migrations previously produced spurious `AlterColumn` or
  `AddConstraint` operations.

## Verification

Run the narrow checks first:

```bash
rg -n "AuthUser|create_resource|create_resource_with_deps|use_effect_event|use_effect_event_with" src crates examples
rg -n "\\.(function|route|handler_with_method)(_named)?\\(|FunctionHandler|Depends(Result|Option)" src crates examples
rg -n "pages\\.rs|server_urls|client/pages|src/shared/(forms|types)\\.rs" src examples
./scripts/validate-version-markers.sh
git diff --check
```

Then run the relevant project gates:

```bash
cargo make fmt-check
cargo make clippy-check
cargo test --doc
cargo doc --no-deps
```

For generated Pages projects, also scaffold a fresh project and verify both
native and WASM surfaces:

```bash
reinhardt-admin startproject /tmp/reinhardt-0-3-pages-check --template pages \
  --reinhardt-version "0.3.0" \
  --features minimal,pages,admin,conf,commands-server,commands-autoreload,db-sqlite,forms,auth-session,middleware,argon2-hasher,static-files \
  --default-features false \
  --no-interactive
```
