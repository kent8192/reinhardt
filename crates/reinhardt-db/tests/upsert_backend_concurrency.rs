//! Live-backend coverage for typed upsert builders and concurrent races.

#![cfg(all(feature = "postgres", feature = "mysql", feature = "sqlite"))]

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind, Error, Result};
use reinhardt_db::orm::connection::{
	BackendsConnection, DatabaseBackend, DatabaseConnection, DatabaseConnectionLease,
};
use reinhardt_db::orm::custom_manager::CustomManager;
use reinhardt_db::orm::expressions::FieldRef;
use reinhardt_db::orm::fields::FieldKwarg;
use reinhardt_db::orm::inspection::{ConstraintInfo, ConstraintType, FieldInfo};
use reinhardt_db::orm::manager::Manager;
use reinhardt_db::orm::model::{FieldSelector, Model};
use reinhardt_db::orm::query::{Filter, FilterOperator, FilterValue, QuerySet};
use reinhardt_db::orm::upsert::UpsertWrite;
use serde::{Deserialize, Serialize};
use serial_test::serial;
use tempfile::TempDir;
use testcontainers::{
	ContainerAsync, GenericImage, ImageExt,
	core::{IntoContainerPort, WaitFor},
	runners::AsyncRunner,
};

#[derive(Clone, Copy, Debug)]
enum LiveBackend {
	Postgres,
	MySql,
	Sqlite,
}

impl LiveBackend {
	fn database_backend(self) -> DatabaseBackend {
		match self {
			Self::Postgres => DatabaseBackend::Postgres,
			Self::MySql => DatabaseBackend::MySql,
			Self::Sqlite => DatabaseBackend::Sqlite,
		}
	}
}

struct BackendFixture {
	backend: LiveBackend,
	connection: DatabaseConnection,
	_lease: DatabaseConnectionLease,
	_container: Option<ContainerAsync<GenericImage>>,
	_directory: Option<TempDir>,
}

impl BackendFixture {
	async fn postgres() -> Self {
		let image = GenericImage::new("postgres", "16-alpine")
			.with_exposed_port(5432.tcp())
			.with_wait_for(WaitFor::message_on_stderr(
				"database system is ready to accept connections",
			))
			.with_startup_timeout(Duration::from_secs(120))
			.with_env_var("POSTGRES_HOST_AUTH_METHOD", "trust");
		let container = image
			.start()
			.await
			.expect("PostgreSQL container should start");
		let port = container
			.get_host_port_ipv4(5432)
			.await
			.expect("PostgreSQL port should be exposed");
		let url = format!("postgres://postgres@127.0.0.1:{port}/postgres?sslmode=disable");
		let (lease, connection) = connect(LiveBackend::Postgres, &url).await;
		Self {
			backend: LiveBackend::Postgres,
			connection,
			_lease: lease,
			_container: Some(container),
			_directory: None,
		}
	}

	async fn mysql() -> Self {
		let image = GenericImage::new("mysql", "8.0")
			.with_exposed_port(3306.tcp())
			.with_wait_for(WaitFor::message_on_stderr(
				"port: 3306  MySQL Community Server",
			))
			.with_startup_timeout(Duration::from_secs(120))
			.with_env_var("MYSQL_ROOT_PASSWORD", "test")
			.with_env_var("MYSQL_DATABASE", "upsert_test");
		let container = image.start().await.expect("MySQL container should start");
		let port = container
			.get_host_port_ipv4(3306)
			.await
			.expect("MySQL port should be exposed");
		let url = format!("mysql://root:test@127.0.0.1:{port}/upsert_test");
		let (lease, connection) = connect(LiveBackend::MySql, &url).await;
		Self {
			backend: LiveBackend::MySql,
			connection,
			_lease: lease,
			_container: Some(container),
			_directory: None,
		}
	}

	async fn sqlite() -> Self {
		let directory = tempfile::Builder::new()
			.prefix("reinhardt-upsert-races-")
			.tempdir_in("/tmp")
			.expect("SQLite temporary directory should be created under /tmp");
		let database_path = directory.path().join("upsert.sqlite");
		let url = format!("sqlite:///{}", database_path.display());
		let (lease, connection) = connect_sqlite(&url, Duration::from_secs(5)).await;
		Self {
			backend: LiveBackend::Sqlite,
			connection,
			_lease: lease,
			_container: None,
			_directory: Some(directory),
		}
	}

	async fn sqlite_busy() -> Self {
		let directory = tempfile::Builder::new()
			.prefix("reinhardt-upsert-busy-")
			.tempdir_in("/tmp")
			.expect("SQLite busy-test directory should be created under /tmp");
		let database_path = directory.path().join("upsert-busy.sqlite");
		let url = format!("sqlite:///{}", database_path.display());
		let (lease, connection) = connect_sqlite(&url, Duration::ZERO).await;
		Self {
			backend: LiveBackend::Sqlite,
			connection,
			_lease: lease,
			_container: None,
			_directory: Some(directory),
		}
	}

