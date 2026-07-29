#![cfg(not(target_arch = "wasm32"))]

use reinhardt_pages::component::{Component, IntoPage, Page, PageElement};
use reinhardt_pages::deps;
use reinhardt_pages::reactive::{
	QueryFamily, QueryOptions, QueryStatus, ResourceState, Signal, queries, use_id, use_query,
	use_resource, use_resource_with_key,
};
use reinhardt_pages::ssr::{SsrOptions, SsrRenderer};
use rstest::rstest;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Barrier;

fn resource_view() -> Page {
	Page::reactive(|| {
		let resource = use_resource(
			|| async { Ok::<_, String>("server-value".to_string()) },
			deps![],
		);

		match resource.get() {
			ResourceState::Success(value) => PageElement::new("div").child(value).into_page(),
			ResourceState::Loading => PageElement::new("div").child("loading").into_page(),
			ResourceState::Error(error) => PageElement::new("div").child(error).into_page(),
		}
	})
}

struct ResourceComponent;

impl Component for ResourceComponent {
	fn render(&self) -> Page {
		let resource = use_resource(
			|| async { Ok::<_, String>("component-server-value".to_string()) },
			deps![],
		);

		match resource.get() {
			ResourceState::Success(value) => PageElement::new("section").child(value).into_page(),
			ResourceState::Loading => PageElement::new("section").child("loading").into_page(),
			ResourceState::Error(error) => PageElement::new("section").child(error).into_page(),
		}
	}

	fn name() -> &'static str {
		"ResourceComponent"
	}
}

