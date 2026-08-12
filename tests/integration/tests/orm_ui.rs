//! ORM compile-time integration tests.
//!
//! This standalone test target keeps trybuild cases on the dedicated UI-test
//! profile instead of the default cross-crate integration-test profile.

#[path = "orm/custom_manager_ui.rs"]
mod custom_manager_ui;

#[path = "orm/queryset_docs_ui.rs"]
mod queryset_docs_ui;

#[path = "orm/upsert_builder_ui.rs"]
mod upsert_builder_ui;