	async fn create_schema(&self) -> Result<()> {
		let (tags, tenant_tags) = match self.backend {
			LiveBackend::Postgres => (
				"CREATE TABLE upsert_tags (
					row_id BIGSERIAL PRIMARY KEY,
					slug TEXT NOT NULL UNIQUE,
					email TEXT NOT NULL UNIQUE,
					value INTEGER NOT NULL,
					create_marker TEXT NOT NULL,
					database_note TEXT NOT NULL DEFAULT 'server-default',
					optional_note TEXT NULL
				)",
				"CREATE TABLE upsert_tenant_tags (
					id BIGSERIAL PRIMARY KEY,
					tenant_id BIGINT NOT NULL,
					slug TEXT NOT NULL,
					value INTEGER NOT NULL,
					CONSTRAINT upsert_tenant_slug_unique UNIQUE (tenant_id, slug)
				)",
			),
			LiveBackend::MySql => (
				"CREATE TABLE upsert_tags (
					row_id BIGINT NOT NULL AUTO_INCREMENT,
					slug VARCHAR(255) NOT NULL UNIQUE,
					email VARCHAR(255) NOT NULL UNIQUE,
					value INTEGER NOT NULL,
					create_marker VARCHAR(255) NOT NULL,
					database_note VARCHAR(255) NOT NULL DEFAULT 'server-default',
					optional_note VARCHAR(255) NULL,
					PRIMARY KEY (row_id)
				) ENGINE=InnoDB",
				"CREATE TABLE upsert_tenant_tags (
					id BIGINT NOT NULL AUTO_INCREMENT,
					tenant_id BIGINT NOT NULL,
					slug VARCHAR(255) NOT NULL,
					value INTEGER NOT NULL,
					PRIMARY KEY (id),
					CONSTRAINT upsert_tenant_slug_unique UNIQUE (tenant_id, slug)
				) ENGINE=InnoDB",
			),
			LiveBackend::Sqlite => (
				"CREATE TABLE upsert_tags (
					row_id INTEGER PRIMARY KEY AUTOINCREMENT,
					slug TEXT NOT NULL UNIQUE,
					email TEXT NOT NULL UNIQUE,
					value INTEGER NOT NULL,
					create_marker TEXT NOT NULL,
					database_note TEXT NOT NULL DEFAULT 'server-default',
					optional_note TEXT NULL
				)",
				"CREATE TABLE upsert_tenant_tags (
					id INTEGER PRIMARY KEY AUTOINCREMENT,
					tenant_id INTEGER NOT NULL,
					slug TEXT NOT NULL,
					value INTEGER NOT NULL,
					CONSTRAINT upsert_tenant_slug_unique UNIQUE (tenant_id, slug)
				)",
			),
		};
		self.connection.execute(tags, vec![]).await?;
		self.connection.execute(tenant_tags, vec![]).await?;
		Ok(())
	}
}

async fn connect(backend: LiveBackend, url: &str) -> (DatabaseConnectionLease, DatabaseConnection) {
	let owner = match backend {
		LiveBackend::Postgres => BackendsConnection::connect_postgres(url).await,
		LiveBackend::MySql => BackendsConnection::connect_mysql(url).await,
		LiveBackend::Sqlite => unreachable!("SQLite uses connect_sqlite"),
	}
	.unwrap_or_else(|error| {
		panic!("{backend:?} should connect after its readiness check: {error}")
	});
	let lease = DatabaseConnectionLease::register(owner).expect("connection should register");
	let connection = lease.handle();
	(lease, connection)
}

async fn connect_sqlite(
	url: &str,
	busy_timeout: Duration,
) -> (DatabaseConnectionLease, DatabaseConnection) {
	let options = sqlx::sqlite::SqliteConnectOptions::from_str(url)
		.expect("SQLite URL should parse")
		.create_if_missing(true)
		.busy_timeout(busy_timeout);
	// Pre-open every pool slot so a probed contender cannot be pending on
	// connection creation or checkout while the holder owns only one slot.
	let pool = sqlx::sqlite::SqlitePoolOptions::new()
		.max_connections(4)
		.min_connections(4)
		.connect_with(options)
		.await
		.expect("SQLite pool should connect");
	let owner = BackendsConnection::from_sqlite_pool(pool);
	let lease =
		DatabaseConnectionLease::register(owner).expect("SQLite connection should register");
	let connection = lease.handle();
	(lease, connection)
}

#[derive(Clone)]
struct TestFields;

impl FieldSelector for TestFields {
	fn with_alias(self, _alias: &str) -> Self {
		self
	}
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct Tag {
	id: Option<i64>,
	slug: String,
	email: String,
	value: i32,
	create_marker: String,
	database_note: String,
	optional_note: Option<String>,
}

impl Model for Tag {
	type PrimaryKey = i64;
	type Fields = TestFields;
	type Objects = Manager<Self>;

	fn table_name() -> &'static str {
		"upsert_tags"
	}

	fn new_fields() -> Self::Fields {
		TestFields
	}

	fn primary_key_column() -> &'static str {
		"row_id"
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = Some(value);
	}

	fn field_metadata() -> Vec<FieldInfo> {
		vec![
			field("id", Some("row_id"), false, true, false, None),
			field("slug", None, false, false, true, None),
			field("email", None, false, false, true, None),
			field("value", None, false, false, false, None),
			field("create_marker", None, false, false, false, None),
			field(
				"database_note",
				None,
				false,
				false,
				false,
				Some(FieldKwarg::String("server-default".to_owned())),
			),
			field("optional_note", None, true, false, false, None),
		]
	}

	fn generated_field_names() -> &'static [&'static str] {
		&["id"]
	}
}

impl Tag {
	fn field_slug() -> FieldRef<Self, String> {
		// SAFETY: the logical and physical names match Tag::field_metadata.
		unsafe { FieldRef::from_model_field("slug", "slug") }
	}

	fn field_email() -> FieldRef<Self, String> {
		// SAFETY: the logical and physical names match Tag::field_metadata.
		unsafe { FieldRef::from_model_field("email", "email") }
	}

	fn field_value() -> FieldRef<Self, i32> {
		// SAFETY: the logical and physical names match Tag::field_metadata.
		unsafe { FieldRef::from_model_field("value", "value") }
	}

	fn field_create_marker() -> FieldRef<Self, String> {
		// SAFETY: the logical and physical names match Tag::field_metadata.
		unsafe { FieldRef::from_model_field("create_marker", "create_marker") }
	}

