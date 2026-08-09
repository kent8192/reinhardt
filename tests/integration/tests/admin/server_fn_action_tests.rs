//! Integration tests for registered admin action dispatch.

use super::server_fn_helpers::{
	TEST_CSRF_TOKEN, make_auth_user, make_staff_request, server_fn_context,
};
use reinhardt_admin::core::{
	AdminActionTransaction, AdminDatabase, AdminRecord, AdminUser, ModelAdmin,
};
use reinhardt_admin::server::execute_admin_action;
use reinhardt_admin::types::{
	AdminAction, AdminActionOutcome, AdminActionRequest, AdminError, ModelPermission,
};
use reinhardt_db::backends::types::QueryValue;
use reinhardt_db::orm::OrmExecutor;
use rstest::*;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{
	Arc,
	atomic::{AtomicUsize, Ordering},
};

struct ActionAdmin {
	calls: Arc<AtomicUsize>,
	allow_change: bool,
	fail_after_write: bool,
	affected: u64,
}

impl ActionAdmin {
	fn new(
		calls: Arc<AtomicUsize>,
		allow_change: bool,
		fail_after_write: bool,
		affected: u64,
	) -> Self {
		Self {
			calls,
			allow_change,
			fail_after_write,
			affected,
		}
	}
}

#[async_trait::async_trait]
impl ModelAdmin for ActionAdmin {
	fn model_name(&self) -> &str {
		"CanonicalActionModel"
	}

	fn table_name(&self) -> &str {
		"test_models"
	}

	fn actions(&self) -> Vec<AdminAction> {
		vec![AdminAction::new(
			"publish",
			"Publish",
			ModelPermission::Change,
			false,
		)]
	}

	async fn execute_action(
		&self,
		action: &str,
		ids: &[String],
		_db: &AdminDatabase,
		transaction: &mut AdminActionTransaction,
		_user: &dyn AdminUser,
	) -> Result<AdminActionOutcome, AdminError> {
		assert_eq!(action, "publish");
		self.calls.fetch_add(1, Ordering::SeqCst);
		let id = ids
			.first()
			.expect("validated action requests contain an ID")
			.parse::<i64>()
			.expect("test action IDs are integers");
		OrmExecutor::execute(
			transaction,
			"UPDATE test_models SET status = $1 WHERE id = $2",
			vec![
				QueryValue::String("published".to_string()),
				QueryValue::Int(id),
			],
		)
		.await
		.map_err(|error| AdminError::DatabaseError(error.to_string()))?;

		if self.fail_after_write {
			Err(AdminError::DatabaseError("action hook failed".to_string()))
		} else {
			Ok(AdminActionOutcome::new(vec![id.to_string()], self.affected))
		}
	}

	async fn has_change_permission(&self, _user: &dyn AdminUser) -> bool {
		self.allow_change
	}
}

async fn create_action_record(db: &super::server_fn_helpers::AdminDatabaseDepends) -> String {
	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Action target"));
	data.insert("status".to_string(), json!("draft"));
	db.create::<AdminRecord>("test_models", None, data)
		.await
		.expect("action target should be created")
		.to_string()
}

async fn action_record_status(
	db: &super::server_fn_helpers::AdminDatabaseDepends,
	id: &str,
) -> serde_json::Value {
	db.get::<AdminRecord>("test_models", "id", id)
		.await
		.expect("action target query should succeed")
		.expect("action target should exist")
		.remove("status")
		.expect("action target should have status")
}

