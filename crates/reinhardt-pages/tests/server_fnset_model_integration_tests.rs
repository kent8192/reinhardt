#![cfg(all(
	feature = "model-server-fnset",
	not(all(target_family = "wasm", target_os = "unknown"))
))]

use std::sync::{Mutex, OnceLock};

use async_trait::async_trait;
use reinhardt_db::backends::types::QueryValue;
use reinhardt_db::orm::{
	DatabaseConnection, DatabaseConnectionLease, FieldSelector, Filter, FilterValue, Manager,
	Model, QuerySet, TransactionExecutor, UniqueFieldRef,
};
use reinhardt_di::params::{FromRequest, ParamContext, ParamResult};
use reinhardt_http::Request;
use reinhardt_pages::server_fn::{
	CreateActionContext, CreateModelInput, ModelServerFnResource, ModelServerFnSet, PageRequest,
	PatchModelInput, ServerFnListQuery, ServerFnResource, ServerFnSetAction, ServerFnSetError,
	ServerFnSetPolicy, UpdateModelInput,
};
use serde::{Deserialize, Serialize};
use serial_test::serial;

#[derive(Clone, Copy, Default)]
enum AccessFailure {
	#[default]
	None,
	Unauthenticated,
	Forbidden,
}

#[derive(Default)]
struct RecordingState {
	events: Vec<&'static str>,
	executor_ids: Vec<usize>,
	access_failure: AccessFailure,
	fail_validation: bool,
	deny_created_object: bool,
	deny_mutated_object: bool,
	fail_to_read: bool,
	fail_destroy: bool,
}

fn state() -> &'static Mutex<RecordingState> {
	static STATE: OnceLock<Mutex<RecordingState>> = OnceLock::new();
	STATE.get_or_init(|| Mutex::new(RecordingState::default()))
}

fn reset_state() {
	*state()
		.lock()
		.expect("recording mutex should not be poisoned") = RecordingState::default();
}

fn record(event: &'static str, executor: Option<&mut dyn TransactionExecutor>) {
	let mut state = state()
		.lock()
		.expect("recording mutex should not be poisoned");
	state.events.push(event);
	if let Some(executor) = executor {
		state
			.executor_ids
			.push(executor as *mut dyn TransactionExecutor as *mut () as usize);
	}
}

#[derive(Clone)]
struct WidgetFields;

impl FieldSelector for WidgetFields {
	fn with_alias(self, _alias: &str) -> Self {
		self
	}
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct Widget {
	id: Option<i64>,
	name: String,
}

impl Model for Widget {
	type PrimaryKey = i64;
	type Fields = WidgetFields;
	type Objects = Manager<Self>;

	fn table_name() -> &'static str {
		"tenant_42_widgets"
	}

	fn new_fields() -> Self::Fields {
		WidgetFields
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = Some(value);
	}
}

#[derive(Clone)]
struct ListQuery {
	page: PageRequest,
	name_prefix: &'static str,
}

impl ServerFnListQuery for ListQuery {
	fn page_request(&self) -> PageRequest {
		self.page
	}
}

struct CreateInput(String);
struct UpdateInput(String);
struct PatchInput(String);

impl CreateModelInput<Widget> for CreateInput {
	fn build(self) -> Result<Widget, ServerFnSetError> {
		Ok(Widget {
			id: None,
			name: self.0,
		})
	}
}

impl UpdateModelInput<Widget> for UpdateInput {
	fn apply(self, model: &mut Widget) -> Result<(), ServerFnSetError> {
		model.name = self.0;
		Ok(())
	}
}

impl PatchModelInput<Widget> for PatchInput {
	fn apply_patch(self, model: &mut Widget) -> Result<(), ServerFnSetError> {
		model.name = self.0;
		Ok(())
	}
}

#[derive(Clone, Copy)]
struct Principal;

#[async_trait]
impl FromRequest for Principal {
	async fn from_request(_request: &Request, _context: &ParamContext) -> ParamResult<Self> {
		Ok(Self)
	}
}

struct RecordingPolicy;

#[async_trait]
impl ServerFnSetPolicy<WidgetResource> for RecordingPolicy {
	type Principal = Principal;