	fn field_optional_note() -> FieldRef<Self, Option<String>> {
		// SAFETY: the logical and physical names match Tag::field_metadata.
		unsafe { FieldRef::from_model_field("optional_note", "optional_note") }
	}
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct TenantTag {
	id: Option<i64>,
	tenant_id: i64,
	slug: String,
	value: i32,
}

impl Model for TenantTag {
	type PrimaryKey = i64;
	type Fields = TestFields;
	type Objects = Manager<Self>;

	fn table_name() -> &'static str {
		"upsert_tenant_tags"
	}

	fn new_fields() -> Self::Fields {
		TestFields
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = Some(value);
	}

	fn field_metadata() -> Vec<FieldInfo> {
		vec![
			field("id", None, false, true, false, None),
			field("tenant_id", None, false, false, false, None),
			field("slug", None, false, false, false, None),
			field("value", None, false, false, false, None),
		]
	}

	fn constraint_metadata() -> Vec<ConstraintInfo> {
		vec![ConstraintInfo {
			name: "upsert_tenant_slug_unique".to_owned(),
			constraint_type: ConstraintType::Unique,
			definition: "UNIQUE (tenant_id, slug)".to_owned(),
			fields: vec!["tenant_id".to_owned(), "slug".to_owned()],
			condition: None,
			deferrable: false,
			nulls_distinct: None,
		}]
	}

	fn generated_field_names() -> &'static [&'static str] {
		&["id"]
	}
}

impl TenantTag {
	fn field_tenant_id() -> FieldRef<Self, i64> {
		// SAFETY: the logical and physical names match TenantTag::field_metadata.
		unsafe { FieldRef::from_model_field("tenant_id", "tenant_id") }
	}

	fn field_slug() -> FieldRef<Self, String> {
		// SAFETY: the logical and physical names match TenantTag::field_metadata.
		unsafe { FieldRef::from_model_field("slug", "slug") }
	}

	fn field_value() -> FieldRef<Self, i32> {
		// SAFETY: the logical and physical names match TenantTag::field_metadata.
		unsafe { FieldRef::from_model_field("value", "value") }
	}
}

fn field(
	name: &str,
	db_column: Option<&str>,
	nullable: bool,
	primary_key: bool,
	unique: bool,
	db_default: Option<FieldKwarg>,
) -> FieldInfo {
	FieldInfo {
		name: name.to_owned(),
		field_type: "test".to_owned(),
		storage_kind: None,
		domain: None,
		nullable,
		primary_key,
		unique,
		blank: false,
		editable: !primary_key,
		default: None,
		db_default,
		db_column: db_column.map(str::to_owned),
		choices: None,
		attributes: HashMap::new(),
	}
}

#[derive(Clone)]
struct TagRaceManager {
	coordinator: HookCoordinator,
}

impl Default for TagRaceManager {
	fn default() -> Self {
		Self {
			coordinator: HookCoordinator::new(1),
		}
	}
}

impl CustomManager for TagRaceManager {
	type Model = Tag;

	fn new() -> Self {
		Self::default()
	}

	fn before_upsert_write(&self, write: &mut UpsertWrite<'_, Tag>) -> Result<()> {
		if matches!(write, UpsertWrite::Create(_)) {
			self.coordinator.arrive_and_wait()?;
		}
		Ok(())
	}
}

#[derive(Clone)]
struct TenantTagRaceManager {
	coordinator: HookCoordinator,
}

impl Default for TenantTagRaceManager {
	fn default() -> Self {
		Self {
			coordinator: HookCoordinator::new(1),
		}
	}
}

impl CustomManager for TenantTagRaceManager {
	type Model = TenantTag;

	fn new() -> Self {
		Self::default()
	}

	fn before_upsert_write(&self, write: &mut UpsertWrite<'_, TenantTag>) -> Result<()> {
		if matches!(write, UpsertWrite::Create(_)) {
			self.coordinator.arrive_and_wait()?;
		}
		Ok(())
	}
}

#[derive(Default)]
struct HookState {
	arrived: usize,
	released: bool,
	aborted: bool,
}

struct HookCoordinatorInner {
	expected: usize,
	state: Mutex<HookState>,
	changed: Condvar,
}

#[derive(Clone)]
struct HookCoordinator(Arc<HookCoordinatorInner>);

impl HookCoordinator {
	fn new(expected: usize) -> Self {
		Self(Arc::new(HookCoordinatorInner {
			expected,
			state: Mutex::new(HookState::default()),
			changed: Condvar::new(),
		}))
	}

	fn arrive_and_wait(&self) -> Result<()> {
		let mut state =
			self.0.state.lock().map_err(|error| {
				Error::Internal(format!("hook coordinator lock poisoned: {error}"))
			})?;
		state.arrived += 1;
		self.0.changed.notify_all();
		while !state.released && !state.aborted {
			state = self.0.changed.wait(state).map_err(|error| {
				Error::Internal(format!("hook coordinator wait poisoned: {error}"))
			})?;
		}
		if state.aborted {
			return Err(Error::Internal(
				"upsert race coordination aborted".to_owned(),
			));
		}
		Ok(())
	}

	fn wait_until_ready(&self) -> Result<()> {
		let deadline = Instant::now() + Duration::from_secs(30);
		let mut state =
			self.0.state.lock().map_err(|error| {
				Error::Internal(format!("hook coordinator lock poisoned: {error}"))
			})?;
		while state.arrived < self.0.expected {
			let remaining = deadline.saturating_duration_since(Instant::now());
			if remaining.is_zero() {
				return Err(Error::Internal(format!(
					"only {} of {} upsert hooks reached the coordinator",
					state.arrived, self.0.expected
				)));
			}
			let (next, timeout) =
				self.0
					.changed
					.wait_timeout(state, remaining)
					.map_err(|error| {
						Error::Internal(format!("hook coordinator wait poisoned: {error}"))
					})?;
			state = next;
			if timeout.timed_out() && state.arrived < self.0.expected {
				return Err(Error::Internal(format!(
					"only {} of {} upsert hooks reached the coordinator",
					state.arrived, self.0.expected
				)));
			}
		}
		Ok(())
	}

