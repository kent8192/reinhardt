//! Admin integration tests module
//!
//! Cross-crate integration tests for admin panel components.

mod admin {
	mod admin_database_tests;
	mod admin_handler_logic_tests;
	mod server_fn_combination_tests;
	mod server_fn_create_tests;
	mod server_fn_delete_tests;
	mod server_fn_detail_tests;
	mod server_fn_e2e_tests;
	mod server_fn_export_tests;
	mod server_fn_fields_tests;
	mod server_fn_helpers;
	mod server_fn_import_tests;
	mod server_fn_list_tests;
	mod server_fn_login_tests;
	mod server_fn_middleware_e2e_tests;
	mod server_fn_middleware_helpers;
	mod server_fn_permission_tests;
	mod server_fn_state_transition_tests;
	mod server_fn_update_tests;
	mod server_fn_usecase_tests;
	mod server_fn_uuid_pk_tests;
}
