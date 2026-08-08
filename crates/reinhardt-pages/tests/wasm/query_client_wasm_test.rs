//! Browser lifecycle coverage for query polling and visibility.

#![cfg(wasm)]

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use gloo_timers::future::TimeoutFuture;

use reinhardt_pages::prelude::{
	Entity, EntityHandle, EntityProjection, EntityValue, EntityVec, OptionalEntity,
	ProjectionMaterialization, ProjectionRemoval, RemovedEntities,
};
use reinhardt_pages::reactive::query::{
	QueryClient, QueryDefaults, QueryFamily, QueryOptions, query_browser_resource_counts,
	query_browser_resource_probe_for_test, set_query_visibility_for_test,
};
use reinhardt_pages::reactive::{
	Effect, EntityDependencies, EntityReader, EntityWriter,
	ProjectionMaterialization as ReactiveProjectionMaterialization,
	ProjectionRemoval as ReactiveProjectionRemoval, ReactiveScope,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct BrowserProject {
	id: u64,
	name: String,
}

impl Entity for BrowserProject {
	type Id = u64;

	const TYPE: &'static str = "wasm.query-client-project";

	fn entity_id(&self) -> Self::Id {
		self.id
	}
}

fn browser_project(id: u64, name: &str) -> BrowserProject {
	BrowserProject {
		id,
		name: name.to_string(),
	}
}

fn assert_public_entity_api() {
	fn assert_projection<T, P: EntityProjection<T>>() {}

	assert_projection::<BrowserProject, EntityValue<BrowserProject>>();
	assert_projection::<Option<BrowserProject>, OptionalEntity<BrowserProject>>();
	assert_projection::<Vec<BrowserProject>, EntityVec<BrowserProject>>();
	let _dependencies = EntityDependencies::default();
	let _removed = RemovedEntities::from_ids::<BrowserProject>([1]);
	let _materialization = ProjectionMaterialization::<BrowserProject>::MissingRequired;
	let _removal = ProjectionRemoval::Unchanged;
	let _reactive_materialization =
		ReactiveProjectionMaterialization::<BrowserProject>::MissingRequired;
	let _reactive_removal = ReactiveProjectionRemoval::Unchanged;
	let _reader: Option<fn(&EntityReader<'_>)> = None;
	let _writer: Option<fn(&mut EntityWriter<'_>)> = None;
}

struct TestGate {
	ready: Rc<Cell<bool>>,
	waker: Rc<RefCell<Option<Waker>>>,
	value: u32,
}

impl Future for TestGate {
	type Output = Result<u32, String>;

	fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
		if self.ready.get() {
			Poll::Ready(Ok(self.value))
		} else {
			self.waker.borrow_mut().replace(context.waker().clone());
			Poll::Pending
		}
	}
}

fn open_gate(ready: &Cell<bool>, waker: &RefCell<Option<Waker>>) {
	ready.set(true);
	if let Some(waker) = waker.borrow_mut().take() {
		waker.wake();
	}
}

async fn settle_after(duration: Duration) {
	TimeoutFuture::new(duration.as_millis() as u32).await;
	TimeoutFuture::new(0).await;
	TimeoutFuture::new(0).await;
}

fn dispatch_visibility_change() {
	let document = web_sys::window()
		.expect("browser window")
		.document()
		.expect("browser document");
	let event = web_sys::Event::new("visibilitychange").expect("visibility event");
	document
		.dispatch_event(&event)
		.expect("dispatch visibility event");
}

#[wasm_bindgen_test(async)]
async fn polling_suspends_while_hidden_and_stale_resume_refetches_immediately() {
	let client = QueryClient::new(QueryDefaults::default());
	let family = QueryFamily::<(), u32, String>::new("wasm.visibility-stale");
	let fetch_count = Rc::new(Cell::new(0));
	let query = client.observe_for_test(
		family.query((), {
			let fetch_count = Rc::clone(&fetch_count);
			move || {
				let call = fetch_count.get() + 1;
				fetch_count.set(call);
				async move { Ok(call) }
			}
		}),
		QueryOptions::new()
			.stale_time(Duration::ZERO)
			.refetch_interval(Duration::from_millis(40)),
	);
	settle_after(Duration::from_millis(5)).await;
	assert_eq!(fetch_count.get(), 1);

	settle_after(Duration::from_millis(50)).await;
	assert_eq!(fetch_count.get(), 2);

	set_query_visibility_for_test(&client, false);
	dispatch_visibility_change();
	settle_after(Duration::from_millis(60)).await;
	assert_eq!(fetch_count.get(), 2);

	set_query_visibility_for_test(&client, true);
	dispatch_visibility_change();
	settle_after(Duration::from_millis(5)).await;
	assert_eq!(fetch_count.get(), 3);
	assert_eq!(query_browser_resource_counts(&client), (1, 1));

	let resources = query_browser_resource_probe_for_test(&client);
	drop(query);
	drop(client);
	assert_eq!(resources.counts(), (0, 0));
}

#[wasm_bindgen_test(async)]
async fn fresh_visibility_resume_restarts_the_full_polling_interval() {
	let client = QueryClient::new(QueryDefaults::default());
	let family = QueryFamily::<(), u32, String>::new("wasm.visibility-fresh");
	let fetch_count = Rc::new(Cell::new(0));
	let _query = client.observe_for_test(
		family.query((), {
			let fetch_count = Rc::clone(&fetch_count);
			move || {
				let call = fetch_count.get() + 1;
				fetch_count.set(call);
				async move { Ok(call) }
			}
		}),
		QueryOptions::new()
			.stale_time(Duration::from_secs(10))
			.refetch_interval(Duration::from_millis(60)),
	);
	settle_after(Duration::from_millis(5)).await;
	assert_eq!(fetch_count.get(), 1);

	set_query_visibility_for_test(&client, false);
	dispatch_visibility_change();
	settle_after(Duration::from_millis(70)).await;
	assert_eq!(fetch_count.get(), 1);

	set_query_visibility_for_test(&client, true);
	dispatch_visibility_change();
	settle_after(Duration::from_millis(30)).await;
	assert_eq!(fetch_count.get(), 1);

	settle_after(Duration::from_millis(40)).await;
	assert_eq!(fetch_count.get(), 2);
}

#[wasm_bindgen_test(async)]
async fn request_completion_while_hidden_does_not_restart_polling() {
	let client = QueryClient::new(QueryDefaults::default());
	let family = QueryFamily::<(), u32, String>::new("wasm.hidden-completion");
	let ready = Rc::new(Cell::new(false));
	let waker = Rc::new(RefCell::new(None));
	let fetch_count = Rc::new(Cell::new(0));
	let query = client.observe_for_test(
		family.query((), {
			let ready = Rc::clone(&ready);
			let waker = Rc::clone(&waker);
			let fetch_count = Rc::clone(&fetch_count);
			move || {
				let call = fetch_count.get() + 1;
				fetch_count.set(call);
				TestGate {
					ready: Rc::clone(&ready),
					waker: Rc::clone(&waker),
					value: call,
				}
			}
		}),
		QueryOptions::new()
			.stale_time(Duration::from_secs(10))
			.refetch_interval(Duration::from_millis(50)),
	);
	settle_after(Duration::from_millis(5)).await;
	assert_eq!(fetch_count.get(), 1);
	assert!(query.is_fetching());

	set_query_visibility_for_test(&client, false);
	dispatch_visibility_change();
	open_gate(&ready, &waker);
	settle_after(Duration::from_millis(5)).await;

	assert_eq!(query.data(), Some(1));
	assert_eq!(query_browser_resource_counts(&client), (1, 0));
	settle_after(Duration::from_millis(60)).await;
	assert_eq!(fetch_count.get(), 1);

	set_query_visibility_for_test(&client, true);
	dispatch_visibility_change();
	settle_after(Duration::from_millis(20)).await;
	assert_eq!(fetch_count.get(), 1);
	assert_eq!(query_browser_resource_counts(&client), (1, 1));

	settle_after(Duration::from_millis(40)).await;
	assert_eq!(fetch_count.get(), 2);
}

#[wasm_bindgen_test(async)]
async fn observer_mounted_while_hidden_waits_for_visibility_before_polling() {
	let client = QueryClient::new(QueryDefaults::default());
	set_query_visibility_for_test(&client, false);
	let family = QueryFamily::<(), u32, String>::new("wasm.initially-hidden");
	let fetch_count = Rc::new(Cell::new(0));
	let _query = client.observe_for_test(
		family.query((), {
			let fetch_count = Rc::clone(&fetch_count);
			move || {
				let call = fetch_count.get() + 1;
				fetch_count.set(call);
				async move { Ok(call) }
			}
		}),
		QueryOptions::new()
			.stale_time(Duration::ZERO)
			.refetch_interval(Duration::from_millis(40)),
	);
	settle_after(Duration::from_millis(5)).await;
	assert_eq!(fetch_count.get(), 1);
	assert_eq!(query_browser_resource_counts(&client), (1, 0));

	settle_after(Duration::from_millis(50)).await;
	assert_eq!(fetch_count.get(), 1);

	set_query_visibility_for_test(&client, true);
	dispatch_visibility_change();
	settle_after(Duration::from_millis(5)).await;
	assert_eq!(fetch_count.get(), 2);
	assert_eq!(query_browser_resource_counts(&client), (1, 1));
}

#[wasm_bindgen_test(async)]
async fn normalized_entities_propagate_reactively_and_release_browser_resources() {
	assert_public_entity_api();

	let client = QueryClient::new(QueryDefaults::default());
	let direct_calls = Rc::new(Cell::new(0));
	let optional_calls = Rc::new(Cell::new(0));
	let vector_calls = Rc::new(Cell::new(0));
	let direct = client.observe_for_test(
		QueryFamily::<(), BrowserProject, String>::new("wasm.normalized-direct")
			.query((), {
				let direct_calls = Rc::clone(&direct_calls);
				move || {
					direct_calls.set(direct_calls.get() + 1);
					async { Ok(browser_project(1, "initial")) }
				}
			})
			.with_entities(EntityValue::new()),
		QueryOptions::new(),
	);
	let optional = client.observe_for_test(
		QueryFamily::<(), Option<BrowserProject>, String>::new("wasm.normalized-optional")
			.query((), {
				let optional_calls = Rc::clone(&optional_calls);
				move || {
					optional_calls.set(optional_calls.get() + 1);
					async { Ok(Some(browser_project(2, "optional"))) }
				}
			})
			.with_entities(OptionalEntity::new()),
		QueryOptions::new(),
	);
	let vector = client.observe_for_test(
		QueryFamily::<(), Vec<BrowserProject>, String>::new("wasm.normalized-vector")
			.query((), {
				let vector_calls = Rc::clone(&vector_calls);
				move || {
					vector_calls.set(vector_calls.get() + 1);
					async {
						Ok(vec![
							browser_project(1, "initial"),
							browser_project(2, "optional"),
						])
					}
				}
			})
			.with_entities(EntityVec::new()),
		QueryOptions::new(),
	);

	settle_after(Duration::from_millis(5)).await;
	assert_eq!(direct_calls.get(), 1);
	assert_eq!(optional_calls.get(), 1);
	assert_eq!(vector_calls.get(), 1);
	assert_eq!(direct.data(), Some(browser_project(1, "initial")));
	assert_eq!(optional.data(), Some(Some(browser_project(2, "optional"))));
	assert_eq!(
		vector.data(),
		Some(vec![
			browser_project(1, "initial"),
			browser_project(2, "optional"),
		])
	);

	let scope = ReactiveScope::new();
	let observed = Rc::new(RefCell::new(Vec::<(
		Option<BrowserProject>,
		Option<BrowserProject>,
		Option<Option<BrowserProject>>,
		Option<Vec<BrowserProject>>,
	)>::new()));
	let entity_handle: EntityHandle<BrowserProject> = scope.enter(|| {
		let entity_handle = client.entity::<BrowserProject>(1);
		let observed = Rc::clone(&observed);
		let handle_for_effect = entity_handle.clone();
		let direct_for_effect = direct.clone();
		let optional_for_effect = optional.clone();
		let vector_for_effect = vector.clone();
		let _effect = Effect::new(move || {
			let snapshot = (
				handle_for_effect.get(),
				direct_for_effect.data(),
				optional_for_effect.data(),
				vector_for_effect.data(),
			);
			observed.borrow_mut().push(snapshot);
		});
		entity_handle
	});
	settle_after(Duration::ZERO).await;
	assert_eq!(
		observed.borrow().last().cloned(),
		Some((
			Some(browser_project(1, "initial")),
			Some(browser_project(1, "initial")),
			Some(Some(browser_project(2, "optional"))),
			Some(vec![
				browser_project(1, "initial"),
				browser_project(2, "optional"),
			]),
		))
	);

	// A complete replacement updates the handle and every dependent query without refetching.
	client.upsert_entity(browser_project(1, "replacement"));
	settle_after(Duration::ZERO).await;
	assert_eq!(entity_handle.get(), Some(browser_project(1, "replacement")));
	assert_eq!(direct.data(), Some(browser_project(1, "replacement")));
	assert_eq!(
		vector.data(),
		Some(vec![
			browser_project(1, "replacement"),
			browser_project(2, "optional"),
		])
	);
	assert_eq!(direct_calls.get(), 1);
	assert_eq!(optional_calls.get(), 1);
	assert_eq!(vector_calls.get(), 1);

	// Both entities publish as one batch; observers never see a mixed old/new combination.
	let before_batch = observed.borrow().len();
	client.update_entities(|entities| {
		entities.upsert(browser_project(1, "batch-one"));
		entities.upsert(browser_project(2, "batch-two"));
	});
	settle_after(Duration::ZERO).await;
	let after_batch = observed.borrow();
	assert!(after_batch.len() > before_batch);
	for observation in &after_batch[before_batch..] {
		assert_eq!(
			observation,
			&(
				Some(browser_project(1, "batch-one")),
				Some(browser_project(1, "batch-one")),
				Some(Some(browser_project(2, "batch-two"))),
				Some(vec![
					browser_project(1, "batch-one"),
					browser_project(2, "batch-two"),
				]),
			)
		);
	}
	drop(after_batch);
	assert_eq!(optional.data(), Some(Some(browser_project(2, "batch-two"))));
	assert_eq!(
		vector.data(),
		Some(vec![
			browser_project(1, "batch-one"),
			browser_project(2, "batch-two"),
		])
	);
	assert_eq!(direct_calls.get(), 1);
	assert_eq!(optional_calls.get(), 1);
	assert_eq!(vector_calls.get(), 1);

	// Removing an optional/vector member changes recipes permanently until a new fetch restores them.
	client.remove_entity::<BrowserProject>(&2);
	settle_after(Duration::ZERO).await;
	assert_eq!(optional.data(), Some(None));
	assert_eq!(vector.data(), Some(vec![browser_project(1, "batch-one")]));
	assert_eq!(optional_calls.get(), 1);
	assert_eq!(vector_calls.get(), 1);

	let resources = query_browser_resource_probe_for_test(&client);
	scope.dispose();
	drop(entity_handle);
	drop(direct);
	drop(optional);
	drop(vector);
	drop(client);
	assert_eq!(resources.counts(), (0, 0));
}
