//! Integration tests for reinhardt-routers-macros
//!
//! This test suite validates the compile-time behavior of the path! macro
//! using trybuild for compile-fail tests and standard tests for success cases.

use reinhardt_routers_macros::path;

/// Basic path validation tests
mod basic_path_tests {
	use super::*;

	#[test]
	fn test_simple_static_paths() {
		// Simple static paths without parameters
		let path1 = path!("/");
		assert_eq!(path1, "/");

		let path2 = path!("/users/");
		assert_eq!(path2, "/users/");

		let path3 = path!("/api/v1/users/");
		assert_eq!(path3, "/api/v1/users/");

		let path4 = path!("/articles/latest/");
		assert_eq!(path4, "/articles/latest/");
	}

	#[test]
	fn test_paths_with_hyphens() {
		// Paths containing hyphens
		let path1 = path!("/user-profiles/");
		assert_eq!(path1, "/user-profiles/");

		let path2 = path!("/api-v1/user-data/");
		assert_eq!(path2, "/api-v1/user-data/");

		let path3 = path!("/contact-us/send-message/");
		assert_eq!(path3, "/contact-us/send-message/");
	}

	#[test]
	fn test_paths_with_underscores() {
		// Paths containing underscores
		let path1 = path!("/user_profiles/");
		assert_eq!(path1, "/user_profiles/");

		let path2 = path!("/api_v1/user_data/");
		assert_eq!(path2, "/api_v1/user_data/");
	}

	#[test]
	fn test_paths_with_dots() {
		// Paths containing dots (for file extensions or versioning)
		let path1 = path!("/files/document.pdf");
		assert_eq!(path1, "/files/document.pdf");

		let path2 = path!("/api/v1.0/users/");
		assert_eq!(path2, "/api/v1.0/users/");

		let path3 = path!("/downloads/report.2024.xlsx");
		assert_eq!(path3, "/downloads/report.2024.xlsx");
	}

	#[test]
	fn test_paths_with_numbers() {
		// Paths containing numeric segments
		let path1 = path!("/api/v1/");
		assert_eq!(path1, "/api/v1/");

		let path2 = path!("/api/v2/users/");
		assert_eq!(path2, "/api/v2/users/");

		let path3 = path!("/year/2024/month/12/");
		assert_eq!(path3, "/year/2024/month/12/");
	}

	#[test]
	fn test_paths_without_trailing_slash() {
		// Paths without trailing slash
		let path1 = path!("/users");
		assert_eq!(path1, "/users");

		let path2 = path!("/api/v1/items");
		assert_eq!(path2, "/api/v1/items");
	}
}

/// Path parameter validation tests
mod path_parameter_tests {
	use super::*;

	#[test]
	fn test_single_parameter_paths() {
		// Paths with a single parameter
		let path1 = path!("/users/{id}/");
		assert_eq!(path1, "/users/{id}/");

		let path2 = path!("/articles/{slug}/");
		assert_eq!(path2, "/articles/{slug}/");

		let path3 = path!("/items/{item_id}/");
		assert_eq!(path3, "/items/{item_id}/");
	}

	#[test]
	fn test_multiple_parameter_paths() {
		// Paths with multiple parameters
		let path1 = path!("/users/{user_id}/posts/{post_id}/");
		assert_eq!(path1, "/users/{user_id}/posts/{post_id}/");

		let path2 = path!("/year/{year}/month/{month}/day/{day}/");
		assert_eq!(path2, "/year/{year}/month/{month}/day/{day}/");

		let path3 = path!("/categories/{category_id}/items/{item_id}/reviews/{review_id}/");
		assert_eq!(
			path3,
			"/categories/{category_id}/items/{item_id}/reviews/{review_id}/"
		);
	}

	#[test]
	fn test_parameter_with_leading_underscore() {
		// Parameters starting with underscore
		let path1 = path!("/users/{_id}/");
		assert_eq!(path1, "/users/{_id}/");

		let path2 = path!("/items/{_item_id}/");
		assert_eq!(path2, "/items/{_item_id}/");

		// Single underscore parameter
		let path3 = path!("/data/{_}/");
		assert_eq!(path3, "/data/{_}/");
	}

