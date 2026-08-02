use std::marker::PhantomData;

use reinhardt_db::orm::{DatabaseConnection, QuerySet, TransactionExecutor};

use super::{ModelServerFnResource, ServerFnSetAction, ServerFnSetError, ServerFnSetPolicy};

/// Transaction-bound context for collection overrides and custom actions.
pub struct CollectionActionContext<'a, R>
where
	R: ModelServerFnResource,
{
	queryset: QuerySet<R::Model>,
	executor: &'a mut dyn TransactionExecutor,
}

/// Transaction-bound context for a standard create override.
///
/// Create authorization does not expose or scope an existing-object queryset.
pub struct CreateActionContext<'a, R>
where
	R: ModelServerFnResource,
{
	principal: &'a <<R as ModelServerFnResource>::Policy as ServerFnSetPolicy<R>>::Principal,
	executor: &'a mut dyn TransactionExecutor,
	_resource: PhantomData<fn() -> R>,
}

impl<'a, R> CreateActionContext<'a, R>
where
	R: ModelServerFnResource,
{
	pub(crate) fn new(
		principal: &'a <<R as ModelServerFnResource>::Policy as ServerFnSetPolicy<R>>::Principal,
		executor: &'a mut dyn TransactionExecutor,
	) -> Self {
		Self {
			principal,
			executor,
			_resource: PhantomData,
		}
	}

	/// Authorize a model before a create override persists it.
	pub async fn authorize_object(&mut self, object: &R::Model) -> Result<(), ServerFnSetError> {
		<R::Policy as ServerFnSetPolicy<R>>::authorize_object(
			self.principal,
			ServerFnSetAction::Create,
			object,
			Some(&mut *self.executor),
		)
		.await
	}

	/// Return the active transaction executor.
	pub fn executor_mut(&mut self) -> &mut (dyn TransactionExecutor + 'a) {
		self.executor
	}
}

/// Read-only context for a policy-scoped collection action.
pub struct CollectionReadActionContext<'a, R>
where
	R: ModelServerFnResource,
{
	queryset: QuerySet<R::Model>,
	connection: &'a DatabaseConnection,
}

impl<'a, R> CollectionReadActionContext<'a, R>
where
	R: ModelServerFnResource,
{
	pub(crate) fn new(queryset: QuerySet<R::Model>, connection: &'a DatabaseConnection) -> Self {
		Self {
			queryset,
			connection,
		}
	}

	/// Return the policy-scoped collection queryset.
	pub fn queryset(&self) -> &QuerySet<R::Model> {
		&self.queryset
	}
	/// Return the explicit read connection.
	pub fn connection(&self) -> &DatabaseConnection {
		self.connection
	}
}

/// Read-only context for an authorized detail action.
pub struct DetailReadActionContext<'a, R>
where
	R: ModelServerFnResource,
{
	object: R::Model,
	connection: &'a DatabaseConnection,
}

impl<'a, R> DetailReadActionContext<'a, R>
where
	R: ModelServerFnResource,
{
	pub(crate) fn new(object: R::Model, connection: &'a DatabaseConnection) -> Self {
		Self { object, connection }
	}

	/// Return the authorized model object.
	pub fn object(&self) -> &R::Model {
		&self.object
	}
	/// Return the explicit read connection.
	pub fn connection(&self) -> &DatabaseConnection {
		self.connection
	}
}

