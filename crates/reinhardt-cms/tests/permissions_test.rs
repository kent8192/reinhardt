//! Tests for permission checker

use reinhardt_cms::error::CmsError;
use reinhardt_cms::permissions::{PermissionChecker, PermissionType, Principal};
use rstest::rstest;
use uuid::Uuid;

#[rstest]
#[tokio::test]
async fn test_grant_and_check_permission() {
	let mut checker = PermissionChecker::new();

	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	// Grant permission
	let permission = checker
		.grant_permission(
			page_id,
			Principal::User(user_id),
			PermissionType::Edit,
			false,
		)
		.await
		.unwrap();

	assert_eq!(permission.page_id, page_id);
	assert_eq!(permission.permission, PermissionType::Edit);

	// Check permission
	let has_permission = checker
		.check_permission(user_id, page_id, PermissionType::Edit)
		.await
		.unwrap();
	assert!(has_permission);

	// Check non-existent permission
	let has_permission = checker
		.check_permission(user_id, page_id, PermissionType::Delete)
		.await
		.unwrap();
	assert!(!has_permission);
}

#[rstest]
#[tokio::test]
async fn test_revoke_permission() {
	let mut checker = PermissionChecker::new();

	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	// Grant permission
	let permission = checker
		.grant_permission(
			page_id,
			Principal::User(user_id),
			PermissionType::Edit,
			false,
		)
		.await
		.unwrap();

	// Verify permission exists
	let has_permission = checker
		.check_permission(user_id, page_id, PermissionType::Edit)
		.await
		.unwrap();
	assert!(has_permission);

	// Revoke permission
	checker.revoke_permission(permission.id).await.unwrap();

	// Verify permission is revoked
	let has_permission = checker
		.check_permission(user_id, page_id, PermissionType::Edit)
		.await
		.unwrap();
	assert!(!has_permission);
}

#[rstest]
#[tokio::test]
async fn test_authenticated_principal() {
	let mut checker = PermissionChecker::new();

	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	// Grant permission to all authenticated users
	checker
		.grant_permission(
			page_id,
			Principal::Authenticated,
			PermissionType::View,
			false,
		)
		.await
		.unwrap();

	// Any authenticated user should have the permission
	let has_permission = checker
		.check_permission(user_id, page_id, PermissionType::View)
		.await
		.unwrap();
	assert!(has_permission);
}

#[rstest]
#[tokio::test]
async fn test_anyone_principal() {
	let mut checker = PermissionChecker::new();

	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	// Grant permission to anyone
	checker
		.grant_permission(page_id, Principal::Anyone, PermissionType::View, false)
		.await
		.unwrap();

	// Anyone should have the permission
	let has_permission = checker
		.check_permission(user_id, page_id, PermissionType::View)
		.await
		.unwrap();
	assert!(has_permission);
}

#[rstest]
#[tokio::test]
async fn test_get_page_permissions() {
	let mut checker = PermissionChecker::new();

	let page_id = Uuid::new_v4();
	let user_id1 = Uuid::new_v4();
	let user_id2 = Uuid::new_v4();

	// Grant multiple permissions
	checker
		.grant_permission(
			page_id,
			Principal::User(user_id1),
			PermissionType::Edit,
			false,
		)
		.await
		.unwrap();

	checker
		.grant_permission(
			page_id,
			Principal::User(user_id2),
			PermissionType::View,
			false,
		)
		.await
		.unwrap();

	// Get all permissions for the page
	let permissions = checker.get_page_permissions(page_id).await.unwrap();
	assert_eq!(permissions.len(), 2);
}

#[rstest]
#[tokio::test]
async fn test_multiple_permission_types() {
	let mut checker = PermissionChecker::new();

	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	// Grant multiple permission types to the same user
	checker
		.grant_permission(
			page_id,
			Principal::User(user_id),
			PermissionType::View,
			false,
		)
		.await
		.unwrap();

	checker
		.grant_permission(
			page_id,
			Principal::User(user_id),
			PermissionType::Edit,
			false,
		)
		.await
		.unwrap();

	// Check both permissions
	assert!(
		checker
			.check_permission(user_id, page_id, PermissionType::View)
			.await
			.unwrap()
	);
	assert!(
		checker
			.check_permission(user_id, page_id, PermissionType::Edit)
			.await
			.unwrap()
	);

	// Check non-granted permission
	assert!(
		!checker
			.check_permission(user_id, page_id, PermissionType::Delete)
			.await
			.unwrap()
	);
}

// === Error Path Tests ===

#[rstest]
#[tokio::test]
async fn test_revoke_nonexistent_permission() {
	// Arrange
	let mut checker = PermissionChecker::new();
	let nonexistent_id = Uuid::new_v4();

	// Act
	let result = checker.revoke_permission(nonexistent_id).await;

	// Assert
	let err = result.unwrap_err();
	assert!(matches!(err, CmsError::PermissionDenied(ref msg) if msg == "Permission not found"));
}

// === Equivalence Partitioning Tests ===

