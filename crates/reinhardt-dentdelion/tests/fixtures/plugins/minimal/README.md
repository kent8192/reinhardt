# Minimal WASM Plugin for Dentdelion

This is a minimal test plugin that implements all required lifecycle functions with no additional capabilities. It serves as a basic fixture for integration tests.

## Structure

```
minimal/
├── Cargo.lock          # Locked fixture dependencies used by --locked builds
├── Cargo.toml          # Plugin manifest with WASM component metadata
├── wit/
│   └── dentdelion.wit  # WIT interface definition (copy from crates/reinhardt-dentdelion/wit/)
└── src/
    └── lib.rs          # Plugin implementation
```

## Implementation

The plugin implements the `reinhardt:dentdelion/plugin` interface with minimal functionality:

- **Metadata**: Returns basic plugin information (name: "minimal", version: "0.1.0")
- **Capabilities**: Returns empty list (no capabilities)
- **Lifecycle**:
  - `on_load()`: Accepts any configuration, returns Ok
  - `on_enable()`: No-op, returns Ok
  - `on_disable()`: No-op, returns Ok
  - `on_unload()`: No-op, returns Ok

## Test Contract

`tests/wasm_integration.rs` copies this complete source tree, except any stale
`target/` artifacts, into a temporary root every time the mandatory lifecycle
integration test runs. It builds the copied manifest with `cargo component build
--release --locked`, places `CARGO_TARGET_DIR` under the same temporary root,
and loads the generated Component. The copied source, any generated source or
lockfile changes, and all target artifacts are removed through RAII when the
test exits.

The repository does not store a generated `minimal_plugin.wasm`. A missing
`cargo-component` command, missing `wasm32-wasip1` target, compilation failure,
or missing Component output fails the test.

## Requirements

```bash
cargo install cargo-component
rustup target add wasm32-wasip1
```

## Focused Test

```bash
cargo nextest run -p reinhardt-dentdelion \
  --test wasm_integration \
  --all-features \
  -E 'test(source_built_minimal_component_enforces_lifecycle_contract)'
```