	fn release(&self) {
		let mut state = self
			.0
			.state
			.lock()
			.unwrap_or_else(std::sync::PoisonError::into_inner);
		state.released = true;
		self.0.changed.notify_all();
	}

	fn abort(&self) {
		let mut state = self
			.0
			.state
			.lock()
			.unwrap_or_else(std::sync::PoisonError::into_inner);
		state.aborted = true;
		self.0.changed.notify_all();
	}
}

struct HookAbortGuard {
	coordinator: HookCoordinator,
	armed: bool,
}

impl HookAbortGuard {
	fn new(coordinator: HookCoordinator) -> Self {
		Self {
			coordinator,
			armed: true,
		}
	}

	fn release(&mut self) {
		self.coordinator.release();
		self.armed = false;
	}

	fn abort(&mut self) {
		self.coordinator.abort();
		self.armed = false;
	}
}

impl Drop for HookAbortGuard {
	fn drop(&mut self) {
		if self.armed {
			self.coordinator.abort();
		}
	}
}

fn spawn_race_thread<T, F>(name: &str, operation: F) -> Result<std::thread::JoinHandle<Result<T>>>
where
	T: Send + 'static,
	F: FnOnce() -> Result<T> + Send + 'static,
{
	std::thread::Builder::new()
		.name(name.to_owned())
		.spawn(operation)
		.map_err(|error| Error::Internal(format!("failed to spawn {name}: {error}")))
}

fn collect_joined<T>(handles: Vec<std::thread::JoinHandle<Result<T>>>) -> Result<Vec<T>> {
	let mut results = Vec::with_capacity(handles.len());
	let mut panic_payload = None;
	for handle in handles {
		match handle.join() {
			Ok(result) => results.push(result),
			Err(payload) if panic_payload.is_none() => panic_payload = Some(payload),
			Err(_) => {}
		}
	}
	if let Some(payload) = panic_payload {
		std::panic::resume_unwind(payload);
	}
	results.into_iter().collect()
}

struct RaceThreads<T> {
	coordination: HookAbortGuard,
	handles: Vec<std::thread::JoinHandle<Result<T>>>,
}

impl<T: Send + 'static> RaceThreads<T> {
	fn new(coordinator: HookCoordinator, first: std::thread::JoinHandle<Result<T>>) -> Self {
		Self {
			coordination: HookAbortGuard::new(coordinator),
			handles: vec![first],
		}
	}

	fn spawn<F>(&mut self, name: &str, operation: F) -> Result<()>
	where
		F: FnOnce() -> Result<T> + Send + 'static,
	{
		let spawned = std::thread::Builder::new()
			.name(name.to_owned())
			.spawn(operation);
		self.attach_spawn(name, spawned)
	}

	fn attach_spawn(
		&mut self,
		name: &str,
		spawned: std::io::Result<std::thread::JoinHandle<Result<T>>>,
	) -> Result<()> {
		let handle =
			spawned.map_err(|error| Error::Internal(format!("failed to spawn {name}: {error}")))?;
		self.handles.push(handle);
		Ok(())
	}

	fn release(&mut self) {
		self.coordination.release();
	}

	fn abort(&mut self) {
		self.coordination.abort();
	}

	fn collect_pair(mut self) -> Result<[T; 2]> {
		let values = self.collect_all()?;
		values.try_into().map_err(|values: Vec<T>| {
			Error::Internal(format!(
				"upsert race joined {} participants instead of two",
				values.len()
			))
		})
	}

	fn collect_all(&mut self) -> Result<Vec<T>> {
		let handles = std::mem::take(&mut self.handles);
		collect_joined(handles)
	}
}

impl<T> Drop for RaceThreads<T> {
	fn drop(&mut self) {
		self.coordination.abort();
		for handle in self.handles.drain(..) {
			let _ = handle.join();
		}
	}
}

fn finish_hook_race<T: Send + 'static>(
	coordinator: &HookCoordinator,
	mut participants: RaceThreads<T>,
) -> Result<[T; 2]> {
	let readiness = coordinator.wait_until_ready();
	if readiness.is_ok() {
		participants.release();
	} else {
		participants.abort();
	}
	let joined = participants.collect_pair();
	readiness?;
	joined
}

struct PendingProbe<F> {
	future: Pin<Box<F>>,
	pending: Option<std::sync::mpsc::Sender<()>>,
}

impl<F> PendingProbe<F> {
	fn new(future: F, pending: std::sync::mpsc::Sender<()>) -> Self {
		Self {
			future: Box::pin(future),
			pending: Some(pending),
		}
	}
}

impl<F: Future> Future for PendingProbe<F> {
	type Output = F::Output;

	fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.get_mut();
		match this.future.as_mut().poll(context) {
			Poll::Pending => {
				if let Some(pending) = this.pending.take() {
					let _ = pending.send(());
				}
				Poll::Pending
			}
			Poll::Ready(output) => Poll::Ready(output),
		}
	}
}

