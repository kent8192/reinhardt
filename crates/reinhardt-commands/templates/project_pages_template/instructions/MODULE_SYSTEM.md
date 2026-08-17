# Rust 2024 Module System

- Use `module.rs` as the entry point when a module has children.
- Put child modules in the sibling `module/` directory.
- Never add `mod.rs` files.
- Keep implementation modules private by default and expose a deliberate API with explicit `pub use`.
- Avoid glob re-exports and deep module nesting.

Example:

```text
src/apps/notes.rs
src/apps/notes/
├── urls.rs
└── client.rs
```

The parent entry point declares the children; it does not need a nested
`mod.rs` file.
