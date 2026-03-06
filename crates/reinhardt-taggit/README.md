# reinhardt-taggit

Simple tagging system for Reinhardt framework, inspired by django-taggit.

## Current Features

- **Tag Model**: Core tag entities with normalized names and URL-friendly slugs
- **Tagged Item Model**: Polymorphic many-to-many relationships
- **Error Types**: Comprehensive error handling for tag operations

## Planned Features

- **TaggableManager**: High-level API for managing tags on model instances
- **#[taggable] Macro**: Zero-boilerplate tagging with attribute macro
- **Query API**: Filter objects by tags, tag clouds, and statistics
- **Tag Normalization**: Configurable tag name normalization strategies

## Quick Start

```rust
use reinhardt_taggit::Tag;

// Create a tag with auto-generated slug
let tag = Tag::from_name("Rust Programming");
assert_eq!(tag.slug, "rust-programming");

// Create a tag with explicit slug
let tag = Tag::new("Rust Programming", "rust-prog");
assert_eq!(tag.slug, "rust-prog");
```

## Status

Under Development - This crate is in active development and not yet ready for production use.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](../../LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