#[test]
fn race_thread_owner_aborts_and_joins_after_second_spawn_failure() {
	let coordinator = HookCoordinator::new(1);
	let thread_coordinator = coordinator.clone();
	let exited = Arc::new(AtomicBool::new(false));
	let thread_exited = exited.clone();
	let first = spawn_race_thread("owner-spawn-failure-first", move || {
		let result = thread_coordinator.arrive_and_wait();
		thread_exited.store(true, Ordering::Release);
		result
	})
	.expect("the first characterization thread should spawn");
	let mut participants = RaceThreads::new(coordinator.clone(), first);
	coordinator
		.wait_until_ready()
		.expect("the first characterization hook should arrive");

	let spawn_error = participants
		.attach_spawn(
			"forced-second-spawn-failure",
			Err(std::io::Error::other("forced spawn failure")),
		)
		.expect_err("the synthetic second spawn must fail");
	assert!(matches!(spawn_error, Error::Internal(_)));
	drop(participants);

	assert!(exited.load(Ordering::Acquire));
}

#[test]
fn race_thread_owner_aborts_and_joins_during_unwind() {
	let coordinator = HookCoordinator::new(1);
	let thread_coordinator = coordinator.clone();
	let exited = Arc::new(AtomicBool::new(false));
	let thread_exited = exited.clone();
	let first = spawn_race_thread("owner-unwind-first", move || {
		let result = thread_coordinator.arrive_and_wait();
		thread_exited.store(true, Ordering::Release);
		result
	})
	.expect("the characterization thread should spawn");

	let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
		let _participants = RaceThreads::new(coordinator.clone(), first);
		coordinator
			.wait_until_ready()
			.expect("the characterization hook should arrive");
		panic!("force race owner unwind");
	}));

	assert!(unwind.is_err());
	assert!(exited.load(Ordering::Acquire));
}

async fn tag_by_slug(connection: &mut DatabaseConnection, slug: &str) -> Result<Option<Tag>> {
	QuerySet::<Tag>::new()
		.filter(Filter::new(
			"slug",
			FilterOperator::Eq,
			FilterValue::String(slug.to_owned()),
		))
		.first_with_db(connection)
		.await
}

async fn tag_count(connection: &mut DatabaseConnection, slug: &str) -> Result<i64> {
	Ok(QuerySet::<Tag>::new()
		.filter(Filter::new(
			"slug",
			FilterOperator::Eq,
			FilterValue::String(slug.to_owned()),
		))
		.all_with_db(connection)
		.await?
		.len() as i64)
}

async fn tenant_tag_count(
	connection: &mut DatabaseConnection,
	tenant_id: i64,
	slug: &str,
) -> Result<i64> {
	Ok(QuerySet::<TenantTag>::new()
		.filter(Filter::new(
			"tenant_id",
			FilterOperator::Eq,
			FilterValue::Int(tenant_id),
		))
		.filter(Filter::new(
			"slug",
			FilterOperator::Eq,
			FilterValue::String(slug.to_owned()),
		))
		.all_with_db(connection)
		.await?
		.len() as i64)
}

async fn verify_basic_cases(connection: &mut DatabaseConnection) -> Result<()> {
	let (first, created) = Tag::objects()
		.get_or_create()
		.lookup(Tag::field_slug(), "rust")
		.default(Tag::field_email(), "rust@example.test")
		.default(Tag::field_value(), 1)
		.default(Tag::field_create_marker(), "created")
		.default(Tag::field_optional_note(), None::<String>)
		.execute_with(connection)
		.await?;
	assert!(created);
	assert!(first.id.is_some());
	assert_eq!(first.database_note, "server-default");
	assert_eq!(first.optional_note, None);

	let (second, created) = Tag::objects()
		.get_or_create()
		.lookup(Tag::field_slug(), "rust")
		.execute_with(connection)
		.await?;
	assert!(!created);
	assert_eq!(second.id, first.id);

	let (composite, created) = TenantTag::objects()
		.get_or_create()
		.lookup(TenantTag::field_tenant_id(), 7)
		.lookup(TenantTag::field_slug(), "composite")
		.default(TenantTag::field_value(), 3)
		.execute_with(connection)
		.await?;
	assert!(created);
	let (same_composite, created) = TenantTag::objects()
		.get_or_create()
		.lookup(TenantTag::field_tenant_id(), 7)
		.lookup(TenantTag::field_slug(), "composite")
		.execute_with(connection)
		.await?;
	assert!(!created);
	assert_eq!(same_composite.id, composite.id);

	let unrelated_error = Tag::objects()
		.get_or_create()
		.lookup(Tag::field_slug(), "unrelated-unique")
		.default(Tag::field_email(), "rust@example.test")
		.default(Tag::field_value(), 2)
		.default(Tag::field_create_marker(), "conflict")
		.execute_with(connection)
		.await
		.expect_err("an unrelated email conflict must remain an error");
	assert_eq!(
		unrelated_error.database_kind(),
		Some(DatabaseErrorKind::UniqueViolation)
	);

	let rollback = connection
		.atomic_write(async |transaction| {
			Tag::objects()
				.get_or_create()
				.lookup(Tag::field_slug(), "rolled-back")
				.default(Tag::field_email(), "rolled-back@example.test")
				.default(Tag::field_value(), 5)
				.default(Tag::field_create_marker(), "rollback")
				.execute_with(transaction)
				.await?;
			Err::<(), Error>(Error::Validation("force rollback".to_owned()))
		})
		.await
		.expect_err("the callback error should roll back get_or_create");
	assert_eq!(rollback.to_string(), "Validation error: force rollback");
	assert_eq!(tag_by_slug(connection, "rolled-back").await?, None);

	connection
		.atomic(async |outer| {
			let nested_error = outer
				.atomic(async |savepoint| {
					Tag::objects()
						.get_or_create()
						.lookup(Tag::field_slug(), "savepoint-rolled-back")
						.default(Tag::field_email(), "savepoint@example.test")
						.default(Tag::field_value(), 6)
						.default(Tag::field_create_marker(), "savepoint")
						.execute_with(savepoint)
						.await?;
					Err::<(), Error>(Error::Validation("force savepoint rollback".to_owned()))
				})
				.await
				.expect_err("the nested callback should roll back to its savepoint");
			assert_eq!(
				nested_error.to_string(),
				"Validation error: force savepoint rollback"
			);
			Ok::<(), Error>(())
		})
		.await?;
	assert_eq!(
		tag_by_slug(connection, "savepoint-rolled-back").await?,
		None
	);

	let update_rollback = connection
		.atomic_write(async |transaction| {
			Tag::objects()
				.update_or_create()
				.lookup(Tag::field_slug(), "update-rolled-back")
				.set(Tag::field_value(), 71)
				.create_default(Tag::field_email(), "update-rolled-back@example.test")
				.create_default(Tag::field_create_marker(), "update-rollback")
				.execute_with(transaction)
				.await?;
			Err::<(), Error>(Error::Validation("force update rollback".to_owned()))
		})
		.await
		.expect_err("the callback error should roll back update_or_create");
	assert_eq!(
		update_rollback.to_string(),
		"Validation error: force update rollback"
	);
	assert_eq!(tag_by_slug(connection, "update-rolled-back").await?, None);

	connection
		.atomic_write(async |outer| {
			let nested_error = outer
				.atomic(async |savepoint| {
					Tag::objects()
						.update_or_create()
						.lookup(Tag::field_slug(), "update-savepoint-rolled-back")
						.set(Tag::field_value(), 72)
						.create_default(
							Tag::field_email(),
							"update-savepoint-rolled-back@example.test",
						)
						.create_default(Tag::field_create_marker(), "update-savepoint")
						.execute_with(savepoint)
						.await?;
					Err::<(), Error>(Error::Validation(
						"force update savepoint rollback".to_owned(),
					))
				})
				.await
				.expect_err("the nested update should roll back to its savepoint");
			assert_eq!(
				nested_error.to_string(),
				"Validation error: force update savepoint rollback"
			);
			Ok::<(), Error>(())
		})
		.await?;
	assert_eq!(
		tag_by_slug(connection, "update-savepoint-rolled-back").await?,
		None
	);
	Ok(())
}