#[rstest]
#[case(PermissionType::View)]
#[case(PermissionType::Edit)]
#[case(PermissionType::Publish)]
#[case(PermissionType::Delete)]
#[case(PermissionType::AddChild)]
#[case(PermissionType::ManagePermissions)]
#[tokio::test]
async fn test_permission_type_check_all_variants(#[case] perm_type: PermissionType) {
	// Arrange
	let mut checker = PermissionChecker::new();
	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	checker
		.grant_permission(page_id, Principal::User(user_id), perm_type, false)
		.await
		.unwrap();

	// Use a different permission type for negative check
	let other_type = if perm_type == PermissionType::View {
		PermissionType::Edit
	} else {
		PermissionType::View
	};

	// Act
	let has_matching = checker
		.check_permission(user_id, page_id, perm_type)
		.await
		.unwrap();
	let has_other = checker
		.check_permission(user_id, page_id, other_type)
		.await
		.unwrap();

	// Assert
	assert_eq!(has_matching, true);
	assert_eq!(has_other, false);
}

#[rstest]
#[case(Principal::User(Uuid::new_v4()))]
#[case(Principal::Group(Uuid::new_v4()))]
#[case(Principal::Authenticated)]
#[case(Principal::Anyone)]
#[tokio::test]
async fn test_principal_variant_grant(#[case] principal: Principal) {
	// Arrange
	let mut checker = PermissionChecker::new();
	let page_id = Uuid::new_v4();

	// Act
	let permission = checker
		.grant_permission(page_id, principal, PermissionType::View, false)
		.await
		.unwrap();

	// Assert
	assert_eq!(permission.page_id, page_id);
	assert_eq!(permission.permission, PermissionType::View);
}

// === Combination Tests ===

#[rstest]
#[tokio::test]
async fn test_permission_multiple_pages_same_user() {
	// Arrange
	let mut checker = PermissionChecker::new();
	let user_id = Uuid::new_v4();
	let page_a = Uuid::new_v4();
	let page_b = Uuid::new_v4();
	let page_c = Uuid::new_v4();

	checker
		.grant_permission(
			page_a,
			Principal::User(user_id),
			PermissionType::Edit,
			false,
		)
		.await
		.unwrap();
	checker
		.grant_permission(
			page_b,
			Principal::User(user_id),
			PermissionType::View,
			false,
		)
		.await
		.unwrap();

	// Act
	let has_edit_a = checker
		.check_permission(user_id, page_a, PermissionType::Edit)
		.await
		.unwrap();
	let has_view_b = checker
		.check_permission(user_id, page_b, PermissionType::View)
		.await
		.unwrap();
	let has_view_c = checker
		.check_permission(user_id, page_c, PermissionType::View)
		.await
		.unwrap();

	// Assert
	assert_eq!(has_edit_a, true);
	assert_eq!(has_view_b, true);
	assert_eq!(has_view_c, false);
}

#[rstest]
#[tokio::test]
async fn test_permission_multiple_users_same_page() {
	// Arrange
	let mut checker = PermissionChecker::new();
	let page_id = Uuid::new_v4();
	let user_a = Uuid::new_v4();
	let user_b = Uuid::new_v4();
	let user_c = Uuid::new_v4();

	checker
		.grant_permission(
			page_id,
			Principal::User(user_a),
			PermissionType::Edit,
			false,
		)
		.await
		.unwrap();
	checker
		.grant_permission(
			page_id,
			Principal::User(user_b),
			PermissionType::View,
			false,
		)
		.await
		.unwrap();
	checker
		.grant_permission(
			page_id,
			Principal::User(user_c),
			PermissionType::Publish,
			false,
		)
		.await
		.unwrap();

	// Act
	let a_edit = checker
		.check_permission(user_a, page_id, PermissionType::Edit)
		.await
		.unwrap();
	let b_view = checker
		.check_permission(user_b, page_id, PermissionType::View)
		.await
		.unwrap();
	let c_publish = checker
		.check_permission(user_c, page_id, PermissionType::Publish)
		.await
		.unwrap();
	// Cross-check: user A should not have Publish
	let a_publish = checker
		.check_permission(user_a, page_id, PermissionType::Publish)
		.await
		.unwrap();

	// Assert
	assert_eq!(a_edit, true);
	assert_eq!(b_view, true);
	assert_eq!(c_publish, true);
	assert_eq!(a_publish, false);
}

// === Boundary Value Tests ===

#[rstest]
#[case(1)]
#[case(5)]
#[case(10)]
#[case(50)]
#[tokio::test]
async fn test_permissions_count_boundaries(#[case] count: usize) {
	// Arrange
	let mut checker = PermissionChecker::new();
	let page_id = Uuid::new_v4();

	// Act
	for _ in 0..count {
		let user_id = Uuid::new_v4();
		checker
			.grant_permission(
				page_id,
				Principal::User(user_id),
				PermissionType::View,
				false,
			)
			.await
			.unwrap();
	}

	// Assert
	let permissions = checker.get_page_permissions(page_id).await.unwrap();
	assert_eq!(permissions.len(), count);
}

// === Decision Table Tests ===