	async fn authorize_action(
		_principal: &Self::Principal,
		_action: ServerFnSetAction,
		executor: Option<&mut dyn TransactionExecutor>,
	) -> Result<(), ServerFnSetError> {
		record("authorize_action", executor);
		match state()
			.lock()
			.expect("recording mutex should not be poisoned")
			.access_failure
		{
			AccessFailure::None => Ok(()),
			AccessFailure::Unauthenticated => Err(ServerFnSetError::Unauthenticated),
			AccessFailure::Forbidden => Err(ServerFnSetError::Forbidden),
		}
	}

	async fn scope_query(
		_principal: &Self::Principal,
		query: QuerySet<Widget>,
		executor: Option<&mut dyn TransactionExecutor>,
	) -> Result<QuerySet<Widget>, ServerFnSetError> {
		record("scope_query", executor);
		Ok(query.filter(Filter::new(
			"id",
			reinhardt_db::orm::FilterOperator::Gte,
			FilterValue::Integer(6),
		)))
	}

	async fn authorize_object(
		_principal: &Self::Principal,
		action: ServerFnSetAction,
		object: &Widget,
		executor: Option<&mut dyn TransactionExecutor>,
	) -> Result<(), ServerFnSetError> {
		record("authorize_object", executor);
		let state = state()
			.lock()
			.expect("recording mutex should not be poisoned");
		if action == ServerFnSetAction::Create && state.deny_created_object {
			return Err(ServerFnSetError::Forbidden);
		}
		if matches!(
			action,
			ServerFnSetAction::Update | ServerFnSetAction::PartialUpdate
		) && state.deny_mutated_object
			&& object.name == "forbidden"
		{
			return Err(ServerFnSetError::Forbidden);
		}
		Ok(())
	}
}

struct WidgetResource;

impl ServerFnResource for WidgetResource {
	type Lookup = String;
	type Read = (i64, String);
	type Create = CreateInput;
	type Update = UpdateInput;
	type Patch = PatchInput;
	type ListQuery = ListQuery;
}

#[async_trait]
impl ModelServerFnResource for WidgetResource {
	type Model = Widget;
	type Policy = RecordingPolicy;
	const PUBLIC_NAME: &'static str = "widget";

	fn lookup_field() -> UniqueFieldRef<Self::Model, Self::Lookup> {
		// SAFETY: The handwritten test schema creates a unique index for `name`.
		unsafe { UniqueFieldRef::from_model_field_with_names("name", "name") }
	}

	fn base_queryset() -> QuerySet<Self::Model> {
		record("base_queryset", None);
		QuerySet::new()
	}

	fn apply_list_query(
		queryset: QuerySet<Self::Model>,
		query: &Self::ListQuery,
	) -> Result<QuerySet<Self::Model>, ServerFnSetError> {
		record("apply_list_query", None);
		Ok(queryset
			.filter(Filter::new(
				"name",
				reinhardt_db::orm::FilterOperator::StartsWith,
				FilterValue::String(query.name_prefix.to_owned()),
			))
			.order_by(&["-name"]))
	}

	async fn to_read(
		model: &Self::Model,
		executor: Option<&mut dyn TransactionExecutor>,
	) -> Result<Self::Read, ServerFnSetError> {
		record("to_read", executor);
		if state()
			.lock()
			.expect("recording mutex should not be poisoned")
			.fail_to_read
		{
			return Err(ServerFnSetError::Application {
				code: "conversion_failed".to_owned(),
				message: "conversion failed".to_owned(),
				details: serde_json::Value::Null,
			});
		}
		Ok((
			model.id.expect("persisted model should have an id"),
			model.name.clone(),
		))
	}

	async fn validate_create(
		_input: &Self::Create,
		executor: &mut dyn TransactionExecutor,
	) -> Result<(), ServerFnSetError> {
		record("validate_create", Some(executor));
		if state()
			.lock()
			.expect("recording mutex should not be poisoned")
			.fail_validation
		{
			return Err(ServerFnSetError::Validation(Default::default()));
		}
		Ok(())
	}

