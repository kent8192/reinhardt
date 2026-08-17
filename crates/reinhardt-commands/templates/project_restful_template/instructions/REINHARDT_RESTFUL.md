# Reinhardt RESTful Guidance

## Project shape

Keep the generated entry points stable:

- `src/bin/manage.rs` is the native management CLI and development server
  entry point.
- `src/lib.rs` re-exports shared project items.
- `src/apps.rs` declares application modules.
- `src/config/apps.rs` owns the installed-app registry.
- `src/config/urls.rs` aggregates the project `UnifiedRouter`.
- `src/config/settings.rs` composes typed settings.
- An app's `models`, `serializers`, `services`, `views`, and `urls` modules own
  its domain and API surface.

## Adding apps

Use the RESTful scaffold so module declarations and app registration stay in
sync:

```bash
cargo run --bin manage startapp users
```

The command adds the app module under `src/apps/`, registers its
`#[app_config]`, and creates the app-local route aggregate. Keep handlers,
serializers, models, and services in that owning app rather than flattening
them into `src/config/`.

## Feature boundaries

Use this extraction test before implementing a feature: "Could this feature be
extracted and moved to another project?" If the answer is no, reduce coupling
until the feature can live inside an app created with `startapp`.

- Keep feature-owned models, serializers, services, views, and routes inside
  that app.
- Keep `src/config/` limited to project-wide settings and route composition.
- Connect apps through explicit serializable DTOs and framework contracts; do
  not reach into another app's private modules, models, or service state.
- Put reusable business logic in the owning app's `services` module so another
  endpoint, command, or test can use it without importing a view.

## Route aggregation

The project router mounts app aggregates under a literal API prefix:

```rust,ignore
#[routes]
pub fn routes() -> UnifiedRouter {
    UnifiedRouter::new().mount(
        "/api/",
        crate::apps::users::urls::server_url_patterns(),
    )
}
```

The app aggregate owns endpoint ordering and registration:

```rust,ignore
pub fn server_url_patterns() -> ServerRouter {
    ServerRouter::new()
        .endpoint(views::list)
        .endpoint(views::create)
        .endpoint(views::retrieve)
}
```

Register literal paths such as `/config/` before a dynamic `/{id}/` path when
both can match the same request shape.

## API contracts

- Keep request and response DTOs serializable, explicit, and versionable.
- Use endpoint macros for method metadata, stable names, and validation.
- Return explicit status codes and structured JSON error bodies.
- Use `pre_validate = true` for compatible extractors and manual validation
  for mixed primitive/validated extractor signatures.
- Keep OpenAPI or contract changes synchronized with the endpoint behavior.

## Models, services, and dependency injection

- Build persisted models with the generated `Model::build()` typestate builder.
- Keep reusable business logic in `services`; handlers should coordinate
  extraction, authorization, service calls, and response mapping.
- Resolve `DatabaseConnection`, `Depends<T>`, and keyed dependencies through
  `#[inject]`; do not create parallel pools or service containers in views.
- Use `CurrentUser<U>` and `guard!()` at the endpoint boundary for authn/authz.
- Keep database and filesystem work in native code paths.

## Settings and secrets

Compose typed settings from `settings/base.toml`, the selected profile, and
`REINHARDT_` environment overrides. Keep shared defaults in the tracked
`settings/*.example.toml` files; generated `settings/*.toml` files are ignored
because they can contain local credentials. Update the examples and Rust
settings fragment together when the settings shape changes.

## Verification

Run the narrowest relevant checks first, then the application checks:

```bash
cargo make fmt-check
cargo make quality
cargo run --bin manage check
cargo run --bin manage showurls
cargo make test
```

For API changes, add focused integration coverage for success, validation, and
error status paths. Do not add skeleton tests; every test must assert behavior
through a Reinhardt component and clean up its state.