#[rstest]
#[case(true, true, true, true)]
#[case(true, true, false, false)]
#[case(true, false, true, false)]
#[case(true, false, false, false)]
#[case(false, true, true, false)]
#[case(false, true, false, false)]
#[case(false, false, true, false)]
#[case(false, false, false, false)]
#[tokio::test]
async fn test_permission_check_matching_decision_table(
	#[case] matching_user: bool,
	#[case] matching_page: bool,
	#[case] matching_type: bool,
	#[case] expected: bool,
) {
	// Arrange
	let mut checker = PermissionChecker::new();
	let granted_user_id = Uuid::new_v4();
	let granted_page_id = Uuid::new_v4();
	let granted_type = PermissionType::Edit;
	let other_type = PermissionType::Delete;

	checker
		.grant_permission(
			granted_page_id,
			Principal::User(granted_user_id),
			granted_type,
			false,
		)
		.await
		.unwrap();

	let check_user = if matching_user {
		granted_user_id
	} else {
		Uuid::new_v4()
	};
	let check_page = if matching_page {
		granted_page_id
	} else {
		Uuid::new_v4()
	};
	let check_type = if matching_type {
		granted_type
	} else {
		other_type
	};

	// Act
	let result = checker
		.check_permission(check_user, check_page, check_type)
		.await
		.unwrap();

	// Assert
	assert_eq!(result, expected);
}

#[rstest]
#[case("user_match", true)]
#[case("user_no_match", false)]
#[case("group", false)]
#[case("authenticated", true)]
#[case("anyone", true)]
#[tokio::test]
async fn test_principal_matching_decision_table(#[case] variant: &str, #[case] expected: bool) {
	// Arrange
	let mut checker = PermissionChecker::new();
	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();
	let other_user_id = Uuid::new_v4();

	let principal = match variant {
		"user_match" => Principal::User(user_id),
		"user_no_match" => Principal::User(other_user_id),
		"group" => Principal::Group(Uuid::new_v4()),
		"authenticated" => Principal::Authenticated,
		"anyone" => Principal::Anyone,
		_ => unreachable!(),
	};

	checker
		.grant_permission(page_id, principal, PermissionType::View, false)
		.await
		.unwrap();

	// Act
	let result = checker
		.check_permission(user_id, page_id, PermissionType::View)
		.await
		.unwrap();

	// Assert
	assert_eq!(result, expected);
}

#[rstest]
#[case(true, false, true)]
#[case(true, true, false)]
#[case(false, false, false)]
#[tokio::test]
async fn test_permission_grant_revoke_check_decision_table(
	#[case] granted: bool,
	#[case] revoked: bool,
	#[case] expected: bool,
) {
	// Arrange
	let mut checker = PermissionChecker::new();
	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	let mut permission_id = None;
	if granted {
		let perm = checker
			.grant_permission(
				page_id,
				Principal::User(user_id),
				PermissionType::Edit,
				false,
			)
			.await
			.unwrap();
		permission_id = Some(perm.id);
	}

	if revoked {
		if let Some(id) = permission_id {
			checker.revoke_permission(id).await.unwrap();
		}
	}

	// Act
	let result = checker
		.check_permission(user_id, page_id, PermissionType::Edit)
		.await
		.unwrap();

	// Assert
	assert_eq!(result, expected);
}

// === Sanity Tests ===

#[rstest]
#[tokio::test]
async fn test_permission_checker_default_trait() {
	// Arrange & Act
	let checker = PermissionChecker::default();
	let page_id = Uuid::new_v4();

	// Assert
	let permissions = checker.get_page_permissions(page_id).await.unwrap();
	assert_eq!(permissions.len(), 0);
}

// === Property-Based Tests ===

#[rstest]
#[tokio::test]
async fn test_permission_check_is_idempotent() {
	// Arrange
	let mut checker = PermissionChecker::new();
	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	checker
		.grant_permission(
			page_id,
			Principal::User(user_id),
			PermissionType::Edit,
			false,
		)
		.await
		.unwrap();

	// Act
	let first_check = checker
		.check_permission(user_id, page_id, PermissionType::Edit)
		.await
		.unwrap();
	let second_check = checker
		.check_permission(user_id, page_id, PermissionType::Edit)
		.await
		.unwrap();

	// Assert
	assert_eq!(first_check, second_check);
	assert_eq!(first_check, true);
}

#[rstest]
#[tokio::test]
async fn test_permission_grant_random_user_ids() {
	// Arrange
	let mut checker = PermissionChecker::new();
	let page_id = Uuid::new_v4();
	let user_ids: Vec<Uuid> = (0..10).map(|_| Uuid::new_v4()).collect();

	// Act
	for &user_id in &user_ids {
		checker
			.grant_permission(
				page_id,
				Principal::User(user_id),
				PermissionType::View,
				false,
			)
			.await
			.unwrap();
	}

	// Assert
	for &user_id in &user_ids {
		let result = checker
			.check_permission(user_id, page_id, PermissionType::View)
			.await
			.unwrap();
		assert_eq!(result, true);
	}
}
