#![cfg(not(target_arch = "wasm32"))]
//! Authentication Integration Tests
//!
//! Tests for the authentication system's integration with reactive signals,
//! global state management, and server synchronization.
//!
//! Success Criteria:
//! 1. Basic authentication state operations work correctly
//! 2. Permission management system functions as expected
//! 3. Reactive updates propagate correctly through signals and effects
//! 4. Global state is shared correctly across instances
//!
//! Test Categories:
//! - Category 1: Basic Authentication State (10 tests)
//! - Category 2: Permission Management (10 tests)
//! - Category 3: Reactive Updates (7 tests)
//! - Category 4: Global State Integration (5 tests)
//!
//! Total: 32 tests

use reinhardt_pages::auth::{AuthData, AuthState, auth_state};
use reinhardt_pages::reactive::{Effect, Signal, with_runtime};
use std::collections::HashSet;

// ============================================================================
// Category 1: Basic Authentication State (10 tests)
// ============================================================================

/// Tests initial unauthenticated state
#[test]
fn test_initial_unauthenticated_state() {
	let state = AuthState::new();

	assert!(!state.is_authenticated());
	assert!(state.user_id().is_none());
	assert!(state.username().is_none());
	assert!(state.email().is_none());
	assert!(!state.is_staff());
	assert!(!state.is_superuser());
}

/// Tests basic login operation
#[test]
fn test_basic_login() {
	let state = AuthState::new();
	state.login("1", "testuser");

	assert!(state.is_authenticated());
	assert_eq!(state.user_id(), Some("1".to_string()));
	assert_eq!(state.username(), Some("testuser".to_string()));
}

/// Tests login with full user information
#[test]
fn test_login_full_with_all_fields() {
	let state = AuthState::new();
	state.login_full(
		"42",
		"admin",
		Some("admin@example.com".to_string()),
		true,
		true,
	);

	assert!(state.is_authenticated());
	assert_eq!(state.user_id(), Some("42".to_string()));
	assert_eq!(state.username(), Some("admin".to_string()));
	assert_eq!(state.email(), Some("admin@example.com".to_string()));
	assert!(state.is_staff());
	assert!(state.is_superuser());
}

/// Tests login without email
#[test]
fn test_login_full_without_email() {
	let state = AuthState::new();
	state.login_full("10", "staff_user", None, true, false);

	assert!(state.is_authenticated());
	assert!(state.email().is_none());
	assert!(state.is_staff());
	assert!(!state.is_superuser());
}

/// Tests logout operation
#[test]
fn test_logout_clears_all_fields() {
	let state = AuthState::new();
	state.login_full(
		"1",
		"user",
		Some("user@example.com".to_string()),
		true,
		false,
	);
	state.logout();

	assert!(!state.is_authenticated());
	assert!(state.user_id().is_none());
	assert!(state.username().is_none());
	assert!(state.email().is_none());
	assert!(!state.is_staff());
	assert!(!state.is_superuser());
}

/// Tests update with AuthData
#[test]
fn test_update_with_auth_data() {
	let state = AuthState::new();
	state.login("1", "initial");

	let new_data = AuthData::authenticated("2", "updated");
	state.update(new_data);

	assert_eq!(state.user_id(), Some("2".to_string()));
	assert_eq!(state.username(), Some("updated".to_string()));
}

/// Tests creating state from server data
#[test]
fn test_from_server_data_authenticated() {
	let data = AuthData::full(
		"99",
		"serveruser",
		Some("server@example.com".to_string()),
		false,
		false,
	);
	let state = AuthState::from_server_data(data);

	assert!(state.is_authenticated());
	assert_eq!(state.user_id(), Some("99".to_string()));
	assert_eq!(state.username(), Some("serveruser".to_string()));
	assert_eq!(state.email(), Some("server@example.com".to_string()));
}

/// Tests creating state from anonymous server data
#[test]
fn test_from_server_data_anonymous() {
	let data = AuthData::anonymous();
	let state = AuthState::from_server_data(data);

	assert!(!state.is_authenticated());
	assert!(state.user_id().is_none());
	assert!(state.username().is_none());
}

