#![cfg(not(target_arch = "wasm32"))]

use futures_util::StreamExt;
use reinhardt_core::types::page::{DeferredNode, Head, SuspenseNode};
use reinhardt_pages::component::suspense::SuspenseBoundary;
use reinhardt_pages::component::{Component, ControlBinding, IntoPage, Page, PageElement};
use reinhardt_pages::deps;
use reinhardt_pages::reactive::{
	QueryFamily, QueryOptions, QueryStatus, ReactiveScope, ResourceState, Signal, queries,
	use_head, use_id, use_query, use_resource, use_resource_with_key,
};
use reinhardt_pages::ssr::{SsrChunk, SsrOptions, SsrRenderer, SsrStream};
use rstest::rstest;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Barrier, oneshot};

fn suspense_resource_view() -> Page {
	Page::reactive(|| {
		let resource = use_resource(
			|| async {
				tokio::time::sleep(Duration::from_millis(5)).await;
				Ok::<_, String>("resolved".to_string())
			},
			deps![],
		);
		let content_resource = resource.clone();

		SuspenseBoundary::new()
			.fallback(|| PageElement::new("span").child("fallback").into_page())
			.track(resource)
			.content(move || {
				resource_to_page(content_resource.get(), "em", "loading", |value| {
					PageElement::new("strong").child(value).into_page()
				})
			})
			.into_page()
	})
}

fn slow_suspense_resource_view() -> Page {
	delayed_suspense_resource_view(Duration::from_secs(60), "too-late")
}

fn delayed_suspense_resource_view(delay: Duration, value: &'static str) -> Page {
	Page::reactive(move || {
		let resource = use_resource(
			move || async move {
				tokio::time::sleep(delay).await;
				Ok::<_, String>(value.to_string())
			},
			deps![],
		);
		let content_resource = resource.clone();

		SuspenseBoundary::new()
			.fallback(|| PageElement::new("span").child("fallback").into_page())
			.track(resource)
			.content(move || {
				resource_to_page(content_resource.get(), "em", "loading", |value| {
					PageElement::new("strong").child(value).into_page()
				})
			})
			.into_page()
	})
}

fn late_stream_query_view(
	value: &'static str,
	gate: Rc<RefCell<Option<oneshot::Receiver<()>>>>,
	query_barrier: Arc<Barrier>,
	fetch_count: Rc<Cell<usize>>,
) -> Page {
	Page::reactive(move || {
		let gate_for_fetch = Rc::clone(&gate);
		let gate_resource = use_resource_with_key(
			format!("tests::stream-gate::{value}"),
			move || {
				let receiver = gate_for_fetch
					.borrow_mut()
					.take()
					.expect("the stream gate fetcher must start exactly once");
				async move {
					receiver
						.await
						.expect("the stream gate sender must remain open");
					Ok::<_, String>(())
				}
			},
			deps![],
		);
		let gate_for_content = gate_resource;
		let query_barrier = Arc::clone(&query_barrier);
		let fetch_count = Rc::clone(&fetch_count);

		SuspenseBoundary::new()
			.fallback(|| PageElement::new("span").child("stream-gated").into_page())
			.track(gate_resource)
			.content(move || match gate_for_content.get() {
				ResourceState::Success(()) => {
					let _request_client = queries();
					let query_barrier = Arc::clone(&query_barrier);
					let fetch_count = Rc::clone(&fetch_count);
					let query =
						use_query(
							QueryFamily::<(), String, String>::new("tests::late-stream-query")
								.query((), move || {
									let query_barrier = Arc::clone(&query_barrier);
									let fetch_count = Rc::clone(&fetch_count);
									async move {
										fetch_count.set(fetch_count.get() + 1);
										query_barrier.wait().await;
										Ok::<_, String>(value.to_string())
									}
								}),
							QueryOptions::default(),
						);

					match query.snapshot().status {
						QueryStatus::Idle | QueryStatus::Pending => PageElement::new("em")
							.child("late-query-loading")
							.into_page(),
						QueryStatus::Success => PageElement::new("strong")
							.child(query.data().expect("successful late query data"))
							.into_page(),
						QueryStatus::Error => PageElement::new("em")
							.child(query.error().expect("failed late query error"))
							.into_page(),
					}
				}
				ResourceState::Loading => PageElement::new("em").child("gate-loading").into_page(),
				ResourceState::Error(error) => PageElement::new("em").child(error).into_page(),
			})
			.into_page()
	})
}

fn has_managed_head_entry(html: &str, tag: &str, semantic_content: &str) -> bool {
	let marker_prefix = format!("<{tag} data-reinhardt-head=\"");
	html.lines()
		.any(|line| line.starts_with(&marker_prefix) && line.contains(semantic_content))
}

fn controlled_select_suspense_option_view(scope: &ReactiveScope) -> Page {
	let selected = scope.enter(|| Signal::new(vec!["rust".to_owned()]));

	PageElement::new("select")
		.attr("multiple", "multiple")
		.control_binding(ControlBinding::select_many(selected))
		.child(Page::reactive(|| {
			let resource = use_resource(
				|| async {
					tokio::time::sleep(Duration::from_millis(5)).await;
					Ok::<_, String>("rust".to_owned())
				},
				deps![],
			);
			let content_resource = resource.clone();

			SuspenseBoundary::new()
				.fallback(|| {
					PageElement::new("option")
						.attr("value", "loading")
						.child("Loading")
						.into_page()
				})
				.track(resource)
				.content(move || {
					resource_to_page(content_resource.get(), "option", "Loading", |value| {
						PageElement::new("option")
							.attr("value", value)
							.child("Rust")
							.into_page()
					})
				})
				.into_page()
		}))
		.into_page()
}

fn controlled_single_select_suspense_duplicate_view(
	scope: &ReactiveScope,
	before_matches: bool,
	inside_matches: bool,
) -> Page {
	let selected = scope.enter(|| Signal::new("duplicate".to_owned()));
	let before_value = if before_matches { "duplicate" } else { "other" };
	let resolved_value = if inside_matches { "duplicate" } else { "other" };

	PageElement::new("select")
		.control_binding(ControlBinding::select_one(selected))
		.child(
			PageElement::new("option")
				.attr("value", before_value)
				.child("Before"),
		)
		.child(Page::reactive(move || {
			let resource = use_resource(
				move || async move {
					tokio::time::sleep(Duration::from_millis(5)).await;
					Ok::<_, String>(resolved_value.to_owned())
				},
				deps![],
			);
			let content_resource = resource.clone();

			SuspenseBoundary::new()
				.fallback(|| {
					PageElement::new("option")
						.attr("value", "loading")
						.child("Loading")
						.into_page()
				})
				.track(resource)
				.content(move || {
					resource_to_page(content_resource.get(), "option", "Loading", |value| {
						PageElement::new("option")
							.attr("value", value)
							.child("Inside Suspense")
							.into_page()
					})
				})
				.into_page()
		}))
		.child(
			PageElement::new("optgroup").child(
				PageElement::new("option")
					.attr("value", "duplicate")
					.child("After"),
			),
		)
		.into_page()
}

fn controlled_single_select_timed_out_suspense_view(
	scope: &ReactiveScope,
	fallback_matches: bool,
) -> Page {
	let selected = scope.enter(|| Signal::new("duplicate".to_owned()));
	let fallback_value = if fallback_matches {
		"duplicate"
	} else {
		"loading"
	};

	PageElement::new("select")
		.control_binding(ControlBinding::select_one(selected))
		.child(Page::reactive(move || {
			let resource = use_resource(
				|| async {
					tokio::time::sleep(Duration::from_secs(60)).await;
					Ok::<_, String>("duplicate".to_owned())
				},
				deps![],
			);
			let content_resource = resource.clone();

			SuspenseBoundary::new()
				.fallback(move || {
					PageElement::new("option")
						.attr("value", fallback_value)
						.child("Fallback")
						.into_page()
				})
				.track(resource)
				.content(move || {
					resource_to_page(content_resource.get(), "option", "Loading", |value| {
						PageElement::new("option")
							.attr("value", value)
							.child("Inside Suspense")
							.into_page()
					})
				})
				.into_page()
		}))
		.child(
			PageElement::new("option")
				.attr("value", "duplicate")
				.child("After"),
		)
		.into_page()
}

