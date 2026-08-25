#![cfg(not(target_arch = "wasm32"))]
//! End-to-end integration tests for the React-style brace-form component
//! invocation introduced by spec §3.5.
//!
//! These tests exercise the full pipeline: parse → validate → codegen →
//! runtime render. The brace form expands to a `bon::Builder` chain on a
//! `<Name>Props` struct, dispatched into `fn <name>(props: <Name>Props) -> Page`.
//!
//! Refs #4668 (P7) #4524.

use reinhardt_core::reactive::ReactiveScope;
use reinhardt_pages::component::Page;
use reinhardt_pages::{Loader, Query, component, loader, page};
use rstest::rstest;

#[loader]
async fn integration_greeting_loader() -> Result<String, String> {
	Ok("unused fetch".to_owned())
}

#[component(
	"/greeting/",
	name = "integration-greeting",
	loader = integration_greeting_loader
)]
fn integration_greeting(Loader(message): Loader<String>) -> Page {
	page!(|message: String| {
		p { { message } }
	})(message)
}

#[component("/optional-query/", name = "integration-optional-query")]
fn integration_optional_query(Query(logs): Query<Option<i64>>) -> Page {
	Page::text(logs.map_or_else(|| "none".to_owned(), |id| id.to_string()))
}

#[derive(bon::Builder)]
struct CardProps {
	item: String,
	// `Option<_>` is implicitly optional under `bon::Builder`; an explicit
	// `#[builder(default)]` would be redundant and is rejected by bon.
	children: Option<Page>,
}

fn card(props: CardProps) -> Page {
	page!(|p: CardProps| {
		article {
			h2 { { p.item.clone() } }
			{ p.children.clone().unwrap_or_else(Page::empty) }
		}
	})(props)
}

#[rstest]
fn brace_invocation_compiles_and_renders() {
	// Arrange + Act
	let v = page!(|| {
		div {
			Card {
				item: "hello".to_string()
			}
		}
	})();

	// Assert — uses render_to_string() to evaluate reactive closures
	// and produce HTML output suitable for substring assertions.
	let s = v.render_to_string();
	assert!(
		s.contains("hello"),
		"render output should contain `hello`, got: {s}"
	);
}

#[rstest]
fn brace_invocation_with_single_child() {
	// Arrange + Act
	let v = page!(|| {
		div {
			Card {
				item: "outer".to_string(),
				p { "inner" }
			}
		}
	})();

	// Assert
	let s = v.render_to_string();
	assert!(
		s.contains("outer") && s.contains("inner"),
		"render output should contain both `outer` and `inner`, got: {s}"
	);
}

#[rstest]
fn brace_invocation_with_multiple_children() {
	// Arrange + Act
	let v = page!(|| {
		div {
			Card {
				item: "outer".to_string(),
				p { "one" },
				p { "two" }
			}
		}
	})();

	// Assert: spec §3.5.3 — ≥2 children are wrapped in Page::fragment.
	let s = v.render_to_string();
	assert!(
		s.contains("outer") && s.contains("one") && s.contains("two"),
		"render output should contain `outer`, `one`, and `two`, got: {s}"
	);
}

#[rstest]
fn nested_component_inside_for_loop() {
	// Spec §3.5.4: page!-in-page! nesting via brace-form components inside a
	// `for` loop. Each loop iteration invokes a component whose body itself
	// is a `page!` macro.
	//
	// This test depends on PR1 (#4527 — auto-wrap behavior for `for`
	// loop bodies). If PR1 has not yet merged into develop/0.2.0, the test
	// may fail at compile or assert time; this is documented in the PR body
	// and will go green once PR #4727 lands.

	#[derive(bon::Builder)]
	struct ItemCardProps {
		title: String,
	}

	fn item_card(p: ItemCardProps) -> Page {
		page!(|p: ItemCardProps| {
			article {
				h2 { { p.title.clone() } }
			}
		})(p)
	}

	let titles = vec!["a".to_string(), "b".to_string(), "c".to_string()];
	let v = page!(|titles: Vec<String>| {
		div {
			for t in titles.iter() {
				ItemCard {
					title: t.clone()
				}
			}
		}
	})(titles);

	let s = v.render_to_string();
	for t in ["a", "b", "c"] {
		assert!(s.contains(t), "missing {t} in render output: {s}");
	}
}

#[test]
fn component_macro_props_render_like_page_brace_invocation() {
	use reinhardt_pages::router::ClientRouter;
	use reinhardt_pages::{Path, component};

	#[component("/users/{id}/", name = "user-detail")]
	fn user_page(Path(id): Path<i64>) -> Page {
		page!(|id: i64| {
			div { { format!("user {id}") } }
		})(id)
	}

	ReactiveScope::run(|| {
		let direct = user_page(UserPageProps::builder().id(7).build());
		let router = ClientRouter::new().component(user_page);
		router.current_path().set("/users/7/".to_string());
		let routed = router.render_current();

		assert_eq!(direct.render_to_string(), routed.render_to_string());
	});
}

#[test]
fn routed_component_reads_prepared_loader_value() {
	use reinhardt_pages::{LoaderStore, RouteLoader, enter_loader_store};

	let store = LoaderStore::new();
	store
		.insert(
			<integration_greeting_loader::marker as RouteLoader>::ID,
			"prepared greeting".to_owned(),
		)
		.expect("the test value is serializable");

	let _scope = enter_loader_store(store);
	let rendered =
		integration_greeting(IntegrationGreetingProps::builder().build()).render_to_string();
	assert_eq!(rendered, "<p>prepared greeting</p>");
}

#[test]
fn routed_component_extracts_optional_query_values() {
	use reinhardt_pages::router::ClientRouter;

	ReactiveScope::run(|| {
		let router = ClientRouter::new().component(integration_optional_query);

		assert_eq!(
			router.render_path("/optional-query/").render_to_string(),
			"none"
		);
		assert_eq!(
			router
				.render_path("/optional-query/?logs=42")
				.render_to_string(),
			"42"
		);
		assert_eq!(
			router
				.render_path("/optional-query/?logs=invalid")
				.render_to_string(),
			"route extraction error on `/optional-query/`: failed to parse `logs`: invalid digit found in string"
		);
	});
}
