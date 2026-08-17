# Macro Usage

Prefer Reinhardt macros for the surface they own. They carry route metadata,
registration, validation, or client/server parity that hand-written
equivalents can miss.

## `#[routes]`

- Keep the project route aggregate in `src/config/urls.rs`.
- Aggregate each app through its `urls` module instead of importing individual
  handlers into project configuration.
- Keep server and client route tables behind their target cfg aliases.

```rust,ignore
#[routes]
pub fn routes() -> UnifiedRouter {
    UnifiedRouter::new()
        .merge(crate::apps::notes::urls::url_patterns())
        .merge(crate::apps::accounts::urls::url_patterns())
}
```

`UnifiedRouter` and the generated app-level `url_patterns()` aggregates expose
the same builder shape on native and WASM targets. Add one `merge` per app; do
not add target `cfg` branches around the apps in the project-level `routes`
function. Retain gates inside an app's `urls.rs` only for its target-specific
route modules.

## Endpoint and component macros

- Use endpoint macros such as `#[get]` and `#[post]` for HTTP routes.
- Use `pre_validate = true` when every extractor implements `Validate`; keep
  manual validation when a handler mixes validated JSON with primitive path
  extractors.
- Keep route-backed `#[component]` declarations in app-local client modules and
  give them an explicit `name` when the component participates in routing.
- Give images meaningful `alt` text and icon-only buttons an accessible label.

### `#[component]`

Use `#[component("/path/", name = "route-name")]` for a route-backed Pages
function. The function must be synchronous, non-generic, and return `Page`.
Path and query parameters are extracted from the route and passed to the
function; keep the component in the owning app's client module.

```rust,ignore
use reinhardt::pages::{Page, Path, component, page};

#[component("/notes/{id}/", name = "notes-detail")]
pub fn notes_detail(Path(id): Path<i64>) -> Page {
    page!(|id: i64| {
        article {
            h1 { { format!("Note {id}") } }
        }
    })(id)
}
```

Register the component through the app's `urls/client_router.rs`. The route
name is the stable key used by route reversal; always choose an explicit,
unique name. Optional `loader = path::to_loader` binds a route-level loader,
which must also provide a matching extracted loader argument.

```rust,ignore
#[get("/health/", name = "health")]
pub async fn health() -> ViewResult<Response> {
    Ok(Response::new(StatusCode::OK).with_body("ok".to_string()))
}
```

## Reactive Pages APIs

Prefer the framework lifecycle helpers over rebuilding reactive state locally:

- Use `bind: signal` for two-way form controls.
- Use `use_form` for validated form state and submission.
- Use `use_action`, `use_query`, and `use_mutation` for asynchronous work.
- Use `watch {}` or the current reactive helper when rendering depends on a
  changing signal value.

## `#[server_fn]`

- Keep declarations in an app-local `server_fn.rs` and split target-specific
  implementation into `server_fn/` when the app grows.
- Request and response types must be serializable because the WASM stub and
  native implementation share the declaration.
- Resolve server dependencies with `#[inject]` rather than creating a second
  connection or service inside the handler.

```rust,ignore
#[server_fn]
pub async fn load_notes(
    #[inject] _db: DatabaseConnection,
) -> Result<Vec<NoteInfo>, ServerFnError> {
    Ok(Vec::new())
}
```

## `#[model(...)]` and `form!`

- `#[model(...)]` applies the model derive; do not add a redundant
  `#[derive(Model)]`.
- Prefer the generated typestate builder (`Model::build()`) over struct
  literals when constructing persistent models.
- Use `form!` widgets whose value type matches the HTML control and keep
  validation in the generated form contract.

```rust,ignore
#[model(app_label = "notes", table_name = "notes")]
#[derive(Serialize, Deserialize)]
pub struct Note {
    #[field(primary_key = true)]
    pub id: i64,
    #[field(max_length = 255)]
    pub title: String,
}
```

## Dependency injection

Use the framework's typed dependencies instead of parallel local containers.
For a non-unique service type, use an explicit key. `CurrentUser<U>` and
`guard!()` keep authentication and authorization at the endpoint boundary.

```rust,ignore
#[get("/me/", name = "current-user")]
pub async fn me(
    #[inject] CurrentUser(user): CurrentUser<User>,
) -> ViewResult<Response> {
    Ok(Response::new(StatusCode::OK).with_body(user.email().to_string()))
}
```

Run `cargo make fmt-check` after changing `page!` or other Pages DSL code;
rustfmt alone does not validate the Pages formatter output.