	#[test]
	fn test_parameter_with_numbers() {
		// Parameters containing numbers
		let path1 = path!("/users/{user_id_123}/");
		assert_eq!(path1, "/users/{user_id_123}/");

		let path2 = path!("/v2_items/{item2_id}/");
		assert_eq!(path2, "/v2_items/{item2_id}/");

		let path3 = path!("/data/{param_1_2_3}/");
		assert_eq!(path3, "/data/{param_1_2_3}/");
	}

	#[test]
	fn test_consecutive_parameters() {
		// Multiple parameters without separating static segments
		let path1 = path!("/{year}/{month}/");
		assert_eq!(path1, "/{year}/{month}/");

		let path2 = path!("/{category}/{subcategory}/{item}/");
		assert_eq!(path2, "/{category}/{subcategory}/{item}/");
	}

	#[test]
	fn test_parameter_at_end_without_slash() {
		// Parameter at the end without trailing slash
		let path1 = path!("/users/{id}");
		assert_eq!(path1, "/users/{id}");

		let path2 = path!("/api/v1/items/{item_id}");
		assert_eq!(path2, "/api/v1/items/{item_id}");
	}

	#[test]
	fn test_mixed_static_and_parameter_segments() {
		// Complex paths mixing static segments and parameters
		let path1 = path!("/api/v1/users/{user_id}/profile/edit/");
		assert_eq!(path1, "/api/v1/users/{user_id}/profile/edit/");

		let path2 = path!("/shops/{shop_id}/products/{product_id}/reviews/latest/");
		assert_eq!(
			path2,
			"/shops/{shop_id}/products/{product_id}/reviews/latest/"
		);
	}
}

/// Wildcard and special pattern tests
mod wildcard_tests {
	use super::*;

	#[test]
	fn test_wildcard_at_end_of_path() {
		// Wildcard at the end of a path is valid
		let path1 = path!("/static/*");
		assert_eq!(path1, "/static/*");

		let path2 = path!("/files/*/");
		assert_eq!(path2, "/files/*/");
	}

	#[test]
	fn test_wildcard_with_parameters_at_end() {
		// Wildcard after parameters at end of path is valid
		let path1 = path!("/users/{id}/*");
		assert_eq!(path1, "/users/{id}/*");
	}
}

/// Edge case tests
mod edge_case_tests {
	use super::*;

	#[test]
	fn test_root_path() {
		// Root path only
		let path = path!("/");
		assert_eq!(path, "/");
	}

	#[test]
	fn test_very_long_path() {
		// Very long path with many segments
		let path = path!(
			"/api/v1/organizations/{org_id}/projects/{project_id}/repositories/{repo_id}/branches/{branch_id}/commits/{commit_id}/"
		);
		assert_eq!(
			path,
			"/api/v1/organizations/{org_id}/projects/{project_id}/repositories/{repo_id}/branches/{branch_id}/commits/{commit_id}/"
		);
	}

	#[test]
	fn test_path_with_many_parameters() {
		// Path with many parameters
		let path = path!("/{a}/{b}/{c}/{d}/{e}/{f}/{g}/{h}/");
		assert_eq!(path, "/{a}/{b}/{c}/{d}/{e}/{f}/{g}/{h}/");
	}

	#[test]
	fn test_parameter_with_long_name() {
		// Parameter with very long name
		let path =
			path!("/users/{very_long_parameter_name_with_many_underscores_and_numbers_123}/");
		assert_eq!(
			path,
			"/users/{very_long_parameter_name_with_many_underscores_and_numbers_123}/"
		);
	}

	#[test]
	fn test_mixed_case_in_static_segments() {
		// Static segments with mixed case letters
		let path1 = path!("/Users/");
		assert_eq!(path1, "/Users/");

		let path2 = path!("/API/V1/Items/");
		assert_eq!(path2, "/API/V1/Items/");
	}
}

