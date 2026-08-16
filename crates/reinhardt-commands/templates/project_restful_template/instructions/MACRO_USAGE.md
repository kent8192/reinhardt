# Macro Usage

- Use `#[routes]` for project route aggregation and keep app routes in app-local `urls` modules.
- Use endpoint macros such as `#[get]` and `#[post]` instead of raw method registration when the macro applies.
- Keep handlers in the owning app's views or endpoint module, not in `src/config/urls.rs`.
- Keep request and response contracts serializable and explicit.
- Use `#[model(app_label = "...")]` with the generated builder; do not add a redundant `#[derive(Model)]`.
- Prefer framework serializers, route helpers, and typed builders over parallel local infrastructure.
- Register viewsets or endpoint aggregates through app-local route modules before mounting them in project configuration.