	async fn validate_update(
		_input: &Self::Update,
		_object: &Self::Model,
		executor: &mut dyn TransactionExecutor,
	) -> Result<(), ServerFnSetError> {
		record("validate_update", Some(executor));
		Ok(())
	}

	async fn validate_patch(
		_input: &Self::Patch,
		_object: &Self::Model,
		executor: &mut dyn TransactionExecutor,
	) -> Result<(), ServerFnSetError> {
		record("validate_patch", Some(executor));
		Ok(())
	}

	async fn perform_create(
		input: Self::Create,
		executor: &mut dyn TransactionExecutor,
	) -> Result<Self::Model, ServerFnSetError> {
		record("perform_create", Some(executor));
		let mut object = input.build()?;
		object
			.save_with_executor(executor)
			.await
			.map_err(|_| ServerFnSetError::Internal)?;
		Ok(object)
	}

	async fn perform_update(
		input: Self::Update,
		object: &mut Self::Model,
		executor: &mut dyn TransactionExecutor,
	) -> Result<(), ServerFnSetError> {
		record("perform_update", Some(executor));
		input.apply(object)?;
		object
			.save_with_executor(executor)
			.await
			.map_err(|_| ServerFnSetError::Internal)
	}

	async fn perform_patch(
		input: Self::Patch,
		object: &mut Self::Model,
		executor: &mut dyn TransactionExecutor,
	) -> Result<(), ServerFnSetError> {
		record("perform_patch", Some(executor));
		input.apply_patch(object)?;
		object
			.save_with_executor(executor)
			.await
			.map_err(|_| ServerFnSetError::Internal)
	}

	async fn perform_destroy(
		object: &Self::Model,
		executor: &mut dyn TransactionExecutor,
	) -> Result<(), ServerFnSetError> {
		record("perform_destroy", Some(executor));
		object
			.delete_with_executor(executor)
			.await
			.map_err(|_| ServerFnSetError::Internal)?;
		if state()
			.lock()
			.expect("recording mutex should not be poisoned")
			.fail_destroy
		{
			return Err(ServerFnSetError::Application {
				code: "destroy_failed".to_owned(),
				message: "destroy failed".to_owned(),
				details: serde_json::Value::Null,
			});
		}
		Ok(())
	}
}

async fn database() -> (
	tempfile::TempDir,
	DatabaseConnectionLease,
	DatabaseConnection,
) {
	let directory = tempfile::Builder::new()
		.prefix("reinhardt-pages-server-fnset-")
		.tempdir_in("/tmp")
		.expect("temporary database directory should be created");
	let url = format!(
		"sqlite:///{}",
		directory.path().join("runtime.sqlite").display()
	);
	let owner = reinhardt_db::backends::DatabaseConnection::connect_sqlite(&url)
		.await
		.expect("SQLite connection should open");
	let lease =
		DatabaseConnectionLease::register(owner).expect("SQLite connection should register");
	let connection = lease.handle();
	connection
		.execute(
			"CREATE TABLE tenant_42_widgets (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
			vec![],
		)
		.await
		.expect("widget table should be created");
	(directory, lease, connection)
}

async fn insert(connection: &DatabaseConnection, id: i64, name: &str) {
	connection
		.execute(
			"INSERT INTO tenant_42_widgets (id, name) VALUES (?, ?)",
			vec![QueryValue::Int(id), QueryValue::String(name.to_owned())],
		)
		.await
		.expect("widget should be inserted");
}

async fn count(connection: &DatabaseConnection) -> u64 {
	let mut connection = connection.clone();
	QuerySet::<Widget>::new()
		.count_with_db(&mut connection)
		.await
		.expect("widget count should succeed") as u64
}

fn assert_one_executor() {
	let state = state()
		.lock()
		.expect("recording mutex should not be poisoned");
	assert!(!state.executor_ids.is_empty());
	assert!(
		state
			.executor_ids
			.iter()
			.all(|id| *id == state.executor_ids[0])
	);
}