/// Tests multiple login/logout cycles
#[test]
fn test_multiple_login_logout_cycles() {
	let state = AuthState::new();

	// Cycle 1
	state.login("1", "user1");
	assert!(state.is_authenticated());
	state.logout();
	assert!(!state.is_authenticated());

	// Cycle 2
	state.login("2", "user2");
	assert_eq!(state.user_id(), Some("2".to_string()));
	state.logout();
	assert!(state.user_id().is_none());

	// Cycle 3
	state.login_full(
		"3",
		"user3",
		Some("user3@example.com".to_string()),
		true,
		false,
	);
	assert!(state.is_staff());
	state.logout();
	assert!(!state.is_staff());
}

/// Tests login overwrite
#[test]
fn test_login_overwrites_previous_session() {
	let state = AuthState::new();
	state.login("1", "first_user");
	state.login("2", "second_user");

	assert_eq!(state.user_id(), Some("2".to_string()));
	assert_eq!(state.username(), Some("second_user".to_string()));
}

// ============================================================================
// Category 2: Permission Management (10 tests)
// ============================================================================

/// Tests basic permission check
#[test]
fn test_has_permission_basic() {
	let state = AuthState::new();
	let mut perms = HashSet::new();
	perms.insert("blog.add_post".to_string());
	state.set_permissions(perms);

	assert!(state.has_permission("blog.add_post"));
	assert!(!state.has_permission("blog.delete_post"));
}

/// Tests superuser has all permissions
#[test]
fn test_superuser_bypass_permission_check() {
	let state = AuthState::new();
	state.login_full("1", "superadmin", None, true, true);

	// Superuser should have any permission without explicit grant
	assert!(state.has_permission("any.random.permission"));
	assert!(state.has_permission("blog.delete_post"));
	assert!(state.has_permission("admin.access"));
}

/// Tests has_any_permission with multiple permissions
#[test]
fn test_has_any_permission_partial_match() {
	let state = AuthState::new();
	let mut perms = HashSet::new();
	perms.insert("blog.view".to_string());
	perms.insert("blog.edit".to_string());
	state.set_permissions(perms);

	assert!(state.has_any_permission(&["blog.view", "blog.delete"]));
	assert!(state.has_any_permission(&["blog.edit", "blog.add"]));
	assert!(!state.has_any_permission(&["blog.delete", "blog.add"]));
}

/// Tests has_any_permission for superuser
#[test]
fn test_has_any_permission_superuser() {
	let state = AuthState::new();
	state.login_full("1", "admin", None, true, true);

	assert!(state.has_any_permission(&["any.perm1", "any.perm2"]));
}

/// Tests has_all_permissions with complete match
#[test]
fn test_has_all_permissions_complete_match() {
	let state = AuthState::new();
	let mut perms = HashSet::new();
	perms.insert("blog.view".to_string());
	perms.insert("blog.edit".to_string());
	perms.insert("blog.delete".to_string());
	state.set_permissions(perms);

	assert!(state.has_all_permissions(&["blog.view", "blog.edit"]));
	assert!(state.has_all_permissions(&["blog.view", "blog.edit", "blog.delete"]));
	assert!(!state.has_all_permissions(&["blog.view", "blog.add"]));
}

/// Tests has_all_permissions for superuser
#[test]
fn test_has_all_permissions_superuser() {
	let state = AuthState::new();
	state.login_full("1", "admin", None, true, true);

	assert!(state.has_all_permissions(&["perm1", "perm2", "perm3"]));
}

/// Tests permission cache update
#[test]
fn test_permission_cache_update() {
	let state = AuthState::new();

	// Set initial permissions
	let mut perms1 = HashSet::new();
	perms1.insert("blog.view".to_string());
	state.set_permissions(perms1);
	assert!(state.has_permission("blog.view"));

	// Update permissions
	let mut perms2 = HashSet::new();
	perms2.insert("blog.edit".to_string());
	state.set_permissions(perms2);
	assert!(!state.has_permission("blog.view"));
	assert!(state.has_permission("blog.edit"));
}

