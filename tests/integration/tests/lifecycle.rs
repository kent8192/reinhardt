//! Lifecycle integration tests for global state initialization ordering.
//!
//! These tests verify that global state patterns follow their specified
//! initialization contracts. They detect ordering bugs (like Issue #3033)
//! by testing the full lifecycle chain rather than bypassing it.

mod lifecycle {
	mod di_registration_lifecycle;
	mod global_router_lifecycle;
	mod reverse_relations_lifecycle;
	mod static_manifest_lifecycle;
	mod url_routes_lifecycle;
}
