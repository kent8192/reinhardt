# reinhardt-manouche

DSL definitions for the `reinhardt-pages` macro system.

## Overview

This crate provides the Abstract Syntax Tree (AST) structures, parsing logic,
validation, and code generation infrastructure for the `page!`, `form!`, and
`head!` macros.

The name "manouche" comes from [Manouche Jazz](https://en.wikipedia.org/wiki/Gypsy_jazz),
a genre of music created by Django Reinhardt in the 1930s.

## Modules

- `core` - DSL types, Untyped/Typed AST, reactive traits
- `parser` - TokenStream -> Untyped AST
- `validator` - Untyped AST -> Typed AST (semantic analysis)
- `ir` - Typed AST -> Intermediate Representation
- `codegen` - IRVisitor trait definition

## Pipeline

```text
TokenStream -> parse -> Untyped AST -> validate -> Typed AST -> lower -> IR -> visit -> TokenStream
```

## Usage

This crate is typically used as a dependency of `reinhardt-pages-macros` to
provide the parsing and validation infrastructure for the page macros.

```rust,ignore
use reinhardt_manouche::parser::parse_page;
use reinhardt_manouche::validator::validate_page;

// Parse TokenStream to untyped AST
let untyped = parse_page(tokens)?;

// Validate and transform to typed AST
let typed = validate_page(&untyped)?;
```

## License

BSD 3-Clause License
