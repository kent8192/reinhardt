//! Admin integration tests module
//!
//! Cross-crate integration tests for admin panel components.

mod admin {
	mod admin_database_tests;
	mod admin_handler_logic_tests;
	mod server_fn_create_tests;
	mod server_fn_delete_tests;
	mod server_fn_detail_tests;
	mod server_fn_e2e_tests;
	mod server_fn_export_tests;
	mod server_fn_fields_tests;
	mod server_fn_helpers;
	mod server_fn_import_tests;
	mod server_fn_list_tests;
	mod server_fn_update_tests;
}