async fn verify_get_races(connection: &mut DatabaseConnection) -> Result<()> {
	let coordinator = HookCoordinator::new(2);
	let first_manager = TagRaceManager {
		coordinator: coordinator.clone(),
	};
	let second_manager = TagRaceManager {
		coordinator: coordinator.clone(),
	};
	let mut first_connection = connection.clone();
	let mut second_connection = connection.clone();
	let runtime = tokio::runtime::Handle::current();
	let first_runtime = runtime.clone();
	let first = spawn_race_thread("single-get-race-first", move || {
		first_runtime.block_on(async move {
			first_manager
				.get_or_create()
				.lookup(Tag::field_slug(), "single-race")
				.default(Tag::field_email(), "single-race@example.test")
				.default(Tag::field_value(), 10)
				.default(Tag::field_create_marker(), "first")
				.execute_with(&mut first_connection)
				.await
		})
	})?;
	let mut participants = RaceThreads::new(coordinator.clone(), first);
	participants.spawn("single-get-race-second", move || {
		runtime.block_on(async move {
			second_manager
				.get_or_create()
				.lookup(Tag::field_slug(), "single-race")
				.default(Tag::field_email(), "single-race@example.test")
				.default(Tag::field_value(), 10)
				.default(Tag::field_create_marker(), "second")
				.execute_with(&mut second_connection)
				.await
		})
	})?;
	let results = finish_hook_race(&coordinator, participants)?;
	assert_eq!(results.iter().filter(|(_, created)| *created).count(), 1);
	assert_eq!(results.iter().filter(|(_, created)| !*created).count(), 1);
	assert_eq!(tag_count(connection, "single-race").await?, 1);

	let coordinator = HookCoordinator::new(2);
	let first_manager = TenantTagRaceManager {
		coordinator: coordinator.clone(),
	};
	let second_manager = TenantTagRaceManager {
		coordinator: coordinator.clone(),
	};
	let mut first_connection = connection.clone();
	let mut second_connection = connection.clone();
	let runtime = tokio::runtime::Handle::current();
	let first_runtime = runtime.clone();
	let first = spawn_race_thread("composite-get-race-first", move || {
		first_runtime.block_on(async move {
			first_manager
				.get_or_create()
				.lookup(TenantTag::field_tenant_id(), 11)
				.lookup(TenantTag::field_slug(), "composite-race")
				.default(TenantTag::field_value(), 1)
				.execute_with(&mut first_connection)
				.await
		})
	})?;
	let mut participants = RaceThreads::new(coordinator.clone(), first);
	participants.spawn("composite-get-race-second", move || {
		runtime.block_on(async move {
			second_manager
				.get_or_create()
				.lookup(TenantTag::field_tenant_id(), 11)
				.lookup(TenantTag::field_slug(), "composite-race")
				.default(TenantTag::field_value(), 2)
				.execute_with(&mut second_connection)
				.await
		})
	})?;
	let results = finish_hook_race(&coordinator, participants)?;
	assert_eq!(results.iter().filter(|(_, created)| *created).count(), 1);
	assert_eq!(results.iter().filter(|(_, created)| !*created).count(), 1);
	assert_eq!(tenant_tag_count(connection, 11, "composite-race").await?, 1);
	Ok(())
}