async fn execute(
	site: super::server_fn_helpers::AdminSiteDepends,
	db: super::server_fn_helpers::AdminDatabaseDepends,
	request: AdminActionRequest,
) -> Result<reinhardt_admin::types::MutationResponse, reinhardt_pages::server_fn::ServerFnError> {
	execute_admin_action(
		"mixedcaseactionmodel".to_string(),
		request,
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
}

#[rstest]
#[tokio::test]
async fn action_dispatch_invokes_registered_action_once_with_canonical_values(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	let (site, db, _lease) = server_fn_context.await;
	let calls = Arc::new(AtomicUsize::new(0));
	site.register(
		"MixedCaseActionModel",
		ActionAdmin::new(calls.clone(), true, false, 2),
	)
	.expect("action model should register");
	let id = create_action_record(&db).await;

	let response = execute(
		site,
		db.clone(),
		AdminActionRequest::new(TEST_CSRF_TOKEN, "publish", vec![id.clone()]),
	)
	.await
	.expect("registered action should succeed");

	assert!(response.success);
	assert_eq!(response.affected, Some(2));
	assert_eq!(calls.load(Ordering::SeqCst), 1);
	assert_eq!(action_record_status(&db, &id).await, json!("published"));
}

#[rstest]
#[case::empty_action("", vec!["1".to_string()])]
#[case::unknown_action("unknown", vec!["1".to_string()])]
#[case::empty_selection("publish", vec![])]
#[case::malformed_primary_key("publish", vec!["bad\u{0000}id".to_string()])]
#[tokio::test]
async fn action_dispatch_rejects_invalid_requests_before_the_hook(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
	#[case] action: &str,
	#[case] ids: Vec<String>,
) {
	let (site, db, _lease) = server_fn_context.await;
	let calls = Arc::new(AtomicUsize::new(0));
	site.register(
		"MixedCaseActionModel",
		ActionAdmin::new(calls.clone(), true, false, 1),
	)
	.expect("action model should register");

	let result = execute(
		site,
		db,
		AdminActionRequest::new(TEST_CSRF_TOKEN, action, ids),
	)
	.await;

	assert!(result.is_err());
	assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[rstest]
#[tokio::test]
async fn action_dispatch_rejects_excessive_selection_before_the_hook(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	let (site, db, _lease) = server_fn_context.await;
	let calls = Arc::new(AtomicUsize::new(0));
	site.register(
		"MixedCaseActionModel",
		ActionAdmin::new(calls.clone(), true, false, 1),
	)
	.expect("action model should register");

	let result = execute(
		site,
		db,
		AdminActionRequest::new(
			TEST_CSRF_TOKEN,
			"publish",
			(0..=1_000).map(|id| id.to_string()).collect(),
		),
	)
	.await;

	assert!(result.is_err());
	assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[rstest]
#[tokio::test]
async fn action_dispatch_rejects_invalid_csrf_before_the_hook(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	let (site, db, _lease) = server_fn_context.await;
	let calls = Arc::new(AtomicUsize::new(0));
	site.register(
		"MixedCaseActionModel",
		ActionAdmin::new(calls.clone(), true, false, 1),
	)
	.expect("action model should register");

	let result = execute(
		site,
		db,
		AdminActionRequest::new("invalid-csrf", "publish", vec!["1".to_string()]),
	)
	.await;

	assert!(result.is_err());
	assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[rstest]
#[tokio::test]
async fn action_dispatch_rejects_denied_permission_before_the_hook(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	let (site, db, _lease) = server_fn_context.await;
	let calls = Arc::new(AtomicUsize::new(0));
	site.register(
		"MixedCaseActionModel",
		ActionAdmin::new(calls.clone(), false, false, 1),
	)
	.expect("action model should register");

	let result = execute(
		site,
		db,
		AdminActionRequest::new(TEST_CSRF_TOKEN, "publish", vec!["1".to_string()]),
	)
	.await;

	assert!(result.is_err());
	assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[rstest]
#[tokio::test]
async fn action_dispatch_rolls_back_hook_mutation_on_error(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	let (site, db, _lease) = server_fn_context.await;
	let calls = Arc::new(AtomicUsize::new(0));
	site.register(
		"MixedCaseActionModel",
		ActionAdmin::new(calls.clone(), true, true, 1),
	)
	.expect("action model should register");
	let id = create_action_record(&db).await;

	let result = execute(
		site,
		db.clone(),
		AdminActionRequest::new(TEST_CSRF_TOKEN, "publish", vec![id.clone()]),
	)
	.await;

	assert_eq!(calls.load(Ordering::SeqCst), 1);
	assert!(result.is_err());
	assert_eq!(action_record_status(&db, &id).await, json!("draft"));
}

#[rstest]
#[tokio::test]
async fn action_dispatch_treats_zero_affected_as_committed_success(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	let (site, db, _lease) = server_fn_context.await;
	let calls = Arc::new(AtomicUsize::new(0));
	site.register(
		"MixedCaseActionModel",
		ActionAdmin::new(calls.clone(), true, false, 0),
	)
	.expect("action model should register");
	let id = create_action_record(&db).await;

	let response = execute(
		site,
		db.clone(),
		AdminActionRequest::new(TEST_CSRF_TOKEN, "publish", vec![id.clone()]),
	)
	.await
	.expect("zero affected action should succeed");

	assert!(response.success);
	assert_eq!(response.affected, Some(0));
	assert_eq!(calls.load(Ordering::SeqCst), 1);
	assert_eq!(action_record_status(&db, &id).await, json!("published"));
}
