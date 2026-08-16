# Macro Usage

- Use `#[routes]` for project route aggregation and keep app routes in app-local `urls` modules.
- Use endpoint macros such as `#[get]` and `#[post]` instead of raw method registration when the macro applies.
- Give route-backed `#[component]` declarations an explicit `name`.
- Keep `#[server_fn]` declarations in app-local `server_fn.rs` and keep their request and response types serializable.
- Prefer `bind: signal`, `use_form`, `use_action`, `use_query`, and `use_mutation` for reactive Pages lifecycles instead of rebuilding state locally.
- Use `#[model(app_label = "...")]` with the generated builder; do not add a redundant `#[derive(Model)]`.
- Give images meaningful `alt` text and textless buttons an accessible label.
- Run `cargo make fmt-check` after changing `page!` or other Pages DSL code.
