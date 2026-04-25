//! Integration tests for {{ project_name }}.
//!
//! Tutorial Part 5 (Testing) builds on this file. The two recommended
//! approaches:
//!
//! 1. **`reinhardt-test` fixtures** (preferred): use shared fixtures that
//!    create tables from your `#[model]` definitions automatically.
//! 2. **Manual SQLite setup**: connect via `sqlx` and create tables with
//!    raw SQL — useful when you need precise control over the schema.
//!
//! See `examples/examples-tutorial-basis/tests/integration.rs` in the
//! Reinhardt repository for a worked example of both styles.

use rstest::*;

#[rstest]
fn smoke_crate_compiles() {
	// Arrange / Act
	// The crate links because this binary is part of the same package.
	let crate_name: &str = env!("CARGO_PKG_NAME");

	// Assert
	assert_eq!(crate_name, "{{ crate_name }}");
}