async fn update_invocation<C>(
	connection: DatabaseConnection,
	manager: C,
	slug: &'static str,
	email: &'static str,
	value: i32,
	create_marker: &'static str,
) -> Result<(Tag, bool)>
where
	C: CustomManager<Model = Tag>,
{
	connection
		.atomic_write(async |transaction| {
			manager
				.update_or_create()
				.lookup(Tag::field_slug(), slug)
				.set(Tag::field_value(), value)
				.create_default(Tag::field_email(), email)
				.create_default(Tag::field_create_marker(), create_marker)
				.execute_with(transaction)
				.await
		})
		.await
}

async fn verify_update_race(connection: &mut DatabaseConnection) -> Result<()> {
	let coordinator = HookCoordinator::new(2);
	let first_manager = TagRaceManager {
		coordinator: coordinator.clone(),
	};
	let second_manager = TagRaceManager {
		coordinator: coordinator.clone(),
	};
	let runtime = tokio::runtime::Handle::current();
	let first_runtime = runtime.clone();
	let first_connection = connection.clone();
	let first = spawn_race_thread("update-race-first", move || {
		first_runtime.block_on(update_invocation(
			first_connection,
			first_manager,
			"update-race",
			"update-race@example.test",
			101,
			"first-create-default",
		))
	})?;
	let mut participants = RaceThreads::new(coordinator.clone(), first);
	let second_connection = connection.clone();
	participants.spawn("update-race-second", move || {
		runtime.block_on(update_invocation(
			second_connection,
			second_manager,
			"update-race",
			"update-race@example.test",
			202,
			"second-create-default",
		))
	})?;
	let results = finish_hook_race(&coordinator, participants)?;
	assert_eq!(results.iter().filter(|(_, created)| *created).count(), 1);
	assert_eq!(results.iter().filter(|(_, created)| !*created).count(), 1);
	assert_eq!(tag_count(connection, "update-race").await?, 1);
	let mut returned_values = results.iter().map(|(tag, _)| tag.value).collect::<Vec<_>>();
	returned_values.sort_unstable();
	assert_eq!(returned_values, [101, 202]);
	let persisted = tag_by_slug(connection, "update-race")
		.await?
		.expect("the raced row should persist");
	let loser = results
		.iter()
		.find(|(_, created)| !*created)
		.expect("one invocation should take the update branch");
	assert_eq!(persisted.value, loser.0.value);
	assert!(matches!(persisted.value, 101 | 202));
	let winner_marker = results
		.iter()
		.find(|(_, created)| *created)
		.map(|(tag, _)| tag.create_marker.as_str())
		.expect("one invocation should create");
	assert_eq!(persisted.create_marker, winner_marker);
	Ok(())
}

async fn verify_sqlite_update_serialization(connection: &mut DatabaseConnection) -> Result<()> {
	let coordinator = HookCoordinator::new(1);
	let first_manager = TagRaceManager {
		coordinator: coordinator.clone(),
	};
	let runtime = tokio::runtime::Handle::current();
	let first_runtime = runtime.clone();
	let first_connection = connection.clone();
	let first = spawn_race_thread("sqlite-update-holder", move || {
		first_runtime.block_on(update_invocation(
			first_connection,
			first_manager,
			"sqlite-update",
			"sqlite-update@example.test",
			301,
			"first",
		))
	})?;
	let mut participants = RaceThreads::new(coordinator.clone(), first);
	let readiness = coordinator.wait_until_ready();
	if readiness.is_err() {
		participants.abort();
		let joined = participants.collect_all();
		joined?;
		return readiness;
	}

	let (pending_tx, pending_rx) = std::sync::mpsc::channel();
	let second_connection = connection.clone();
	participants.spawn("sqlite-update-contender", move || {
		runtime.block_on(PendingProbe::new(
			update_invocation(
				second_connection,
				Manager::<Tag>::new(),
				"sqlite-update",
				"sqlite-update@example.test",
				302,
				"second-create-default-must-not-apply",
			),
			pending_tx,
		))
	})?;
	// The pool has three pre-opened idle connections while A holds one. This
	// notification therefore observes B's polled atomic-write future waiting
	// on BEGIN IMMEDIATE rather than waiting for pool capacity.
	let pending = pending_rx
		.recv_timeout(Duration::from_secs(30))
		.map_err(|error| Error::Internal(format!("SQLite contender was never pending: {error}")));
	if pending.is_ok() {
		participants.release();
	} else {
		participants.abort();
	}
	let results = participants.collect_pair();
	pending?;
	let results = results?;
	assert!(results[0].1);
	assert!(!results[1].1);
	assert_eq!(tag_count(connection, "sqlite-update").await?, 1);
	let persisted = tag_by_slug(connection, "sqlite-update")
		.await?
		.expect("the serialized SQLite row should persist");
	assert_eq!(persisted.value, 302);
	assert_eq!(persisted.create_marker, "first");
	Ok(())
}

struct OneshotReleaseGuard(Option<tokio::sync::oneshot::Sender<()>>);

impl OneshotReleaseGuard {
	fn new(sender: tokio::sync::oneshot::Sender<()>) -> Self {
		Self(Some(sender))
	}

	fn release(&mut self) {
		if let Some(sender) = self.0.take() {
			let _ = sender.send(());
		}
	}
}

impl Drop for OneshotReleaseGuard {
	fn drop(&mut self) {
		self.release();
	}
}

