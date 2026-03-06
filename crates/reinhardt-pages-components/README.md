# reinhardt-pages-components

UI component library for Reinhardt web framework with declarative `page!` and `form!` macros.

## Features

- **Declarative UI**: Build UIs using intuitive macro syntax
- **Type-Safe**: Compile-time validation with Rust's type system
- **Responsive**: Mobile-first design with 6 breakpoints
- **Accessible**: WAI-ARIA compliant components
- **Customizable**: Flexible theme system with CSS variables

## Component Categories

### Layout
- Container, Grid (Row/Col), Layout, Sidebar, Spacer

### Navigation
- Nav, Breadcrumb, Tabs, Pagination

### Feedback
- Alert, Toast, Modal, Tooltip, Progress

### Data Display
- Badge, Card, Avatar, Stat

### Input
- Button, Dropdown, Accordion

### Form
- LoginForm, RegisterForm, SearchForm, ContactForm, PasswordResetForm, SettingsForm

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
reinhardt-pages-components = "0.1"
```

Basic usage:

```rust
use reinhardt_pages_components::*;

let container = Container::new()
	.fluid(true)
	.add_child(Box::new(Alert {
		variant: Variant::Success,
		dismissible: true,
		icon: None,
		message: "Welcome!".into(),
	}));

let html = container.render();
```

Using macros:

```rust
use reinhardt_pages_components::*;

page! {
	Container {
		fluid: true,
		children: [
			Alert {
				variant: Success,
				dismissible: true,
				message: "Welcome!",
			},
		],
	}
}
```

## Documentation

See [API documentation](https://docs.rs/reinhardt-pages-components) for detailed usage.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE))
- MIT license ([LICENSE-MIT](../../LICENSE-MIT))

at your option.
