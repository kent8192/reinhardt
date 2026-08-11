use reinhardt_pages::reactive::{QueryFamily, QueryOptions, RetryPolicy, use_query};

#[cfg(all(native, feature = "testing"))]
use std::cell::Cell;
#[cfg(all(native, feature = "testing"))]
use std::rc::Rc;
#[cfg(all(native, feature = "testing"))]
use std::time::Duration;

#[cfg(all(native, feature = "testing"))]
use reinhardt_core::types::page::{IntoPage, Page, PageElement};
#[cfg(all(native, feature = "testing"))]
use reinhardt_pages::reactive::hooks::use_action;
#[cfg(all(native, feature = "testing"))]
use reinhardt_pages::reactive::{QuerySnapshot, QueryStatus, queries};
#[cfg(all(native, feature = "testing"))]
use reinhardt_pages::testing::component::{Role, render};

#[cfg(all(native, feature = "testing"))]
const JOBS: QueryFamily<u64, Vec<String>, String> = QueryFamily::new("acceptance.jobs");

#[cfg(all(native, feature = "testing"))]
async fn list_jobs(project_id: u64, calls: Rc<Cell<usize>>) -> Result<Vec<String>, String> {
	let call = calls.get() + 1;
	calls.set(call);
	Ok(vec![format!("project {project_id} job {call}")])
}

#[cfg(all(native, feature = "testing"))]
async fn retry_job(project_id: u64, calls: Rc<Cell<usize>>) -> Result<u64, String> {
	calls.set(calls.get() + 1);
	Ok(project_id)
}

#[cfg(all(native, feature = "testing"))]
fn jobs_snapshot_page(label: &'static str, snapshot: QuerySnapshot<Vec<String>, String>) -> Page {
	match snapshot.status {
		QueryStatus::Idle => PageElement::new("p")
			.child(format!("{label}: idle"))
			.into_page(),
		QueryStatus::Pending => PageElement::new("p")
			.child(format!("{label}: loading"))
			.into_page(),
		QueryStatus::Success => PageElement::new("p")
			.child(format!(
				"{label}: {}",
				snapshot
					.data
					.expect("successful jobs snapshots contain data")
					.join(", ")
			))
			.into_page(),
		QueryStatus::Error => PageElement::new("p")
			.child(format!(
				"{label}: {}",
				snapshot
					.error
					.expect("failed jobs snapshots contain an error")
			))
			.into_page(),
	}
}

#[cfg(all(native, feature = "testing"))]
fn jobs_component(project_id: u64, label: &'static str, list_calls: Rc<Cell<usize>>) -> Page {
	let jobs = use_query(
		JOBS.query(project_id, move || {
			list_jobs(project_id, Rc::clone(&list_calls))
		}),
		QueryOptions::new().refetch_interval(Duration::from_secs(5)),
	);

	Page::reactive(move || jobs_snapshot_page(label, jobs.snapshot()))
}

#[cfg(all(native, feature = "testing"))]
fn jobs_screen(project_id: u64, list_calls: Rc<Cell<usize>>, retry_calls: Rc<Cell<usize>>) -> Page {
	let client = queries();
	let retry = use_action(move |request| {
		let client = client.clone();
		let retry_calls = Rc::clone(&retry_calls);
		async move {
			let result = retry_job(request, retry_calls).await?;
			client.invalidate_family(JOBS);
			Ok::<_, String>(result)
		}
	});

	PageElement::new("main")
		.child(
			PageElement::new("button")
				.listener("click", move |_| retry.dispatch(project_id))
				.child("Retry job"),
		)
		.child(jobs_component(project_id, "Queue", Rc::clone(&list_calls)))
		.child(jobs_component(project_id, "Sidebar", list_calls))
		.into_page()
}

#[cfg(all(native, feature = "testing"))]
fn retrying_query_component(calls: Rc<Cell<usize>>) -> Page {
	let query = use_query(
		QueryFamily::<(), String, String>::new("tests.component-retry-settle").query(
			(),
			move || {
				let attempt = calls.get() + 1;
				calls.set(attempt);
				async move {
					if attempt == 1 {
						Err("temporary".to_string())
					} else {
						Ok("recovered".to_string())
					}
				}
			},
		),
		QueryOptions::new().retry(RetryPolicy::exponential().max_attempts(2)),
	);

	Page::reactive(move || match query.snapshot().status {
		reinhardt_pages::reactive::QueryStatus::Success => PageElement::new("p")
			.child(query.data().expect("successful query data"))
			.into_page(),
		_ => PageElement::new("p").child("query-loading").into_page(),
	})
}

#[test]
#[should_panic(expected = "use_query requires an active QueryClient")]
fn use_query_rejects_missing_application_context() {
	let family = QueryFamily::<(), String, String>::new("tests.no-client");
	let _query = use_query(
		family.query((), || async { Ok("value".to_string()) }),
		QueryOptions::default(),
	);
}

#[cfg(all(native, feature = "testing"))]
#[tokio::test]
async fn jobs_screen_deduplicates_shared_reads_and_refetches_after_retry() {
	// Arrange
	let list_calls = Rc::new(Cell::new(0));
	let retry_calls = Rc::new(Cell::new(0));
	let screen = render({
		let list_calls = Rc::clone(&list_calls);
		let retry_calls = Rc::clone(&retry_calls);
		move || jobs_screen(42, list_calls, retry_calls)
	});

	// Act
	screen.settle().await;

	// Assert
	assert_eq!(list_calls.get(), 1);
	assert_eq!(
		screen
			.try_get_by_text("Sidebar: project 42 job 1")
			.expect("the sidebar renders the shared jobs result")
			.text(),
		"Sidebar: project 42 job 1"
	);
	assert_eq!(
		screen
			.try_get_by_text("Queue: project 42 job 1")
			.expect("the queue renders the shared jobs result")
			.text(),
		"Queue: project 42 job 1"
	);

	// Act
	screen.get_by_role(Role::Button, "Retry job").click();
	screen.settle().await;

	// Assert
	assert_eq!(retry_calls.get(), 1);
	assert_eq!(list_calls.get(), 2);
	assert_eq!(
		screen
			.try_get_by_text("Sidebar: project 42 job 2")
			.expect("the sidebar renders the refetched jobs result")
			.text(),
		"Sidebar: project 42 job 2"
	);
	assert_eq!(
		screen
			.try_get_by_text("Queue: project 42 job 2")
			.expect("the queue renders the refetched jobs result")
			.text(),
		"Queue: project 42 job 2"
	);
}

#[cfg(all(native, feature = "testing"))]
#[tokio::test]
async fn component_screen_settle_waits_for_query_retry_backoff() {
	// Arrange
	let calls = Rc::new(Cell::new(0));
	let screen = render({
		let calls = Rc::clone(&calls);
		move || retrying_query_component(calls)
	});

	// Act
	screen.settle().await;

	// Assert
	assert_eq!(calls.get(), 2);
	assert_eq!(screen.pretty(), "<p>\n  recovered\n</p>\n");
}
