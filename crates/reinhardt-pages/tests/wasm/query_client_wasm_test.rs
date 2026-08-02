//! Browser lifecycle coverage for query polling and visibility.

#![cfg(wasm)]

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use gloo_timers::future::TimeoutFuture;
use reinhardt_pages::reactive::query::{
	QueryClient, QueryDefaults, QueryFamily, QueryOptions, query_browser_resource_counts,
	query_browser_resource_probe_for_test, set_query_visibility_for_test,
};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

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