fn assert_events(expected: &[&'static str]) {
	assert_eq!(
		state()
			.lock()
			.expect("recording mutex should not be poisoned")
			.events,
		expected
	);
}

#[tokio::test(flavor = "multi_thread")]
#[serial(model_server_fnset_runtime)]
async fn list_validates_and_applies_policy_before_count_and_slice() {
	let (_directory, _lease, connection) = database().await;
	for id in 1..=30 {
		insert(&connection, id, &format!("widget-{id:02}")).await;
	}
	reset_state();

	let page = ModelServerFnSet::<WidgetResource>::list(
		&Principal,
		&connection,
		ListQuery {
			page: PageRequest::default(),
			name_prefix: "widget-1",
		},
	)
	.await
	.expect("default page should load");

	assert_eq!(page.total, 10);
	assert_eq!(page.limit, 25);
	assert_eq!(page.offset, 0);
	assert_eq!(page.items.len(), 10);
	assert_eq!(page.items[0].1, "widget-19");
	assert_eq!(page.items[9].1, "widget-10");
	let events = state()
		.lock()
		.expect("recording mutex should not be poisoned")
		.events
		.clone();
	assert_eq!(
		&events[..4],
		[
			"authorize_action",
			"base_queryset",
			"scope_query",
			"apply_list_query"
		]
	);
	assert_eq!(events[4..], vec!["to_read"; 10]);

	reset_state();
	let page = ModelServerFnSet::<WidgetResource>::list(
		&Principal,
		&connection,
		ListQuery {
			page: PageRequest {
				limit: Some(5),
				offset: 10,
			},
			name_prefix: "widget",
		},
	)
	.await
	.expect("offset page should load");
	assert_eq!(
		(page.total, page.limit, page.offset, page.items.len()),
		(25, 5, 10, 5)
	);
	assert_eq!(
		page.items
			.iter()
			.map(|item| item.1.as_str())
			.collect::<Vec<_>>(),
		[
			"widget-20",
			"widget-19",
			"widget-18",
			"widget-17",
			"widget-16"
		]
	);

	reset_state();
	let error = ModelServerFnSet::<WidgetResource>::list(
		&Principal,
		&connection,
		ListQuery {
			page: PageRequest {
				limit: Some(0),
				offset: 0,
			},
			name_prefix: "widget",
		},
	)
	.await
	.expect_err("zero limit should fail validation");
	assert!(matches!(error, ServerFnSetError::Validation(_)));
	assert_events(&[]);
}

#[tokio::test(flavor = "multi_thread")]
#[serial(model_server_fnset_runtime)]
async fn retrieve_maps_cardinality_and_authorization_deterministically() {
	let (_directory, _lease, connection) = database().await;
	insert(&connection, 1, "unique").await;
	reset_state();
	let read =
		ModelServerFnSet::<WidgetResource>::retrieve(&Principal, &connection, "unique".to_owned())
			.await
			.expect("unique row should load");
	assert_eq!(read, (1, "unique".to_owned()));
	assert_eq!(
		state()
			.lock()
			.expect("recording mutex should not be poisoned")
			.events,
		[
			"authorize_action",
			"base_queryset",
			"authorize_object",
			"to_read"
		]
	);

	let missing =
		ModelServerFnSet::<WidgetResource>::retrieve(&Principal, &connection, "missing".to_owned())
			.await;
	assert_eq!(
		missing,
		Err(ServerFnSetError::NotFound {
			resource: "widget".to_owned(),
		})
	);
	assert!(
		!serde_json::to_string(&missing)
			.expect("not-found error should serialize")
			.contains("tenant_42_widgets")
	);

	insert(&connection, 2, "duplicate").await;
	insert(&connection, 3, "duplicate").await;
	let duplicate = ModelServerFnSet::<WidgetResource>::retrieve(
		&Principal,
		&connection,
		"duplicate".to_owned(),
	)
	.await;
	assert!(matches!(duplicate, Err(ServerFnSetError::Internal)));

	reset_state();
	state()
		.lock()
		.expect("recording mutex should not be poisoned")
		.access_failure = AccessFailure::Unauthenticated;
	let unauthenticated =
		ModelServerFnSet::<WidgetResource>::retrieve(&Principal, &connection, "unique".to_owned())
			.await;
	assert!(matches!(
		unauthenticated,
		Err(ServerFnSetError::Unauthenticated)
	));
	reset_state();
	state()
		.lock()
		.expect("recording mutex should not be poisoned")
		.access_failure = AccessFailure::Forbidden;
	let forbidden =
		ModelServerFnSet::<WidgetResource>::retrieve(&Principal, &connection, "unique".to_owned())
			.await;
	assert!(matches!(forbidden, Err(ServerFnSetError::Forbidden)));
}

#[tokio::test(flavor = "multi_thread")]
#[serial(model_server_fnset_runtime)]
async fn mutations_share_one_executor_and_rollback_post_persistence_failures() {
	let (_directory, _lease, connection) = database().await;
	insert(&connection, 1, "original").await;

	reset_state();
	state()
		.lock()
		.expect("recording mutex should not be poisoned")
		.fail_to_read = true;
	let create = ModelServerFnSet::<WidgetResource>::create(
		&Principal,
		&connection,
		CreateInput("created".to_owned()),
	)
	.await;
	assert!(matches!(create, Err(ServerFnSetError::Application { .. })));
	assert_eq!(count(&connection).await, 1);
	assert_one_executor();
	assert_events(&[
		"authorize_action",
		"validate_create",
		"perform_create",
		"authorize_object",
		"to_read",
	]);

	reset_state();
	state()
		.lock()
		.expect("recording mutex should not be poisoned")
		.fail_to_read = true;
	let update = ModelServerFnSet::<WidgetResource>::update(
		&Principal,
		&connection,
		"original".to_owned(),
		UpdateInput("updated".to_owned()),
	)
	.await;
	assert!(matches!(update, Err(ServerFnSetError::Application { .. })));
	assert_one_executor();
	assert_events(&[
		"authorize_action",
		"base_queryset",
		"authorize_object",
		"validate_update",
		"perform_update",
		"authorize_object",
		"to_read",
	]);
	let original = QuerySet::<Widget>::new()
		.filter(Filter::new(
			"id",
			reinhardt_db::orm::FilterOperator::Eq,
			FilterValue::Integer(1_i64),
		))
		.one_with_db(&connection)
		.await
		.expect("original row should load");
	assert_eq!(original[0].name, "original");

	reset_state();
	state()
		.lock()
		.expect("recording mutex should not be poisoned")
		.fail_to_read = true;
	let patch = ModelServerFnSet::<WidgetResource>::partial_update(
		&Principal,
		&connection,
		"original".to_owned(),
		PatchInput("patched".to_owned()),
	)
	.await;
	assert!(matches!(patch, Err(ServerFnSetError::Application { .. })));
	assert_one_executor();
	assert_events(&[
		"authorize_action",
		"base_queryset",
		"authorize_object",
		"validate_patch",
		"perform_patch",
		"authorize_object",
		"to_read",
	]);

	reset_state();
	state()
		.lock()
		.expect("recording mutex should not be poisoned")
		.fail_destroy = true;
	let destroy =
		ModelServerFnSet::<WidgetResource>::destroy(&Principal, &connection, "original".to_owned())
			.await;
	assert!(matches!(destroy, Err(ServerFnSetError::Application { .. })));
	assert_eq!(count(&connection).await, 1);
	assert_one_executor();
	assert_events(&[
		"authorize_action",
		"base_queryset",
		"authorize_object",
		"perform_destroy",
	]);

	reset_state();
	let created = ModelServerFnSet::<WidgetResource>::create(
		&Principal,
		&connection,
		CreateInput("created".to_owned()),
	)
	.await
	.expect("create should commit");
	assert_eq!(created.1, "created");
	assert_one_executor();

	reset_state();
	let updated = ModelServerFnSet::<WidgetResource>::update(
		&Principal,
		&connection,
		"original".to_owned(),
		UpdateInput("updated".to_owned()),
	)
	.await
	.expect("update should commit");
	assert_eq!(updated.1, "updated");
	assert_one_executor();

	reset_state();
	let patched = ModelServerFnSet::<WidgetResource>::partial_update(
		&Principal,
		&connection,
		"updated".to_owned(),
		PatchInput("patched".to_owned()),
	)
	.await
	.expect("partial update should commit");
	assert_eq!(patched.1, "patched");
	assert_one_executor();

	reset_state();
	ModelServerFnSet::<WidgetResource>::destroy(&Principal, &connection, "patched".to_owned())
		.await
		.expect("destroy should commit");
	assert_eq!(count(&connection).await, 1);
	assert_one_executor();
}

#[tokio::test(flavor = "current_thread")]
#[serial(model_server_fnset_runtime)]
async fn mutation_errors_rollback_explicitly_on_current_thread_runtimes() {
	let (_directory, _lease, connection) = database().await;
	insert(&connection, 1, "original").await;

	reset_state();
	state()
		.lock()
		.expect("recording mutex should not be poisoned")
		.access_failure = AccessFailure::Forbidden;
	let forbidden = ModelServerFnSet::<WidgetResource>::create(
		&Principal,
		&connection,
		CreateInput("forbidden".to_owned()),
	)
	.await;
	assert!(matches!(forbidden, Err(ServerFnSetError::Forbidden)));
	assert_eq!(count(&connection).await, 1);
	assert_events(&["authorize_action"]);

	reset_state();
	state()
		.lock()
		.expect("recording mutex should not be poisoned")
		.fail_validation = true;
	let invalid = ModelServerFnSet::<WidgetResource>::create(
		&Principal,
		&connection,
		CreateInput("invalid".to_owned()),
	)
	.await;
	assert!(matches!(invalid, Err(ServerFnSetError::Validation(_))));
	assert_eq!(count(&connection).await, 1);
	assert_events(&["authorize_action", "validate_create"]);

	reset_state();
	state()
		.lock()
		.expect("recording mutex should not be poisoned")
		.fail_to_read = true;
	let conversion_failure = ModelServerFnSet::<WidgetResource>::create(
		&Principal,
		&connection,
		CreateInput("created".to_owned()),
	)
	.await;
	assert!(matches!(
		conversion_failure,
		Err(ServerFnSetError::Application { .. })
	));
	assert_eq!(count(&connection).await, 1);
	assert_one_executor();
	assert_events(&[
		"authorize_action",
		"validate_create",
		"perform_create",
		"authorize_object",
		"to_read",
	]);
}

#[tokio::test(flavor = "current_thread")]
#[serial(model_server_fnset_runtime)]
async fn create_authorizes_the_created_object_before_committing() {
	let (_directory, _lease, connection) = database().await;
	insert(&connection, 1, "original").await;
	reset_state();
	state()
		.lock()
		.expect("recording mutex should not be poisoned")
		.deny_created_object = true;

	let result = ModelServerFnSet::<WidgetResource>::create(
		&Principal,
		&connection,
		CreateInput("forbidden".to_owned()),
	)
	.await;

	assert!(matches!(result, Err(ServerFnSetError::Forbidden)));
	assert_eq!(count(&connection).await, 1);
	assert_one_executor();
	assert_events(&[
		"authorize_action",
		"validate_create",
		"perform_create",
		"authorize_object",
	]);
}

#[tokio::test(flavor = "current_thread")]
#[serial(model_server_fnset_runtime)]
async fn update_and_patch_reauthorize_mutated_objects_before_committing() {
	let (_directory, _lease, connection) = database().await;
	insert(&connection, 1, "original").await;

	reset_state();
	state()
		.lock()
		.expect("recording mutex should not be poisoned")
		.deny_mutated_object = true;
	let update = ModelServerFnSet::<WidgetResource>::update(
		&Principal,
		&connection,
		"original".to_owned(),
		UpdateInput("forbidden".to_owned()),
	)
	.await;
	assert!(matches!(update, Err(ServerFnSetError::Forbidden)));
	let original = QuerySet::<Widget>::new()
		.filter(Filter::new(
			"id",
			reinhardt_db::orm::FilterOperator::Eq,
			FilterValue::Integer(1_i64),
		))
		.one_with_db(&connection)
		.await
		.expect("rejected update should roll back");
	assert_eq!(original[0].name, "original");
	assert_one_executor();
	assert_events(&[
		"authorize_action",
		"base_queryset",
		"authorize_object",
		"validate_update",
		"perform_update",
		"authorize_object",
	]);

	reset_state();
	state()
		.lock()
		.expect("recording mutex should not be poisoned")
		.deny_mutated_object = true;
	let patch = ModelServerFnSet::<WidgetResource>::partial_update(
		&Principal,
		&connection,
		"original".to_owned(),
		PatchInput("forbidden".to_owned()),
	)
	.await;
	assert!(matches!(patch, Err(ServerFnSetError::Forbidden)));
	let original = QuerySet::<Widget>::new()
		.filter(Filter::new(
			"id",
			reinhardt_db::orm::FilterOperator::Eq,
			FilterValue::Integer(1_i64),
		))
		.one_with_db(&connection)
		.await
		.expect("rejected patch should roll back");
	assert_eq!(original[0].name, "original");
	assert_one_executor();
	assert_events(&[
		"authorize_action",
		"base_queryset",
		"authorize_object",
		"validate_patch",
		"perform_patch",
		"authorize_object",
	]);
}

#[tokio::test(flavor = "current_thread")]
#[serial(model_server_fnset_runtime)]
async fn transactional_custom_detail_actions_receive_authorized_object_and_rollback() {
	let (_directory, _lease, connection) = database().await;
	insert(&connection, 1, "original").await;
	reset_state();

	let result = ModelServerFnSet::<WidgetResource>::transactional_detail_action(
		&Principal,
		&connection,
		"original".to_owned(),
		ServerFnSetAction::Custom("publish"),
		|mut context| {
			Box::pin(async move {
				let (object, executor) = context.parts_mut();
				object.name = "published".to_owned();
				object
					.save_with_executor(executor)
					.await
					.map_err(|_| ServerFnSetError::Internal)?;
				Err::<(), _>(ServerFnSetError::Conflict {
					code: "publish_failed".to_owned(),
					message: "publish failed".to_owned(),
				})
			})
		},
	)
	.await;

	assert!(matches!(result, Err(ServerFnSetError::Conflict { .. })));
	let original = QuerySet::<Widget>::new()
		.filter(Filter::new(
			"id",
			reinhardt_db::orm::FilterOperator::Eq,
			FilterValue::Integer(1_i64),
		))
		.one_with_db(&connection)
		.await
		.expect("original row should load after rollback");
	assert_eq!(original[0].name, "original");
}

#[tokio::test(flavor = "current_thread")]
#[serial(model_server_fnset_runtime)]
async fn transactional_detail_overrides_reauthorize_the_mutated_object() {
	let (_directory, _lease, connection) = database().await;
	insert(&connection, 1, "original").await;
	reset_state();
	state()
		.lock()
		.expect("recording mutex should not be poisoned")
		.deny_mutated_object = true;

	let result = ModelServerFnSet::<WidgetResource>::transactional_detail_action(
		&Principal,
		&connection,
		"original".to_owned(),
		ServerFnSetAction::Update,
		|mut context| {
			Box::pin(async move {
				context.object_mut().name = "forbidden".to_owned();
				let (object, executor) = context.parts_mut();
				object
					.save_with_executor(executor)
					.await
					.map_err(|_| ServerFnSetError::Internal)?;
				Ok::<_, ServerFnSetError>(())
			})
		},
	)
	.await;

	assert!(matches!(result, Err(ServerFnSetError::Forbidden)));
	let original = QuerySet::<Widget>::new()
		.filter(Filter::new(
			"id",
			reinhardt_db::orm::FilterOperator::Eq,
			FilterValue::Integer(1_i64),
		))
		.one_with_db(&connection)
		.await
		.expect("rejected override should roll back");
	assert_eq!(original[0].name, "original");
	assert_events(&[
		"authorize_action",
		"base_queryset",
		"authorize_object",
		"authorize_object",
	]);
}

#[tokio::test(flavor = "current_thread")]
#[serial(model_server_fnset_runtime)]
async fn create_overrides_can_authorize_objects_before_persistence() {
	let (_directory, _lease, connection) = database().await;
	insert(&connection, 1, "original").await;
	reset_state();
	state()
		.lock()
		.expect("recording mutex should not be poisoned")
		.deny_created_object = true;

	let result = ModelServerFnSet::<WidgetResource>::transactional_create_action(
		&Principal,
		&connection,
		|mut context: CreateActionContext<'_, WidgetResource>| {
			Box::pin(async move {
				let mut object = Widget {
					id: None,
					name: "created".to_owned(),
				};
				context.authorize_object(&object).await?;
				object
					.save_with_executor(context.executor_mut())
					.await
					.map_err(|_| ServerFnSetError::Internal)?;
				Ok::<_, ServerFnSetError>(())
			})
		},
	)
	.await;

	assert!(matches!(result, Err(ServerFnSetError::Forbidden)));
	let rows = connection
		.query("SELECT COUNT(*) AS count FROM tenant_42_widgets", vec![])
		.await
		.expect("widget count query should succeed");
	assert_eq!(rows[0].get::<i64>("count").expect("count column"), 1);
	assert_events(&["authorize_action", "authorize_object"]);
}