fn validate_sqlite_busy(error: &Error) -> Result<()> {
	let database_error = error.database_error().ok_or_else(|| {
		Error::Internal(format!(
			"SQLite lock contender returned a non-database error: {error}"
		))
	})?;
	if !matches!(
		database_error.kind(),
		DatabaseErrorKind::Query | DatabaseErrorKind::Serialization
	) {
		return Err(Error::Internal(format!(
			"SQLite lock contender returned {:?}, not a busy classification",
			database_error.kind()
		)));
	}
	let code = database_error.code().ok_or_else(|| {
		Error::Internal(format!(
			"SQLite lock contender omitted its busy/locked result code: {database_error}"
		))
	})?;
	let numeric_code = code.parse::<i32>().map_err(|parse_error| {
		Error::Internal(format!(
			"SQLite lock contender returned non-numeric result code '{code}': {parse_error}"
		))
	})?;
	let primary_code = numeric_code & 0xff;
	if !matches!(primary_code, 5 | 6) {
		return Err(Error::Internal(format!(
			"SQLite lock contender returned unrelated result code {code}"
		)));
	}
	let message = database_error.message().to_ascii_lowercase();
	if !message.contains("busy") && !message.contains("locked") {
		return Err(Error::Internal(format!(
			"SQLite result code {code} lacked a busy/locked diagnostic: {database_error}"
		)));
	}
	Ok(())
}

#[test]
fn sqlite_busy_validation_rejects_broad_database_errors() {
	let timeout = Error::from(DatabaseError::new(
		DatabaseErrorKind::Timeout,
		"Pool timed out",
	));
	let generic_query = Error::from(DatabaseError::new(
		DatabaseErrorKind::Query,
		"unrelated SQL error",
	));
	let extended_busy = Error::from(
		DatabaseError::new(DatabaseErrorKind::Query, "database is locked").with_code("261"),
	);

	assert!(validate_sqlite_busy(&timeout).is_err());
	assert!(validate_sqlite_busy(&generic_query).is_err());
	assert!(validate_sqlite_busy(&extended_busy).is_ok());
}

async fn verify_sqlite_busy_retry(mut fixture: BackendFixture) -> Result<()> {
	fixture.create_schema().await?;
	let (locked_tx, locked_rx) = tokio::sync::oneshot::channel();
	let (release_tx, release_rx) = tokio::sync::oneshot::channel();
	let holder_connection = fixture.connection.clone();
	let holder = async move {
		holder_connection
			.atomic_write(async |transaction| {
				let result = Tag::objects()
					.update_or_create()
					.lookup(Tag::field_slug(), "sqlite-busy")
					.set(Tag::field_value(), 401)
					.create_default(Tag::field_email(), "sqlite-busy@example.test")
					.create_default(Tag::field_create_marker(), "holder")
					.execute_with(transaction)
					.await?;
				locked_tx.send(()).map_err(|_| {
					Error::Internal("SQLite holder signal receiver dropped".to_owned())
				})?;
				release_rx.await.map_err(|error| {
					Error::Internal(format!("SQLite holder was not released: {error}"))
				})?;
				Ok::<_, Error>(result)
			})
			.await
	};
	let contender_connection = fixture.connection.clone();
	let contender = async move {
		locked_rx.await.map_err(|error| {
			Error::Internal(format!(
				"SQLite holder did not acquire write intent: {error}"
			))
		})?;
		let mut release_guard = OneshotReleaseGuard::new(release_tx);
		let result = update_invocation(
			contender_connection,
			Manager::<Tag>::new(),
			"sqlite-busy",
			"sqlite-busy@example.test",
			402,
			"contender-create-default-must-not-apply",
		)
		.await;
		release_guard.release();
		Ok::<_, Error>(result)
	};
	let (held, contender_result) = tokio::join!(holder, contender);
	let held = held?;
	let contender_result = contender_result?;
	let contender_error = match contender_result {
		Ok(_) => {
			return Err(Error::Internal(
				"zero busy timeout unexpectedly let the locked contender succeed".to_owned(),
			));
		}
		Err(error) => error,
	};
	validate_sqlite_busy(&contender_error)?;
	assert!(held.1);

	let (retried, created) = update_invocation(
		fixture.connection.clone(),
		Manager::<Tag>::new(),
		"sqlite-busy",
		"sqlite-busy@example.test",
		402,
		"retry-create-default-must-not-apply",
	)
	.await?;
	assert!(!created);
	assert_eq!(retried.value, 402);
	assert_eq!(tag_count(&mut fixture.connection, "sqlite-busy").await?, 1);
	let persisted = tag_by_slug(&mut fixture.connection, "sqlite-busy")
		.await?
		.expect("the retried SQLite row should persist");
	assert_eq!(persisted.value, 402);
	assert_eq!(persisted.create_marker, "holder");
	Ok(())
}

async fn verify_backend(mut fixture: BackendFixture) -> Result<()> {
	assert_eq!(
		fixture.connection.backend(),
		fixture.backend.database_backend()
	);
	fixture.create_schema().await?;
	verify_basic_cases(&mut fixture.connection).await?;
	verify_get_races(&mut fixture.connection).await?;
	match fixture.backend {
		LiveBackend::Postgres | LiveBackend::MySql => {
			verify_update_race(&mut fixture.connection).await?
		}
		LiveBackend::Sqlite => verify_sqlite_update_serialization(&mut fixture.connection).await?,
	}
	Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial(upsert_backend_concurrency)]
async fn postgres_upsert_builders_cover_basic_cases_and_races() {
	verify_backend(BackendFixture::postgres().await)
		.await
		.expect("PostgreSQL typed upsert behavior should be atomic");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial(upsert_backend_concurrency)]
async fn mysql_upsert_builders_cover_basic_cases_and_races() {
	verify_backend(BackendFixture::mysql().await)
		.await
		.expect("MySQL typed upsert behavior should be atomic");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial(upsert_backend_concurrency)]
async fn sqlite_upsert_builders_cover_basic_cases_and_races() {
	verify_backend(BackendFixture::sqlite().await)
		.await
		.expect("SQLite typed upsert behavior should serialize normal writers");
	verify_sqlite_busy_retry(BackendFixture::sqlite_busy().await)
		.await
		.expect("SQLite lock failures should be classified and explicitly retried");
}
