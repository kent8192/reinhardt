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
    let router = UnifiedRouter::new();

    #[cfg(server)]
    let router = router.server(|server| {
        server.mount("/", crate::apps::notes::urls::server_url_patterns())
    });

    #[cfg(client)]
    let router = router.mount_unified(
        "/",
        UnifiedRouter::new().client(|_| crate::apps::notes::urls::client_url_patterns()),
    );

    router
}
```

## Endpoint and component macros

- Use endpoint macros such as `#[get]` and `#[post]` for HTTP routes.
- Use `pre_validate = true` when every extractor implements `Validate`; keep
  manual validation when a handler mixes validated JSON with primitive path
  extractors.
- Keep route-backed `#[component]` declarations in app-local client modules and
  give them an explicit `name` when the component participates in routing.
- Give images meaningful `alt` text and icon-only buttons an accessible label.

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
