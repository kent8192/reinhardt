//! Macro compile-time integration tests.
//!
//! This standalone test target keeps trybuild cases on the dedicated UI-test
//! profile instead of the default cross-crate integration-test profile.

#[path = "macros/admin_list_select_related_ui.rs"]
mod admin_list_select_related_ui;
#[path = "macros/admin_relation_ui.rs"]
mod admin_relation_ui;

#[path = "macros/http_error_ui.rs"]
mod http_error_ui;

#[path = "macros/model_info_ui.rs"]
mod model_info_ui;

#[path = "macros/model_form_server_context_ui.rs"]
mod model_form_server_context_ui;

#[path = "macros/model_unique_field_ref_ui.rs"]
mod model_unique_field_ref_ui;

#[path = "macros/model_enum_ui.rs"]
mod model_enum_ui;

#[path = "macros/model_file_field_ui.rs"]
mod model_file_field_ui;