/// Tests permissions are cleared on logout
#[test]
fn test_permissions_cleared_on_logout() {
	let state = AuthState::new();
	let mut perms = HashSet::new();
	perms.insert("blog.add_post".to_string());
	perms.insert("blog.edit_post".to_string());
	state.set_permissions(perms);
	state.login("1", "user");

	state.logout();

	assert!(!state.has_permission("blog.add_post"));
	assert!(!state.has_permission("blog.edit_post"));
}

/// Tests permissions from AuthData
#[test]
fn test_permissions_from_auth_data_initialization() {
	let data = AuthData {
		is_authenticated: true,
		user_id: Some("1".to_string()),
		username: Some("user".to_string()),
		email: None,
		is_staff: false,
		is_superuser: false,
		permissions: vec![
			"blog.view".to_string(),
			"blog.edit".to_string(),
			"comment.add".to_string(),
		],
	};
	let state = AuthState::from_server_data(data);

	assert!(state.has_permission("blog.view"));
	assert!(state.has_permission("blog.edit"));
	assert!(state.has_permission("comment.add"));
	assert!(!state.has_permission("blog.delete"));
}

/// Tests permissions update via AuthData update
#[test]
fn test_permissions_updated_via_auth_data_update() {
	let state = AuthState::new();
	state.login("1", "user");

	let data = AuthData {
		is_authenticated: true,
		user_id: Some("1".to_string()),
		username: Some("user".to_string()),
		email: None,
		is_staff: false,
		is_superuser: false,
		permissions: vec!["blog.view".to_string(), "blog.delete".to_string()],
	};
	state.update(data);

	assert!(state.has_permission("blog.view"));
	assert!(state.has_permission("blog.delete"));
	assert!(!state.has_permission("blog.edit"));
}

// ============================================================================
// Category 3: Reactive Updates (7 tests)
// ============================================================================

/// Tests signal updates when login is called
#[test]
fn test_signal_updates_on_login() {
	let state = AuthState::new();
	let auth_signal = state.is_authenticated_signal();
	let user_id_signal = state.user_id_signal();

	assert!(!auth_signal.get());
	assert!(user_id_signal.get().is_none());

	state.login("100", "reactive_user");

	assert!(auth_signal.get());
	assert_eq!(user_id_signal.get(), Some("100".to_string()));
}

/// Tests signal updates when logout is called
#[test]
fn test_signal_updates_on_logout() {
	let state = AuthState::new();
	state.login("200", "temp_user");

	let auth_signal = state.is_authenticated_signal();
	assert!(auth_signal.get());

	state.logout();

	assert!(!auth_signal.get());
}

/// Tests effect triggered by authentication change
#[test]
#[serial_test::serial(reactive)]
fn test_effect_triggered_by_auth_change() {
	let state = AuthState::new();
	let auth_signal = state.is_authenticated_signal();
	let triggered = Signal::new(false);

	let triggered_clone = triggered.clone();
	let _effect = Effect::new(move || {
		if auth_signal.get() {
			triggered_clone.set(true);
		}
	});

	// Effect should not be triggered initially
	assert!(!triggered.get());

	// Trigger effect by logging in
	state.login("1", "effect_test");
	with_runtime(|rt| rt.flush_updates());

	// Effect should be triggered
	assert!(triggered.get());
}

/// Tests effect triggered by user_id change
#[test]
#[serial_test::serial(reactive)]
fn test_effect_triggered_by_user_id_change() {
	let state = AuthState::new();
	let user_id_signal = state.user_id_signal();
	let captured_id = Signal::new(None);

	let captured_clone = captured_id.clone();
	let _effect = Effect::new(move || {
		captured_clone.set(user_id_signal.get());
	});

	state.login("42", "user");
	with_runtime(|rt| rt.flush_updates());

	assert_eq!(captured_id.get(), Some("42".to_string()));
}