#[tokio::test]
async fn ssr_resolved_resource_serializes_state() {
	let mut renderer = SsrRenderer::new();
	let html = renderer
		.render_page_with_view_head_to_string(resource_view())
		.await;

	assert!(html.contains(r#""resources""#));
	assert!(html.contains("server-value"));
	assert!(!html.contains(">loading<"));
}

#[tokio::test]
async fn ssr_resource_fetcher_can_allocate_reactive_state_after_render() {
	let view = Page::reactive(|| {
		let resource = use_resource(
			|| async {
				let scoped_signal = Signal::new(1_i32);
				Ok::<_, String>(scoped_signal.get().to_string())
			},
			deps![],
		);

		match resource.get() {
			ResourceState::Success(value) => PageElement::new("div").child(value).into_page(),
			ResourceState::Loading => PageElement::new("div").child("loading").into_page(),
			ResourceState::Error(error) => PageElement::new("div").child(error).into_page(),
		}
	});

	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(html.contains(">1<"));
	assert!(!html.contains(">loading<"));
}

#[tokio::test]
async fn ssr_replays_component_render_for_top_level_resource() {
	let component = ResourceComponent;
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_to_string(&component).await;

	assert!(html.contains("component-server-value"));
	assert!(!html.contains(">loading<"));
}

#[tokio::test]
async fn buffered_ssr_resolves_resources_discovered_during_replay() {
	let view = Page::reactive(|| {
		let outer = use_resource(|| async { Ok::<_, String>("outer".to_string()) }, deps![]);

		match outer.get() {
			ResourceState::Success(_) => Page::reactive(|| {
				let inner =
					use_resource(|| async { Ok::<_, String>("inner".to_string()) }, deps![]);

				match inner.get() {
					ResourceState::Success(value) => {
						PageElement::new("div").child(value).into_page()
					}
					ResourceState::Loading => {
						PageElement::new("div").child("inner-loading").into_page()
					}
					ResourceState::Error(error) => PageElement::new("div").child(error).into_page(),
				}
			}),
			ResourceState::Loading => PageElement::new("div").child("outer-loading").into_page(),
			ResourceState::Error(error) => PageElement::new("div").child(error).into_page(),
		}
	});

	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(html.contains(">inner<"));
	assert!(!html.contains("outer-loading"));
	assert!(!html.contains("inner-loading"));
	assert_eq!(renderer.state().resource_count(), 2);
}

#[tokio::test]
async fn buffered_ssr_resets_use_id_between_discovery_and_replay() {
	let view = Page::reactive(|| {
		let input_id = use_id();
		let resource = use_resource(|| async { Ok::<_, String>("ready".to_string()) }, deps![]);

		match resource.get() {
			ResourceState::Success(value) => Page::fragment([
				PageElement::new("label")
					.attr("for", input_id.clone())
					.child(value),
				PageElement::new("input").attr("id", input_id),
			]),
			ResourceState::Loading => PageElement::new("span")
				.attr("id", input_id)
				.child("loading")
				.into_page(),
			ResourceState::Error(error) => PageElement::new("span").child(error).into_page(),
		}
	});

	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(html.contains(r#"for="reinhardt-id-0""#));
	assert!(html.contains(r#"id="reinhardt-id-0""#));
	assert!(!html.contains("reinhardt-id-1"));
}

#[tokio::test]
async fn ssr_resource_error_serializes_state() {
	let view = Page::reactive(|| {
		let resource = use_resource(
			|| async { Err::<String, _>("server-error".to_string()) },
			deps![],
		);

		match resource.get() {
			ResourceState::Success(value) => PageElement::new("div").child(value).into_page(),
			ResourceState::Loading => PageElement::new("div").child("loading").into_page(),
			ResourceState::Error(error) => PageElement::new("div").child(error).into_page(),
		}
	});

	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(html.contains("server-error"));
	assert!(!html.contains(">loading<"));
}

#[tokio::test]
async fn ssr_resource_timeout_leaves_loading_unserialized() {
	let view = Page::reactive(|| {
		let resource = use_resource(
			|| async {
				tokio::time::sleep(Duration::from_secs(60)).await;
				Ok::<_, String>("delayed".to_string())
			},
			deps![],
		);

		match resource.get() {
			ResourceState::Success(value) => PageElement::new("div").child(value).into_page(),
			ResourceState::Loading => PageElement::new("div").child("loading").into_page(),
			ResourceState::Error(error) => PageElement::new("div").child(error).into_page(),
		}
	});

	let mut renderer =
		SsrRenderer::with_options(SsrOptions::new().resource_timeout(Duration::from_millis(1)));
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(html.contains(">loading<"));
	assert!(!html.contains("delayed"));
	assert!(!html.contains(r#""rh-res-0""#));
}

#[tokio::test]
async fn explicit_resource_key_is_serialized() {
	let view = Page::reactive(|| {
		let resource = use_resource_with_key(
			"polls.detail.42",
			|| async { Ok::<_, String>("question".to_string()) },
			deps![],
		);

		match resource.get() {
			ResourceState::Success(value) => PageElement::new("div").child(value).into_page(),
			ResourceState::Loading => PageElement::new("div").child("loading").into_page(),
			ResourceState::Error(error) => PageElement::new("div").child(error).into_page(),
		}
	});

	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(html.contains("polls.detail.42"));
	assert!(html.contains("question"));
}

#[tokio::test]
async fn explicit_internal_resource_key_advances_implicit_allocator() {
	let view = Page::reactive(|| {
		let explicit = use_resource_with_key(
			"rh-res-0",
			|| async { Ok::<_, String>("explicit".to_string()) },
			deps![],
		);
		let implicit = use_resource(
			|| async { Ok::<_, String>("implicit".to_string()) },
			deps![],
		);

		Page::fragment([
			match explicit.get() {
				ResourceState::Success(value) => PageElement::new("p").child(value).into_page(),
				ResourceState::Loading => {
					PageElement::new("p").child("explicit-loading").into_page()
				}
				ResourceState::Error(error) => PageElement::new("p").child(error).into_page(),
			},
			match implicit.get() {
				ResourceState::Success(value) => PageElement::new("p").child(value).into_page(),
				ResourceState::Loading => {
					PageElement::new("p").child("implicit-loading").into_page()
				}
				ResourceState::Error(error) => PageElement::new("p").child(error).into_page(),
			},
		])
	});

	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(html.contains(">explicit<"));
	assert!(html.contains(">implicit<"));
	assert!(renderer.state().get_resource_state("rh-res-0").is_some());
	assert!(renderer.state().get_resource_state("rh-res-1").is_some());
	assert_eq!(renderer.state().resource_count(), 2);
}

#[tokio::test]
async fn pending_ssr_resource_reuse_does_not_create_duplicate_fetcher() {
	let fetcher_calls = Rc::new(Cell::new(0));
	let first_calls = Rc::clone(&fetcher_calls);
	let second_calls = Rc::clone(&fetcher_calls);
	let view = Page::reactive(move || {
		let first_calls = Rc::clone(&first_calls);
		let first = use_resource_with_key(
			"shared-resource",
			move || {
				first_calls.set(first_calls.get() + 1);
				async { Ok::<_, String>("shared".to_string()) }
			},
			deps![],
		);

		let second_calls = Rc::clone(&second_calls);
		let second = use_resource_with_key(
			"shared-resource",
			move || {
				second_calls.set(second_calls.get() + 1);
				async { Ok::<_, String>("shared".to_string()) }
			},
			deps![],
		);

		Page::fragment([
			match first.get() {
				ResourceState::Success(value) => PageElement::new("p")
					.child(format!("first-{value}"))
					.into_page(),
				ResourceState::Loading => PageElement::new("p").child("first-loading").into_page(),
				ResourceState::Error(error) => PageElement::new("p").child(error).into_page(),
			},
			match second.get() {
				ResourceState::Success(value) => PageElement::new("p")
					.child(format!("second-{value}"))
					.into_page(),
				ResourceState::Loading => PageElement::new("p").child("second-loading").into_page(),
				ResourceState::Error(error) => PageElement::new("p").child(error).into_page(),
			},
		])
	});

	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(html.contains("first-shared"));
	assert!(html.contains("second-shared"));
	assert_eq!(fetcher_calls.get(), 1);
	assert_eq!(renderer.state().resource_count(), 1);
}

#[rstest]
#[tokio::test]
async fn pending_ssr_query_reuse_does_not_create_duplicate_fetcher() {
	let fetcher_calls = Rc::new(Cell::new(0));
	let query_id = QueryFamily::<(), String, String>::new("shared-query")
		.key(())
		.id();
	let first_calls = Rc::clone(&fetcher_calls);
	let second_calls = Rc::clone(&fetcher_calls);
	let view = Page::reactive(move || {
		let first_calls = Rc::clone(&first_calls);
		let first = use_query(
			QueryFamily::<(), _, _>::new("shared-query").query((), move || {
				first_calls.set(first_calls.get() + 1);
				async { Ok::<_, String>("shared".to_string()) }
			}),
			QueryOptions::default(),
		);

		let second_calls = Rc::clone(&second_calls);
		let second = use_query(
			QueryFamily::<(), _, _>::new("shared-query").query((), move || {
				second_calls.set(second_calls.get() + 1);
				async { Ok::<_, String>("shared".to_string()) }
			}),
			QueryOptions::default(),
		);

		Page::fragment([
			match first.snapshot().status {
				QueryStatus::Idle | QueryStatus::Pending => {
					PageElement::new("p").child("first-loading").into_page()
				}
				QueryStatus::Success => PageElement::new("p")
					.child(format!(
						"first-{}",
						first
							.data()
							.expect("a successful query snapshot contains data")
					))
					.into_page(),
				QueryStatus::Error => PageElement::new("p")
					.child(
						first
							.error()
							.expect("an error query snapshot contains an error"),
					)
					.into_page(),
			},
			match second.snapshot().status {
				QueryStatus::Idle | QueryStatus::Pending => {
					PageElement::new("p").child("second-loading").into_page()
				}
				QueryStatus::Success => PageElement::new("p")
					.child(format!(
						"second-{}",
						second
							.data()
							.expect("a successful query snapshot contains data")
					))
					.into_page(),
				QueryStatus::Error => PageElement::new("p")
					.child(
						second
							.error()
							.expect("an error query snapshot contains an error"),
					)
					.into_page(),
			},
		])
	});

	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(html.contains("first-shared"));
	assert!(html.contains("second-shared"));
	assert_eq!(fetcher_calls.get(), 1);
	assert_eq!(
		renderer
			.state()
			.get_resource_state(&format!("query:{query_id}")),
		Some(&serde_json::json!({
			"status": "Success",
			"data": "shared",
			"error": null,
			"refetch_error": null,
			"is_fetching": false,
			"is_stale": false
		}))
	);
	assert_eq!(renderer.state().resource_count(), 1);
}

fn request_query_view(value: &'static str, fetch_barrier: Arc<Barrier>) -> Page {
	Page::reactive(move || {
		let _request_client = queries();
		let fetch_barrier = Arc::clone(&fetch_barrier);
		let query = use_query(
			QueryFamily::<(), String, String>::new("tests::request-isolation").query(
				(),
				move || {
					let fetch_barrier = Arc::clone(&fetch_barrier);
					async move {
						fetch_barrier.wait().await;
						Ok::<_, String>(value.to_string())
					}
				},
			),
			QueryOptions::default(),
		);

		match query.snapshot().status {
			QueryStatus::Idle | QueryStatus::Pending => {
				PageElement::new("p").child("loading").into_page()
			}
			QueryStatus::Success => PageElement::new("p")
				.child(query.data().expect("successful query data"))
				.into_page(),
			QueryStatus::Error => PageElement::new("p")
				.child(query.error().expect("failed query error"))
				.into_page(),
		}
	})
}

#[tokio::test]
async fn concurrent_ssr_requests_keep_query_clients_isolated() {
	let mut first_renderer = SsrRenderer::new();
	let mut second_renderer = SsrRenderer::new();
	let fetch_barrier = Arc::new(Barrier::new(2));
	let query_id = QueryFamily::<(), String, String>::new("tests::request-isolation")
		.key(())
		.id();

	let (first_html, second_html) = tokio::join!(
		first_renderer.render_page_with_view_head_to_string(request_query_view(
			"first-request",
			Arc::clone(&fetch_barrier),
		)),
		second_renderer.render_page_with_view_head_to_string(request_query_view(
			"second-request",
			fetch_barrier,
		)),
	);

	assert!(first_html.contains("first-request"));
	assert!(!first_html.contains("second-request"));
	assert!(second_html.contains("second-request"));
	assert!(!second_html.contains("first-request"));
	assert_eq!(
		first_renderer
			.state()
			.get_resource_state(&format!("query:{query_id}"))
			.and_then(|snapshot| snapshot.get("data")),
		Some(&serde_json::json!("first-request"))
	);
	assert_eq!(
		second_renderer
			.state()
			.get_resource_state(&format!("query:{query_id}"))
			.and_then(|snapshot| snapshot.get("data")),
		Some(&serde_json::json!("second-request"))
	);
}

#[tokio::test]
async fn disabled_ssr_query_does_not_register_fetch_work() {
	let fetch_count = Rc::new(Cell::new(0));
	let fetch_count_for_view = Rc::clone(&fetch_count);
	let view = Page::reactive(move || {
		let fetch_count = Rc::clone(&fetch_count_for_view);
		let query = use_query(
			QueryFamily::<(), String, String>::new("tests::disabled-ssr").query((), move || {
				let fetch_count = Rc::clone(&fetch_count);
				async move {
					fetch_count.set(fetch_count.get() + 1);
					Ok::<_, String>("unexpected".to_string())
				}
			}),
			QueryOptions::default().enabled(false),
		);
		assert_eq!(query.snapshot().status, QueryStatus::Idle);
		PageElement::new("p").child("disabled").into_page()
	});
	let mut renderer = SsrRenderer::new();

	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(html.contains(">disabled<"));
	assert_eq!(fetch_count.get(), 0);
	assert_eq!(renderer.state().resource_count(), 0);
}

#[tokio::test]
async fn reused_renderer_does_not_emit_previous_resource_state() {
	let mut renderer = SsrRenderer::new();
	let first_html = renderer
		.render_page_with_view_head_to_string(resource_view())
		.await;
	assert!(first_html.contains("server-value"));
	assert_eq!(renderer.state().resource_count(), 1);

	let second_view = PageElement::new("p").child("plain").into_page();
	let second_html = renderer
		.render_page_with_view_head_to_string(second_view)
		.await;

	assert!(second_html.contains("plain"));
	assert!(!second_html.contains("server-value"));
	assert!(!second_html.contains(r#""resources""#));
	assert_eq!(renderer.state().resource_count(), 0);
}