/// Integration tests combining path macro with expected use cases
mod integration_scenarios {
	use super::*;

	#[test]
	fn test_restful_api_paths() {
		// RESTful API route patterns
		let list_path = path!("/api/users/");
		assert_eq!(list_path, "/api/users/");

		let detail_path = path!("/api/users/{id}/");
		assert_eq!(detail_path, "/api/users/{id}/");

		let nested_path = path!("/api/users/{user_id}/posts/{post_id}/comments/");
		assert_eq!(
			nested_path,
			"/api/users/{user_id}/posts/{post_id}/comments/"
		);
	}

	#[test]
	fn test_versioned_api_paths() {
		// API versioning patterns
		let v1_path = path!("/api/v1/users/");
		assert_eq!(v1_path, "/api/v1/users/");

		let v2_path = path!("/api/v2/users/{id}/");
		assert_eq!(v2_path, "/api/v2/users/{id}/");

		let v1_2_path = path!("/api/v1.2/items/{item_id}/");
		assert_eq!(v1_2_path, "/api/v1.2/items/{item_id}/");
	}

	#[test]
	fn test_admin_panel_paths() {
		// Admin panel route patterns
		let dashboard = path!("/admin/dashboard/");
		assert_eq!(dashboard, "/admin/dashboard/");

		let user_edit = path!("/admin/users/{id}/edit/");
		assert_eq!(user_edit, "/admin/users/{id}/edit/");

		let settings = path!("/admin/settings/general/");
		assert_eq!(settings, "/admin/settings/general/");
	}

	#[test]
	fn test_file_serving_paths() {
		// File serving patterns
		let static_file = path!("/static/css/main.css");
		assert_eq!(static_file, "/static/css/main.css");

		let media_file = path!("/media/uploads/{filename}");
		assert_eq!(media_file, "/media/uploads/{filename}");

		let wildcard_static = path!("/static/*");
		assert_eq!(wildcard_static, "/static/*");
	}

	#[test]
	fn test_auth_paths() {
		// Authentication route patterns
		let login = path!("/auth/login/");
		assert_eq!(login, "/auth/login/");

		let logout = path!("/auth/logout/");
		assert_eq!(logout, "/auth/logout/");

		let password_reset = path!("/auth/password-reset/{token}/");
		assert_eq!(password_reset, "/auth/password-reset/{token}/");
	}

	#[test]
	fn test_multi_tenant_paths() {
		// Multi-tenant route patterns
		let tenant_home = path!("/tenant/{tenant_id}/");
		assert_eq!(tenant_home, "/tenant/{tenant_id}/");

		let tenant_users = path!("/tenant/{tenant_id}/users/{user_id}/");
		assert_eq!(tenant_users, "/tenant/{tenant_id}/users/{user_id}/");

		let tenant_settings = path!("/tenant/{tenant_id}/settings/billing/");
		assert_eq!(tenant_settings, "/tenant/{tenant_id}/settings/billing/");
	}

	#[test]
	fn test_date_based_paths() {
		// Date-based route patterns
		let year_archive = path!("/archive/{year}/");
		assert_eq!(year_archive, "/archive/{year}/");

		let month_archive = path!("/archive/{year}/{month}/");
		assert_eq!(month_archive, "/archive/{year}/{month}/");

		let day_archive = path!("/archive/{year}/{month}/{day}/");
		assert_eq!(day_archive, "/archive/{year}/{month}/{day}/");
	}

	#[test]
	fn test_slugified_paths() {
		// Slug-based route patterns
		let article_by_slug = path!("/articles/{slug}/");
		assert_eq!(article_by_slug, "/articles/{slug}/");

		let category_items = path!("/category/{category_slug}/items/");
		assert_eq!(category_items, "/category/{category_slug}/items/");

		let nested_slug = path!("/blog/{year}/{month}/{slug}/");
		assert_eq!(nested_slug, "/blog/{year}/{month}/{slug}/");
	}