fn pending_nested_boundary(label: &'static str) -> Page {
	Page::Suspense(SuspenseNode::new(
		None,
		|| true,
		move || PageElement::new("span").child(label).into_page(),
		|| {
			PageElement::new("strong")
				.child("nested-content")
				.into_page()
		},
	))
}

fn resource_to_page(
	state: ResourceState<String, String>,
	loading_tag: &'static str,
	loading_text: &'static str,
	success: impl FnOnce(String) -> Page,
) -> Page {
	match state {
		ResourceState::Success(value) => success(value),
		ResourceState::Loading => PageElement::new(loading_tag)
			.child(loading_text)
			.into_page(),
		ResourceState::Error(error) => PageElement::new(loading_tag).child(error).into_page(),
	}
}

struct KeyedResourceComponent {
	key: &'static str,
	value: &'static str,
}

impl Component for KeyedResourceComponent {
	fn render(&self) -> Page {
		let key = self.key.to_string();
		let value = self.value.to_string();
		Page::reactive(move || {
			let fetch_value = value.clone();
			let resource = use_resource_with_key(
				key.clone(),
				move || {
					let value = fetch_value.clone();
					async move { Ok::<_, String>(value) }
				},
				deps![],
			);
			resource_to_page(resource.get(), "em", "loading", |value| {
				PageElement::new("strong").child(value).into_page()
			})
		})
	}

	fn name() -> &'static str {
		"KeyedResourceComponent"
	}
}

struct ImplicitResourceComponent {
	value: &'static str,
}

impl Component for ImplicitResourceComponent {
	fn render(&self) -> Page {
		let value = self.value.to_string();
		Page::reactive(move || {
			let fetch_value = value.clone();
			let resource = use_resource(
				move || {
					let value = fetch_value.clone();
					async move { Ok::<_, String>(value) }
				},
				deps![],
			);
			let key = resource
				.ssr_key()
				.expect("SSR resources should expose their hydration key")
				.to_string();
			resource_to_page(resource.get(), "em", "loading", |value| {
				PageElement::new("strong")
					.attr("data-resource-key", key)
					.child(value)
					.into_page()
			})
		})
	}

	fn name() -> &'static str {
		"ImplicitResourceComponent"
	}
}

struct IdComponent {
	label: &'static str,
}

impl Component for IdComponent {
	fn render(&self) -> Page {
		let label = self.label;
		Page::reactive(move || {
			let input_id = use_id();
			Page::fragment([
				PageElement::new("label")
					.attr("for", input_id.clone())
					.child(label),
				PageElement::new("input").attr("id", input_id),
			])
		})
	}

	fn name() -> &'static str {
		"IdComponent"
	}
}

#[tokio::test]
async fn marker_renders_accumulate_explicit_resource_state() {
	let mut renderer = SsrRenderer::new();
	let first = KeyedResourceComponent {
		key: "first-island",
		value: "first-ready",
	};
	let second = KeyedResourceComponent {
		key: "second-island",
		value: "second-ready",
	};

	let first_html = renderer.render_with_marker(&first).await;
	let second_html = renderer.render_with_marker(&second).await;

	assert!(first_html.contains("first-ready"));
	assert!(second_html.contains("second-ready"));
	assert_eq!(renderer.state().resource_count(), 2);
	assert!(
		renderer
			.state()
			.get_resource_state("first-island")
			.is_some()
	);
	assert!(
		renderer
			.state()
			.get_resource_state("second-island")
			.is_some()
	);
}