/// Tests effect triggered by permission change
#[test]
#[serial_test::serial(reactive)]
fn test_effect_triggered_by_permission_change() {
	let state = AuthState::new();
	let perm_signal = state.permissions_signal();
	let perm_count = Signal::new(0);

	let count_clone = perm_count.clone();
	let _effect = Effect::new(move || {
		count_clone.set(perm_signal.get().len());
	});

	let mut perms = HashSet::new();
	perms.insert("perm1".to_string());
	perms.insert("perm2".to_string());
	state.set_permissions(perms);
	with_runtime(|rt| rt.flush_updates());

	assert_eq!(perm_count.get(), 2);
}

/// Tests multiple effects on same signal
#[test]
#[serial_test::serial(reactive)]
fn test_multiple_effects_on_same_signal() {
	let state = AuthState::new();
	let auth_signal = state.is_authenticated_signal();

	let effect1_triggered = Signal::new(false);
	let effect2_triggered = Signal::new(false);

	let e1 = effect1_triggered.clone();
	let _effect1 = Effect::new(move || {
		if auth_signal.get() {
			e1.set(true);
		}
	});

	let e2 = effect2_triggered.clone();
	let auth_signal_2 = state.is_authenticated_signal();
	let _effect2 = Effect::new(move || {
		if auth_signal_2.get() {
			e2.set(true);
		}
	});

	state.login("1", "multi_effect");
	with_runtime(|rt| rt.flush_updates());

	assert!(effect1_triggered.get());
	assert!(effect2_triggered.get());
}

/// Tests signal clone shares same underlying value
#[test]
fn test_signal_clone_shares_value() {
	let state = AuthState::new();
	let signal1 = state.username_signal();
	let signal2 = state.username_signal();

	state.login("1", "shared");

	assert_eq!(signal1.get(), Some("shared".to_string()));
	assert_eq!(signal2.get(), Some("shared".to_string()));
}

// ============================================================================
// Category 4: Global State Integration (5 tests)
// ============================================================================

/// Tests global auth_state function returns same instance
#[test]
#[serial_test::serial(auth)]
fn test_global_auth_state_singleton() {
	let state1 = auth_state();
	let state2 = auth_state();

	state1.login("999", "global_user");

	assert!(state2.is_authenticated());
	assert_eq!(state2.user_id(), Some("999".to_string()));

	// Cleanup
	state1.logout();
}

/// Tests global state persists across function calls
#[test]
#[serial_test::serial(auth)]
fn test_global_state_persistence() {
	// Get two instances of global auth state
	let state1 = auth_state();
	let state2 = auth_state();

	// Login with first instance
	state1.login("100", "persistent");

	// Verify second instance sees the change
	assert!(state2.is_authenticated());
	assert_eq!(state2.username(), Some("persistent".to_string()));

	// Cleanup
	state1.logout();
}

/// Tests global state updates propagate
#[test]
#[serial_test::serial(auth)]
fn test_global_state_update_propagation() {
	let state1 = auth_state();
	let state2 = auth_state();

	let signal1 = state1.is_authenticated_signal();
	let signal2 = state2.is_authenticated_signal();

	state1.login("1", "propagate");

	assert!(signal1.get());
	assert!(signal2.get());

	// Cleanup
	state1.logout();
}

/// Tests global state logout affects all references
#[test]
#[serial_test::serial(auth)]
fn test_global_state_logout_affects_all() {
	let state1 = auth_state();
	let state2 = auth_state();

	state1.login("1", "shared");
	assert!(state2.is_authenticated());

	state1.logout();
	assert!(!state2.is_authenticated());
}

/// Tests global state permission changes propagate
#[test]
#[serial_test::serial(auth)]
fn test_global_state_permission_propagation() {
	let state1 = auth_state();
	let state2 = auth_state();

	let mut perms = HashSet::new();
	perms.insert("test.permission".to_string());
	state1.set_permissions(perms);

	assert!(state2.has_permission("test.permission"));

	// Cleanup
	state1.logout();
}