#[tokio::test(flavor = "current_thread")]
#[serial(model_server_fnset_runtime)]
async fn transactional_create_overrides_authorize_without_scoping_and_rollback() {
	let (_directory, _lease, connection) = database().await;
	insert(&connection, 1, "original").await;
	reset_state();

	let result = ModelServerFnSet::<WidgetResource>::transactional_create_action(
		&Principal,
		&connection,
		|mut context| {
			Box::pin(async move {
				record("create_override", Some(context.executor_mut()));
				let mut object = Widget {
					id: None,
					name: "created".to_owned(),
				};
				object
					.save_with_executor(context.executor_mut())
					.await
					.map_err(|_| ServerFnSetError::Internal)?;
				Err::<(), _>(ServerFnSetError::Conflict {
					code: "create_failed".to_owned(),
					message: "create failed".to_owned(),
				})
			})
		},
	)
	.await;

	assert!(matches!(result, Err(ServerFnSetError::Conflict { .. })));
	assert_eq!(count(&connection).await, 1);
	assert_events(&["authorize_action", "create_override"]);
	assert_one_executor();
}

// Generated endpoints are intentionally not invoked by this metadata-only test module;
// its public fixture types also exist solely to satisfy generated public signatures.
#[allow(dead_code, unused_imports, unreachable_pub)]
mod generated_metadata {
	include!("ui/server_fnset/pass/model_crud_types.inc");