	#[test]
	fn test_action_based_paths() {
		// Action-based route patterns
		let create = path!("/users/create/");
		assert_eq!(create, "/users/create/");

		let edit = path!("/users/{id}/edit/");
		assert_eq!(edit, "/users/{id}/edit/");

		let delete = path!("/users/{id}/delete/");
		assert_eq!(delete, "/users/{id}/delete/");

		let confirm = path!("/users/{id}/delete/confirm/");
		assert_eq!(confirm, "/users/{id}/delete/confirm/");
	}
}

/// Unicode and international character tests
mod unicode_tests {
	use super::*;

	#[test]
	fn test_ascii_only_paths() {
		// Ensure basic ASCII paths work correctly
		let path1 = path!("/hello/world/");
		assert_eq!(path1, "/hello/world/");

		let path2 = path!("/user-123/data_456/");
		assert_eq!(path2, "/user-123/data_456/");
	}

	// Note: The current implementation only supports ASCII characters
	// in path segments (excluding parameters). If Unicode support is
	// needed in the future, additional tests should be added here.
}

/// Macro expansion tests
mod macro_behavior_tests {
	use super::*;

	#[test]
	fn test_macro_returns_string_literal() {
		// Verify that the macro returns the input string literal
		let path = path!("/users/{id}/");

		// The path can be used as a &str
		let _as_str: &str = path;

		// The path can be compared with string literals
		assert_eq!(path, "/users/{id}/");
	}

	#[test]
	fn test_macro_in_const_context() {
		// Verify that the macro can be used in const context
		const USER_PATH: &str = path!("/users/");
		const ITEM_PATH: &str = path!("/items/{id}/");

		assert_eq!(USER_PATH, "/users/");
		assert_eq!(ITEM_PATH, "/items/{id}/");
	}

	#[test]
	fn test_macro_in_static_context() {
		// Verify that the macro can be used in static context
		static API_USERS: &str = path!("/api/users/");
		static API_USER_DETAIL: &str = path!("/api/users/{id}/");

		assert_eq!(API_USERS, "/api/users/");
		assert_eq!(API_USER_DETAIL, "/api/users/{id}/");
	}

	#[test]
	fn test_multiple_macro_invocations() {
		// Multiple macro invocations in the same scope
		let path1 = path!("/path1/");
		let path2 = path!("/path2/");
		let path3 = path!("/path3/{id}/");

		assert_eq!(path1, "/path1/");
		assert_eq!(path2, "/path2/");
		assert_eq!(path3, "/path3/{id}/");
	}
}

/// Performance and scalability tests
mod performance_tests {
	use super::*;

	#[test]
	fn test_deeply_nested_path() {
		// Test path with many nested segments
		let path = path!("/a/b/c/d/e/f/g/h/i/j/k/l/m/n/o/p/q/r/s/t/u/v/w/x/y/z/");
		assert_eq!(
			path,
			"/a/b/c/d/e/f/g/h/i/j/k/l/m/n/o/p/q/r/s/t/u/v/w/x/y/z/"
		);
	}

	#[test]
	fn test_many_parameters_in_path() {
		// Test path with many parameters
		let path = path!("/{p1}/{p2}/{p3}/{p4}/{p5}/{p6}/{p7}/{p8}/{p9}/{p10}/");
		assert_eq!(path, "/{p1}/{p2}/{p3}/{p4}/{p5}/{p6}/{p7}/{p8}/{p9}/{p10}/");
	}

	#[test]
	fn test_long_static_segment() {
		// Test path with very long static segment
		let path = path!("/this_is_a_very_long_static_segment_name_that_contains_many_characters/");
		assert_eq!(
			path,
			"/this_is_a_very_long_static_segment_name_that_contains_many_characters/"
		);
	}

	#[test]
	fn test_long_parameter_name() {
		// Test path with very long parameter name
		let path = path!(
			"/{this_is_a_very_long_parameter_name_with_many_underscores_and_alphanumeric_chars_123}/"
		);
		assert_eq!(
			path,
			"/{this_is_a_very_long_parameter_name_with_many_underscores_and_alphanumeric_chars_123}/"
		);
	}
}