#[tokio::test]
async fn marker_renders_preserve_unique_use_id_values() {
	let mut renderer = SsrRenderer::new();
	let first_html = renderer
		.render_with_marker(&IdComponent { label: "first" })
		.await;
	let second_html = renderer
		.render_with_marker(&IdComponent { label: "second" })
		.await;

	assert!(first_html.contains(r#"for="reinhardt-id-0""#));
	assert!(first_html.contains(r#"id="reinhardt-id-0""#));
	assert!(second_html.contains(r#"for="reinhardt-id-1""#));
	assert!(second_html.contains(r#"id="reinhardt-id-1""#));
}

#[tokio::test]
async fn marker_renders_preserve_unique_implicit_resource_keys() {
	let mut renderer = SsrRenderer::new();
	let first_html = renderer
		.render_with_marker(&ImplicitResourceComponent {
			value: "first-ready",
		})
		.await;
	let second_html = renderer
		.render_with_marker(&ImplicitResourceComponent {
			value: "second-ready",
		})
		.await;

	assert!(first_html.contains(r#"data-resource-key="rh-res-0""#));
	assert!(first_html.contains("first-ready"));
	assert!(second_html.contains(r#"data-resource-key="rh-res-1""#));
	assert!(second_html.contains("second-ready"));
	assert!(renderer.state().get_resource_state("rh-res-0").is_some());
	assert!(renderer.state().get_resource_state("rh-res-1").is_some());
	assert_eq!(renderer.state().resource_count(), 2);
}

#[tokio::test]
async fn marker_resource_state_resets_after_full_document_render() {
	let mut renderer = SsrRenderer::new();
	let first_html = renderer
		.render_with_marker(&ImplicitResourceComponent {
			value: "first-ready",
		})
		.await;

	let full_page = renderer
		.render_page_with_view_head_to_string(
			PageElement::new("main").child("new-document").into_page(),
		)
		.await;
	let second_html = renderer
		.render_with_marker(&ImplicitResourceComponent {
			value: "second-ready",
		})
		.await;

	assert!(first_html.contains(r#"data-resource-key="rh-res-0""#));
	assert!(full_page.contains("new-document"));
	assert!(second_html.contains(r#"data-resource-key="rh-res-0""#));
	assert!(!second_html.contains(r#"data-resource-key="rh-res-1""#));
	assert!(second_html.contains("second-ready"));
	assert_eq!(renderer.state().resource_count(), 1);
}

#[tokio::test]
async fn cloned_renderers_do_not_share_marker_resource_state() {
	let mut first_renderer = SsrRenderer::new();
	let mut second_renderer = first_renderer.clone();

	let first_html = first_renderer
		.render_with_marker(&ImplicitResourceComponent {
			value: "first-ready",
		})
		.await;
	let second_html = second_renderer
		.render_with_marker(&ImplicitResourceComponent {
			value: "second-ready",
		})
		.await;

	assert!(first_html.contains(r#"data-resource-key="rh-res-0""#));
	assert!(second_html.contains(r#"data-resource-key="rh-res-0""#));
	assert!(!second_html.contains(r#"data-resource-key="rh-res-1""#));
	assert_eq!(first_renderer.state().resource_count(), 1);
	assert_eq!(second_renderer.state().resource_count(), 1);
}

#[tokio::test]
async fn buffered_suspense_emits_resolved_content_directly() {
	let mut renderer = SsrRenderer::new();
	let html = renderer
		.render_page_with_view_head_to_string(suspense_resource_view())
		.await;

	assert!(html.contains("resolved"));
	assert!(!html.contains("rh-suspense-start:rh-suspense-0"));
	assert!(!html.contains(r#"data-rh-suspense="resolved""#));
	assert!(!html.contains(r#"data-rh-suspense="pending""#));
	assert!(!html.contains(r#"data-rh-suspense-chunk="rh-suspense-0""#));
}

#[tokio::test]
async fn buffered_suspense_rechecks_custom_pending_after_resource_resolution() {
	let view = Page::reactive(|| {
		let resource = use_resource(
			|| async { Ok::<_, String>("resolved".to_string()) },
			deps![],
		);
		let tracked_key = resource.ssr_key().unwrap().to_string();
		let content_resource = resource.clone();

		Page::Suspense(SuspenseNode::new_with_tracked_resources(
			Some("custom-pending".to_string()),
			vec![tracked_key],
			|| true,
			|| {
				PageElement::new("span")
					.child("custom-fallback")
					.into_page()
			},
			move || {
				resource_to_page(content_resource.get(), "em", "loading", |value| {
					PageElement::new("strong").child(value).into_page()
				})
			},
		))
	});

	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(html.contains("custom-fallback"));
	assert!(html.contains(r#"data-rh-suspense="pending""#));
	assert!(!html.contains("<strong"));
}

#[tokio::test]
async fn buffered_suspense_caches_head_from_resolved_content_render() {
	let content_calls = Rc::new(Cell::new(0));
	let render_calls = Rc::clone(&content_calls);
	let view = Page::Suspense(SuspenseNode::new(
		Some("head-cache".to_string()),
		|| false,
		|| PageElement::new("span").child("fallback").into_page(),
		move || {
			let call_index = render_calls.get();
			render_calls.set(call_index + 1);
			let content = PageElement::new("main").child("resolved").into_page();
			if call_index == 1 {
				content.with_head(Head::new().title("Resolved Suspense Head"))
			} else {
				content
			}
		},
	));

	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(has_managed_head_entry(
		&html,
		"title",
		"\">Resolved Suspense Head</title>",
	));
	assert!(html.contains("resolved"));
	assert_eq!(content_calls.get(), 2);
}

#[tokio::test]
async fn buffered_suspense_fallback_discards_pending_content_head() {
	// Arrange
	let view = Page::fragment([
		Page::reactive(|| {
			use_head(
				|| Head::new().meta_description("buffered retained metadata"),
				deps![],
			);
			Page::Empty
		}),
		Page::Suspense(SuspenseNode::new(
			Some("buffered-pending-head".to_string()),
			|| true,
			|| {
				PageElement::new("span")
					.child("buffered fallback")
					.into_page()
					.with_head(
						Head::new()
							.title("Buffered Fallback Head")
							.meta_description("buffered fallback metadata")
							.canonical("https://example.test/buffered-fallback")
							.base_url("https://example.test/buffered-fallback/"),
					)
			},
			|| {
				Page::reactive(|| {
					use_head(
						|| {
							Head::new()
								.meta_description("buffered pending hook metadata")
								.canonical("https://example.test/buffered-pending-hook")
						},
						deps![],
					);
					PageElement::new("main")
						.child("pending content")
						.into_page()
						.with_head(
							Head::new()
								.title("Buffered Pending Content Head")
								.meta_description("buffered pending static metadata")
								.canonical("https://example.test/buffered-pending-static")
								.base_url("https://example.test/buffered-pending-static/"),
						)
				})
			},
		)),
	]);
	let mut renderer = SsrRenderer::new();

	// Act
	let html = renderer.render_page_with_view_head_to_string(view).await;

	// Assert
	assert!(has_managed_head_entry(
		&html,
		"title",
		"\">Buffered Fallback Head</title>",
	));
	assert!(html.contains("buffered retained metadata"));
	assert!(html.contains("buffered fallback metadata"));
	assert!(html.contains("https://example.test/buffered-fallback"));
	assert!(html.contains("buffered fallback"));
	assert!(!html.contains("Buffered Pending Content Head"));
	assert!(!html.contains("buffered pending static metadata"));
	assert!(!html.contains("https://example.test/buffered-pending-static"));
	assert!(!html.contains("buffered pending hook metadata"));
	assert!(!html.contains("https://example.test/buffered-pending-hook"));
}

#[tokio::test]
async fn buffered_suspense_caches_head_after_replay_resources_settle() {
	let view = Page::Suspense(SuspenseNode::new(
		Some("replay-head".to_string()),
		|| false,
		|| PageElement::new("span").child("outer-fallback").into_page(),
		|| {
			let outer = use_resource(|| async { Ok::<_, String>("outer".to_string()) }, deps![]);
			match outer.get() {
				ResourceState::Success(_) => {
					let inner = use_resource(
						|| async { Ok::<_, String>("inner-ready".to_string()) },
						deps![],
					);

					match inner.get() {
						ResourceState::Success(value) => PageElement::new("main")
							.child(value)
							.into_page()
							.with_head(Head::new().title("Inner Ready Head")),
						ResourceState::Loading => {
							PageElement::new("main").child("inner-loading").into_page()
						}
						ResourceState::Error(error) => {
							PageElement::new("main").child(error).into_page()
						}
					}
				}
				ResourceState::Loading => PageElement::new("em").child("outer-loading").into_page(),
				ResourceState::Error(error) => PageElement::new("em").child(error).into_page(),
			}
		},
	));

	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(has_managed_head_entry(
		&html,
		"title",
		"\">Inner Ready Head</title>",
	));
	assert!(html.contains("inner-ready"));
	assert!(!html.contains("inner-loading"));
}

#[tokio::test]
async fn buffered_suspense_replays_resolved_content_inside_boundary() {
	let outer_fetches = Rc::new(Cell::new(0));
	let content_fetches = Rc::clone(&outer_fetches);
	let view = Page::Suspense(SuspenseNode::new(
		Some("boundary-replay".to_string()),
		|| false,
		|| PageElement::new("span").child("fallback").into_page(),
		move || {
			let fetches = Rc::clone(&content_fetches);
			let outer = use_resource(
				move || {
					let fetches = Rc::clone(&fetches);
					async move {
						fetches.set(fetches.get() + 1);
						Ok::<_, String>("outer".to_string())
					}
				},
				deps![],
			);
			resource_to_page(outer.get(), "em", "outer-loading", |_| {
				let inner =
					use_resource(|| async { Ok::<_, String>("inner".to_string()) }, deps![]);
				resource_to_page(inner.get(), "em", "inner-loading", |value| {
					PageElement::new("strong").child(value).into_page()
				})
			})
		},
	));

	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(html.contains("inner"));
	assert_eq!(outer_fetches.get(), 1);
}

#[tokio::test]
async fn buffered_resolved_suspense_replay_restores_deterministic_counters() {
	let view = Page::reactive(|| {
		let gate = use_resource(|| async { Ok::<_, String>("gate".to_string()) }, deps![]);

		resource_to_page(gate.get(), "em", "gate-loading", |_| {
			Page::Suspense(SuspenseNode::new(
				Some("buffered-replay".to_string()),
				|| false,
				|| {
					let id = use_id();
					Page::fragment([
						pending_nested_boundary("fallback-nested"),
						PageElement::new("span")
							.attr("id", id)
							.child("fallback")
							.into_page(),
					])
				},
				|| {
					Page::reactive(|| {
						let content = use_resource(
							|| async { Ok::<_, String>("content".to_string()) },
							deps![],
						);
						let id = use_id();
						resource_to_page(content.get(), "em", "content-loading", |value| {
							Page::fragment([
								pending_nested_boundary("content-nested"),
								PageElement::new("strong")
									.attr("id", id)
									.child(value)
									.into_page(),
							])
						})
					})
				},
			))
		})
	});

	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(html.contains("content"));
	assert!(html.contains(r#"id="reinhardt-id-0""#));
	assert!(!html.contains("reinhardt-id-1"));
	assert!(html.contains("rh-suspense-start:rh-suspense-0"));
	assert!(!html.contains("rh-suspense-start:rh-suspense-1"));
}

#[tokio::test]
async fn buffered_suspense_replays_external_resource_tracked_by_boundary() {
	let view = Page::reactive(|| {
		let gate = use_resource(|| async { Ok::<_, String>("gate".to_string()) }, deps![]);

		resource_to_page(gate.get(), "p", "gate-loading", |_| {
			Page::reactive(|| {
				let shared =
					use_resource(|| async { Ok::<_, String>("shared".to_string()) }, deps![]);
				let tracked_key = shared.ssr_key().unwrap().to_string();
				let outside_resource = shared.clone();
				let boundary_pending = shared.clone();
				let boundary_content = shared.clone();

				let outside =
					resource_to_page(outside_resource.get(), "p", "outside-loading", |value| {
						PageElement::new("p")
							.child(format!("outside-{value}"))
							.into_page()
					});

				Page::fragment([
					outside,
					Page::Suspense(SuspenseNode::new_with_tracked_resources(
						Some("buffered-shared".to_string()),
						vec![tracked_key],
						move || boundary_pending.is_loading(),
						|| {
							PageElement::new("span")
								.child("boundary-fallback")
								.into_page()
						},
						move || {
							resource_to_page(
								boundary_content.get(),
								"em",
								"boundary-loading",
								|value| {
									PageElement::new("strong")
										.child(format!("boundary-{value}"))
										.into_page()
								},
							)
						},
					)),
				])
			})
		})
	});

	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(html.contains("outside-shared"));
	assert!(html.contains("boundary-shared"));
	assert!(!html.contains("outside-loading"));
	assert!(!html.contains("boundary-fallback"));
}

#[tokio::test]
async fn buffered_deferred_head_updates_after_replay_settles() {
	let view = Page::Deferred(DeferredNode::new(
		"deferred-head",
		|| PageElement::new("span").child("fallback").into_page(),
		|| {
			let first = use_resource(|| async { Ok::<_, String>("first".to_string()) }, deps![]);
			resource_to_page(first.get(), "em", "first-loading", |_| {
				let second =
					use_resource(|| async { Ok::<_, String>("second".to_string()) }, deps![]);
				match second.get() {
					ResourceState::Success(_) => PageElement::new("strong")
						.child("deferred-ready")
						.into_page()
						.with_head(Head::new().title("Resolved Deferred Head")),
					ResourceState::Loading => PageElement::new("em")
						.child("second-loading")
						.into_page()
						.with_head(Head::new().title("Loading Deferred Head")),
					ResourceState::Error(error) => PageElement::new("em").child(error).into_page(),
				}
			})
		},
	));

	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(html.contains("deferred-ready"));
	assert!(has_managed_head_entry(
		&html,
		"title",
		"\">Resolved Deferred Head</title>",
	));
	assert!(!has_managed_head_entry(
		&html,
		"title",
		"\">Loading Deferred Head</title>",
	));
}

#[tokio::test]
async fn buffered_page_without_state_script_keeps_resource_dom_pending() {
	let mut options = SsrOptions::new();
	options.include_state_script = false;
	let mut renderer = SsrRenderer::with_options(options);
	let html = renderer
		.render_page_with_view_head_to_string(suspense_resource_view())
		.await;

	assert!(html.contains("fallback"));
	assert!(html.contains(r#"data-rh-suspense="pending""#));
	assert!(!html.contains("resolved"));
	assert!(!html.contains("ssr-state"));
	assert_eq!(renderer.state().resource_count(), 0);
}

#[tokio::test]
async fn streaming_page_without_state_script_skips_resource_replacements() {
	let mut options = SsrOptions::new();
	options.include_state_script = false;
	let mut renderer = SsrRenderer::with_options(options);
	let mut stream = renderer
		.render_page_with_view_head(suspense_resource_view())
		.await;
	let mut html = String::new();
	while let Some(chunk) = stream.next().await {
		html.push_str(&chunk.into_string());
	}

	assert!(html.contains("fallback"));
	assert!(html.contains(r#"data-rh-suspense="pending""#));
	assert!(!html.contains("resolved"));
	assert!(!html.contains(r#"data-rh-suspense-chunk="rh-suspense-0""#));
	assert!(!html.contains("ssr-state"));
}

#[tokio::test]
async fn streaming_single_select_without_state_script_commits_fallback_selection() {
	// Arrange
	let mut options = SsrOptions::new();
	options.include_state_script = false;
	let reactive_scope = ReactiveScope::new();
	let view = controlled_single_select_timed_out_suspense_view(&reactive_scope, true);
	let mut renderer = SsrRenderer::with_options(options);

	// Act
	let html = renderer
		.render_page_with_view_head(view)
		.await
		.collect_string()
		.await;

	// Assert
	assert_eq!(html.matches("selected=\"selected\"").count(), 1, "{html}");
	assert!(html.contains("selected=\"selected\">Fallback</option>"));
}

#[rstest]
#[tokio::test]
async fn streaming_controlled_select_replacement_preserves_selected_values() {
	// Arrange
	let reactive_scope = ReactiveScope::new();
	let view = controlled_select_suspense_option_view(&reactive_scope);
	let mut buffered_renderer = SsrRenderer::new();
	let mut streaming_renderer = SsrRenderer::new();

	// Act
	let buffered = buffered_renderer
		.render_page_with_view_head_to_string(view.clone())
		.await;
	let mut stream = streaming_renderer.render_page_with_view_head(view).await;
	let _shell = stream.next().await.unwrap().into_string();
	let replacement = stream.next().await.unwrap().into_string();
	let replacement_content = replacement
		.split_once('>')
		.unwrap()
		.1
		.split_once("</template>")
		.unwrap()
		.0;

	// Assert
	assert!(buffered.contains(
		"<select multiple=\"multiple\"><option value=\"rust\" selected=\"selected\">Rust</option></select>"
	));
	assert_eq!(
		replacement_content,
		"<option value=\"rust\" selected=\"selected\">Rust</option>"
	);
}

#[rstest]
#[case(true, true, "Before")]
#[case(false, true, "Inside Suspense")]
#[case(false, false, "After")]
#[tokio::test]
async fn streaming_controlled_single_select_preserves_first_duplicate_in_tree_order(
	#[case] before_matches: bool,
	#[case] inside_matches: bool,
	#[case] selected_label: &str,
) {
	// Arrange
	let reactive_scope = ReactiveScope::new();
	let view = controlled_single_select_suspense_duplicate_view(
		&reactive_scope,
		before_matches,
		inside_matches,
	);
	let mut buffered_renderer = SsrRenderer::new();
	let mut streaming_renderer = SsrRenderer::new();

	// Act
	let buffered = buffered_renderer
		.render_page_with_view_head_to_string(view.clone())
		.await;
	let streaming = streaming_renderer
		.render_page_with_view_head(view)
		.await
		.collect_string()
		.await;

	// Assert
	assert_eq!(
		buffered.matches("selected=\"selected\"").count(),
		1,
		"{buffered}"
	);
	assert!(buffered.contains(&format!("selected=\"selected\">{selected_label}</option>")));
	assert_eq!(
		streaming.matches("selected=\"selected\"").count(),
		1,
		"{streaming}"
	);
	assert!(streaming.contains(&format!("selected=\"selected\">{selected_label}</option>")));
}

#[rstest]
#[case(true, "Fallback")]
#[case(false, "After")]
#[tokio::test]
async fn streaming_timed_out_single_select_uses_emitted_fallback_tree_order(
	#[case] fallback_matches: bool,
	#[case] selected_label: &str,
) {
	// Arrange
	let reactive_scope = ReactiveScope::new();
	let view = controlled_single_select_timed_out_suspense_view(&reactive_scope, fallback_matches);
	let mut renderer =
		SsrRenderer::with_options(SsrOptions::new().resource_timeout(Duration::from_millis(1)));

	// Act
	let html = renderer
		.render_page_with_view_head(view)
		.await
		.collect_string()
		.await;

	// Assert
	assert_eq!(html.matches("selected=\"selected\"").count(), 1, "{html}");
	assert!(html.contains(&format!("selected=\"selected\">{selected_label}</option>")));
	assert!(!html.contains("data-rh-suspense-chunk"));
}

#[tokio::test]
async fn suspense_stream_emits_shell_replacement_and_closing_chunks() {
	let mut renderer = SsrRenderer::new();
	let mut stream = renderer
		.render_page_with_view_head(suspense_resource_view())
		.await;

	let shell = stream.next().await.unwrap().into_string();
	let replacement = stream.next().await.unwrap().into_string();
	let closing = stream.next().await.unwrap().into_string();

	assert!(shell.contains(r#"data-rh-suspense="pending""#));
	assert!(replacement.contains(r#"data-rh-suspense-chunk="rh-suspense-0""#));
	assert!(replacement.contains("resolved"));
	assert!(closing.contains(r#"<script id="ssr-state" type="application/json">"#));
	assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn streaming_shell_drains_external_resources_discovered_during_replay() {
	let view = Page::reactive(|| {
		let first = use_resource(
			|| async { Ok::<_, String>("first-ready".to_string()) },
			deps![],
		);
		let first_state = first.clone();

		resource_to_page(first_state.get(), "em", "first-loading", |_| {
			Page::reactive(|| {
				let second = use_resource(
					|| async { Ok::<_, String>("second-ready".to_string()) },
					deps![],
				);
				match second.get() {
					ResourceState::Success(value) => PageElement::new("strong")
						.child(value)
						.into_page()
						.with_head(Head::new().title("Second Ready Head")),
					ResourceState::Loading => PageElement::new("em")
						.child("second-loading")
						.into_page()
						.with_head(Head::new().title("Second Loading Head")),
					ResourceState::Error(error) => PageElement::new("em").child(error).into_page(),
				}
			})
		})
	});

	let mut renderer = SsrRenderer::new();
	let mut stream = renderer.render_page_with_view_head(view).await;
	let shell = stream.next().await.unwrap().into_string();
	let closing = stream.next().await.unwrap().into_string();

	assert!(shell.contains("second-ready"));
	assert!(!shell.contains("second-loading"));
	assert!(has_managed_head_entry(
		&shell,
		"title",
		"\">Second Ready Head</title>",
	));
	assert!(!has_managed_head_entry(
		&shell,
		"title",
		"\">Second Loading Head</title>",
	));
	assert!(closing.contains("rh-res-1"));
	assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn streaming_shell_resolves_resource_used_outside_and_tracked_boundary() {
	let view = Page::reactive(|| {
		let resource = use_resource(
			|| async { Ok::<_, String>("shared-ready".to_string()) },
			deps![],
		);
		let tracked_key = resource.ssr_key().unwrap().to_string();
		let outside_resource = resource.clone();
		let boundary_pending = resource.clone();
		let boundary_content = resource.clone();

		let outside = resource_to_page(outside_resource.get(), "p", "outside-loading", |value| {
			PageElement::new("p")
				.child(format!("outside-{value}"))
				.into_page()
		});

		Page::fragment([
			outside,
			Page::Suspense(SuspenseNode::new_with_tracked_resources(
				Some("tracked".to_string()),
				vec![tracked_key],
				move || boundary_pending.is_loading(),
				|| {
					PageElement::new("span")
						.child("boundary-fallback")
						.into_page()
				},
				move || {
					resource_to_page(boundary_content.get(), "em", "boundary-loading", |value| {
						PageElement::new("strong")
							.child(format!("boundary-{value}"))
							.into_page()
					})
				},
			)),
		])
	});

	let mut renderer = SsrRenderer::new();
	let mut stream = renderer.render_page_with_view_head(view).await;
	let shell = stream.next().await.unwrap().into_string();
	let closing = stream.next().await.unwrap().into_string();

	assert!(shell.contains("outside-shared-ready"));
	assert!(shell.contains("boundary-shared-ready"));
	assert!(!shell.contains("outside-loading"));
	assert!(!shell.contains("boundary-fallback"));
	assert!(closing.contains("rh-res-0"));
	assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn suspense_stream_returns_shell_before_resource_resolves() {
	let mut renderer = SsrRenderer::new();
	let mut stream = tokio::time::timeout(
		Duration::from_millis(50),
		renderer.render_page_with_view_head(delayed_suspense_resource_view(
			Duration::from_millis(200),
			"resolved-later",
		)),
	)
	.await
	.expect("render_page should return before the boundary resource resolves");

	let shell = tokio::time::timeout(Duration::from_millis(50), stream.next())
		.await
		.expect("shell chunk should be available immediately")
		.expect("shell chunk")
		.into_string();

	assert!(shell.contains(r#"data-rh-suspense="pending""#));
	assert!(shell.contains("fallback"));
	assert!(!shell.contains("resolved-later"));

	let replacement = tokio::time::timeout(Duration::from_secs(1), stream.next())
		.await
		.expect("replacement chunk should arrive after the resource resolves")
		.expect("replacement chunk")
		.into_string();

	assert!(replacement.contains(r#"data-rh-suspense-chunk="rh-suspense-0""#));
	assert!(replacement.contains("resolved-later"));
}

#[tokio::test]
async fn concurrent_streams_scope_queries_created_after_suspense_resumes() {
	let (first_gate_sender, first_gate_receiver) = oneshot::channel();
	let (second_gate_sender, second_gate_receiver) = oneshot::channel();
	let query_barrier = Arc::new(Barrier::new(2));
	let first_fetch_count = Rc::new(Cell::new(0));
	let second_fetch_count = Rc::new(Cell::new(0));
	let mut first_renderer = SsrRenderer::new();
	let mut second_renderer = SsrRenderer::new();
	let mut first_stream = first_renderer
		.render_page_with_view_head(late_stream_query_view(
			"first-late-query",
			Rc::new(RefCell::new(Some(first_gate_receiver))),
			Arc::clone(&query_barrier),
			Rc::clone(&first_fetch_count),
		))
		.await;
	let mut second_stream = second_renderer
		.render_page_with_view_head(late_stream_query_view(
			"second-late-query",
			Rc::new(RefCell::new(Some(second_gate_receiver))),
			query_barrier,
			Rc::clone(&second_fetch_count),
		))
		.await;

	let first_shell = first_stream
		.next()
		.await
		.expect("first stream shell")
		.into_string();
	let second_shell = second_stream
		.next()
		.await
		.expect("second stream shell")
		.into_string();
	assert!(first_shell.contains("stream-gated"));
	assert!(second_shell.contains("stream-gated"));
	assert_eq!(first_fetch_count.get(), 0);
	assert_eq!(second_fetch_count.get(), 0);

	first_gate_sender
		.send(())
		.expect("first stream gate receiver");
	second_gate_sender
		.send(())
		.expect("second stream gate receiver");
	let (first_replacement, second_replacement) =
		tokio::join!(first_stream.next(), second_stream.next());
	let first_replacement = first_replacement
		.expect("first stream replacement")
		.into_string();
	let second_replacement = second_replacement
		.expect("second stream replacement")
		.into_string();

	assert!(first_replacement.contains("first-late-query"));
	assert!(!first_replacement.contains("second-late-query"));
	assert!(second_replacement.contains("second-late-query"));
	assert!(!second_replacement.contains("first-late-query"));
	assert_eq!(first_fetch_count.get(), 1);
	assert_eq!(second_fetch_count.get(), 1);

	let first_closing = first_stream
		.next()
		.await
		.expect("first stream closing chunk")
		.into_string();
	let second_closing = second_stream
		.next()
		.await
		.expect("second stream closing chunk")
		.into_string();
	assert!(first_closing.contains("first-late-query"));
	assert!(!first_closing.contains("second-late-query"));
	assert!(second_closing.contains("second-late-query"));
	assert!(!second_closing.contains("first-late-query"));
	assert!(first_stream.next().await.is_none());
	assert!(second_stream.next().await.is_none());
}

#[tokio::test]
async fn suspense_replacement_uses_script_nonce() {
	let mut renderer = SsrRenderer::with_options(SsrOptions::new().script_nonce("nonce-123"));
	let mut stream = renderer
		.render_page_with_view_head(suspense_resource_view())
		.await;

	let _shell = stream.next().await.unwrap();
	let replacement = stream.next().await.unwrap().into_string();
	assert!(replacement.contains(r#"<script nonce="nonce-123">"#));
}

#[tokio::test]
async fn suspense_replacement_escapes_boundary_id_for_script() {
	let boundary_id = "x</script><script>alert(1)</script>";
	let view = Page::reactive(move || {
		let resource = use_resource(
			|| async { Ok::<_, String>("safe-content".to_string()) },
			deps![],
		);
		let tracked_key = resource.ssr_key().unwrap().to_string();
		let pending_resource = resource.clone();
		let content_resource = resource.clone();

		Page::Suspense(SuspenseNode::new_with_tracked_resources(
			Some(boundary_id.to_string()),
			vec![tracked_key],
			move || pending_resource.is_loading(),
			|| PageElement::new("span").child("fallback").into_page(),
			move || match content_resource.get() {
				ResourceState::Success(value) => {
					PageElement::new("strong").child(value).into_page()
				}
				ResourceState::Loading => PageElement::new("em").child("loading").into_page(),
				ResourceState::Error(error) => PageElement::new("em").child(error).into_page(),
			},
		))
	});

	let mut renderer = SsrRenderer::new();
	let mut stream = renderer.render_page_with_view_head(view).await;
	let shell = stream.next().await.unwrap().into_string();
	let replacement = stream.next().await.unwrap().into_string();

	assert!(!shell.contains(boundary_id));
	assert!(!replacement.contains(boundary_id));
	assert!(shell.contains("rh-suspense-start:rh-suspense-id-"));
	assert!(replacement.contains(r#"data-rh-suspense-chunk="rh-suspense-id-"#));
	assert_eq!(replacement.matches("</script>").count(), 1);
	assert!(!replacement.contains("<script>alert(1)</script>"));
}

#[tokio::test]
async fn streaming_shared_resource_replaces_every_tracking_boundary() {
	let view = Page::reactive(|| {
		let resource = use_resource(
			|| async {
				tokio::time::sleep(Duration::from_millis(5)).await;
				Ok::<_, String>("shared".to_string())
			},
			deps![],
		);
		let tracked_key = resource.ssr_key().unwrap().to_string();
		let first_pending = resource.clone();
		let first_content = resource.clone();
		let second_pending = resource.clone();
		let second_content = resource.clone();

		Page::fragment([
			Page::Suspense(SuspenseNode::new_with_tracked_resources(
				Some("first".to_string()),
				vec![tracked_key.clone()],
				move || first_pending.is_loading(),
				|| PageElement::new("span").child("first-fallback").into_page(),
				move || {
					resource_to_page(first_content.get(), "em", "first-loading", |value| {
						PageElement::new("strong")
							.child(format!("first-{value}"))
							.into_page()
					})
				},
			)),
			Page::Suspense(SuspenseNode::new_with_tracked_resources(
				Some("second".to_string()),
				vec![tracked_key],
				move || second_pending.is_loading(),
				|| {
					PageElement::new("span")
						.child("second-fallback")
						.into_page()
				},
				move || {
					resource_to_page(second_content.get(), "em", "second-loading", |value| {
						PageElement::new("strong")
							.child(format!("second-{value}"))
							.into_page()
					})
				},
			)),
		])
	});

	let mut renderer = SsrRenderer::new();
	let mut stream = renderer.render_page_with_view_head(view).await;
	let shell = stream.next().await.unwrap().into_string();
	let first_replacement = stream.next().await.unwrap().into_string();
	let second_replacement = stream.next().await.unwrap().into_string();
	let combined_replacements = format!("{first_replacement}{second_replacement}");

	assert!(shell.contains("first-fallback"));
	assert!(shell.contains("second-fallback"));
	assert!(combined_replacements.contains(r#"data-rh-suspense-chunk="first""#));
	assert!(combined_replacements.contains(r#"data-rh-suspense-chunk="second""#));
	assert!(combined_replacements.contains("first-shared"));
	assert!(combined_replacements.contains("second-shared"));
}

#[tokio::test]
async fn streaming_shared_group_preserves_resolved_boundary_when_peer_times_out() {
	let view = Page::reactive(|| {
		let shared = use_resource(|| async { Ok::<_, String>("shared".to_string()) }, deps![]);
		let slow = use_resource(
			|| async {
				tokio::time::sleep(Duration::from_secs(60)).await;
				Ok::<_, String>("slow".to_string())
			},
			deps![],
		);
		let shared_key = shared.ssr_key().unwrap().to_string();
		let slow_key = slow.ssr_key().unwrap().to_string();
		let first_pending = shared.clone();
		let first_content = shared.clone();
		let second_shared_pending = shared.clone();
		let second_slow_pending = slow.clone();
		let second_content = shared.clone();

		Page::fragment([
			Page::Suspense(SuspenseNode::new_with_tracked_resources(
				Some("first".to_string()),
				vec![shared_key.clone()],
				move || first_pending.is_loading(),
				|| PageElement::new("span").child("first-fallback").into_page(),
				move || {
					resource_to_page(first_content.get(), "em", "first-loading", |value| {
						PageElement::new("strong")
							.child(format!("first-{value}"))
							.into_page()
					})
				},
			)),
			Page::Suspense(SuspenseNode::new_with_tracked_resources(
				Some("second".to_string()),
				vec![shared_key, slow_key],
				move || second_shared_pending.is_loading() || second_slow_pending.is_loading(),
				|| {
					PageElement::new("span")
						.child("second-fallback")
						.into_page()
				},
				move || {
					resource_to_page(second_content.get(), "em", "second-loading", |value| {
						PageElement::new("strong")
							.child(format!("second-{value}"))
							.into_page()
					})
				},
			)),
		])
	});

	let mut renderer =
		SsrRenderer::with_options(SsrOptions::new().resource_timeout(Duration::from_millis(1)));
	let mut stream = renderer.render_page_with_view_head(view).await;
	let shell = stream.next().await.unwrap().into_string();
	let first_replacement = stream.next().await.unwrap().into_string();
	let closing = stream.next().await.unwrap().into_string();

	assert!(shell.contains("first-fallback"));
	assert!(shell.contains("second-fallback"));
	assert!(first_replacement.contains(r#"data-rh-suspense-chunk="first""#));
	assert!(first_replacement.contains("first-shared"));
	assert!(!first_replacement.contains(r#"data-rh-suspense-chunk="second""#));
	assert!(closing.contains("ssr-state"));
	assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn streaming_shared_timeout_keeps_all_tracking_boundaries_pending() {
	let view = Page::reactive(|| {
		let shared = use_resource(
			|| async {
				tokio::time::sleep(Duration::from_secs(60)).await;
				Ok::<_, String>("shared".to_string())
			},
			deps![],
		);
		let tracked_key = shared.ssr_key().unwrap().to_string();
		let first_pending = shared.clone();
		let first_content = shared.clone();
		let second_pending = shared.clone();
		let second_content = shared.clone();

		Page::fragment([
			Page::Suspense(SuspenseNode::new_with_tracked_resources(
				Some("first".to_string()),
				vec![tracked_key.clone()],
				move || first_pending.is_loading(),
				|| PageElement::new("span").child("first-fallback").into_page(),
				move || match first_content.get() {
					ResourceState::Success(value) => PageElement::new("strong")
						.child(format!("first-{value}"))
						.into_page(),
					ResourceState::Loading => {
						PageElement::new("em").child("first-loading").into_page()
					}
					ResourceState::Error(error) => PageElement::new("em").child(error).into_page(),
				},
			)),
			Page::Suspense(SuspenseNode::new_with_tracked_resources(
				Some("second".to_string()),
				vec![tracked_key],
				move || second_pending.is_loading(),
				|| {
					PageElement::new("span")
						.child("second-fallback")
						.into_page()
				},
				move || match second_content.get() {
					ResourceState::Success(value) => PageElement::new("strong")
						.child(format!("second-{value}"))
						.into_page(),
					ResourceState::Loading => {
						PageElement::new("em").child("second-loading").into_page()
					}
					ResourceState::Error(error) => PageElement::new("em").child(error).into_page(),
				},
			)),
		])
	});

	let mut renderer =
		SsrRenderer::with_options(SsrOptions::new().resource_timeout(Duration::from_millis(1)));
	let mut stream = renderer.render_page_with_view_head(view).await;
	let shell = stream.next().await.unwrap().into_string();
	let closing = stream.next().await.unwrap().into_string();

	assert!(shell.contains("first-fallback"));
	assert!(shell.contains("second-fallback"));
	assert!(!closing.contains(r#"data-rh-suspense-chunk="first""#));
	assert!(!closing.contains(r#"data-rh-suspense-chunk="second""#));
	assert!(!closing.contains("first-loading"));
	assert!(!closing.contains("second-loading"));
	assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn suspense_timeout_keeps_fallback_without_replacement() {
	let mut renderer =
		SsrRenderer::with_options(SsrOptions::new().resource_timeout(Duration::from_millis(1)));
	let html = renderer
		.render_page_with_view_head_to_string(slow_suspense_resource_view())
		.await;

	assert!(html.contains(r#"data-rh-suspense="pending""#));
	assert!(html.contains("fallback"));
	assert!(!html.contains(r#"data-rh-suspense-chunk="rh-suspense-0""#));
	assert!(!html.contains("too-late"));
}

#[tokio::test]
async fn streamed_replacement_preserves_nested_suspense_markers() {
	let view = Page::reactive(|| {
		let outer = use_resource(
			|| async {
				tokio::time::sleep(Duration::from_millis(5)).await;
				Ok::<_, String>("outer".to_string())
			},
			deps![],
		);
		let outer_content = outer.clone();

		SuspenseBoundary::new()
			.fallback(|| PageElement::new("span").child("outer-fallback").into_page())
			.track(outer)
			.content(move || match outer_content.get() {
				ResourceState::Success(_) => Page::reactive(|| {
					let inner = use_resource(
						|| async {
							tokio::time::sleep(Duration::from_millis(20)).await;
							Ok::<_, String>("inner-resolved".to_string())
						},
						deps![],
					);
					let inner_content = inner.clone();

					SuspenseBoundary::new()
						.fallback(|| PageElement::new("span").child("inner-fallback").into_page())
						.track(inner)
						.content(move || match inner_content.get() {
							ResourceState::Success(value) => {
								PageElement::new("strong").child(value).into_page()
							}
							ResourceState::Loading => {
								PageElement::new("em").child("inner-loading").into_page()
							}
							ResourceState::Error(error) => {
								PageElement::new("em").child(error).into_page()
							}
						})
						.into_page()
				}),
				ResourceState::Loading => PageElement::new("em").child("outer-loading").into_page(),
				ResourceState::Error(error) => PageElement::new("em").child(error).into_page(),
			})
			.into_page()
	});

	let mut renderer = SsrRenderer::new();
	let mut stream = renderer.render_page_with_view_head(view).await;
	let shell = stream.next().await.unwrap().into_string();
	let outer_replacement = stream.next().await.unwrap().into_string();
	let inner_replacement = stream.next().await.unwrap().into_string();

	assert!(shell.contains("outer-fallback"));
	assert!(outer_replacement.contains("inner-fallback"));
	assert!(outer_replacement.contains("rh-suspense-start:rh-suspense-1"));
	assert!(inner_replacement.contains(r#"data-rh-suspense-chunk="rh-suspense-1""#));
	assert!(inner_replacement.contains("inner-resolved"));
}

#[tokio::test]
async fn streamed_replacement_waits_for_nested_resource_read_by_outer_content() {
	let view = Page::reactive(|| {
		let outer = use_resource(
			|| async {
				tokio::time::sleep(Duration::from_millis(5)).await;
				Ok::<_, String>("outer".to_string())
			},
			deps![],
		);
		let outer_content = outer.clone();

		SuspenseBoundary::new()
			.fallback(|| PageElement::new("span").child("outer-fallback").into_page())
			.track(outer)
			.content(move || match outer_content.get() {
				ResourceState::Success(_) => Page::reactive(|| {
					let shared = use_resource(
						|| async {
							tokio::time::sleep(Duration::from_millis(5)).await;
							Ok::<_, String>("shared".to_string())
						},
						deps![],
					);
					let outer_status = if shared.is_loading() {
						"outer-loading"
					} else if shared.is_success() {
						"outer-success"
					} else {
						"outer-error"
					};
					let inner_content = shared.clone();

					Page::fragment([
						PageElement::new("p").child(outer_status).into_page(),
						SuspenseBoundary::new()
							.fallback(|| {
								PageElement::new("span").child("inner-fallback").into_page()
							})
							.track(shared)
							.content(move || match inner_content.get() {
								ResourceState::Success(value) => {
									PageElement::new("strong").child(value).into_page()
								}
								ResourceState::Loading => {
									PageElement::new("em").child("inner-loading").into_page()
								}
								ResourceState::Error(error) => {
									PageElement::new("em").child(error).into_page()
								}
							})
							.into_page(),
					])
				}),
				ResourceState::Loading => PageElement::new("em").child("outer-loading").into_page(),
				ResourceState::Error(error) => PageElement::new("em").child(error).into_page(),
			})
			.into_page()
	});

	let mut renderer = SsrRenderer::new();
	let mut stream = renderer.render_page_with_view_head(view).await;
	let shell = stream.next().await.unwrap().into_string();
	let outer_replacement = stream.next().await.unwrap().into_string();

	assert!(shell.contains("outer-fallback"));
	assert!(outer_replacement.contains("outer-success"));
	assert!(outer_replacement.contains("shared"));
	assert!(!outer_replacement.contains("outer-loading"));
	assert!(!outer_replacement.contains("inner-fallback"));
}

#[tokio::test]
async fn streaming_suspense_restores_deterministic_counters_after_hidden_content() {
	let view = Page::reactive(|| {
		let resource = use_resource(
			|| async {
				tokio::time::sleep(Duration::from_millis(5)).await;
				Ok::<_, String>("ready".to_string())
			},
			deps![],
		);
		let content_resource = resource.clone();

		SuspenseBoundary::new()
			.fallback(|| {
				let id = use_id();
				Page::fragment([
					pending_nested_boundary("fallback-nested"),
					PageElement::new("span")
						.attr("id", id)
						.child("fallback")
						.into_page(),
				])
			})
			.track(resource)
			.content(move || {
				let id = use_id();
				match content_resource.get() {
					ResourceState::Success(value) => Page::fragment([
						pending_nested_boundary("content-nested"),
						PageElement::new("strong")
							.attr("id", id)
							.child(value)
							.into_page(),
					]),
					ResourceState::Loading => Page::fragment([
						pending_nested_boundary("loading-nested"),
						PageElement::new("em")
							.attr("id", id)
							.child("loading")
							.into_page(),
					]),
					ResourceState::Error(error) => PageElement::new("em").child(error).into_page(),
				}
			})
			.into_page()
	});

	let mut renderer = SsrRenderer::new();
	let mut stream = renderer.render_page_with_view_head(view).await;
	let shell = stream.next().await.unwrap().into_string();
	let replacement = stream.next().await.unwrap().into_string();

	assert!(shell.contains(r#"id="reinhardt-id-0""#));
	assert!(!shell.contains("reinhardt-id-1"));
	assert!(shell.contains("rh-suspense-start:rh-suspense-1"));
	assert!(!shell.contains("rh-suspense-start:rh-suspense-2"));
	assert!(replacement.contains(r#"id="reinhardt-id-0""#));
	assert!(!replacement.contains("reinhardt-id-1"));
	assert!(replacement.contains("rh-suspense-start:rh-suspense-1"));
	assert!(!replacement.contains("rh-suspense-start:rh-suspense-2"));
}

#[tokio::test]
async fn streaming_suspense_restores_resource_keys_before_fallback() {
	let view = Page::Suspense(SuspenseNode::new(
		Some("resource-key-boundary".to_string()),
		|| false,
		|| {
			Page::reactive(|| {
				let resource = use_resource(
					|| async { Ok::<_, String>("fallback-ready".to_string()) },
					deps![],
				);
				let key = resource.ssr_key().unwrap().to_string();
				match resource.get() {
					ResourceState::Success(value) => PageElement::new("span")
						.attr("data-resource-key", key)
						.child(value)
						.into_page(),
					ResourceState::Loading => PageElement::new("span")
						.attr("data-resource-key", key)
						.child("fallback-loading")
						.into_page(),
					ResourceState::Error(error) => PageElement::new("em").child(error).into_page(),
				}
			})
		},
		|| {
			Page::reactive(|| {
				let resource = use_resource(
					|| async {
						tokio::time::sleep(Duration::from_millis(5)).await;
						Ok::<_, String>("content-ready".to_string())
					},
					deps![],
				);
				let key = resource.ssr_key().unwrap().to_string();
				match resource.get() {
					ResourceState::Success(value) => PageElement::new("strong")
						.attr("data-resource-key", key)
						.child(value)
						.into_page(),
					ResourceState::Loading => PageElement::new("em")
						.attr("data-resource-key", key)
						.child("content-loading")
						.into_page(),
					ResourceState::Error(error) => PageElement::new("em").child(error).into_page(),
				}
			})
		},
	));

	let mut renderer = SsrRenderer::new();
	let mut stream = renderer.render_page_with_view_head(view).await;
	let shell = stream.next().await.unwrap().into_string();
	let replacement = stream.next().await.unwrap().into_string();

	assert!(shell.contains("fallback-ready"));
	assert!(shell.contains(r#"data-resource-key="rh-res-0""#));
	assert!(!shell.contains(r#"data-resource-key="rh-res-1""#));
	assert!(replacement.contains("content-ready"));
	assert!(replacement.contains(r#"data-resource-key="rh-res-0""#));
}

#[tokio::test]
async fn streaming_discovery_restores_resource_keys_after_pending_suspense_content() {
	let sibling_calls = Rc::new(Cell::new(0));
	let render_calls = Rc::clone(&sibling_calls);
	let view = Page::reactive(move || {
		let sibling_calls = Rc::clone(&render_calls);
		Page::fragment([
			Page::Suspense(SuspenseNode::new(
				Some("discovery-key-boundary".to_string()),
				|| false,
				|| PageElement::new("span").child("fallback").into_page(),
				|| {
					Page::reactive(|| {
						let resource = use_resource(
							|| async {
								tokio::time::sleep(Duration::from_millis(5)).await;
								Ok::<_, String>("boundary-ready".to_string())
							},
							deps![],
						);
						match resource.get() {
							ResourceState::Success(value) => {
								PageElement::new("strong").child(value).into_page()
							}
							ResourceState::Loading => {
								PageElement::new("em").child("boundary-loading").into_page()
							}
							ResourceState::Error(error) => {
								PageElement::new("em").child(error).into_page()
							}
						}
					})
				},
			)),
			Page::reactive(move || {
				let calls = Rc::clone(&sibling_calls);
				let resource = use_resource(
					move || {
						calls.set(calls.get() + 1);
						async { Ok::<_, String>("sibling-ready".to_string()) }
					},
					deps![],
				);
				match resource.get() {
					ResourceState::Success(value) => PageElement::new("p").child(value).into_page(),
					ResourceState::Loading => {
						PageElement::new("p").child("sibling-loading").into_page()
					}
					ResourceState::Error(error) => PageElement::new("p").child(error).into_page(),
				}
			}),
		])
	});

	let mut renderer = SsrRenderer::new();
	let mut stream = renderer.render_page_with_view_head(view).await;
	let shell = stream.next().await.unwrap().into_string();

	assert!(shell.contains("sibling-ready"));
	assert!(!shell.contains("sibling-loading"));
	assert_eq!(sibling_calls.get(), 1);
}

#[tokio::test]
async fn streaming_resource_state_helpers_mark_external_reads() {
	let view = Page::reactive(|| {
		let resource = use_resource(|| async { Ok::<_, String>("ready".to_string()) }, deps![]);
		if resource.is_loading() {
			PageElement::new("p").child("loading").into_page()
		} else if resource.is_success() {
			PageElement::new("p").child("success").into_page()
		} else if resource.is_error() {
			PageElement::new("p").child("error").into_page()
		} else {
			PageElement::new("p").child("unknown").into_page()
		}
	});

	let mut renderer = SsrRenderer::new();
	let mut stream = renderer.render_page_with_view_head(view).await;
	let shell = stream.next().await.unwrap().into_string();

	assert!(shell.contains(">success<"));
	assert!(!shell.contains(">loading<"));
	assert_eq!(renderer.state().resource_count(), 1);
}

#[tokio::test]
async fn streaming_shell_preserves_head_discovered_in_reactive_render() {
	// Arrange
	let view = Page::reactive(|| {
		PageElement::new("main")
			.child("reactive body")
			.into_page()
			.with_head(Head::new().title("Reactive Shell Head"))
	});
	let mut buffered_renderer = SsrRenderer::new();
	let mut streaming_renderer = SsrRenderer::new();

	// Act
	let buffered = buffered_renderer
		.render_page_with_view_head_to_string(view.clone())
		.await;
	let mut stream = streaming_renderer.render_page_with_view_head(view).await;
	let shell = stream.next().await.unwrap().into_string();

	// Assert
	let buffered_head = buffered
		.split_once("<head>\n")
		.and_then(|(_, html)| html.split_once("</head>"))
		.map(|(head, _)| head)
		.expect("buffered output should contain a head section");
	let shell_head = shell
		.split_once("<head>\n")
		.and_then(|(_, html)| html.split_once("</head>"))
		.map(|(head, _)| head)
		.expect("streaming shell should contain a head section");
	assert_eq!(shell_head, buffered_head);
	assert!(has_managed_head_entry(
		&shell,
		"title",
		"\">Reactive Shell Head</title>",
	));
	assert!(shell.contains("reactive body"));
}

#[tokio::test]
async fn streaming_shell_uses_fallback_head_without_pending_content_head() {
	// Arrange
	let content_calls = Rc::new(Cell::new(0));
	let render_calls = Rc::clone(&content_calls);
	let view = Page::fragment([
		Page::reactive(|| {
			use_head(
				|| Head::new().meta_description("streaming retained metadata"),
				deps![],
			);
			Page::Empty
		}),
		Page::Suspense(SuspenseNode::new(
			Some("pending-head".to_string()),
			|| false,
			|| {
				PageElement::new("span")
					.child("fallback")
					.into_page()
					.with_head(
						Head::new()
							.title("Fallback Head")
							.meta_description("streaming fallback metadata")
							.canonical("https://example.test/streaming-fallback")
							.base_url("https://example.test/streaming-fallback/"),
					)
			},
			move || {
				render_calls.set(render_calls.get() + 1);
				Page::reactive(|| {
					use_head(
						|| {
							Head::new()
								.meta_description("streaming pending hook metadata")
								.canonical("https://example.test/streaming-pending-hook")
						},
						deps![],
					);
					let resource = use_resource(
						|| async {
							tokio::time::sleep(Duration::from_millis(5)).await;
							Ok::<_, String>("ready".to_string())
						},
						deps![],
					);
					match resource.get() {
						ResourceState::Success(value) => {
							PageElement::new("strong").child(value).into_page()
						}
						ResourceState::Loading => {
							PageElement::new("em").child("loading").into_page()
						}
						ResourceState::Error(error) => {
							PageElement::new("em").child(error).into_page()
						}
					}
				})
				.with_head(
					Head::new()
						.title("Pending Content Head")
						.meta_description("streaming pending static metadata")
						.canonical("https://example.test/streaming-pending-static")
						.base_url("https://example.test/streaming-pending-static/"),
				)
			},
		)),
	]);
	let mut renderer = SsrRenderer::new();

	// Act
	let mut stream = renderer.render_page_with_view_head(view).await;
	let shell = stream.next().await.unwrap().into_string();

	// Assert
	assert!(has_managed_head_entry(
		&shell,
		"title",
		"\">Fallback Head</title>",
	));
	assert!(shell.contains("streaming retained metadata"));
	assert!(shell.contains("streaming fallback metadata"));
	assert!(shell.contains("https://example.test/streaming-fallback"));
	assert!(!shell.contains("Pending Content Head"));
	assert!(!shell.contains("streaming pending static metadata"));
	assert!(!shell.contains("https://example.test/streaming-pending-static"));
	assert!(!shell.contains("streaming pending hook metadata"));
	assert!(!shell.contains("https://example.test/streaming-pending-hook"));
	assert_eq!(content_calls.get(), 2);
}

#[tokio::test]
async fn suspense_boundary_ids_reset_between_reused_stream_renders() {
	let mut renderer = SsrRenderer::new();

	let mut first = renderer
		.render_page_with_view_head(delayed_suspense_resource_view(
			Duration::from_millis(5),
			"first",
		))
		.await;
	let first_shell = first.next().await.unwrap().into_string();

	let mut second = renderer
		.render_page_with_view_head(delayed_suspense_resource_view(
			Duration::from_millis(5),
			"second",
		))
		.await;
	let second_shell = second.next().await.unwrap().into_string();

	assert!(first_shell.contains("rh-suspense-start:rh-suspense-0"));
	assert!(second_shell.contains("rh-suspense-start:rh-suspense-0"));
	assert!(!second_shell.contains("rh-suspense-start:rh-suspense-1"));
}

#[tokio::test]
async fn custom_pending_suspense_renders_fallback_on_ssr() {
	let view = Page::Suspense(SuspenseNode::new(
		Some("custom-pending".to_string()),
		|| true,
		|| {
			PageElement::new("span")
				.child("custom-fallback")
				.into_page()
		},
		|| {
			PageElement::new("strong")
				.child("custom-content")
				.into_page()
		},
	));

	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(html.contains("custom-fallback"));
	assert!(!html.contains("custom-content"));
}

#[test]
fn suspense_page_render_to_string_uses_active_branch() {
	let view = Page::Suspense(SuspenseNode::new(
		Some("string-pending".to_string()),
		|| true,
		|| {
			PageElement::new("span")
				.child("string-fallback")
				.into_page()
		},
		|| {
			PageElement::new("strong")
				.child("string-content")
				.into_page()
		},
	));

	let html = view.render_to_string();

	assert!(html.contains("string-fallback"));
	assert!(!html.contains("string-content"));
}

#[tokio::test]
async fn suspense_boundary_into_page_preserves_table_row_root() {
	let view = PageElement::new("table")
		.child(
			PageElement::new("tbody")
				.child(
					SuspenseBoundary::new()
						.fallback(|| {
							PageElement::new("tr")
								.child(PageElement::new("td").child("fallback"))
								.into_page()
						})
						.content(|| {
							PageElement::new("tr")
								.child(PageElement::new("td").child("resolved"))
								.into_page()
						})
						.into_page(),
				)
				.into_page(),
		)
		.into_page();

	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(html.contains("<tbody><tr><td>resolved</td></tr></tbody>"));
	assert!(!html.contains("<tbody><div"));
}

#[tokio::test]
async fn collecting_ssr_stream_produces_full_html() {
	let stream = SsrStream::from_chunks([
		SsrChunk::Html("<!DOCTYPE html>".to_string()),
		SsrChunk::Html("<html></html>".to_string()),
	]);

	assert_eq!(
		stream.collect_string().await,
		"<!DOCTYPE html><html></html>"
	);
}