	struct ArticleActions;

	#[server_fnset(name = "article-api", actions = ArticleActions)]
	fn article_fns() -> ModelServerFnSet<ArticleResource> {
		ModelServerFnSet::new()
	}

	#[server_fnset(for = article_fns)]
	impl ArticleActions {
		#[action(detail = true, transactional = true)]
		async fn publish(
			lookup: i64,
			input: PublishArticle,
			#[inject] context: DetailActionContext<ArticleResource>,
		) -> Result<ArticleDto, ServerFnSetError> {
			Ok(ArticleDto {
				id: lookup,
				title: format!("{}:{}", context.object().title, input.label),
			})
		}
	}

	#[test]
	fn linked_sets_use_the_declared_name_for_all_ordered_metadata() {
		let metadata = article_fns().metadata();
		let actual: Vec<_> = metadata
			.actions
			.iter()
			.map(|action| {
				(
					action.path,
					action.name,
					action.detail,
					action.transactional,
				)
			})
			.collect();

		assert_eq!(metadata.name, "article-api");
		assert_eq!(
			actual,
			vec![
				(
					"/api/server_fn/article-api/list",
					"article-api-list",
					false,
					false,
				),
				(
					"/api/server_fn/article-api/retrieve",
					"article-api-retrieve",
					true,
					false,
				),
				(
					"/api/server_fn/article-api/create",
					"article-api-create",
					false,
					true,
				),
				(
					"/api/server_fn/article-api/update",
					"article-api-update",
					true,
					true,
				),
				(
					"/api/server_fn/article-api/partial-update",
					"article-api-partial-update",
					true,
					true,
				),
				(
					"/api/server_fn/article-api/destroy",
					"article-api-destroy",
					true,
					true,
				),
				(
					"/api/server_fn/article-api/publish",
					"article-api-publish",
					true,
					true,
				),
			],
		);
	}
}
