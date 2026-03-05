# reinhardt-pages-components-macros

Procedural macros for [reinhardt-pages-components](../reinhardt-pages-components).

This crate provides the `page!` and `form!` macros for declarative UI construction.

## Usage

This crate is typically used through `reinhardt-pages-components`:

```rust
use reinhardt_pages_components::*;

page! {
	Container {
		children: [Alert { message: "Hello" }],
	}
}
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE))
- MIT license ([LICENSE-MIT](../../LICENSE-MIT))

at your option.
