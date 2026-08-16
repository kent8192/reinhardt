# CLAUDE.md

## Purpose

These instructions apply to work on `{{ project_name }}`, a Reinhardt Pages project.

## Project Structure

- Use Rust 2024 module layout: `module.rs` with a sibling `module/` directory.
- Never create `mod.rs`.
- Keep applications under `src/apps/` and project configuration under `src/config/`.
- Use `cargo run --bin manage startapp <name> --with-pages` to add applications so generated registries stay synchronized.

## Dependencies and Imports

- Import framework APIs through the `reinhardt` facade.
- Import external crates only when they are declared in `Cargo.toml`.
- Prefer borrowing over unnecessary allocation or cloning.

## Code Quality

- Manage files, locks, connections, temporary state, and other resources with RAII guards.
- Remove obsolete code instead of retaining commented-out implementations.

## Testing

- Give every test meaningful assertions.
- Tests of framework behavior must exercise at least one Reinhardt component.
- Keep Arrange, Act, and Assert phases clear.
- Clean up every artifact created by a test.

## Documentation

Update `README.md` and other relevant documentation whenever behavior, configuration, commands, or project layout changes.

## Pages Native/WASM Boundaries

- Keep shared data contracts and target-neutral declarations outside client-only and server-only modules.
- Keep browser code under `src/client/` or an app's `client/` modules.
- Keep native-only models, forms, views, admin wiring, and infrastructure under an app's `server/` modules.
- Respect the generated `client` and `server` cfg aliases; do not import native-only dependencies into WASM code.
- Verify both native and browser targets after changing a shared boundary.

## Verification

Run the generated project checks before handing off changes:

For a fresh environment, run `cargo make install-tools` first. It installs the
WASM target, `cargo-nextest`, `wasm-pack`, and `cargo-watch`. Install Google
Chrome and a matching ChromeDriver and make both available on `PATH` before
running `cargo make wasm-test`.

```bash
cargo make install-tools
cargo make fmt-check
cargo make quality
cargo make test
cargo make wasm-test
```

Run `cargo make wasm-build-dev` when a change affects the browser bundle or shared client code.

## Guidance Synchronization

`AGENTS.md` and `CLAUDE.md` are a deliberate mirror pair. Update both files in the same change and keep their content identical except for the filename used as the top-level title.