impl<'a, R> CollectionActionContext<'a, R>
where
	R: ModelServerFnResource,
{
	// This constructor is reserved for the sibling model action runtime.
	#[allow(dead_code)]
	pub(crate) fn new(
		queryset: QuerySet<R::Model>,
		executor: &'a mut dyn TransactionExecutor,
	) -> Self {
		Self { queryset, executor }
	}

	/// Return the policy-scoped collection queryset.
	pub fn queryset(&self) -> &QuerySet<R::Model> {
		&self.queryset
	}

	/// Mutably access the policy-scoped collection queryset.
	pub fn queryset_mut(&mut self) -> &mut QuerySet<R::Model> {
		&mut self.queryset
	}

	/// Return the active transaction executor.
	pub fn executor_mut(&mut self) -> &mut (dyn TransactionExecutor + 'a) {
		self.executor
	}

	/// Borrow the scoped queryset and active executor together.
	pub fn parts_mut(&mut self) -> (&mut QuerySet<R::Model>, &mut (dyn TransactionExecutor + 'a)) {
		(&mut self.queryset, self.executor)
	}
}

/// Transaction-bound context for detail overrides and custom actions.
pub struct DetailActionContext<'a, R>
where
	R: ModelServerFnResource,
{
	object: &'a mut R::Model,
	executor: &'a mut dyn TransactionExecutor,
}

impl<'a, R> DetailActionContext<'a, R>
where
	R: ModelServerFnResource,
{
	// This constructor is reserved for the sibling model action runtime.
	#[allow(dead_code)]
	pub(crate) fn new(object: &'a mut R::Model, executor: &'a mut dyn TransactionExecutor) -> Self {
		Self { object, executor }
	}

	/// Return the authorized model object.
	pub fn object(&self) -> &R::Model {
		&*self.object
	}

	/// Mutably access the authorized model object.
	pub fn object_mut(&mut self) -> &mut R::Model {
		&mut *self.object
	}

	/// Return the active transaction executor.
	pub fn executor_mut(&mut self) -> &mut (dyn TransactionExecutor + 'a) {
		self.executor
	}

	/// Borrow the authorized object and active executor together.
	pub fn parts_mut(&mut self) -> (&mut R::Model, &mut (dyn TransactionExecutor + 'a)) {
		(&mut *self.object, self.executor)
	}
}

#[cfg(test)]
mod tests {
	use std::sync::{Arc, Mutex};

	use async_trait::async_trait;
	use reinhardt_db::backends::error::Result as BackendResult;
	use reinhardt_db::backends::types::{DatabaseType, QueryResult, QueryValue, Row};
	use reinhardt_db::orm::events::{
		EventRegistry, EventResult, MapperEvents, set_active_registry,
	};
	use reinhardt_db::orm::{FieldSelector, Manager, Model};
	use serde::{Deserialize, Serialize};
	use serde_json::Value as JsonValue;
	use serial_test::serial;

	use super::*;
	use crate::server_fn::{
		AllowAllPolicy, PageRequest, ServerFnListQuery, ServerFnResource, ServerFnSetError,
	};

	struct RecordingExecutor {
		operations: Arc<Mutex<Vec<&'static str>>>,
	}

	impl RecordingExecutor {
		fn record(&self, operation: &'static str) {
			self.operations
				.lock()
				.expect("operations mutex should not be poisoned")
				.push(operation);
		}

		fn model_row() -> Row {
			let mut row = Row::new();
			row.insert("id".to_owned(), QueryValue::Int(7));
			row.insert("name".to_owned(), QueryValue::String("updated".to_owned()));
			row
		}
	}

	#[async_trait]
	impl TransactionExecutor for RecordingExecutor {
		fn backend(&self) -> DatabaseType {
			DatabaseType::Postgres
		}

		async fn execute(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> BackendResult<QueryResult> {
			self.record("save");
			Ok(QueryResult {
				rows_affected: 1,
				last_insert_id: None,
			})
		}

		async fn fetch_one(&mut self, sql: &str, _params: Vec<QueryValue>) -> BackendResult<Row> {
			if sql.to_ascii_uppercase().contains("COUNT") {
				self.record("count");
				let mut row = Row::new();
				row.insert("count".to_owned(), QueryValue::Int(1));
				Ok(row)
			} else {
				self.record("save");
				Ok(Self::model_row())
			}
		}

		async fn fetch_all(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> BackendResult<Vec<Row>> {
			Ok(vec![Self::model_row()])
		}

		async fn fetch_optional(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> BackendResult<Option<Row>> {
			Ok(Some(Self::model_row()))
		}

		async fn commit(self: Box<Self>) -> BackendResult<()> {
			Ok(())
		}

		async fn rollback(self: Box<Self>) -> BackendResult<()> {
			Ok(())
		}
	}

	#[derive(Clone)]
	struct WidgetFields;

	impl FieldSelector for WidgetFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	#[derive(Clone, Deserialize, Serialize)]
	struct Widget {
		id: Option<i64>,
		name: String,
	}

	impl Model for Widget {
		type PrimaryKey = i64;
		type Fields = WidgetFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"widgets"
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

	struct WidgetListQuery;

	impl ServerFnListQuery for WidgetListQuery {
		fn page_request(&self) -> PageRequest {
			PageRequest::default()
		}
	}

	struct WidgetResource;

	impl ServerFnResource for WidgetResource {
		type Lookup = i64;
		type Read = i64;
		type Create = i64;
		type Update = i64;
		type Patch = i64;
		type ListQuery = WidgetListQuery;
	}

	#[async_trait]
	impl ModelServerFnResource for WidgetResource {
		type Model = Widget;
		type Policy = AllowAllPolicy;

		fn lookup_field() -> reinhardt_db::orm::UniqueFieldRef<Self::Model, Self::Lookup> {
			// SAFETY: The handwritten test model declares `id` as its unique primary key.
			unsafe { reinhardt_db::orm::UniqueFieldRef::from_model_field_with_names("id", "id") }
		}

		async fn to_read(
			model: &Self::Model,
			_executor: Option<&mut dyn TransactionExecutor>,
		) -> Result<Self::Read, ServerFnSetError> {
			model.id.ok_or_else(|| ServerFnSetError::NotFound {
				resource: "widget".to_string(),
			})
		}
	}

	struct RecordingMapperEvents {
		operations: Arc<Mutex<Vec<&'static str>>>,
	}

	#[async_trait]
	impl MapperEvents for RecordingMapperEvents {
		async fn before_update(&self, _instance_id: &str, _values: &JsonValue) -> EventResult {
			self.operations
				.lock()
				.expect("operations mutex should not be poisoned")
				.push("before_update");
			EventResult::Continue
		}

		async fn after_update(&self, _instance_id: &str) -> EventResult {
			self.operations
				.lock()
				.expect("operations mutex should not be poisoned")
				.push("after_update");
			EventResult::Continue
		}
	}

	#[tokio::test(flavor = "current_thread")]
	#[serial(model_server_fnset_context_events)]
	async fn context_parts_share_one_executor_for_queries_persistence_and_events() {
		let operations = Arc::new(Mutex::new(Vec::new()));
		let registry = Arc::new(EventRegistry::new());
		registry.register_mapper_listener(
			Widget::table_name().to_owned(),
			Arc::new(RecordingMapperEvents {
				operations: operations.clone(),
			}),
		);
		let _registry_guard = set_active_registry(registry);
		let mut executor = RecordingExecutor {
			operations: operations.clone(),
		};

		{
			let mut context =
				CollectionActionContext::<WidgetResource>::new(QuerySet::new(), &mut executor);
			let (queryset, transaction_executor) = context.parts_mut();
			let count = queryset
				.count_with_executor(transaction_executor)
				.await
				.expect("collection query should use the context executor");
			assert_eq!(count, 1);
		}

		{
			let mut object = Widget {
				id: Some(7),
				name: "updated".to_owned(),
			};
			let mut context =
				DetailActionContext::<WidgetResource>::new(&mut object, &mut executor);
			let (object, transaction_executor) = context.parts_mut();
			object
				.save_with_executor(transaction_executor)
				.await
				.expect("model persistence should use the context executor");
		}

		assert_eq!(
			operations
				.lock()
				.expect("operations mutex should not be poisoned")
				.as_slice(),
			["count", "before_update", "save", "after_update"]
		);
	}
}
